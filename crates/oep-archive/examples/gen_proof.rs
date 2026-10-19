//! Regenerate the committed P10-M3-T02 proof under `docs/P10-M3/proof/`.
//!
//! The proof is the escape-hatch acceptance on real material: a seeded P8
//! vault -> `export-all` archive -> `verify-import` report. Both the archive
//! bytes and the report are committed, so CI (tests/test_p10_m3_export_all.py)
//! validates the committed proof by re-hashing; this example is how a
//! maintainer regenerates it after a real change.
//!
//! Run: cargo run -p oep-archive --example gen_proof

use std::path::Path;

use oep_archive::export_all;
use oep_archive::verify_import;
use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload};
use oep_pdm::store::VaultStore;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture present")
}

fn b64(b: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b)
}

fn proof_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/P10-M3/proof")
}

fn main() {
    let root = proof_root();
    let _ = std::fs::remove_dir_all(&root);
    let vault_dir = root.join("vault-seed");
    let archive_dir = root.join("archive");

    let mut store = VaultStore::new();
    store
        .set_persist_dir(&vault_dir)
        .expect("persist seed vault");

    // Same three-file vault as crates/oep-archive/tests/export_all_contract.rs.
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
                    content: b64(&fixture("box-part.step")),
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
                    content: b64(&fixture("box-mini.step")),
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
                    content: b64(b"hello from an opaque vault blob (customer owns it)"),
                },
            },
        )
        .unwrap();
    store.save().expect("save seed vault");

    let manifest = export_all(&store, &archive_dir, Some("keelworks-proof")).expect("export-all");
    println!(
        "export-all OK: {} artifacts ({} geometry, {} model, {} raw, {} skipped)",
        manifest.artifacts.len(),
        manifest.summary.geometry_artifacts,
        manifest.summary.model_artifacts,
        manifest.summary.raw_artifacts,
        manifest.summary.skipped
    );

    let report =
        verify_import(&archive_dir, Some(&root.join("import-report.json"))).expect("verify-import");
    println!(
        "verify-import OK: step={} reimported={} skipped={} failed={}  ok={}",
        report.summary.total_step,
        report.summary.reimported,
        report.summary.skipped,
        report.summary.failed,
        report.summary.ok
    );
    println!("proof root: {}", root.display());
}
