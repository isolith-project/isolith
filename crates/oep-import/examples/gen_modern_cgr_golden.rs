//! P7-M2-T01 - generate the committed importer-hub golden document for the
//! real latched modern CGR mesh (NIST STC-06, V5-6R2023) so the canonical
//! modern CGR -> oep-geo output is byte-lockable when the private corpus is
//! present, and independently re-parseable in CI (no corpus).
//!
//! Usage: OEP_CGR_SAMPLES=<dir containing nist_stc_06_asme1_V56R2023.CATPart>
//!        cargo run -p oep-import --example gen_modern_cgr_golden
//!
//! Writes crates/oep-import/tests/golden/cgr-modern-stc06-import.oepgeo.json.

use std::path::PathBuf;

fn main() {
    let mut found: Option<PathBuf> = None;
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            let cand = PathBuf::from(chunk).join("nist_stc_06_asme1_V56R2023.CATPart");
            if cand.is_file() {
                found = Some(cand);
                break;
            }
        }
    }
    let Some(sample) = found else {
        eprintln!(
            "RED/GREEN REFERENCE MISSING: no nist_stc_06_asme1_V56R2023.CATPart (set OEP_CGR_SAMPLES)"
        );
        std::process::exit(2);
    };
    let bytes = std::fs::read(&sample).expect("read NIST STC-06 CATPart");
    let outcome = oep_import::import_bytes(oep_import::SourceKind::Cgr, &bytes)
        .expect("import does not panic");
    let oep_import::ImportOutcome::Imported { document, .. } = outcome else {
        panic!("STC-06 must import through the modern CGR lane, got {outcome:?}");
    };
    let errors = document.validate();
    assert!(
        errors.is_empty(),
        "STC-06 document must validate: {errors:?}"
    );
    let feat = &document.features[0];
    let count = |k: &str| feat.parameters.get(k).and_then(|v| v.as_u64());
    assert_eq!(count("records"), Some(18));
    assert_eq!(count("triangle_count"), Some(1389));
    assert_eq!(count("index_count"), Some(2003));
    assert_eq!(count("vertex_count"), Some(1037));
    assert_eq!(count("max_index"), Some(1990));
    let json = document.to_json_pretty().expect("serialize canonical JSON");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/cgr-modern-stc06-import.oepgeo.json");
    std::fs::write(&out, json.as_bytes()).expect("write golden");
    println!("wrote golden: {}", out.display());
    println!(
        "parts={} bodies={} features={} | bbox={:?}",
        document.parts.len(),
        document.bodies.len(),
        document.features.len(),
        document.bodies[0].bbox
    );
}
