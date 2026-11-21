//! Document I/O lifecycle (wave A, card UI-A7 UI-OPEN-SAVE).
//!
//! The open/save half of the document lifecycle, engine-side and egui-free:
//! opening a STEP through the existing importer hub, saving the canonical
//! model as a `.oepdoc` document, and reopening it. Everything here is plain
//! std + engine code so it can be shared by the panel ([`crate::app::panels::doc`])
//! and exercised under the same rules the engine already follows: imports go
//! through `oep_import` (never a new parser), the canonical model is
//! `oep_geo::Document`, and a document is deterministic (same bytes in, same
//! bytes out through `to_json_pretty`/`from_json_pretty`).
//!
//! ## Document hash
//!
//! The engine does not expose a document-level hash today (it hashes rendered
//! *frames* with FNV-1a 64 in oep-viewer, and archive blobs with SHA-256 in
//! oep-archive). This card therefore defines [`document_hash`] in this file
//! per the card instruction ("if none exists, define it in doc_io.rs and say
//! so in the proof"), using the SAME hashing scheme the engine already uses
//! for frames - FNV-1a 64 with the oep-viewer constants applied to the
//! canonical `.oepdoc` bytes (`to_json_pretty`, which is deterministic and
//! documented as such by oep-geo). No second hashing scheme is invented.
//!
//! Because the serialization is canonical, `document_hash(open(oepdoc)) ==
//! document_hash(document)` after a save/reopen round trip is a *byte-level*
//! equality, not an approximate one.
//!
//! ## Recent files
//!
//! The recent-list (persisted paths) lives with the rest of the app's user
//! state in the config directory that `crate::config` resolves (`layout_path`
//! names that directory; the recent file sits next to `layout.conf`), so a
//! restart reads the same list back. The panel owns *what* goes in; this
//! module owns *how* it is stored.

use std::path::{Path, PathBuf};

use oep_import::{Document, ImportOutcome, SourceKind};

/// File extension for our own document format.
pub const OEPDOC_EXTENSION: &str = ".oepdoc";

/// Maximum number of recent files kept in the config store.
pub const RECENT_MAX: usize = 5;

/// The committed demo part: the editor-sketch-extrude solid, resolved from
/// this crate's manifest dir so it works regardless of the process cwd (tests
/// run from the crate, `shot.sh` from the repo root). `None` only when the
/// tree is missing the file.
pub fn demo_step_path() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let candidate = root.join("demos/editor-sketch-extrude/edpart.step");
    candidate.exists().then_some(candidate)
}

/// Open a STEP part through the existing importer hub into the canonical
/// [`Document`] model. Never a new parser: detection + dispatch happen in
/// `oep_import`, exactly like every other source the workbench loads.
///
/// Any recognized-but-unimportable input is a typed `Err` (never a panic and
/// never half-loaded geometry): the caller keeps whatever document it had.
pub fn import_step(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    import_step_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The bytes-typed half of [`import_step`], exposed for reuse by the recent
/// self-check path.
pub fn import_step_bytes(bytes: &[u8]) -> Result<Document, String> {
    match oep_import::import_bytes(SourceKind::Step, bytes) {
        Ok(ImportOutcome::Imported { document, .. }) => Ok(document),
        Ok(ImportOutcome::Skipped { source, reason })
        | Ok(ImportOutcome::Reserved { source, reason }) => {
            Err(format!("{source:?} skipped: {reason}"))
        }
        Err(e) => Err(format!("import failed: {e}")),
    }
}

/// Open our own `.oepdoc` document (the canonical oep-geo JSON) back from
/// disk. Older schema versions are promoted to the current one by
/// `from_json_pretty`, so an old file still loads.
pub fn read_oepdoc(path: &Path) -> Result<Document, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Document::from_json_pretty(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Save the current document as a `.oepdoc` (canonical deterministic JSON).
pub fn write_oepdoc(path: &Path, document: &Document) -> Result<(), String> {
    let text = document
        .to_json_pretty()
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// FNV-1a-64 over the deterministic `.oepdoc` bytes of `document`.
///
/// Same constants and scheme as `oep_viewer::backend::frame_hash` (the
/// engine's existing FNV-1a fingerprint). `to_json_pretty` cannot fail for a
/// canonical in-memory [`Document`] (plain serde derive, no custom
/// serializers); the `expect` documents that invariant rather than silently
/// hashing a substitution that would mask an I/O bug.
pub fn document_hash(document: &Document) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let text = document
        .to_json_pretty()
        .expect("canonical oep-geo Document always serializes");
    let mut h = OFFSET;
    for &byte in text.as_bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Path of the persisted recent-file list in `config_dir`.
pub fn recent_path(config_dir: &Path) -> PathBuf {
    config_dir.join("recent.conf")
}

/// Read the persisted recent list back (newest first); missing or unreadable
/// store yields an empty list (never an error).
pub fn load_recent(config_dir: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(recent_path(config_dir)) else {
        return Vec::new();
    };
    text.lines().map(PathBuf::from).collect()
}

/// Persist the recent list (newest first). Best-effort: a failure to write
/// the store must not fail the save that just succeeded, so it is swallowed
/// after being surfaced for diagnostics.
pub fn save_recent(config_dir: &Path, recent: &[PathBuf]) {
    let body: Vec<String> = recent.iter().map(|p| p.display().to_string()).collect();
    let _ = std::fs::write(recent_path(config_dir), body.join("\n"));
}

/// Record `path` as the most-recent file: move it to the front (dedup) and
/// cap the list at [`RECENT_MAX`]. Returns the new list (already persisted).
pub fn push_recent(config_dir: &Path, path: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = load_recent(config_dir)
        .into_iter()
        .filter(|p| p != path)
        .collect();
    list.insert(0, path.to_path_buf());
    list.truncate(RECENT_MAX);
    save_recent(config_dir, &list);
    list
}
