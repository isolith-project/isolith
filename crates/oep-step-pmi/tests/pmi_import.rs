//! P4-M2-T01 integration tests: STEP AP242 semantic PMI import into the
//! `oep_pmi` model (GD&T, datums, tolerances).
//!
//! Real-material gate: the three NIST MBE PMI Validation fixtures committed
//! under `tests/../fixtures/`. The `"Feature Control Frame (..)"` / NIST test
//! names and the MMC + spherical/cylindrical zone forms were verified by
//! probing step-io against these exact files before this test was written, so
//! every number here is a fact about real material, not an invented ideal.

use oep_pmi::{Characteristic, MaterialCondition, PmiDocument};
use oep_step_pmi::{import, import_strict, Error};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name))
        .expect("fixture readable")
}

/// A real AP242 (edition 2) part with 28 GD&T tolerances + datums (NIST FTC-06).
#[test]
fn golden_ftc06_e2_maps_into_oep_pmi() {
    let imp = import(&fixture("nist_ftc_06_asme1_ap242-e2.stp")).expect("parse real fixture");
    assert!(
        imp.report.pmi_present,
        "fixture carries PMI and must be flagged present"
    );
    assert_eq!(
        imp.report.tolerance_count, 28,
        "real tolerance count observed on this file"
    );
    assert_eq!(
        imp.report.datum_count, 10,
        "real datum count observed on this file"
    );
    assert!(
        imp.report.frame_count >= 8,
        "datum frames built from tolerance datum refs"
    );
    assert!(
        imp.report.dimension_count > 0,
        "dimensional characteristics are counted, not dropped"
    );

    // The mapped document is a valid oep-pmi document (no structural violations).
    let doc: &PmiDocument = &imp.document;
    assert!(
        doc.validate().is_empty(),
        "mapped document must validate clean"
    );
    assert_eq!(doc.format(), "oep-pmi");
    assert_eq!(doc.version(), 1);

    // Spot semantic facts about this part (ASME Y14.5 content):
    //  - a flatness tolerance (form, no datum frame);
    //  - a perpendicularity tolerance referencing a datum;
    //  - position tolerances with MMC (GEOMETRIC_TOLERANCE_WITH_MODIFIERS).
    assert!(
        doc.tolerances
            .iter()
            .any(|t| t.characteristic == Characteristic::Flatness && t.frame_id.is_none()),
        "flatness (datum-free form tolerance) present"
    );
    assert!(
        doc.tolerances
            .iter()
            .any(|t| t.characteristic == Characteristic::Perpendicularity && t.frame_id.is_some()),
        "perpendicularity with a datum frame present"
    );
    let m = doc
        .tolerances
        .iter()
        .find(|t| t.material_condition == Some(MaterialCondition::Mmc));
    assert!(m.is_some(), "position tolerance with MMC modifier present");
    if let Some(m) = m {
        assert!(m.characteristic == Characteristic::Position);
        assert!(
            m.frame_id.is_some(),
            "MMC position tolerance references a frame"
        );
    }
    // A diameter zone is present on a position tolerance (cylindrical/spherical form).
    assert!(
        doc.tolerances.iter().any(|t| t.diameter_zone),
        "a position tolerance carries a diameter (cylindrical) zone"
    );
}

/// A real AP242 (edition 3) part with 25 GD&T tolerances + 6 datums (NIST STC-06).
#[test]
fn golden_stc06_e3_maps_into_oep_pmi() {
    let imp = import(&fixture("nist_stc_06_asme1_ap242-e3.stp")).expect("parse real fixture");
    assert!(imp.report.pmi_present);
    assert!(
        imp.report.tolerance_count >= 20,
        "real tolerance count on this file"
    );
    assert_eq!(
        imp.report.datum_count, 6,
        "real datum count observed on this file"
    );
    assert!(imp.document.validate().is_empty());
    assert!(
        imp.document
            .tolerances
            .iter()
            .any(|t| t.characteristic == Characteristic::Perpendicularity),
        "perpendicularity tolerances mapped"
    );
    assert!(
        imp.document
            .tolerances
            .iter()
            .any(|t| t.characteristic == Characteristic::ProfileOfSurface),
        "surface profile tolerances mapped"
    );
}

/// Real AP242 part with NO PMI (NIST FTC-08, tessellated geometry only):
/// the import must honestly flag absence, never fabricate PMI.
#[test]
fn fixture_without_pmi_is_honestly_flagged() {
    let imp = import(&fixture("nist_ftc_08_asme1_ap242-e4-tg.stp")).expect("parse real fixture");
    assert!(
        !imp.report.pmi_present,
        "this fixture has no PMI and must be flagged absent"
    );
    assert_eq!(imp.report.tolerance_count, 0);
    assert_eq!(imp.report.datum_count, 0);
    assert_eq!(imp.report.frame_count, 0);
    assert!(
        imp.document.validate().is_empty(),
        "empty-but-valid document, nothing fabricated"
    );
    assert!(imp.no_pmi(), "no_pmi() helper reports absence");

    // Strict mode turns honest absence into a typed error.
    let strict = import_strict(&fixture("nist_ftc_08_asme1_ap242-e4-tg.stp"));
    assert!(
        matches!(strict, Err(Error::NoPmi { .. })),
        "strict import rejects a PMI-less fixture"
    );
}

/// The mapped document round-trips deterministically through the oep-pmi format.
#[test]
fn mapped_document_round_trips_deterministically() {
    let imp = import(&fixture("nist_stc_06_asme1_ap242-e3.stp")).expect("parse");
    let json = serde_json::to_string_pretty(&imp.document).expect("serialize");
    let back: PmiDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(imp.document, back, "round trip is the value-level identity");
    // Deterministic bytes: serialize twice => identical output.
    let json2 = serde_json::to_string_pretty(&imp.document).expect("serialize again");
    assert_eq!(json, json2, "serialization is deterministic");
}

/// The mapped document's id space is unique across all four collections and
/// frame refs/tolerance frame ids resolve (oep-pmi structural invariants), so
/// the import is usable by the importer hub.
#[test]
fn mapped_document_id_discipline() {
    let imp = import(&fixture("nist_ftc_06_asme1_ap242-e2.stp")).expect("parse");
    let doc = &imp.document;

    let mut ids: Vec<&str> = Vec::new();
    for d in &doc.datums {
        ids.push(&d.id);
    }
    for f in &doc.frames {
        ids.push(&f.id);
    }
    for t in &doc.tolerances {
        ids.push(&t.id);
    }
    for n in &doc.notes {
        ids.push(&n.id);
    }
    let mut uniq = ids.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        ids.len(),
        "one document = one id space; no duplicate ids"
    );

    let frame_ids: std::collections::HashSet<&str> =
        doc.frames.iter().map(|f| f.id.as_str()).collect();
    for t in &doc.tolerances {
        if let Some(fid) = &t.frame_id {
            assert!(
                frame_ids.contains(fid.as_str()),
                "tolerance frame_id resolves: {fid}"
            );
        }
    }
    let datum_ids: std::collections::HashSet<&str> =
        doc.datums.iter().map(|d| d.label.as_str()).collect();
    for f in &doc.frames {
        for r in &f.refs {
            assert!(
                r.precedence >= 1 && r.precedence <= 3,
                "precedence in 1..=3"
            );
            let label = doc
                .datums
                .iter()
                .find(|d| d.id == r.datum_id)
                .map(|d| d.label.as_str());
            assert!(label.is_some(), "frame ref datum_id resolves");
            if let Some(l) = label {
                assert!(l.len() == 1, "datum label is a single letter: {l}");
            }
        }
        assert!(datum_ids.len() >= f.refs.len());
    }
}

/// Junk / truncated / non-STEP input gives a typed error, never a panic.
#[test]
fn junk_input_errors_never_panic() {
    assert!(matches!(import(b""), Err(Error::NotStep { .. })));
    assert!(matches!(
        import(b"hello world this is not a step file"),
        Err(Error::NotStep { .. })
    ));
    let mut truncated = fixture("nist_stc_06_asme1_ap242-e3.stp");
    truncated.truncate(truncated.len() / 2);
    // Truncation may parse-fail or parse with a degraded model; either way it
    // must be a typed result, never a panic.
    let _ = import(&truncated);
    let head = b"ISO-10303-21;\nHEADER;\nENDSEC;".to_vec();
    // A header-only stub (no DATA) may parse or reject; either way it must be
    // a typed result, never a panic.
    match import(&head) {
        Ok(_) | Err(_) => {}
    }
}
