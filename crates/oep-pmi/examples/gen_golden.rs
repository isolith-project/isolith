//! Bootstrap generator for the committed golden PMI document.
//!
//! Run: `cargo run -p oep-pmi --example gen_golden > crates/oep-pmi/tests/golden/example-bracket.oeppmi.json`
//! (or `cargo -q run ...`). Kept in-repo so the canonical example builder stays
//! reachable on demand; the production path for refreshing the golden is the
//! `OEP_PMI_REGEN=1` test in `tests/pmi_serde.rs`.

fn main() {
    let doc = build();
    print!("{}", doc.to_json_pretty().expect("serialize example"));
}

fn build() -> oep_pmi::PmiDocument {
    use oep_pmi::{
        Characteristic, Datum, DatumFrame, DatumFrameRef, GeometricTolerance, MaterialCondition,
        Note,
    };

    let mut doc = oep_pmi::PmiDocument::new("mm");

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
