//! P11-T04 acceptance (Rust integration): content-hash integrity on vault
//! blobs is enforced at BOTH load and read/retrieve.
//!
//! Acceptance wording: "blob retrieval verifies the stored content hash",
//! "verified by a tampered-blob test that fails verification". This file
//! drives the persisted-store path end to end: save a blob, tamper with the
//! bytes ON DISK (exactly what an attacker or a failing disk leaves behind),
//! then show the store refuses to load it. The in-memory read gate (same
//! guarantee for a tampered in-process store) is unit-tested in store.rs;
//! this file proves the guarantee across the persistence boundary.

use std::path::PathBuf;

use base64::Engine as _;

use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload};
use oep_pdm::store::{StoreError, VaultStore};

/// Scratch dir unique to this process (repo test convention).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oep-pdm-blob-int-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

const PAYLOAD: &[u8] = b"ISO-10303-21; FILE_DESCRIPTION(('blob-integrity-int'),'2;1');";

/// Fresh persisted vault with one released revision whose blob is on disk.
/// Returns (store, item_id, blob_sha256).
fn persisted_seeded_vault(dir: &std::path::Path) -> (VaultStore, String, String) {
    let mut store = VaultStore::new();
    store.set_persist_dir(dir).expect("persist dir");
    let item = store
        .create_item(&CreateItemRequest {
            name: "persisted-blob".into(),
            kind: "part".into(),
            description: None,
            owner: Some("int-test".into()),
            context: Some("p11-t04".into()),
        })
        .expect("create item");
    store
        .create_revision(
            &item.id,
            &CreateRevisionRequest {
                state: Some("released".into()),
                comment: Some("seed".into()),
                created_by: None,
                file: FileUpload {
                    filename: "blob.step".into(),
                    format: "step".into(),
                    content_type: None,
                    content: base64::engine::general_purpose::STANDARD.encode(PAYLOAD),
                },
            },
        )
        .expect("create revision");
    let sha = {
        let (_, file, _) = store.open_latest(&item.id).expect("peek stored file");
        file.blob_sha256
    };
    (store, item.id.clone(), sha)
}

fn blob_file(dir: &std::path::Path, sha: &str) -> PathBuf {
    dir.join("blobs").join(sha)
}

/// Control: save -> reload -> retrieve succeeds and hashes to the stored sha.
#[test]
fn intact_persisted_blob_survives_reload_and_verifies() {
    let dir = scratch("intact");
    let (_store, iid, sha) = persisted_seeded_vault(&dir);

    let reloaded = VaultStore::load(&dir).expect("intact vault reloads");
    let (rev, file, bytes) = reloaded.open_latest(&iid).expect("open after reload");
    assert_eq!(rev, "1");
    assert_eq!(file.blob_sha256, sha);
    assert_eq!(bytes, PAYLOAD);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tampered-blob core acceptance: a single flipped byte in the on-disk
/// blob file makes the store refuse to LOAD it (verification fails, and the
/// corrupt bytes are never served).
#[test]
fn tampered_blob_file_fails_load_with_integrity() {
    let dir = scratch("tamper-load");
    let (_store, _iid, sha) = persisted_seeded_vault(&dir);

    let path = blob_file(&dir, &sha);
    assert!(path.is_file(), "blob must exist on disk under its content hash");
    let mut bytes = std::fs::read(&path).expect("read blob");
    let pos = bytes.len() / 2;
    bytes[pos] ^= 0xFF;
    std::fs::write(&path, &bytes).expect("write tampered blob");

    match VaultStore::load(&dir) {
        Err(StoreError::Integrity(detail)) => {
            assert!(
                detail.contains(&sha),
                "integrity error must name the broken blob: {detail}"
            );
        }
        other => panic!("expected load to fail with Integrity, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
