//! P7-M4-T01  -  generate the committed sketcher golden document: a bracket
//! profile sketch driven by variable sets `width=80`/`height=40` drives a
//! model body, materialized as a canonical `oep-geo` Document.
//!
//! Usage: cargo run -p oep-sketch --example gen_sketch_golden
//!
//! Writes crates/oep-sketch/tests/golden/bracket-sketch-drive.oepgeo.json  -
//! the canonical Document the sketcher produces at width=80 (the solve is
//! deterministic: fresh numeric model, converged to tolerance). The golden is
//! byte-locked by the Rust acceptance test and independently re-parsed by
//! tests/test_p7m4t01_sketch_acceptance.py (stdlib JSON).

use oep_sketch::drive::drive_sketch_to_document;
use oep_sketch::sketch::{DimExpr, Sketch};
use oep_sketch::varset::VariableSet;
use std::path::PathBuf;

fn rect_sketch() -> Sketch {
    let mut s = Sketch::new("bracket-profile");
    let p0 = s.add_point("p0", 0.0, 0.0);
    let p1 = s.add_point("p1", 3.0, 1.0);
    let p2 = s.add_point("p2", 4.0, 4.0);
    let p3 = s.add_point("p3", 1.0, 5.0);
    let l0 = s.add_line("l0", &p0, &p1);
    let l1 = s.add_line("l1", &p1, &p2);
    s.add_line("l2", &p2, &p3);
    s.add_line("l3", &p3, &p0);
    s.constrain_fixed(&p0, 0.0, 0.0);
    s.constrain_horizontal(&l0);
    s.constrain_horizontal("l2");
    s.constrain_vertical(&l1);
    s.constrain_vertical("l3");
    s.constrain_length(&l0, DimExpr::Variable("width".to_string()));
    s.constrain_length(&l1, DimExpr::Variable("height".to_string()));
    s
}

fn main() {
    let sketch = rect_sketch();
    let mut vars = VariableSet::new();
    vars.set("width", 80.0);
    vars.set("height", 40.0);
    let doc = drive_sketch_to_document(
        "oep-part-bracket",
        "bracket-profile",
        &sketch,
        &vars,
        "width",
    )
    .expect("drives a model dimension");
    let errors = doc.validate();
    assert!(errors.is_empty(), "document must validate: {errors:?}");
    let json = doc.to_json_pretty().expect("serialize canonical JSON");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/bracket-sketch-drive.oepgeo.json");
    std::fs::write(&out, json.as_bytes()).expect("write golden");
    println!("wrote golden: {}", out.display());
    println!(
        "parts={} bodies={} features={}",
        doc.parts.len(),
        doc.bodies.len(),
        doc.features.len()
    );
}
