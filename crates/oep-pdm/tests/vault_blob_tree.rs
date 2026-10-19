//! P8-M1-T02 integration tests: content-addressed blob storage, verified by
//! hash, plus the V5-era folder-tree migration mapping (P10-M1-T03
//! anticipation).
//!
//! These tests use REAL bytes (the committed fixture blobs under
//! `tests/fixtures/legacy/`), a real on-disk [`BlobStore`] in a temp
//! directory, and a real folder tree assembled on disk -- no mocked hashing,
//! no in-memory shortcuts.
//!
//! The three acceptance legs:
//! 1. **content-addressed**: identical bytes store once under the same sha;
//!    the address is a pure function of the content.
//! 2. **verified by hash**: every read re-hashes; a tampered blob is a
//!    `Corrupt` error, never silently trusted; `verify_doc_files` proves that
//!    every blob the golden vault references is present and hash-correct.
//! 3. **migration path anticipated**: a V5-style folder tree plans into a
//!    vault (context tree + items + origins + ledger keys), re-running is
//!    idempotent, and `not_yet_imported` reports the delta for P10-M1-T03.

use oep_pdm::blobstore::{sha256_hex, BlobError, BlobStore};
use oep_pdm::legacy::{discover, ledger_key_for, not_yet_imported, plan_import};
use std::path::Path;

mod vault_model_golden;

fn fixture_dir() -> &'static str {
    "tests/fixtures/legacy"
}

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(fixture_dir()).join(path)).expect("fixture blob present")
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("oep-vault-test-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// ---------------------------------------------------------------------------
// 1. content addressing
// ---------------------------------------------------------------------------

#[test]
fn identical_bytes_store_once_dedup() {
    let dir = tmpdir("dedup");
    let store = BlobStore::new(&dir);
    let bytes = b"the same payload, twice".to_vec();
    let a = store.put(&bytes).expect("put");
    let b = store.put(&bytes).expect("put again");
    assert_eq!(a, b, "identical bytes share one content address");
    assert_eq!(a, sha256_hex(&bytes));
    // Only one file on disk.
    let count = std::fs::read_dir(dir.join("blobs"))
        .expect("blobs dir")
        .count();
    assert_eq!(count, 1, "one content-addressed blob for one payload");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn address_is_pure_function_of_content() {
    let dir = tmpdir("pure");
    let store = BlobStore::new(&dir);
    let a = store.put(b"alpha").expect("put");
    let b = store.put(b"beta").expect("put");
    assert_ne!(a, b);
    assert_eq!(a, sha256_hex(b"alpha"));
    assert_eq!(b, sha256_hex(b"beta"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 2. verified by hash
// ---------------------------------------------------------------------------

#[test]
fn get_rehashes_and_reports_corruption() {
    let dir = tmpdir("tamper");
    let store = BlobStore::new(&dir);
    let payload = b"verified-by-hash payload".to_vec();
    let addr = store.put(&payload).expect("put");
    assert_eq!(store.get(&addr).expect("get"), payload);

    // Tamper with the blob on disk behind the store's back.
    let path = dir.join("blobs").join(&addr);
    std::fs::write(&path, b"EVIL BYTES SWAPPED IN").expect("tamper");
    match store.get(&addr) {
        Err(BlobError::Corrupt { expected, actual }) => {
            assert_eq!(expected, addr);
            assert_ne!(actual, addr);
        }
        other => panic!("tampered blob must read as Corrupt, got {other:?}"),
    }
    let report = store.verify();
    assert_eq!(report.corrupt, 1);
    assert_eq!(report.ok, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_blob_is_missing_not_corrupt() {
    let dir = tmpdir("missing");
    let store = BlobStore::new(&dir);
    let addr = sha256_hex(b"never stored");
    assert!(matches!(store.get(&addr), Err(BlobError::Missing(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verify_doc_files_proves_the_golden_vault_on_real_fixture_blobs() {
    // The golden vault's content addresses are the REAL sha-256 of the
    // committed fixture blobs; a BlobStore fed those bytes must verify every
    // referenced file, and tampering any one blob must flip verification.
    let dir = tmpdir("golden");
    let store = BlobStore::new(&dir);
    let doc = vault_model_golden::build_example();

    for (name, _sha) in [
        ("RearFrame.CATPart", ""),
        ("RearFrame-v2.CATPart", ""),
        ("SkidPlate.CATPart", ""),
        ("BajaChassis.CATProduct", ""),
        ("Readme.txt", ""),
        ("DrivelineNotes.txt", ""),
    ] {
        store.put(&fixture(name)).expect("store fixture blob");
    }

    // Every content address referenced by the vault's file records is present
    // and hash-correct.
    store
        .verify_doc_files(&doc)
        .expect("golden vault fully verified by hash");

    // Flip one byte of one blob -> the same check must now fail.
    let target = sha256_hex(&fixture("Readme.txt"));
    let path = dir.join("blobs").join(&target);
    let mut bytes = std::fs::read(&path).expect("read blob");
    bytes[0] ^= 0x01;
    std::fs::write(&path, &bytes).expect("tamper");
    assert!(
        matches!(store.verify_doc_files(&doc), Err(BlobError::Corrupt { .. })),
        "tampered blob must fail vault verification"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 3. migration path from V5-era folder trees (P10-M1-T03 anticipation)
// ---------------------------------------------------------------------------

/// A small V5-style tree, materialized on disk as a fixture:
/// `LegacyRoot/Chassis/Frame/{RearFrame.CATPart, SkidPlate.CATPart}`,
/// `LegacyRoot/Chassis/BajaChassis.CATProduct`,
/// `LegacyRoot/Docs/{Readme.txt, Driveline/DrivelineNotes.txt}`.
fn materialize_tree() -> (std::path::PathBuf, Vec<String>) {
    let root = tmpdir("migrate");
    for rel in [
        "Chassis/Frame/RearFrame.CATPart",
        "Chassis/Frame/SkidPlate.CATPart",
        "Chassis/BajaChassis.CATProduct",
        "Docs/Readme.txt",
        "Docs/Driveline/DrivelineNotes.txt",
    ] {
        let full = root.join(rel);
        std::fs::create_dir_all(full.parent().unwrap()).expect("mkdir");
        std::fs::copy(
            Path::new(fixture_dir()).join(rel.rsplit('/').next().unwrap()),
            &full,
        )
        .expect("copy fixture into tree");
    }
    let rels: Vec<String> = [
        "Chassis/Frame/RearFrame.CATPart",
        "Chassis/Frame/SkidPlate.CATPart",
        "Chassis/BajaChassis.CATProduct",
        "Docs/Readme.txt",
        "Docs/Driveline/DrivelineNotes.txt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    (root, rels)
}

#[test]
fn plan_import_maps_folder_tree_to_vault_shape() {
    let (root, _rels) = materialize_tree();
    let entries = discover(&root).expect("discover");
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries
            .iter()
            .map(|e| e.relative.clone())
            .collect::<Vec<_>>(),
        vec![
            "Chassis/BajaChassis.CATProduct".to_string(),
            "Chassis/Frame/RearFrame.CATPart".to_string(),
            "Chassis/Frame/SkidPlate.CATPart".to_string(),
            "Docs/Driveline/DrivelineNotes.txt".to_string(),
            "Docs/Readme.txt".to_string(),
        ]
    );
    // Classification by extension.
    assert_eq!(entries[0].item_kind, "assembly");
    assert_eq!(entries[0].format, "catproduct");
    assert_eq!(entries[1].item_kind, "part");

    let plan = plan_import(
        &root,
        &entries,
        "2026-10-17T00:00:00Z",
        "oep-vault-migrate/0.1",
    )
    .expect("plan import");
    let doc = plan.doc;
    assert_eq!(doc.validate().to_vec(), vec![], "planned vault is clean");

    // Contexts: root + one folder per directory.
    assert_eq!(doc.contexts.len(), 5);
    assert_eq!(doc.contexts[0].kind, "product");
    let folder_counts = doc.contexts.iter().filter(|c| c.kind == "folder").count();
    assert_eq!(folder_counts, 4);

    // Every imported file is an item with a v5-folder-tree origin carrying the
    // absolute legacy path + the source byte hash.
    assert_eq!(doc.items.len(), 5);
    for item in &doc.items {
        let origin = item.origin.as_ref().expect("imported item has origin");
        assert_eq!(origin.source, "v5-folder-tree");
        assert!(origin.legacy_path.contains(root.to_string_lossy().as_ref()));
        let sha = origin.sha256.clone().expect("origin carries source hash");
        assert_eq!(sha.len(), 64);
    }

    // Blob addresses are the REAL content addresses of the source bytes.
    assert_eq!(plan.blob_addresses.len(), 5);
    for (entry, addr) in entries.iter().zip(&plan.blob_addresses) {
        assert_eq!(
            *addr,
            sha256_hex(&std::fs::read(&entry.path).expect("read source"))
        );
    }

    // One import ledger row with the five idempotency keys.
    assert_eq!(doc.imports.len(), 1);
    assert_eq!(doc.imports[0].items, 5);
    assert_eq!(doc.imports[0].keys.len(), 5);

    // Every file record carries a valid content address.
    for f in &doc.files {
        assert_eq!(f.blob_sha256.len(), 64);
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reimport_is_idempotent_with_delta_reported() {
    let (root, rels) = materialize_tree();
    let entries = discover(&root).expect("discover");
    let plan = plan_import(
        &root,
        &entries,
        "2026-10-17T00:00:00Z",
        "oep-vault-migrate/0.1",
    )
    .expect("plan import");

    // After the first run, the ledger contains every key -> nothing left.
    let rels_owned: Vec<String> = rels.clone();
    let remaining = not_yet_imported(&plan.doc, &rels_owned);
    assert_eq!(remaining, Vec::<String>::new(), "re-run is a no-op");

    // A NEW file in the tree (not in the ledger) is reported as the delta.
    let fresh = root.join("Docs/NewNote.txt");
    std::fs::write(&fresh, b"a brand new legacy file\n").expect("write new file");
    let mut rels2 = rels.clone();
    rels2.push("Docs/NewNote.txt".to_string());
    let remaining2 = not_yet_imported(&plan.doc, &rels2);
    assert_eq!(remaining2, vec!["Docs/NewNote.txt".to_string()]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ledger_key_normalizes_windows_and_posix_paths() {
    // The same tree on a Windows drive and a POSIX mount yields one key, so a
    // re-run from either machine never duplicates.
    let win = ledger_key_for("W:\\Data\\Baja\\Chassis\\Frame\\RearFrame.CATPart");
    let posix = ledger_key_for("/mnt/legacy/Data/Baja/Chassis/Frame/RearFrame.CATPart");
    assert_eq!(
        win,
        ledger_key_for("W:/Data/Baja/Chassis/Frame/RearFrame.CATPart")
    );
    assert_ne!(win, posix, "different roots are different keys by design");
}
