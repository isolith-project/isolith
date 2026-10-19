//! P10-M3-T02 acceptance contract (Rust side): export-all writes a documented
//! neutral archive from a real P8 vault, and that archive re-imports into
//! oep-geo.
//!
//! This is the RED->GREEN acceptance for the escape-hatch lane: it runs on
//! real committed STEP material (a faceted box, a geometry-less box, and a
//! non-CAD note) through the platform's own exporters.
//!
//! Checks (all must hold):
//!  1. `export_all` produces manifest / vault-metadata / retention-policy /
//!     checksums and the neutral geometry + model artifacts.
//!  2. Geometry artifacts are truly there and well-formed (STEP byte-equal,
//!     GLB non-empty, STL structurally sized, 3DXML a ZIP package, oep-geo
//!     JSON a valid document).
//!  3. Tolerant failure: a geometry-less STEP and an unknown format are
//!     recorded as skips with reasons, never hard failures.
//!  4. `verify_import` re-imports the archive: vault metadata rebuilds and
//!     the neutral STEP re-imports into oep-geo documents.
//!  5. No proprietary lock-in: raw blobs live under `raw/` apart from the
//!     neutral `geometry/` tree.

use std::path::{Path, PathBuf};

use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload};
use oep_pdm::store::VaultStore;
use sha2::{Digest, Sha256};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

// ---- tiny test helpers ----------------------------------------------------

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture present")
}

fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn sha256(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    format!("{:x}", h.finalize())
}

/// Minimal temp dir with auto-cleanup (no external tempfile dep).
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "oep-archive-{tag}-{}-{nanos:x}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

// ---- fixture vault ----------------------------------------------------------

/// A P8 vault with three items exercising the three visited file kinds.
fn seed_vault() -> VaultStore {
    let mut store = VaultStore::new();
    let box_part = fixture("box-part.step");
    let box_mini = fixture("box-mini.step");
    let note = b"hello from an opaque vault blob (customer owns it)".to_vec();

    let a = store
        .create_item(&CreateItemRequest {
            name: "Box Part A".to_string(),
            kind: "part".to_string(),
            description: Some("a real faceted STEP part".to_string()),
            owner: Some("p10-test".to_string()),
            context: Some("p10-m3".to_string()),
        })
        .unwrap();
    store
        .create_revision(
            &a.id,
            &CreateRevisionRequest {
                state: Some("released".to_string()),
                comment: Some("rev 1".to_string()),
                created_by: Some("p10-test".to_string()),
                file: FileUpload {
                    filename: "box-part.step".to_string(),
                    format: "step".to_string(),
                    content_type: Some("application/step".to_string()),
                    content: b64(&box_part),
                },
            },
        )
        .unwrap();

    let b = store
        .create_item(&CreateItemRequest {
            name: "Box Mini B".to_string(),
            kind: "part".to_string(),
            description: Some("a geometry-less STEP (analytic-only)".to_string()),
            owner: Some("p10-test".to_string()),
            context: None,
        })
        .unwrap();
    store
        .create_revision(
            &b.id,
            &CreateRevisionRequest {
                state: Some("working".to_string()),
                comment: None,
                created_by: Some("p10-test".to_string()),
                file: FileUpload {
                    filename: "box-mini.step".to_string(),
                    format: "step".to_string(),
                    content_type: Some("application/step".to_string()),
                    content: b64(&box_mini),
                },
            },
        )
        .unwrap();

    let c = store
        .create_item(&CreateItemRequest {
            name: "Note C".to_string(),
            kind: "document".to_string(),
            description: Some("a non-CAD vault file".to_string()),
            owner: Some("p10-test".to_string()),
            context: None,
        })
        .unwrap();
    store
        .create_revision(
            &c.id,
            &CreateRevisionRequest {
                state: Some("under_review".to_string()),
                comment: None,
                created_by: Some("p10-test".to_string()),
                file: FileUpload {
                    filename: "note.txt".to_string(),
                    format: "txt".to_string(),
                    content_type: Some("text/plain".to_string()),
                    content: b64(&note),
                },
            },
        )
        .unwrap();

    store
}

/// Assert every checksums.sha256 line re-hashes to the listed digest.
fn assert_checksums(archive: &Path) {
    let sums = read(&archive.join("checksums.sha256"));
    let text = String::from_utf8(sums).expect("checksums utf8");
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split("  ").collect();
        assert_eq!(parts.len(), 2, "checksum line shape: {line:?}");
        let (digest, rel) = (parts[0], parts[1]);
        assert_eq!(sha256(&read(&archive.join(rel))), digest, "rehash {rel}");
    }
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

// ---- acceptance ------------------------------------------------------------

#[test]
fn export_all_writes_a_documented_neutral_archive() {
    let store = seed_vault();
    let tmp = TempDir::new("export");
    let archive = tmp.join("archive");
    let manifest = oep_archive::export_all(&store, &archive, Some("p10-test")).unwrap();

    assert_eq!(manifest.format, "oep-archive");
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.source_vault.owner, "p10-test");
    assert_eq!(manifest.summary.items, 3);
    assert_eq!(manifest.summary.revisions, 3);
    assert_eq!(manifest.summary.files, 3);
    assert_eq!(manifest.summary.raw_artifacts, 3);

    // Root machine dumps exist.
    assert!(archive.join("manifest.json").is_file());
    assert!(archive.join("vault-metadata.json").is_file());
    assert!(archive.join("retention-policy.json").is_file());
    assert!(archive.join("checksums.sha256").is_file());

    // Geometry is really there.
    let box_part = fixture("box-part.step");
    let step_path = archive.join("geometry/box-part-a/1/box-part.step");
    assert!(step_path.is_file(), "STEP passthrough present");
    assert_eq!(read(&step_path), box_part, "STEP passthrough byte-equal");

    let glb = read(&archive.join("geometry/box-part-a/1/box-part.glb"));
    assert!(glb.starts_with(b"glTF"), "GLB magic");
    assert!(glb.len() > 100);

    let stl = read(&archive.join("geometry/box-part-a/1/box-part.stl"));
    let tri_count = u32_le(&stl[80..84]);
    assert!(tri_count > 0, "STL has facets");
    assert_eq!(
        stl.len() as u64,
        84 + tri_count as u64 * 50,
        "STL structural size"
    );

    let arc = read(&archive.join("geometry/box-part-a/1/box-part.3dxml"));
    assert!(arc.starts_with(b"PK"), "3DXML is a ZIP package");

    // oep-geo own-format model proves the re-import path.
    let model_json = read(&archive.join("model/box-part-a/1/box-part.oep-geo.json"));
    let doc: oep_geo::Document = serde_json::from_slice(&model_json).unwrap();
    assert_eq!(doc.format, "oep-geo");
    assert!(!doc.parts.is_empty(), "document has the re-imported part");
    assert_eq!(doc.validate().len(), 0, "document validates");

    // Tolerant failure: the geometry-less STEP still ships its neutral bytes
    // (passthrough), while the derived meshes are recorded skips with reasons.
    let mini_step = archive.join("geometry/box-mini-b/1/box-mini.step");
    assert!(
        mini_step.is_file(),
        "geometry-less STEP passthrough present"
    );
    let mini_skips: Vec<_> = manifest
        .artifacts
        .iter()
        .filter(|r| r.item_name == "Box Mini B" && r.skipped)
        .collect();
    assert!(!mini_skips.is_empty());
    for s in &mini_skips {
        assert!(s.reason.is_some(), "skip carries a reason: {}", s.path);
    }

    // Unknown-format file: raw preserved, derived geometry all recorded skips.
    let raw_note = read(&archive.join("raw/note-c/1/note.txt"));
    assert_eq!(
        raw_note,
        b"hello from an opaque vault blob (customer owns it)"
    );
    let note_skips: Vec<_> = manifest
        .artifacts
        .iter()
        .filter(|r| r.item_name == "Note C" && r.skipped)
        .collect();
    assert_eq!(note_skips.len(), 4, "step/glb/stl/3dxml all recorded skips");

    // Every artifact's sha256 in the manifest matches its bytes on disk.
    for r in &manifest.artifacts {
        if r.skipped {
            continue;
        }
        let bytes = read(&archive.join(&r.path));
        assert_eq!(r.sha256, sha256(&bytes), "manifest sha for {}", r.path);
    }

    assert_checksums(&archive);
}

#[test]
fn archive_reimports_into_oep_geo() {
    let store = seed_vault();
    let tmp = TempDir::new("import");
    let archive = tmp.join("archive");
    oep_archive::export_all(&store, &archive, Some("p10-test")).unwrap();

    let report = oep_archive::verify_import(&archive, None).unwrap();
    assert_eq!(report.format, "oep-import-report");
    assert_eq!(report.version, 1);
    assert!(report.vault.ok);
    assert_eq!(report.vault.items_rebuilt, 3);
    assert_eq!(report.vault.revisions_rebuilt, 3);
    assert_eq!(report.vault.files, 3);

    // Two written STEP files: box-part re-imports, the geometry-less box is a
    // classified skip (P4 tolerant doctrine), nothing fails.
    assert_eq!(report.summary.total_step, 2);
    assert_eq!(report.summary.reimported, 1);
    assert_eq!(report.summary.skipped, 1);
    assert_eq!(report.summary.failed, 0);
    assert!(report.summary.ok, "archive re-imports cleanly");

    let doc_row = report
        .documents
        .iter()
        .find(|d| d.source.ends_with("box-part.step"))
        .expect("box-part re-import row");
    assert_eq!(doc_row.status, "reimported");
    assert!(doc_row.parts >= 1);
    assert!(doc_row.bodies >= 1);
    assert!(doc_row.valid);

    // The report file was written next to the archive.
    assert!(archive.join("import-report.json").is_file());
}

#[test]
fn archive_without_manifest_is_rejected() {
    let tmp = TempDir::new("bad");
    let dir = tmp.join("not-an-archive");
    std::fs::create_dir_all(&dir).unwrap();
    let err = oep_archive::verify_import(&dir, None).unwrap_err();
    assert!(
        err.to_string().contains("missing") || err.to_string().contains("not found"),
        "missing manifest => {err}"
    );
}
