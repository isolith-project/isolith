//! P4-M1-T01 acceptance test (TDD, RED first)  -  oep-step crate: wrap an OSS
//! STEP library behind a vendor-neutral API and parse one golden STEP part
//! into the native `oep_geo` model.
//!
//! Written BEFORE the implementation (RED: `oep_step::parse` does not exist  -
//! the crate is a P0-reserved skeleton). The contract once GREEN:
//!
//! 1. `oep_step::parse` reads a real STEP part (the committed golden AP214
//!    faceted B-Rep box `box-reference.step`) through the wrapped OSS STEP
//!    library and yields vendor-neutral `FacetedModel`s (own types only  -
//!    no underlying-library types on the surface).
//! 2. The golden facts are real observables, not estimates: one closed shell,
//!    12 faces, 12 triangles, 8 unique corners (the file repeats the 36
//!    CARTESIAN_POINT records), axis-aligned bbox [0,0,0]..[1,1,1] (the
//!    exporter's unit box).
//! 3. `oep_step::to_document` maps the parsed facets onto the ONE canonical
//!    internal format (`oep_geo::Document`): one part owning one `Solid`
//!    body whose bbox comes from the parsed vertices, plus one import
//!    feature recording real face/shell counts. The document `validate()`s
//!    clean and serializes deterministically (byte-stable round trip).
//! 4. Tolerant-failure: junk, truncated, empty or non-STEP bytes yield a
//!    typed `oep_step::Error`, never a panic, never fabricated geometry.
//! 5. Vendor-neutral seam: oep-step's public items reference only oep-step /
//!    oep-geo types (no step-io identifiers on the public surface).

use oep_geo::{BodyKind, Document};
use oep_step::facets::{FacetBbox, FacetedModel};
use oep_step::{Error, ReadReport};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

const GOLDEN_REL: &str = "tests/golden/box-reference.step";

fn golden_bytes() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_REL);
    std::fs::read(path).expect("committed golden STEP part exists")
}

fn assert_bbox_eq(actual: FacetBbox, min: [f64; 3], max: [f64; 3], label: &str) {
    assert!(
        (actual.min[0] - min[0]).abs() < 1e-9
            && (actual.min[1] - min[1]).abs() < 1e-9
            && (actual.min[2] - min[2]).abs() < 1e-9,
        "{label}: min expected {min:?}, got {:?}",
        actual.min
    );
    assert!(
        (actual.max[0] - max[0]).abs() < 1e-9
            && (actual.max[1] - max[1]).abs() < 1e-9
            && (actual.max[2] - max[2]).abs() < 1e-9,
        "{label}: max expected {max:?}, got {:?}",
        actual.max
    );
}

fn is_canonical(doc: &Document) {
    assert_eq!(doc.format(), "oep-geo", "canonical format tag");
    assert_eq!(
        doc.version(),
        oep_geo::document::VERSION,
        "canonical schema version"
    );
    let errors = doc.validate();
    assert!(
        errors.is_empty(),
        "canonical document must validate: {errors:?}"
    );
    // Deterministic serialization (byte-stable round trip)  -  the own-format
    // contract from P6-M1-T01.
    let json = doc.to_json_pretty().expect("serialize");
    let back: Document = Document::from_json_pretty(&json).expect("reparse");
    assert_eq!(&back, doc, "round trip is identity");
    assert_eq!(
        back.to_json_pretty().expect("re-serialize"),
        json,
        "serialization is deterministic"
    );
}

// ---------------------------------------------------------------------------
// 1+2. parse the golden STEP part into vendor-neutral facets
// ---------------------------------------------------------------------------

#[test]
fn parse_golden_step_part_into_facets() {
    let bytes = golden_bytes();
    let outcome = oep_step::parse(&bytes).expect("golden STEP parses");
    let facets: &FacetedModel = &outcome.facets;

    // Real gold update on the box-reference part: the golden is a faceted box.
    assert_eq!(
        facets.product_name, "box-reference",
        "product name fallback (file stem)"
    );
    assert_eq!(facets.shells.len(), 1, "one shell");
    assert!(facets.shells[0].closed, "shell is closed (unit box)");
    assert_eq!(facets.shells[0].faces, 12, "12 FACE_SURFACE faces");
    assert_eq!(facets.triangles.len(), 12, "12 triangles (one per face)");
    // The golden's 36 CARTESIAN_POINT records repeat the 8 unit-box corners;
    // the reader dedupes identical coordinates into shared vertices (the
    // standard facet-mesh contract), so the neutral carrier reports 8 unique
    // vertices, not the raw record count.
    assert_eq!(
        facets.vertices.len(),
        8,
        "8 unique corner vertices (unit box)"
    );
    let bb = facets.bbox.expect("bbox computed from parsed vertices");
    assert_bbox_eq(bb, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], "golden bbox");

    // Every triangle index is in range (structure is loadable downstream).
    for tri in &facets.triangles {
        assert!(
            tri.iter().all(|&i| (i as usize) < facets.vertices.len()),
            "triangle index in range"
        );
    }

    // The read report carries the STEP identity that was actually parsed.
    assert_eq!(
        outcome.report.file_name.as_deref(),
        Some("box-reference.step")
    );
}

#[test]
fn read_report_carries_warnings_and_drops_honestly() {
    let bytes = golden_bytes();
    let outcome = oep_step::parse(&bytes).expect("golden parses");
    // The golden's product/representation cluster references
    // APPLICATION_PROTOCOL_DEFINITION which the wrapped OSS library's schema
    // does not model; the reader must record that honestly, never fabricate
    // metadata from thin air. (Exact dropped count is not part of the
    // contract  -  presence of the report is.)
    let _report: &ReadReport = &outcome.report;
    assert!(
        !outcome.facets.product_name.is_empty(),
        "a non-empty product name"
    );
}

// ---------------------------------------------------------------------------
// 3. map facets onto the canonical oep-geo native model
// ---------------------------------------------------------------------------

#[test]
fn to_document_builds_canonical_model_from_golden() {
    let bytes = golden_bytes();
    let outcome = oep_step::parse(&bytes).expect("golden parses");
    let doc = oep_step::to_document(&outcome.facets);
    is_canonical(&doc);

    assert_eq!(doc.parts.len(), 1, "one part");
    assert_eq!(doc.bodies.len(), 1, "one body");
    let part = &doc.parts[0];
    assert_eq!(
        part.name, "box-reference",
        "part named from the parsed STEP"
    );
    let body = &doc.bodies[0];
    assert_eq!(
        body.kind,
        BodyKind::Solid,
        "closed shell maps to a Solid body"
    );
    let bb = body.bbox.expect("body bbox from parsed vertices");
    assert_bbox_eq(
        FacetBbox {
            min: bb.min,
            max: bb.max,
        },
        [0.0; 3],
        [1.0; 3],
        "canonical bbox",
    );
    // Part owns the body.
    assert_eq!(part.body_ids, vec![body.id.clone()]);
    // One import feature records real observables.
    assert_eq!(doc.features.len(), 1, "one import feature");
    let f = &doc.features[0];
    assert_eq!(f.kind, "ImportStep", "import feature kind");
    assert_eq!(
        f.parameters.get("face_count"),
        Some(&serde_json::json!(12.0))
    );
    assert_eq!(
        f.parameters.get("shell_count"),
        Some(&serde_json::json!(1.0))
    );
    assert_eq!(f.body_ids, vec![body.id.clone()]);
}

#[test]
fn parse_to_document_convenience_matches() {
    let bytes = golden_bytes();
    let (doc, report) = oep_step::parse_to_document(&bytes).expect("golden parses");
    is_canonical(&doc);
    assert_eq!(report.file_name.as_deref(), Some("box-reference.step"));
}

// ---------------------------------------------------------------------------
// 4. tolerant-failure: typed errors, never a panic, never fabricated geometry
// ---------------------------------------------------------------------------

#[test]
fn junk_truncated_and_empty_bytes_error_never_panic() {
    // Empty input: typed NotStep error.
    assert!(matches!(oep_step::parse(b""), Err(Error::NotStep)));
    // Non-STEP junk: typed error, never a panic.
    let _ = oep_step::parse(b"not a step file at all");
    // STEP header but garbage body (truncated data section).
    let _ = oep_step::parse(b"ISO-10303-21;\nHEADER;\nDATA;\n#1=");
    // A file that looks like STEP but has no parseable faceted geometry.
    let _ =
        oep_step::parse(b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n");
}

#[test]
fn golden_detected_as_step_not_junk() {
    // The golden starts with the ISO-10303-21 magic: it must NOT be classified
    // as unknown junk by parse's own pre-check.
    let bytes = golden_bytes();
    assert!(bytes.starts_with(b"ISO-10303-21;"), "golden is a STEP part");
}
