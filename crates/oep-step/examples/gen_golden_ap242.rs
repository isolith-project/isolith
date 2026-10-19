//! P4-M2-T02 example: export the canonical bracket `oep-pmi` document to AP242.
//! Used to (re)generate the committed golden (`tests/golden/example-bracket-ap242.stp`)
//! and as a human-inspectable sample of the emitted semantic PMI.
//!
//! Usage:
//!   OEP_STEP_REGEN=1 cargo run -p oep-step --example gen_golden_ap242

use oep_pmi::{
    Characteristic, Datum, DatumFrame, DatumFrameRef, GeometricTolerance, MaterialCondition, Note,
    PmiDocument,
};
use oep_step::pmi::writer::{write_ap242_pmi, Ap242Opts};
use std::io::Write as _;

pub fn build_example() -> PmiDocument {
    let mut doc = PmiDocument::new("mm");

    let datum_a = Datum::new("datum_A", "A").with_feature("oep-feature-bracket-extrude");
    let datum_b = Datum::new("datum_B", "B").with_feature("oep-feature-bracket-hole");
    let datum_c = Datum::new("datum_C", "C").with_feature("oep-feature-bracket-hole");
    for d in [&datum_a, &datum_b, &datum_c] {
        doc.push_datum(d.clone());
    }

    let drf = DatumFrame::new("drf_abc", "A-B-C")
        .with_ref(DatumFrameRef::new("datum_A", 1).with_modifier(MaterialCondition::Rfs))
        .with_ref(DatumFrameRef::new("datum_B", 2).with_modifier(MaterialCondition::Rfs))
        .with_ref(DatumFrameRef::new("datum_C", 3));
    doc.push_frame(drf);

    let pos = GeometricTolerance::new("gt_1", Characteristic::Position, 0.2)
        .with_diameter_zone(true)
        .with_material_condition(MaterialCondition::Mmc)
        .with_frame("drf_abc")
        .with_feature("oep-feature-bracket-hole");
    let flat = GeometricTolerance::new("gt_flat", Characteristic::Flatness, 0.05)
        .with_feature("oep-feature-bracket-extrude");
    let perp = GeometricTolerance::new("gt_perp", Characteristic::Perpendicularity, 0.1)
        .with_material_condition(MaterialCondition::Rfs)
        .with_frame("drf_abc")
        .with_feature("oep-feature-bracket-hole");
    for t in [&pos, &flat, &perp] {
        doc.push_tolerance(t.clone());
    }

    let hole_note = Note::new("note_holes", "4X Ø6.00 THRU")
        .with_feature("oep-feature-bracket-hole")
        .with_position([40.0, 20.0, 0.0]);
    let units_note = Note::new("note_units", "ALL DIMENSIONS IN MM");
    doc.push_note(hole_note);
    doc.push_note(units_note);

    doc
}

fn main() {
    let doc = build_example();
    let mut out = Vec::new();
    let opts = Ap242Opts {
        product_name: "example-bracket",
        file_name: "example-bracket-ap242.stp",
        version: "0.1.0",
    };
    // Refuse to write a golden for an invalid model (honest gate).
    assert!(doc.validate().is_empty(), "example doc must validate");
    write_ap242_pmi(&doc, &opts, &mut out).expect("export");

    if std::env::var("OEP_STEP_REGEN").as_deref() == Ok("1") {
        let path = "crates/oep-step/tests/golden/example-bracket-ap242.stp";
        std::fs::create_dir_all("crates/oep-step/tests/golden").expect("mkdir");
        std::fs::write(path, &out).expect("write golden");
        eprintln!("wrote {path}");
    } else {
        // print to stdout for inspection
        std::io::stdout().write_all(&out).expect("stdout");
    }
}
