//! P4-M4-T01 acceptance: QIF basic-cut round-trip on real MBD fixture.
//!
//! RED-first contract (this file is the acceptance CANARY): the crate must
//! import a real QIF 3.0 MBD document (NIST CTC-01 ASME1 AP242, committed as
//! a dev-only fixture) into the basic-cut [`QifDoc`] model, export it back to
//! deterministic QIF XML, re-import the exported bytes, and land on an equal
//! model (structural round-trip). It also byte-locks the canonical export to
//! the committed golden (regenerate with OEP_QIF_REGEN=1), asserts the NIST
//! fixture's MBD content (3 datums, 14 characteristics across 6 GD&T kinds,
//! mm units, ASME Y14.5 standard) and that `self_check` finds no unresolved
//! references.

use std::path::PathBuf;

use oep_qif::{parse, self_check, write};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn read_golden(name: &str) -> Vec<u8> {
    std::fs::read(golden_dir().join(name))
        .unwrap_or_else(|e| panic!("cannot read golden {name}: {e}"))
}

#[test]
fn real_nist_fixture_round_trips_basic_cut() {
    let bytes = read_golden("nist_ctc_01_asme1_ap242.qif");
    let (doc, report) = parse(&bytes).expect("parse real NIST MBD fixture");

    // Scope of the basic cut: the MBD quality data present in this fixture.
    assert_eq!(doc.datums.len(), 3, "datum A/B/C");
    assert!(doc.datums.iter().any(|d| d.label == "A"));
    assert!(doc.datums.iter().any(|d| d.label == "B"));
    assert!(doc.datums.iter().any(|d| d.label == "C"));
    assert_eq!(doc.frames.len(), 2, "two datum reference frames");
    assert_eq!(
        doc.char_defs.len(),
        14,
        "fourteen characteristic definitions"
    );
    assert_eq!(
        doc.char_nominals.len(),
        14,
        "fourteen characteristic nominals"
    );
    let kinds: std::collections::BTreeSet<_> = doc.char_defs.iter().map(|d| d.kind).collect();
    assert_eq!(kinds.len(), 6, "the six basic-cut GD&T kinds");
    assert!(doc.primary_linear_unit.is_some());
    assert_eq!(doc.primary_linear_unit.as_ref().unwrap().name, "mm");
    assert!(doc
        .standards
        .iter()
        .any(|s| s.org == "ASME" && s.designator.contains("Y14.5")));
    // Product part identity is part of the basic cut.
    assert!(!doc.parts.is_empty(), "part identity carried");

    // The geometry/topology/visualization subtrees must be skipped, honestly.
    assert!(
        report.skipped_sections > 0,
        "large out-of-cut geometry subtrees must be reported as skips"
    );
    assert!(report
        .skips
        .iter()
        .any(|s| s.element == "Product/GeometrySet"
            || s.element.ends_with("/GeometrySet")
            || s.element == "GeometrySet"));
    let problems = self_check(&doc);
    assert!(problems.is_empty(), "self_check clean, got {problems:?}");

    // Round trip: export -> re-import -> equal model.
    let xml = write(&doc).expect("write QIF");
    let (doc2, _r2) = parse(xml.as_bytes()).expect("parse exported QIF");
    assert_eq!(doc2, doc, "exported then re-imported model must equal");
}

#[test]
fn export_is_byte_stable_golden() {
    let bytes = read_golden("nist_ctc_01_asme1_ap242.qif");
    let (doc, _report) = parse(&bytes).unwrap();
    let xml = write(&doc).unwrap();
    let golden = golden_dir().join("nist-mbd-basic-export.qif");
    if std::env::var_os("OEP_QIF_REGEN").is_some() {
        std::fs::write(&golden, &xml).unwrap();
    }
    let committed = read_golden("nist-mbd-basic-export.qif");
    assert_eq!(
        xml.as_bytes(),
        committed.as_slice(),
        "canonical export drifted from committed golden (OEP_QIF_REGEN=1 to refresh)"
    );
}

#[test]
fn canonical_basic_fixture_round_trips() {
    let bytes = read_golden("basic-mbd.qif");
    let (doc, report) = parse(&bytes).expect("parse canonical basic fixture");
    assert!(
        report.skips.is_empty(),
        "canonical basic fixture is fully IN"
    );
    assert_eq!(doc.char_defs.len(), 6);
    assert_eq!(doc.char_nominals.len(), 6);
    assert!(self_check(&doc).is_empty());
    let xml = write(&doc).unwrap();
    let (doc2, _) = parse(xml.as_bytes()).unwrap();
    assert_eq!(doc2, doc);
}

#[test]
fn not_a_qif_document_is_a_typed_error() {
    let err = parse(b"this is not qif").unwrap_err();
    assert!(matches!(err, oep_qif::Error::NotQif(_)), "got {err:?}");
}

#[test]
fn unsupported_version_is_typed() {
    let bytes = read_golden("basic-mbd.qif");
    let text = String::from_utf8_lossy(&bytes).to_string();
    let text2 = text.replace("versionQIF=\"3.0.0\"", "versionQIF=\"5.0.0\"");
    assert_ne!(text2, text, "patch should have applied");
    let err = parse(text2.as_bytes()).unwrap_err();
    assert!(matches!(err, oep_qif::Error::UnsupportedVersion(_)));
}
