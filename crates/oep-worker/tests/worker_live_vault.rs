//! P9-M3-T02 worker integration: a convert JobSpec executes against the LIVE
//! P8 vault and its output is stored + hash-verified there.
//!
//! Boots the real axum `PdmService` on an ephemeral port (same pattern as
//! P8-M6-T02's adapter acceptance), seeds corpus inputs as vault items, and
//! drives `oep_worker::run_job`. The P9-M5-T02 GA-gate contract is enforced
//! here: the vault's own `FileRef.blob_sha256` must equal the job's artifact
//! sha-256, and a declared `expected_sha256` must match too.

mod common;

use oep_job::spec::{
    ExecutorRef, InputRef, JobSpec, OutputRef, OutputTarget, ResourceCaps, VaultObjectRef,
};
use oep_pdm::adapter::PdmAdapter;
use oep_pdm::service::PdmService;
use oep_worker::{run_job, JobStatus, WorkerError};

fn boot_vault() -> (PdmService, PdmAdapter) {
    let srv = PdmService::start("127.0.0.1:0").expect("real PdmService boots");
    let mut adapter = PdmAdapter::new(&srv.base_url()).expect("adapter binds");
    let _ = adapter.health().expect("health ok");
    (srv, adapter)
}

fn seed_step_item(adapter: &mut PdmAdapter, item: &str, fixture: &str) {
    let bytes = common::fixture_bytes(fixture);
    let saved = adapter
        .save_revisions_to_pdm(item, fixture, "step", bytes, "P9-M3-T02 seed")
        .expect("seed item");
    assert!(!saved.blob_sha256.is_empty());
}

fn convert_spec(name: &str, function: &str, input_item: &str, output_item: &str) -> JobSpec {
    let mut spec = JobSpec::new(
        ExecutorRef::new("convert", function),
        ResourceCaps::conservative_default(),
    );
    spec.name = Some(name.to_string());
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::Item {
            item_id: input_item.to_string(),
        },
    ));
    spec.output.push(OutputRef {
        name: "mesh".to_string(),
        format: match function {
            "cgr2gltf" => "gltf".to_string(),
            _ => "oep-mesh".to_string(),
        },
        content_type: None,
        target: Some(OutputTarget::AttachItem {
            item_id: output_item.to_string(),
        }),
        expected_sha256: None,
        required: true,
    });
    spec
}

#[test]
fn step_to_mesh_job_stores_and_hash_verifies_in_vault() {
    let (_srv, mut adapter) = boot_vault();
    seed_step_item(&mut adapter, "box-ref", "box-reference.step");

    let spec = convert_spec("step-box-ref", "step2mesh", "box-ref", "box-ref-mesh");
    assert!(spec.is_valid(), "spec valid: {:?}", spec.validate());

    let outcome = run_job(&spec, &mut adapter, "jb-1").expect("step2mesh job runs ok");
    assert_eq!(outcome.status, JobStatus::Ok);
    assert_eq!(outcome.output_format, "oep-mesh");
    // The vault's recorded content address equals the artifact sha-256.
    assert_eq!(outcome.vault_blob_sha256, outcome.artifact_sha256);
    assert_ne!(outcome.output_revision, "");
    assert_eq!(outcome.output_item, "box-ref-mesh");

    // Read back through the vault: the stored file re-hashes to the record.
    let opened = adapter
        .open_from_pdm("box-ref-mesh")
        .expect("output readable from vault");
    assert_eq!(common::sha256_hex(&opened.bytes), outcome.artifact_sha256);
    assert_eq!(opened.blob_sha256, outcome.artifact_sha256);
}

#[test]
fn step_to_mesh_expected_sha256_must_match() {
    let (_srv, mut adapter) = boot_vault();
    seed_step_item(&mut adapter, "box-ref2", "box-reference.step");

    let mut spec = convert_spec("step-box-ref2", "step2mesh", "box-ref2", "box-ref2-mesh");
    // Hash the intended artifact directly, then pin it as expected_sha256.
    let bytes = common::fixture_bytes("box-reference.step");
    let expected = oep_worker::step_to_mesh(&bytes, "box-reference.step")
        .expect("pre-convert")
        .sha256;
    spec.output[0].expected_sha256 = Some(expected.clone());
    let outcome = run_job(&spec, &mut adapter, "jb-2").expect("pinned-sha job succeeds");
    assert_eq!(outcome.status, JobStatus::Ok);
    assert_eq!(outcome.artifact_sha256, expected);

    // A wrong pin must fail with a typed hash mismatch.
    spec.output[0].expected_sha256 = Some("0".repeat(64));
    match run_job(&spec, &mut adapter, "jb-2b") {
        Err(WorkerError::OutputHashMismatch { .. }) => {}
        other => panic!("wrong expected_sha256 must be a hash mismatch, got: {other:?}"),
    }
}

#[test]
fn cgr_to_gltf_job_real_material_gated() {
    let Some(path) = common::real_cgr_path() else {
        eprintln!("SKIP: no real CGR material (set OEP_CGR_SAMPLES)");
        return;
    };
    let bytes = std::fs::read(&path).expect("read cgr");
    let (_srv, mut adapter) = boot_vault();
    adapter
        .save_revisions_to_pdm(
            "stc06",
            "nist_stc_06_asme1_V56R2023.CATPart",
            "cgr",
            bytes,
            "seed",
        )
        .expect("seed cgr item");

    let spec = convert_spec("cgr-stc06", "cgr2gltf", "stc06", "stc06-gltf");
    let outcome = run_job(&spec, &mut adapter, "jb-3").expect("cgr2gltf job ok");
    assert_eq!(outcome.status, JobStatus::Ok);
    assert_eq!(outcome.output_format, "gltf");
    assert_eq!(outcome.vault_blob_sha256, outcome.artifact_sha256);
    let opened = adapter
        .open_from_pdm("stc06-gltf")
        .expect("glb readable from vault");
    assert_eq!(&opened.bytes[0..4], b"glTF");
    assert_eq!(opened.blob_sha256, outcome.artifact_sha256);
}

#[test]
fn non_convertible_input_is_classified_skip_not_failure() {
    let (_srv, mut adapter) = boot_vault();
    adapter
        .save_revisions_to_pdm(
            "doc-only",
            "box-mini.step",
            "step",
            common::fixture_bytes("box-mini.step"),
            "seed",
        )
        .expect("seed doc-only step item");

    let spec = convert_spec("step-doc-only", "step2mesh", "doc-only", "doc-only-mesh");
    let outcome =
        run_job(&spec, &mut adapter, "jb-4").expect("skip is a typed outcome, not an error");
    match &outcome.status {
        JobStatus::Skipped(note) => assert!(!note.is_empty()),
        other => panic!("product-only input should be a classified skip, got {other:?}"),
    }
}

#[test]
fn invalid_spec_is_rejected_before_vault_touch() {
    let (_srv, mut adapter) = boot_vault();
    // Executor kind outside the closed vocabulary -> invalid spec.
    let mut spec = JobSpec::new(
        oep_job::spec::ExecutorRef::new("convert", "no-such-worker"),
        ResourceCaps::conservative_default(),
    );
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::Item {
            item_id: "whatever".to_string(),
        },
    ));
    spec.output.push(OutputRef {
        name: "mesh".to_string(),
        format: "oep-mesh".to_string(),
        content_type: None,
        target: Some(OutputTarget::AttachItem {
            item_id: "whatever-out".to_string(),
        }),
        expected_sha256: None,
        required: true,
    });
    match run_job(&spec, &mut adapter, "jb-5") {
        Err(WorkerError::UnknownWorker(_)) => {}
        other => panic!("unknown worker should be typed, got: {other:?}"),
    }
}
