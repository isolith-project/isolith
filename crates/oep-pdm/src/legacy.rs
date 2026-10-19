//! V5-era folder-tree migration planning (P10-M1-T03 anticipation).
//!
//! Dassault V5-era deployments keep their data as **folder trees**: a root
//! directory holding `CATPart`/`CATProduct`/`CATDrawing`/aux files, organized
//! into nested folders with no PDM metadata. P10-M1-T03 ("migration path from
//! V5-era folder trees") will ingest such trees; this module anticipates that
//! card **in the schema** by defining the exact mapping a tree takes into the
//! vault object model:
//!
//! - every **directory** becomes a [`Context`] (`folder` kind) nested under
//!   its parent directory's context; the tree root becomes a `product` context;
//! - every **file** becomes an [`Item`] (kind from extension), a first
//!   [`Revision`] (`"1"`, state `working`), a [`File`] record content-addressed
//!   by the file's real sha-256, and an [`Origin`] provenance record carrying
//!   the absolute legacy path + collected-at + source hash;
//! - the run records one [`ImportLedger`] row whose `keys` are the idempotency
//!   keys (sha-256 of the normalized legacy path) of every imported file, so a
//!   re-run skips already-imported paths instead of duplicating vault content.
//!
//! Assembly **composition** (a `.CATProduct` referencing child `.CATPart`
//! geometry) is deliberately out of scope here: folding a product's component
//! graph faithfully needs the CATProduct reader lane (P5/P6), and P10-M1-T03
//! decides the fidelity to claim. This card anticipates the *tree shape* only,
//! and documents that the folder→context + file→item mapping is the stable
//! contract the migration card builds on.

use crate::blobstore::sha256_hex;
use crate::model::{Context, File, ImportLedger, Item, Origin, Revision, RevisionOrigin, VaultDoc};
use sha2::Digest as _;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Legacy-file records discovered under a tree root (relative to root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyEntry {
    /// Absolute path of the file in the legacy tree.
    pub path: PathBuf,
    /// Path relative to the tree root (POSIX-style, e.g. `Chassis/Frame/x.CATPart`).
    pub relative: String,
    /// Leaf file name.
    pub filename: String,
    /// Detected product format tag (`catpart`, `catproduct`, `txt`, `bin`).
    pub format: String,
    /// Detected vault item kind (`part`, `assembly`, `document`).
    pub item_kind: String,
}

/// A failure while mapping a legacy tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError {
    /// Human-readable reason.
    pub message: String,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImportError {}

/// Map a file extension to a `(item_kind, format)` pair for the vault.
///
/// The table is closed and documented; the migration card decides fidelity
/// beyond this basic classification (e.g. CATProduct component resolution).
pub fn classify(filename: &str) -> (String, String) {
    let lower = match filename.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    };
    match lower.as_str() {
        "catpart" => (
            crate::model::ITEM_KIND_PART.to_string(),
            "catpart".to_string(),
        ),
        "catproduct" => (
            crate::model::ITEM_KIND_ASSEMBLY.to_string(),
            "catproduct".to_string(),
        ),
        "catdrawing" => (
            crate::model::ITEM_KIND_DOCUMENT.to_string(),
            "catdrawing".to_string(),
        ),
        "txt" | "doc" | "docx" | "pdf" | "md" => {
            (crate::model::ITEM_KIND_DOCUMENT.to_string(), lower)
        }
        _ => (
            crate::model::ITEM_KIND_DOCUMENT.to_string(),
            "bin".to_string(),
        ),
    }
}

/// Deterministic vault id from a canonical key (first `n` sha-256 hex chars).
fn id_from(prefix: &str, key: &str, n: usize) -> String {
    let mut h = sha2::Sha256::new();
    h.update(prefix.as_bytes());
    h.update(key.as_bytes());
    format!("{:x}", h.finalize())[..n.min(64)].to_string()
}
/// The idempotency key for a legacy path: sha-256 of the normalized path.
///
/// Normalization: the path is converted to POSIX style, then components are
/// joined on `/` with a single leading slash. Two different legacy machines
/// pointing at the same tree therefore produce the same key.
pub fn ledger_key_for(path: &str) -> String {
    let norm: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    sha256_hex(format!("/{}", norm.join("/")).as_bytes())
}

/// Recursively discover legacy files under `root`, in deterministic order
/// (sorted by relative path). A path with no regular file is not an entry.
pub fn discover(root: &Path) -> Result<Vec<LegacyEntry>, ImportError> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read = std::fs::read_dir(&dir).map_err(|e| ImportError {
            message: format!("cannot read {}: {e}", dir.display()),
        })?;
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in read.flatten() {
            children.push(entry.path());
        }
        children.sort();
        // Push directories in reverse so the smallest sorts first on pop, and
        // files are walked depth-first for a stable, predictable order.
        for child in children.iter().rev() {
            if child.is_dir() {
                stack.push(child.clone());
            }
        }
        for child in children {
            if child.is_file() {
                let relative = child
                    .strip_prefix(root)
                    .unwrap_or(&child)
                    .to_string_lossy()
                    .replace('\\', "/");
                let filename = child
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let (item_kind, format) = classify(&filename);
                out.push(LegacyEntry {
                    path: child,
                    relative,
                    filename,
                    format,
                    item_kind,
                });
            }
        }
    }
    out.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(out)
}

/// A planned folder-tree import: the vault document plus the content addresses
/// of its file blobs, in file-record order.
#[derive(Debug, Clone)]
pub struct ImportPlan {
    /// The vault document produced by the mapping.
    pub doc: VaultDoc,
    /// Content addresses of the blobs, aligned with `doc.files` order.
    pub blob_addresses: Vec<String>,
}

/// Map discovered legacy entries into a vault document without touching any
/// store. Each entry becomes one item, one revision and one file; every file's
/// blob bytes are hashed (content addressing) but NOT written anywhere.
///
/// Pass the same `collected_at`/`importer` strings for a deterministic output.
pub fn plan_import(
    root: &Path,
    entries: &[LegacyEntry],
    collected_at: &str,
    importer: &str,
) -> Result<ImportPlan, ImportError> {
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "vault".to_string());
    let root_key = ledger_key_for(&root.to_string_lossy());

    let mut doc = VaultDoc::new();

    // 1) Contexts: one per distinct directory, folder-nested.
    let root_ctx_id = id_from("ctx:", &root_key, 12);
    doc.push_context(Context::new(
        root_ctx_id.clone(),
        root_name,
        crate::model::CONTEXT_KIND_PRODUCT,
    ));

    let mut dir_to_ctx: BTreeMap<String, String> = BTreeMap::new(); // abs dir -> ctx id
    dir_to_ctx.insert(root.to_string_lossy().to_string(), root_ctx_id.clone());

    // Every directory under the root becomes a `folder` Context whose parent is
    // its on-disk parent directory's Context, so the migrated tree re-materializes
    // EXACTLY as it was on disk (a tree, not a forest -- `root-folder` would
    // otherwise be a validation violation). The walk is breadth-first so a parent
    // directory's Context always exists before its children are resolved, and it
    // uses lstat (never follows symlinks) so a symlink loop cannot cycle the walk.
    // Note this is deliberately based on the directory tree itself, not on the
    // entries: an intermediate folder with no direct file still gets a Context.
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut frontier: Vec<PathBuf> = vec![root.to_path_buf()];
    while !frontier.is_empty() {
        let mut next: Vec<PathBuf> = Vec::new();
        for dir in &frontier {
            if let Ok(read) = std::fs::read_dir(dir) {
                let mut children: Vec<PathBuf> = Vec::new();
                for entry in read.flatten() {
                    let path = entry.path();
                    let is_dir = std::fs::symlink_metadata(&path)
                        .map(|m| m.is_dir())
                        .unwrap_or(false);
                    if is_dir {
                        children.push(path);
                    }
                }
                children.sort();
                dirs.extend(children.iter().cloned());
                next.extend(children);
            }
        }
        frontier = next;
    }
    for dir in dirs {
        let dir_abs = dir.to_string_lossy().to_string();
        let parent_abs = dir
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        let parent_ctx = dir_to_ctx
            .get(&parent_abs)
            .cloned()
            .unwrap_or_else(|| root_ctx_id.clone());
        let ctx_id = id_from("ctx:", &dir_abs, 12);
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        doc.push_context(
            Context::new(ctx_id.clone(), name, crate::model::CONTEXT_KIND_FOLDER)
                .with_parent(parent_ctx),
        );
        dir_to_ctx.insert(dir_abs, ctx_id);
    }

    // 2) Items / revisions / files from each entry. Content-addressed by the
    //    entry's real byte hash.
    let mut blob_addresses = Vec::new();
    for entry in entries {
        let bytes = std::fs::read(&entry.path).map_err(|e| ImportError {
            message: format!("cannot read {}: {e}", entry.path.display()),
        })?;
        let sha = sha256_hex(&bytes);
        let key = ledger_key_for(&entry.relative);
        let item_id = id_from("item:", &key, 16);
        let ctx_id = dir_to_ctx
            .get(
                &entry
                    .path
                    .parent()
                    .unwrap_or(root)
                    .to_string_lossy()
                    .to_string(),
            )
            .cloned();
        let stem = entry
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.filename.clone());

        let item = Item::new(item_id.clone(), stem.clone(), entry.item_kind.clone())
            .with_created_at(collected_at)
            .with_origin(Origin::v5_folder_tree(
                entry.path.to_string_lossy().to_string(),
                collected_at,
                Some(sha.clone()),
                importer,
            ));
        let item = match &ctx_id {
            Some(cid) => item.with_context(cid.clone()),
            None => item,
        };
        doc.push_item(item);

        let rev_id = id_from("rev:", &format!("{key}#{stem}"), 16);
        let file_id = id_from("file:", &key, 16);
        let revision = Revision::new(rev_id, item_id, "1", crate::model::STATE_WORKING, importer)
            .with_created_at(collected_at)
            .with_file(file_id.clone())
            .with_origin(RevisionOrigin::v5_folder_tree(
                entry.path.to_string_lossy().to_string(),
            ));
        doc.push_revision(revision);

        doc.push_file(
            File::new(
                file_id,
                entry.filename.clone(),
                entry.format.clone(),
                sha.clone(),
            )
            .with_size(bytes.len() as u64),
        );
        blob_addresses.push(sha);
    }

    // 3) Import ledger row (idempotency keys of everything imported).
    let keys: Vec<String> = entries
        .iter()
        .map(|e| ledger_key_for(&e.relative))
        .collect();
    doc.push_import(ImportLedger {
        kind: crate::model::ORIGIN_SOURCE_V5_FOLDER_TREE.to_string(),
        root_legacy_path: root.to_string_lossy().to_string(),
        imported_at: collected_at.to_string(),
        importer: importer.to_string(),
        contexts: doc.contexts.len() as u64,
        items: doc.items.len() as u64,
        files: doc.files.len() as u64,
        blobs: blob_addresses.len() as u64,
        keys,
    });

    Ok(ImportPlan {
        doc,
        blob_addresses,
    })
}

/// Which relative (ledger) keys of `entries` are NOT yet present in `doc`'s
/// import ledger -- the re-run/delta answer P10-M1-T03 needs.
pub fn not_yet_imported(doc: &VaultDoc, relative_paths: &[String]) -> Vec<String> {
    let done: std::collections::HashSet<&str> = doc
        .imports
        .iter()
        .flat_map(|r| r.keys.iter().map(|k| k.as_str()))
        .collect();
    relative_paths
        .iter()
        .filter(|rel| !done.contains(ledger_key_for(rel).as_str()))
        .cloned()
        .collect()
}

/// How many vault objects a [`merge_vault`] run newly appended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeCounts {
    /// Contexts appended by the merge.
    pub contexts: u64,
    /// Items appended by the merge.
    pub items: u64,
    /// Revisions appended by the merge.
    pub revisions: u64,
    /// File records appended by the merge.
    pub files: u64,
    /// Relationships appended by the merge.
    pub relationships: u64,
}

/// Merge `delta` into `base` in place for a real (write) import run.
///
/// One vault = one id space across all five model collections
/// (`duplicate-id` is a validation violation), so a merge must never append an
/// id `base` already holds. Objects whose id is already present are skipped;
/// import-ledger rows are append-only records (one per run) and are always
/// appended. Ids are deterministic (`id_from(prefix, ledger_key_for(path), n)`),
/// so a delta re-plan of a folder that already exists in `base` yields the same
/// context id and is correctly de-duplicated here. Returns how many objects
/// were newly appended.
pub fn merge_vault(base: &mut VaultDoc, delta: &VaultDoc) -> MergeCounts {
    let mut counts = MergeCounts::default();

    let mut ctx_ids: std::collections::HashSet<String> =
        base.contexts.iter().map(|c| c.id.clone()).collect();
    for c in &delta.contexts {
        if ctx_ids.insert(c.id.clone()) {
            base.push_context(c.clone());
            counts.contexts += 1;
        }
    }

    let mut item_ids: std::collections::HashSet<String> =
        base.items.iter().map(|i| i.id.clone()).collect();
    for i in &delta.items {
        if item_ids.insert(i.id.clone()) {
            base.push_item(i.clone());
            counts.items += 1;
        }
    }

    let mut rev_ids: std::collections::HashSet<String> =
        base.revisions.iter().map(|r| r.id.clone()).collect();
    for r in &delta.revisions {
        if rev_ids.insert(r.id.clone()) {
            base.push_revision(r.clone());
            counts.revisions += 1;
        }
    }

    let mut file_ids: std::collections::HashSet<String> =
        base.files.iter().map(|f| f.id.clone()).collect();
    for f in &delta.files {
        if file_ids.insert(f.id.clone()) {
            base.push_file(f.clone());
            counts.files += 1;
        }
    }

    let mut rel_ids: std::collections::HashSet<String> =
        base.relationships.iter().map(|r| r.id.clone()).collect();
    for r in &delta.relationships {
        if rel_ids.insert(r.id.clone()) {
            base.push_relationship(r.clone());
            counts.relationships += 1;
        }
    }

    // Import ledger: append-only rows, no id -- every completed run records one.
    for row in &delta.imports {
        base.push_import(row.clone());
    }

    counts
}

/// Plan an import of exactly the entries not yet recorded in `base`'s ledger.
///
/// This is the building block a real (write) import run applies: it filters
/// `entries` to the [`not_yet_imported`] delta, plans that subset with
/// [`plan_import`] (same deterministic ids, folder-tree -> context mapping,
/// `Origin` provenance, one ledger row for the delta), and reports how many
/// already-imported files were skipped. Planning never touches a store.
pub fn plan_delta(
    base: &VaultDoc,
    root: &Path,
    entries: &[LegacyEntry],
    collected_at: &str,
    importer: &str,
) -> Result<(ImportPlan, usize), ImportError> {
    let rels: Vec<String> = entries.iter().map(|e| e.relative.clone()).collect();
    let keep: std::collections::HashSet<String> =
        not_yet_imported(base, &rels).into_iter().collect();
    let skipped = entries.len().saturating_sub(keep.len());
    let delta: Vec<LegacyEntry> = entries
        .iter()
        .filter(|e| keep.contains(&e.relative))
        .cloned()
        .collect();
    let plan = plan_import(root, &delta, collected_at, importer)?;
    Ok((plan, skipped))
}
