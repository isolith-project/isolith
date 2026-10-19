//! P6-M4-T02 — generate the committed importer-hub golden document from a real
//! license-clean 3DXML (cube1.3dxml) so the canonical `oep-geo` output is
//! independently checkable even in CI (which has no corpus).
//!
//! Usage: OEP_3DXML_SAMPLES=<dir containing cube1.3dxml> cargo run -p oep-import
//!        --example gen_golden
//!
//! Writes crates/oep-import/tests/golden/cube1-import.oepgeo.json — the
//! canonical Document the P3 adapter produces for the real Abaqus cube1 3DXML.
//! The golden is byte-locked by tests/importer_hub.rs and independently
//! re-parsed by tests/test_oep_import_hub_acceptance.py (stdlib JSON).

use std::path::PathBuf;

fn find_cube1() -> Option<PathBuf> {
    for cand in [
        std::env::var("OEP_3DXML_SAMPLES").ok().map(PathBuf::from),
        Some(PathBuf::from("samples")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/data/3dxml")),
    ]
    .into_iter()
    .flatten()
    {
        if cand.join("cube1.3dxml").exists() {
            return Some(cand.join("cube1.3dxml"));
        }
    }
    None
}

fn main() {
    let Some(cube) = find_cube1() else {
        eprintln!("RED/GREEN REFERENCE MISSING: no cube1.3dxml (set OEP_3DXML_SAMPLES)");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&cube).expect("read cube1.3dxml");
    let outcome = oep_import::import_bytes(oep_import::SourceKind::ThreeDXml, &bytes)
        .expect("import does not panic");
    let oep_import::ImportOutcome::Imported { document, .. } = outcome else {
        panic!("cube1 must import, got {outcome:?}");
    };
    let errors = document.validate();
    assert!(
        errors.is_empty(),
        "cube1 document must validate: {errors:?}"
    );
    let json = document.to_json_pretty().expect("serialize canonical JSON");
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/cube1-import.oepgeo.json");
    std::fs::write(&out, json.as_bytes()).expect("write golden");
    println!("wrote golden: {}", out.display());
    println!(
        "parts={} bodies={} assemblies={}",
        document.parts.len(),
        document.bodies.len(),
        document.assemblies.len()
    );
}
