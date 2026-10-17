//! P9-M3-T02 unit contract: the pure convert worker functions are total,
//! deterministic, and honest (typed skip, never fabrication).
//!
//! Run: `cargo test -p oep-worker --test convert_functions`.
//! The CGR end-to-end case is gated on local real material (OEP_CGR_SAMPLES),
//! exactly like the P2/P4 corpus tests — real CGR binaries stay out of the
//! shared repo (clean-room invariant).

mod common;

use oep_worker::convert::{cgr_to_gltf, step_to_mesh, ConvertError};

// ----- CGR -> glTF ----------------------------------------------------------

#[test]
fn cgr_to_gltf_real_modern_material_gated() {
    let Some(path) = common::real_cgr_path() else {
        eprintln!("SKIP: no real CGR material (set OEP_CGR_SAMPLES)");
        return;
    };
    let bytes = std::fs::read(&path).expect("read real CGR material");
    let artifact = cgr_to_gltf(&bytes, &path.to_string_lossy()).expect("real CGR converts");
    assert_eq!(artifact.format, "gltf");
    assert!(artifact.filename.ends_with(".glb"));
    assert!(artifact.content_type.contains("gltf"));
    // The P2-M7 modern anchor counts (NIST V5-6R2023 STC-06, 2 readers agree).
    assert_eq!(artifact.vertex_count, 1037);
    assert_eq!(artifact.triangle_count, 1389);
    // GLB magic.
    assert_eq!(&artifact.bytes[0..4], b"glTF");
    // Deterministic content address + self-verification.
    let again = cgr_to_gltf(&bytes, &path.to_string_lossy()).expect("rerun");
    assert_eq!(artifact.sha256, again.sha256);
    assert!(artifact.verify_sha256());
}

#[test]
fn cgr_to_gltf_garbage_is_typed_not_panic() {
    for input in [
        b"".as_slice(),
        b"hello world".as_slice(),
        b"\x00\x01\x02\x03".as_slice(),
    ] {
        match cgr_to_gltf(input, "junk.bin") {
            Err(ConvertError::NotCgr) | Err(ConvertError::ClassifiedSkip(_)) => {}
            other => panic!("garbage should be a typed non-convert, got: {other:?}"),
        }
    }
}

// ----- STEP -> mesh ----------------------------------------------------------

#[test]
fn step_to_mesh_committed_fixture_deterministic() {
    let bytes = common::fixture_bytes("box-reference.step");
    let artifact = step_to_mesh(&bytes, "box-reference.step").expect("box-reference converts");
    assert_eq!(artifact.format, "oep-mesh");
    assert!(artifact.filename.ends_with(".oepmesh.json"));
    assert_eq!(artifact.vertex_count, 8);
    assert_eq!(artifact.triangle_count, 12);
    assert!(artifact.verify_sha256());

    // Deterministic bytes (content address stable across runs).
    let again = step_to_mesh(&bytes, "box-reference.step").expect("rerun");
    assert_eq!(artifact.sha256, again.sha256);
    assert_eq!(artifact.bytes, again.bytes);

    // The document is valid, parseable, well-formed oep-mesh v1.
    let doc: serde_json::Value = serde_json::from_slice(&artifact.bytes).expect("valid JSON");
    assert_eq!(doc["format"], "oep-mesh");
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["vertex_count"], 8);
    assert_eq!(doc["triangle_count"], 12);
    assert!(doc["bbox"]["min"][0].is_f64());
}

#[test]
fn step_to_mesh_multi_fixtures_all_deterministic() {
    let sources = ["box-reference.step", "box-part.step", "assembly.step"];
    let mut seen = std::collections::HashSet::new();
    for name in sources {
        let bytes = common::fixture_bytes(name);
        let artifact =
            step_to_mesh(&bytes, name).unwrap_or_else(|e| panic!("fixture converts: {name}: {e}"));
        assert!(artifact.verify_sha256());
        // Distinct fixtures produce distinct artifacts (no accidental alias).
        assert!(
            seen.insert(artifact.sha256.clone()),
            "fixture {} should hash uniquely",
            name
        );
    }
}

#[test]
fn step_to_mesh_no_faceted_geometry_is_skip_not_panic() {
    let bytes = common::fixture_bytes("box-mini.step"); // product-only STEP
    match step_to_mesh(&bytes, "box-mini.step") {
        Err(ConvertError::ClassifiedSkip(_)) => {}
        other => panic!("product-only STEP should be a classified skip, got: {other:?}"),
    }
}

#[test]
fn step_to_mesh_garbage_is_typed_not_panic() {
    for input in [
        b"".as_slice(),
        b"ISO-10303".as_slice(),
        b"<not step>".as_slice(),
    ] {
        match step_to_mesh(input, "junk.stp") {
            Err(ConvertError::NotStep) | Err(ConvertError::ClassifiedSkip(_)) => {}
            other => panic!("garbage should be a typed non-convert, got: {other:?}"),
        }
    }
}
