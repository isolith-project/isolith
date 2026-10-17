//! P9-M3-T02 batch acceptance: convert jobs run across the corpus, outputs
//! hash-verified in the P8 vault, and the run is reproducible (same artifact
//! hashes across runs). This is the dogfood proof — P2 (oep-cgr) and P4
//! (oep-step) feed the P9 convert lane, whose outputs land back in the P8
//! vault with verified content addresses.

mod common;

use oep_job::spec::{
    ExecutorRef, InputRef, JobSpec, OutputRef, OutputTarget, ResourceCaps, VaultObjectRef,
};
use oep_pdm::adapter::PdmAdapter;
use oep_pdm::service::PdmService;
use oep_worker::batch::{run_batch, BatchManifest};

/// Independent boot helper.
fn boot_vault() -> (PdmService, PdmAdapter) {
    let srv = PdmService::start("127.0.0.1:0").expect("real PdmService boots");
    let mut adapter = PdmAdapter::new(&srv.base_url()).expect("adapter binds");
    let _ = adapter.health().expect("health ok");
    (srv, adapter)
}

fn seed_item(adapter: &mut PdmAdapter, item: &str, fixture: &str, format: &str) {
    let bytes = common::fixture_bytes(fixture);
    let saved = adapter
        .save_revisions_to_pdm(item, fixture, format, bytes, "batch corpus seed")
        .expect("seed item");
    assert!(!saved.blob_sha256.is_empty());
}

struct JobDef {
    name: String,
    function: String,
    input_item: String,
    output_item: String,
    format: String,
}

fn convert_job(def: &JobDef, kind_of: &str) -> JobSpec {
    let mut spec = JobSpec::new(
        ExecutorRef::new("convert", def.function.clone()),
        ResourceCaps::conservative_default(),
    );
    spec.name = Some(def.name.clone());
    spec.description = Some(format!("batch convert job ({kind_of})"));
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::Item {
            item_id: def.input_item.clone(),
        },
    ));
    spec.output.push(OutputRef {
        name: "out".to_string(),
        format: def.format.clone(),
        content_type: None,
        target: Some(OutputTarget::AttachItem {
            item_id: def.output_item.clone(),
        }),
        expected_sha256: None,
        required: true,
    });
    spec
}

fn build_corpus(cgr_present: bool) -> Vec<JobDef> {
    let mut defs = Vec::new();
    // Committed STEP fixtures (CI replayable).
    for (name, input, output) in [
        ("step-box-ref", "corpus-box-ref", "out-box-ref"),
        ("step-box-part", "corpus-box-part", "out-box-part"),
        ("step-assembly", "corpus-assembly", "out-assembly"),
        // A product-only STEP that must classify as a skip, not a failure.
        ("step-doc-only", "corpus-box-mini", "out-box-mini"),
    ] {
        defs.push(JobDef {
            name: name.to_string(),
            function: "step2mesh".to_string(),
            input_item: input.to_string(),
            output_item: output.to_string(),
            format: "oep-mesh".to_string(),
        });
    }

    // Real CGR material (dev-only; gated exactly like the P2/P4 corpus tests).
    if cgr_present {
        defs.push(JobDef {
            name: "cgr-stc06".to_string(),
            function: "cgr2gltf".to_string(),
            input_item: "corpus-stc06".to_string(),
            output_item: "out-stc06-gltf".to_string(),
            format: "gltf".to_string(),
        });
    }
    defs
}

fn seed_local_path(adapter: &mut PdmAdapter, item: &str, path: &std::path::Path, format: &str) {
    let bytes = std::fs::read(path).expect("read real corpus file");
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| item.to_string());
    adapter
        .save_revisions_to_pdm(item, &fname, format, bytes, "batch real-corpus seed")
        .expect("seed real file");
}

#[test]
fn batch_convert_runs_across_corpus_hash_verified_and_reproducible() {
    let (_srv, mut adapter) = boot_vault();

    // Seed committed STEP fixtures.
    seed_item(&mut adapter, "corpus-box-ref", "box-reference.step", "step");
    seed_item(&mut adapter, "corpus-box-part", "box-part.step", "step");
    seed_item(&mut adapter, "corpus-assembly", "assembly.step", "step");
    seed_item(&mut adapter, "corpus-box-mini", "box-mini.step", "step");
    // Seed real CGR when present.
    let cgr_present = common::real_cgr_path().is_some();
    if let Some(path) = common::real_cgr_path() {
        seed_local_path(&mut adapter, "corpus-stc06", &path, "cgr");
    }

    let defs = build_corpus(cgr_present);
    let manifest = BatchManifest {
        format: oep_worker::batch::FORMAT.to_string(),
        version: 1,
        description: Some("P9-M3-T02 corpus batch (test)".to_string()),
        jobs: defs.iter().map(|d| convert_job(d, "test")).collect(),
    };

    let report = run_batch(&manifest, &mut adapter, &_srv.base_url()).expect("batch runs");
    assert_eq!(report.summary.total as usize, defs.len());

    if cgr_present {
        // 3 step converts + 1 cgr convert + 1 skip (box-mini).
        assert_eq!(
            report.summary.ok, 4,
            "expected 4 successful converts: {report}"
        );
        assert_eq!(report.summary.skipped, 1);
    } else {
        eprintln!("SKIP: CGR leg not exercised (set OEP_CGR_SAMPLES)");
        assert_eq!(report.summary.ok, 3);
        assert_eq!(report.summary.skipped, 1);
    }
    assert_eq!(report.summary.failed, 0);
    // Every ok output is hash-verified: vault content address == artifact sha.
    assert_eq!(report.summary.hash_verified, report.summary.ok);

    // Per-record invariants.
    for rec in &report.jobs {
        match rec.status.as_str() {
            "ok" => {
                assert_eq!(rec.vault_blob_sha256, rec.artifact_sha256);
                assert!(!rec.output_revision.is_empty());
                assert!(!rec.vault_blob_sha256.is_empty());
            }
            "skip" => assert!(!rec.note.is_empty()),
            other => panic!("unexpected status {other} in {rec:?}"),
        }
    }

    // Reproducibility: re-running the identical manifest against a fresh
    // vault yields the same artifact content addresses for identical inputs.
    let (_srv2, mut adapter2) = boot_vault();
    seed_item(
        &mut adapter2,
        "corpus-box-ref",
        "box-reference.step",
        "step",
    );
    seed_item(&mut adapter2, "corpus-box-part", "box-part.step", "step");
    seed_item(&mut adapter2, "corpus-assembly", "assembly.step", "step");
    seed_item(&mut adapter2, "corpus-box-mini", "box-mini.step", "step");
    if let Some(path) = common::real_cgr_path() {
        seed_local_path(&mut adapter2, "corpus-stc06", &path, "cgr");
    }
    let report2 =
        run_batch(&manifest, &mut adapter2, &_srv2.base_url()).expect("second batch runs");
    assert_eq!(report2.summary.hash_verified, report2.summary.ok);
    let mut by_name = std::collections::HashMap::new();
    for rec in &report.jobs {
        by_name.insert(rec.job_name.clone(), rec.artifact_sha256.clone());
    }
    for rec in &report2.jobs {
        if let Some(first) = by_name.get(&rec.job_name) {
            assert_eq!(
                first, &rec.artifact_sha256,
                "artifact for {} must be deterministic across runs",
                rec.job_name
            );
        }
    }
}
