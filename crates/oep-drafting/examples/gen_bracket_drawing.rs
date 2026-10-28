//! EDITOR-2D-DRAFTING — generate the committed drafting goldens.
//!
//! Usage: cargo run -p oep-drafting --example gen_bracket_drawing
//!
//! Writes, under `tests/golden/`:
//! - `bracket-drawing.oepgeo.json` — the canonical `oep-geo` v2 Document
//!   (part + solid body bbox 80x40x6 + `Sketch`/`VariableSet`/`Extrude`
//!   features + the Drawing record: A4 sheet, three standard views, three
//!   variable-driven dimensions) at width=80/height=40/depth=6;
//! - `bracket-drawing.svg` — the rendered vector drawing (projected edges +
//!   title block + dimensions), byte-locked by
//!   `tests/drawing_acceptance.rs` and independently re-parsed by
//!   `tests/test_oep_drafting_acceptance.py`;
//! - `bracket-drawing.pdf` — the same drawing as a minimal PDF 1.4 document.
//!
//! Every coordinate is computed from the model (the feature graph is replayed
//! into a solid mesh and projected) and every dimension label is resolved
//! from the variable set — nothing is fabricated.

use oep_drafting::builder::drive_bracket_drawing;
use oep_drafting::{render_document, write_drawing_pdf, write_drawing_svg};
use oep_sketch::sketch::{DimExpr, Sketch};
use oep_sketch::varset::VariableSet;
use std::path::PathBuf;

/// The acceptance rectangle: bottom-left pinned at (0,0), width/height driven
/// by variable-set entries `width` and `height` (FreeCAD-v1 variable set).
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
    s.constrain_length(&l0, DimExpr::Variable("width".into()));
    s.constrain_length(&l1, DimExpr::Variable("height".into()));
    s
}

fn main() {
    let sketch = rect_sketch();
    let mut vars = VariableSet::new();
    vars.set("width", 80.0);
    vars.set("height", 40.0);
    vars.set("depth", 6.0);

    let driven = drive_bracket_drawing(
        "oep-part-bracket",
        "bracket",
        &sketch,
        &vars,
        "width",
        "height",
        "depth",
        "Bracket",
        Some("ISO-BKT-001"),
        Some("6061-T6"),
        Some("A"),
        Some("oep-drafting"),
        Some("2026-10-22"),
        Some("Isolith"),
    )
    .expect("sketch+extrude drives a model and a drawing");

    let errors = driven.document.validate();
    assert!(errors.is_empty(), "document must validate: {errors:?}");

    let render = render_document(&driven.document, &driven.graph).expect("renderable");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");

    let doc_json = driven
        .document
        .to_json_pretty()
        .expect("serialize canonical JSON");
    std::fs::write(
        manifest.join("bracket-drawing.oepgeo.json"),
        doc_json.as_bytes(),
    )
    .expect("write document golden");

    let mut svg = Vec::new();
    write_drawing_svg(&render, &mut svg).expect("svg");
    std::fs::write(manifest.join("bracket-drawing.svg"), svg).expect("write svg golden");

    let mut pdf = Vec::new();
    write_drawing_pdf(&render, &mut pdf).expect("pdf");
    std::fs::write(manifest.join("bracket-drawing.pdf"), pdf).expect("write pdf golden");

    println!("wrote goldens under {}", manifest.display());
    for dv in &render.dimensions {
        println!(
            "  dim {} (view {}) = {} [variable {:?}, measured {}]",
            dv.id, dv.view_id, dv.label, dv.variable, dv.measured
        );
    }
    for v in &render.views {
        println!(
            "  view {} {} ({}x{} mm) edges={}",
            v.id,
            v.label,
            v.extent_w,
            v.extent_h,
            v.edges_paper.len()
        );
    }
}
