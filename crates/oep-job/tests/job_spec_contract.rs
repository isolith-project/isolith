//! Integration contract for the P9 job/task spec wire format.
//!
//! The committed examples under `docs/P9-M1/examples/` are the shared
//! cross-language fixtures: this Rust test deserializes them and runs full
//! validation (GREEN), and `tests/test_p9_job_spec.py` validates the same
//! bytes against the committed JSON Schema. Both languages peer at the same
//! document — a byte change that breaks one breaks the other, so the examples
//! can never drift from the schema or from what the crate accepts.
//!
//! Also pins serde round-trip stability: `serialize -> deserialize` is value
//! `==`, and optional fields survive the trip.

use oep_job::spec::{ExecutorRef, InputRef, JobSpec, OutputTarget, ResourceCaps, VaultObjectRef};
use oep_job::{spec, validate};

const RENDER_EXAMPLE: &str = concat!(
    include_str!("../../../docs/P9-M1/examples/render-job.json"),
    "\n"
);

#[test]
fn committed_render_example_is_valid() {
    let spec: JobSpec =
        serde_json::from_str(RENDER_EXAMPLE).expect("committed render-job.json must deserialize");
    assert_eq!(spec.format, spec::FORMAT);
    assert_eq!(spec.version, spec::VERSION);
    assert!(spec.is_valid(), "{:?}", spec.validate());
    assert_eq!(spec.p8_input_resources().len(), 2);
    // The render example requests one GPU strictly and a thumbnail output.
    let gpu = spec
        .resources
        .gpu
        .as_ref()
        .expect("render example needs a GPU");
    assert!(gpu.required);
    assert_eq!(gpu.count, 1);
    assert!(spec
        .output
        .iter()
        .any(|o| o.name == "thumbnail" && o.format == "png"));
}

#[test]
fn committed_example_references_a_real_blob() {
    let spec: JobSpec = serde_json::from_str(RENDER_EXAMPLE).unwrap();
    for input in &spec.input {
        if let VaultObjectRef::File { blob_sha256 } = &input.reference {
            assert!(validate::is_blob_address(blob_sha256));
        }
    }
}

#[test]
fn serde_round_trip_is_value_equal_and_deterministic() {
    let mut spec = JobSpec::new(
        ExecutorRef::new("convert", "cgr2gltf"),
        ResourceCaps::conservative_default(),
    );
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::File {
            blob_sha256: "ebb33faa9835e5daf63c81ee9eb146f9619329d62b42b96a8b7b98fd5ed21883".into(),
        },
    ));
    spec.output.push(oep_job::OutputRef::blob("out", "gltf"));
    spec.output[0].target = Some(OutputTarget::AttachItem {
        item_id: "part-1".into(),
    });

    let json = serde_json::to_string_pretty(&spec).unwrap();
    let again: JobSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spec, again, "round-trip must be value-equal");

    let json2 = serde_json::to_string_pretty(&spec).unwrap();
    assert_eq!(
        json, json2,
        "serialization must be deterministic (no HashMap)"
    );
}

#[test]
fn unknown_fields_are_tolerated() {
    // version-1 docs may gain optional fields without a format bump; the
    // reader must ignore them.
    let mut json = RENDER_EXAMPLE.to_string();
    json = json.replacen(
        "\"format\": \"oep-job\"",
        "\"format\": \"oep-job\", \"future_field\": 42",
        1,
    );
    let spec: JobSpec = serde_json::from_str(&json).expect("unknown fields must be tolerated");
    assert!(spec.is_valid(), "{:?}", spec.validate());
}

#[test]
fn tampered_header_is_rejected_at_validation() {
    let mut json = RENDER_EXAMPLE.to_string();
    json = json.replacen("\"version\": 1", "\"version\": 99", 1);
    let spec: JobSpec = serde_json::from_str(&json).unwrap();
    assert!(!spec.is_valid());
    assert_eq!(spec.validate()[0].code, "unsupported-version");
}
