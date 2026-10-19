//! P4-M2-T02 integration tests: AP242 PMI re-export round-trip.
//!
//! Builds the canonical bracket `oep-pmi` document (the same example whose
//! schema golden lives in `crates/oep-pmi`), exports it to an AP242 PART 21
//! file with [`oep_step::pmi::writer`], re-imports the emitted subset with
//! [`oep_step::pmi::reader`], and checks the round-trip matches **within
//! tolerance** — the "over and back" the card is about.

use oep_pmi::{
    Characteristic, Datum, DatumFrame, DatumFrameRef, GeometricTolerance, MaterialCondition, Note,
    PmiDocument,
};
use oep_step::pmi::reader::read_ap242_pmi;
use oep_step::pmi::writer::{write_ap242_pmi, Ap242Opts};

/// Numeric comparison tolerance: values must round-trip within this relative
/// difference (the writer emits shortest-round-trip reals, so the round-trip
/// is exact for typical values; the tolerance guards float spelling).
const TOL: f64 = 1e-9;

/// Build the canonical example: GD&T + datums + notes on the bracket product.
/// Mirrors `build_example()` in `crates/oep-pmi/tests/pmi_serde.rs` so the two
/// goldens describe the same logical model.
fn build_example() -> PmiDocument {
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

fn export(doc: &PmiDocument) -> String {
    let mut out = Vec::new();
    let opts = Ap242Opts {
        product_name: "example-bracket",
        file_name: "example-bracket-ap242.stp",
        version: "0.1.0",
    };
    write_ap242_pmi(doc, &opts, &mut out).expect("export");
    String::from_utf8(out).expect("utf8")
}

fn assert_close(a: f64, b: f64, ctx: &str) {
    let diff = (a - b).abs();
    assert!(
        diff <= TOL.max(a.abs() * TOL).max(b.abs() * TOL),
        "{ctx}: values differ beyond tolerance: {a} vs {b}"
    );
}

#[track_caller]
fn assert_roundtrip_matches(original: &PmiDocument, back: &PmiDocument) {
    // units
    assert_eq!(original.units(), back.units(), "units mismatch");

    // datums: same ids, labels, feature ids
    assert_eq!(original.datums.len(), back.datums.len(), "datum count");
    for (i, d) in original.datums.iter().enumerate() {
        let b = &back.datums[i];
        assert_eq!(d.id, b.id, "datum {} id", d.id);
        assert_eq!(d.label, b.label, "datum {} label", d.id);
        assert_eq!(d.feature_ids, b.feature_ids, "datum {} features", d.id);
    }

    // frames: same ids, precedence, modifiers
    assert_eq!(original.frames.len(), back.frames.len(), "frame count");
    for (i, f) in original.frames.iter().enumerate() {
        let b = &back.frames[i];
        assert_eq!(f.id, b.id, "frame {} id", f.id);
        assert_eq!(f.refs.len(), b.refs.len(), "frame {} ref count", f.id);
        for (j, r) in f.refs.iter().enumerate() {
            let br = &b.refs[j];
            assert_eq!(r.datum_id, br.datum_id, "frame {} ref {} datum", f.id, j);
            assert_eq!(
                r.precedence, br.precedence,
                "frame {} ref {} precedence",
                f.id, j
            );
            assert_eq!(r.modifier, br.modifier, "frame {} ref {} modifier", f.id, j);
        }
    }

    // tolerances: ids, characteristics, values, zones, modifiers, frames, features
    assert_eq!(
        original.tolerances.len(),
        back.tolerances.len(),
        "tolerance count"
    );
    for (i, t) in original.tolerances.iter().enumerate() {
        let b = &back.tolerances[i];
        assert_eq!(t.id, b.id, "tolerance {} id", t.id);
        assert_eq!(
            t.characteristic, b.characteristic,
            "tolerance {} char",
            t.id
        );
        assert_close(
            t.value,
            b.value,
            &format!("tolerance {} primary value", t.id),
        );
        assert_eq!(
            t.diameter_zone, b.diameter_zone,
            "tolerance {} diameter",
            t.id
        );
        assert_eq!(
            t.material_condition, b.material_condition,
            "tolerance {} mc",
            t.id
        );
        assert_eq!(
            t.frame_id.as_deref(),
            b.frame_id.as_deref(),
            "tolerance {} frame",
            t.id
        );
        match (t.second_value, b.second_value) {
            (Some(a), Some(bb)) => assert_close(a, bb, &format!("tolerance {} second value", t.id)),
            (None, None) => {}
            (a, bb) => panic!(
                "tolerance {} second value mismatch: {:?} vs {:?}",
                t.id, a, bb
            ),
        }
        assert_eq!(t.feature_ids, b.feature_ids, "tolerance {} features", t.id);
    }

    // notes: same ids, text, features, position (position is dropped on
    // export — honest limitation; only compare when both are present)
    assert_eq!(original.notes.len(), back.notes.len(), "note count");
    for (i, n) in original.notes.iter().enumerate() {
        let b = &back.notes[i];
        assert_eq!(n.id, b.id, "note {} id", n.id);
        assert_eq!(n.text, b.text, "note {} text", n.id);
        assert_eq!(n.feature_ids, b.feature_ids, "note {} features", n.id);
        // position is presentation info, dropped on export -> back.position None
        assert!(b.position.is_none(), "note {} position not restored", n.id);
    }
}

/// The full acceptance: build -> export -> re-import -> compare within
/// tolerance, and the model validates cleanly at each stage.
#[test]
fn roundtrip_reimports_exported_pmi_within_tolerance() {
    let original = build_example();
    assert!(original.validate().is_empty(), "source doc must validate");

    let step = export(&original);
    let back = read_ap242_pmi(&step, "mm").expect("re-import exported PMI");
    assert!(back.validate().is_empty(), "re-imported doc must validate");

    assert_roundtrip_matches(&original, &back);
}

/// The exported file must carry every GD&T family we emit, spelled with the
/// real AP242 entity names (so an independent reader sees the same set).
#[test]
fn export_uses_real_ap242_entity_names() {
    let step = export(&build_example());
    // Every one of the 14 characteristics maps to its AP242 MIM tolerance
    // subtype; assert the representative set we use appears verbatim.
    for entity in [
        "POSITION_TOLERANCE",
        "FLATNESS_TOLERANCE",
        "PERPENDICULARITY_TOLERANCE",
        "GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE",
        "GEOMETRIC_TOLERANCE_WITH_MODIFIERS",
        "DATUM_SYSTEM",
        "DATUM_REFERENCE_COMPARTMENT",
        "TOLERANCE_ZONE_FORM('cylindrical or circular')",
        "( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )",
    ] {
        assert!(step.contains(entity), "export missing entity {entity}");
    }
}

/// Determinism: exporting the same document twice yields identical bytes
/// (required for the committed golden byte-lock).
#[test]
fn export_is_deterministic() {
    let doc = build_example();
    assert_eq!(export(&doc), export(&doc));
}

/// Reject a structurally invalid document instead of silently emitting PMI.
#[test]
fn refuses_invalid_document() {
    let mut doc = PmiDocument::new("mm");
    // broken frame: references a datum that does not exist.
    let frame = DatumFrame::new("bad_frame", "X").with_ref(DatumFrameRef::new("ghost", 1));
    doc.push_frame(frame);

    let mut out = Vec::new();
    let opts = Ap242Opts::default();
    let err = write_ap242_pmi(&doc, &opts, &mut out).expect_err("must refuse invalid doc");
    assert!(
        err.to_string().contains("invalid"),
        "expected InvalidDocument, got {err}"
    );
}

const GOLDEN_PATH: &str = "tests/golden/example-bracket-ap242.stp";
const GOLDEN: &str = include_str!("golden/example-bracket-ap242.stp");

/// The writer's output for the canonical example is byte-locked to the
/// committed golden (generated by `crates/oep-step/examples/gen_golden_ap242.rs`
/// with `OEP_STEP_REGEN=1`), and independently re-parsed on the Python side by
/// `tests/test_oep_step_ap242_acceptance.py`. Two-sided: a drift in writer or
/// golden is caught here or there.
#[test]
fn export_matches_committed_golden() {
    let doc = build_example();
    let mut out = Vec::new();
    let opts = Ap242Opts {
        product_name: "example-bracket",
        file_name: "example-bracket-ap242.stp",
        version: "0.1.0",
    };
    write_ap242_pmi(&doc, &opts, &mut out).expect("export");
    let text = String::from_utf8(out).expect("utf8");
    assert_eq!(text, GOLDEN, "writer output drifted from committed golden");
    let _ = GOLDEN_PATH;
}
