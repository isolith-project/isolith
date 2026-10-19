//! P7-M2-T01 - generate the committed importer-hub golden document for a real
//! STEP part (the committed license-clean `box-reference.step` fixture from
//! the P4 read lane) so the canonical STEP -> oep-geo output is independently
//! checkable in CI (which has no private corpus).
//!
//! Usage: cargo run -p oep-import --example gen_step_golden
//!
//! Writes crates/oep-import/tests/golden/box-reference-import.oepgeo.json -
//! the canonical Document the P4 STEP adapter produces. Byte-locked by
//! tests/importer_hub.rs and independently re-parsed by
//! tests/test_p7m2t01_interchange_workbench.py.

use std::path::PathBuf;

fn main() {
    let step_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/oep-step/tests/golden/box-reference.step");
    let bytes = std::fs::read(&step_path).unwrap_or_else(|e| {
        eprintln!("missing STEP fixture at {}: {e}", step_path.display());
        std::process::exit(2);
    });
    let outcome = oep_import::import_bytes(oep_import::SourceKind::Step, &bytes)
        .expect("import does not panic");
    let oep_import::ImportOutcome::Imported { document, .. } = outcome else {
        panic!("box-reference.step must import, got {outcome:?}");
    };
    let errors = document.validate();
    assert!(errors.is_empty(), "STEP document must validate: {errors:?}");
    let json = document.to_json_pretty().expect("serialize canonical JSON");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/box-reference-import.oepgeo.json");
    std::fs::write(&out, json.as_bytes()).expect("write golden");
    println!("wrote golden: {}", out.display());
    println!(
        "parts={} bodies={} features={} assemblies={}",
        document.parts.len(),
        document.bodies.len(),
        document.features.len(),
        document.assemblies.len()
    );
}
