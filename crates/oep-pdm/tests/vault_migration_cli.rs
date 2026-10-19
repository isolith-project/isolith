//! P10-M1-T03 acceptance: the `oep-vault-migrate` CLI end-to-end.
//!
//! These tests drive the REAL compiled binary (`env!("CARGO_BIN_EXE_...")`)
//! against a REPRESENTATIVE V5-era folder tree materialized on disk with real
//! bytes (fake-but-representative CATPart/CATProduct/CATDrawing/txt/pdf files in
//! nested folders, no proprietary content), and assert the four acceptance
//! legs of the migration card:
//!
//! 1. dry-run (`plan`) prints the mapping and writes NOTHING -- the vault
//!    directory is not even created;
//! 2. real import (`import`) writes `vault.json` + content-addressed blobs,
//!    preserving item+revision+file relationships per the P8-M1-T02 schema:
//!    folder tree -> nested Contexts, file -> item/revision/file, `Origin`
//!    provenance, one `imports` ledger row;
//! 3. checksums are verified AGAINST SOURCE: every `blob_sha256` equals the
//!    recomputed sha-256 of the legacy bytes, every blob byte-matches its
//!    source, and `verify` detects both a tampered source file and a tampered
//!    stored blob;
//! 4. idempotency: a re-run imports nothing and the vault document is
//!    byte-identical; a new file in the tree is imported as the delta while the
//!    first run's ledger row and content stay untouched.

use std::path::{Path, PathBuf};
use std::process::Command;

use oep_pdm::blobstore::sha256_hex;
use oep_pdm::model::VaultDoc;

/// Absolute path of the compiled CLI under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_oep-vault-migrate")
}

/// Fresh, empty temp directory under the test-friendly location.
fn tmpdir(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("oep-migrate-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run the CLI with `args`, returning (exit code, stdout, stderr).
fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin()).args(args).output().expect("spawn CLI");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const T0: &str = "2026-10-17T00:00:00Z";

/// The representative V5-era tree (fake representative bytes, nested folders).
/// Returns the tree root and the list of relative paths, in discover order.
fn materialize(root: &Path) -> Vec<String> {
    let spec: &[(&str, &[u8])] = &[
        (
            "Chassis/BajaChassis.CATProduct",
            b"fake catproduct: BajaChassis assembly (representative, not proprietary)\n",
        ),
        (
            "Chassis/Frame/RearFrame.CATPart",
            b"fake catpart: rear suspension frame geometry\n",
        ),
        (
            "Chassis/Frame/SkidPlate.CATPart",
            b"fake catpart: skid plate geometry\n",
        ),
        (
            "Docs/Driveline/DrivelineNotes.txt",
            b"fake txt: driveline design notes\n",
        ),
        ("Docs/Readme.txt", b"fake txt: build readme\n"),
        (
            "Misc/SuspensionInfo.pdf",
            b"%PDF-1.4 fake drawing payload\n",
        ),
    ];
    for (rel, bytes) in spec {
        let full = root.join(rel);
        std::fs::create_dir_all(full.parent().unwrap()).expect("mkdir");
        std::fs::write(&full, bytes).expect("write fixture file");
    }
    spec.iter().map(|(rel, _)| rel.to_string()).collect()
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap()).expect("mkdir");
    std::fs::write(&full, bytes).expect("write file");
}

fn read_vault(dir: &Path) -> VaultDoc {
    let text = std::fs::read_to_string(dir.join("vault.json")).expect("vault.json present");
    VaultDoc::from_json_pretty(&text).expect("vault.json parses")
}

/// Map context ids by name for parentage assertions.
fn ctx_parents(doc: &VaultDoc) -> std::collections::HashMap<String, String> {
    let by_id: std::collections::HashMap<String, &oep_pdm::model::Context> =
        doc.contexts.iter().map(|c| (c.id.clone(), c)).collect();
    let mut out = std::collections::HashMap::new();
    for c in &doc.contexts {
        let parent = c
            .parent_id
            .as_ref()
            .and_then(|id| by_id.get(id))
            .map(|p| p.kind.clone() + ":" + &p.name)
            .unwrap_or_else(|| "ROOT".to_string());
        out.insert(c.name.clone(), parent);
    }
    out
}

#[test]
fn cli_plan_is_dry_run_and_writes_nothing() {
    let base = tmpdir("plan");
    let root = base.join("Baja");
    materialize(&root);
    let vault = base.join("vault-out");

    let (code, out, err) = run(&[
        "plan",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0, "plan exits 0; stderr: {err}");
    assert!(
        out.contains("dry-run"),
        "plan announces dry-run; got: {out}"
    );
    // Dry-run must never materialize the vault directory (not even an empty one).
    assert!(
        !vault.exists(),
        "dry-run wrote nothing: vault dir must not exist"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn cli_import_preserves_schema_with_checksums_against_source() {
    let base = tmpdir("import");
    let root = base.join("Baja");
    let rels = materialize(&root);
    let vault = base.join("vault-out");

    let (code, _out, err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0, "import exits 0; stderr: {err}");

    let doc = read_vault(&vault);
    let errors = doc.validate();
    assert_eq!(
        errors,
        vec![],
        "imported vault validates clean; got {errors:?}"
    );

    // Folder tree re-materializes as nested Contexts (a tree, not a forest).
    assert_eq!(doc.contexts.len(), 6, "root + 5 folders");
    assert_eq!(doc.contexts[0].kind, "product");
    let parents = ctx_parents(&doc);
    assert_eq!(
        parents.get("Frame").map(|s| s.as_str()),
        Some("folder:Chassis")
    );
    assert_eq!(
        parents.get("Driveline").map(|s| s.as_str()),
        Some("folder:Docs"),
        "deep nesting is preserved: Driveline lives under Docs"
    );
    assert_eq!(
        parents.get("Misc").map(|s| s.as_str()),
        Some("product:Baja")
    );

    // One item + one revision + one file per discovered entry, with Origin.
    assert_eq!(doc.items.len(), rels.len());
    assert_eq!(doc.revisions.len(), rels.len());
    assert_eq!(doc.files.len(), rels.len());
    for item in &doc.items {
        let origin = item.origin.as_ref().expect("item carries Origin");
        assert_eq!(origin.source, "v5-folder-tree");
        assert_eq!(origin.importer, "oep-vault-migrate/0.1");
    }

    // Content addressing is REAL: every blob_sha256 == recomputed sha of source.
    for rel in &rels {
        let item = doc
            .items
            .iter()
            .find(|i| {
                i.origin
                    .as_ref()
                    .map(|o| o.legacy_path.replace('\\', "/").ends_with(rel))
                    .unwrap_or(false)
            })
            .expect("item for relative path");
        let rev = doc
            .revisions
            .iter()
            .find(|r| r.item_id == item.id)
            .expect("revision for item");
        let file = doc
            .files
            .iter()
            .find(|f| rev.file_ids.first().map(|id| id == &f.id).unwrap_or(false))
            .expect("file for revision");
        let source = std::fs::read(root.join(rel)).expect("read source");
        let expected = sha256_hex(&source);
        assert_eq!(
            file.blob_sha256, expected,
            "blob address == source hash for {rel}"
        );
        assert_eq!(
            item.origin.as_ref().unwrap().sha256.as_deref(),
            Some(expected.as_str()),
            "Origin tracks the source hash for {rel}"
        );
        // The blob bytes on disk byte-match the source (checksums vs source).
        let on_disk = std::fs::read(vault.join("blobs").join(&file.blob_sha256))
            .expect("blob present by content address");
        assert_eq!(on_disk, source, "vault blob byte-matches source for {rel}");
        assert_eq!(file.size as usize, source.len());
    }

    // Exactly one import-ledger row covering every idempotency key.
    assert_eq!(doc.imports.len(), 1);
    assert_eq!(doc.imports[0].keys.len(), rels.len());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn cli_reimport_is_idempotent_and_new_files_import_as_delta() {
    let base = tmpdir("delta");
    let root = base.join("Baja");
    materialize(&root);
    let vault = base.join("vault-out");

    let (code, _out, err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0, "first import; {err}");
    let after_first = std::fs::read(vault.join("vault.json")).expect("vault");

    // No-op re-run: exit 0, nothing changes -- vault.json is byte-identical.
    let (code, out, _err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0);
    assert!(
        out.contains("nothing to import"),
        "re-run is a no-op: {out}"
    );
    let after_rerun = std::fs::read(vault.join("vault.json")).expect("vault");
    assert_eq!(
        after_first, after_rerun,
        "no-op re-run leaves vault.json byte-identical"
    );

    // A brand-new file appears in the tree -> the delta is imported, the first
    // run's content is untouched, and a second ledger row is appended.
    write(
        &root,
        "Chassis/Frame/NewBracket.CATPart",
        b"a new legacy part\n",
    );
    let (code, _out, err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0, "delta import; {err}");

    let doc = read_vault(&vault);
    assert!(doc.validate().is_empty());
    assert_eq!(doc.items.len(), 7, "6 original + 1 delta");
    assert_eq!(doc.files.len(), 7);
    assert_eq!(doc.imports.len(), 2, "one ledger row per completed run");
    assert_eq!(doc.imports[0].keys.len(), 6);
    assert_eq!(doc.imports[1].keys.len(), 1);

    // First-run ledger row still lists only the original idempotency keys, so
    // a subsequent re-run is again a no-op (only the delta is imported once).
    let (code, out, _err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0);
    assert!(
        out.contains("nothing to import"),
        "post-delta re-run no-op: {out}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn cli_verify_checksums_and_detects_tamper_both_directions() {
    let base = tmpdir("verify");
    let root = base.join("Baja");
    materialize(&root);
    let vault = base.join("vault-out");

    let (code, _out, _err) = run(&[
        "import",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
        "--collected-at",
        T0,
    ]);
    assert_eq!(code, 0, "import before verify");

    // Clean tree verifies.
    let (code, out, _err) = run(&[
        "verify",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "clean tree verifies; got: {out}");
    assert!(out.contains("verify PASS"));

    // Tamper a SOURCE file: verify must catch it and fail non-zero.
    write(
        &root,
        "Docs/Readme.txt",
        b"tampered source bytes after import\n",
    );
    let (code, _out, err) = run(&[
        "verify",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "verify fails on tampered source; stderr: {err}");
    assert!(err.contains("verify failed"));

    // Restore the source; tamper the STORED BLOB behind the store's back:
    // verify must fail (blob store scan reports corruption).
    write(&root, "Docs/Readme.txt", b"fake txt: build readme\n");
    let doc = read_vault(&vault);
    let readme_file = doc
        .files
        .iter()
        .find(|f| f.filename == "Readme.txt")
        .expect("readme file record");
    std::fs::write(
        vault.join("blobs").join(&readme_file.blob_sha256),
        b"EVIL BYTES SWAPPED IN\n",
    )
    .expect("tamper stored blob");
    let (code, _out, err) = run(&[
        "verify",
        root.to_str().unwrap(),
        "--vault",
        vault.to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "verify fails on tampered blob; stderr: {err}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// Focused unit coverage of the new legacy building blocks the CLI is built on.
// ---------------------------------------------------------------------------

#[test]
fn legacy_merge_vault_appends_only_new_ids_and_always_ledger_rows() {
    use oep_pdm::legacy::{discover, merge_vault, plan_delta};

    let base = tmpdir("merge");
    let root = base.join("Baja");
    let rels = materialize(&root);
    let entries = discover(&root).expect("discover");
    let (plan, _) = plan_delta(
        &VaultDoc::new(),
        &root,
        &entries,
        T0,
        "oep-vault-migrate/0.1",
    )
    .expect("delta plan from empty base == full plan");

    let mut doc = VaultDoc::new();
    let counts = merge_vault(&mut doc, &plan.doc);
    assert_eq!(counts.items, rels.len() as u64);
    assert_eq!(doc.items.len(), rels.len());

    // Merging the same delta again appends nothing (ids already present), but
    // the ledger row IS appended (one row per completed run, by design).
    let counts2 = merge_vault(&mut doc, &plan.doc);
    assert_eq!(counts2.items, 0);
    assert_eq!(counts2.contexts, 0);
    assert_eq!(doc.imports.len(), 2, "ledger rows are append-only");

    // A context that already exists is not re-appended, but its id stays valid,
    // so re-running plan+merge cannot produce duplicate-id violations.
    let (plan_b, _) =
        plan_delta(&doc, &root, &entries, T0, "oep-vault-migrate/0.1").expect("delta from merged");
    assert_eq!(
        plan_b.doc.items.len(),
        0,
        "after a full merge the delta is empty"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn legacy_plan_delta_reports_only_not_yet_imported() {
    use oep_pdm::legacy::{discover, plan_delta, plan_import};

    let base = tmpdir("delta_unit");
    let root = base.join("Baja");
    materialize(&root);
    let entries = discover(&root).expect("discover");

    // Base = the FULL first-run plan (all files already imported).
    let full = plan_import(&root, &entries, T0, "oep-vault-migrate/0.1").expect("full plan");
    // A new file lands in the tree; from `full`'s ledger it is the only delta.
    write(&root, "Misc/NewPart.CATPart", b"new part after first run\n");
    let entries2 = discover(&root).expect("discover again");
    let (delta_plan, skipped) =
        plan_delta(&full.doc, &root, &entries2, T0, "oep-vault-migrate/0.1").expect("delta plan");
    assert_eq!(skipped, 6, "6 original files are already imported");
    assert_eq!(delta_plan.doc.items.len(), 1);
    assert_eq!(
        delta_plan.doc.items[0].origin.as_ref().unwrap().legacy_path,
        root.join("Misc/NewPart.CATPart")
            .to_string_lossy()
            .to_string()
    );
    let _ = std::fs::remove_dir_all(&base);
}
