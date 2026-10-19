//! P4-M4-T03 acceptance test (TDD) - oep-step JT reader visualization wedge.
//!
//! Contract, per workstreams/P4-M4/jt-reader-plan.md and the ADR section 9 acceptance:
//!
//! 1. `oep_step::jt::parse` reads a committed golden JT fixture
//!    (`corpus/jt/golden-box-a.jt`, a license-clean synthesized file written
//!    from the documented byte layout) and yields a [`JtModel`] with one part
//!    named `golden-box-a`.
//! 2. The golden part carries a decoded facet mesh: the fixture is a box
//!    primitive of size 1 x 2 x 3 (Table 52 interpretation), so the wedge's
//!    box tessellation yields 8 vertices and 12 triangles with bounding volume
//!    6.0 (asserted within tolerance, so a real numeric pin, not a shape
//!    coincidence).
//! 3. `jt::to_document` maps the parsed model onto the canonical oep-geo
//!    Document: one part owning one Solid body whose bbox comes from the
//!    decoded vertices, plus one ImportJt feature recording real counts. The
//!    document validates and serializes deterministically.
//! 4. Tolerant-failure: the corrupt corpus (junk, empty, truncated, bad byte
//!    order, out-of-range directory offset, block overrun, non-LSG root)
//!    returns typed `Error`s and never panics.
//! 5. Vendor-neutral seam: only oep-step/oep-geo types appear on public items;
//!    the pytest test test_p4m4_jt_reader.py runs the vocab scan against this
//!    module.

use oep_geo::Document;
use oep_step::jt::{self, Error, FacetMesh, JtModel};

fn golden_bytes() -> Vec<u8> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/jt/golden-box-a.jt");
    std::fs::read(path).expect("committed golden JT fixture exists")
}

fn corrupt(name: &str) -> Vec<u8> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/jt/{name}"));
    std::fs::read(path).expect("committed corrupt fixture exists")
}

fn assert_volume_tolerance(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "{label}: expected volume {expected}, got {actual}"
    );
}

#[test]
fn parses_golden_into_vendor_neutral_model() {
    let bytes = golden_bytes();
    let model: JtModel = jt::parse(&bytes).expect("golden parses");

    assert!(
        model.version.starts_with("Version "),
        "version string from header"
    );
    assert!(!model.big_endian, "fixture is little-endian");
    assert_eq!(model.parts.len(), 1, "one part node");
    let part = &model.parts[0];
    assert_eq!(part.object_id, 2, "object id from the LSG");
    assert_eq!(part.name, "golden-box-a", "name via string property atom");

    let mesh: &FacetMesh = part.mesh.as_ref().expect("part has a decoded mesh");
    assert_eq!(mesh.vertices.len(), 8, "box primitive -> 8 vertices");
    assert_eq!(mesh.triangles.len(), 12, "box primitive -> 12 triangles");
    let bounds = mesh.bounds().expect("mesh bounds");
    assert_volume_tolerance(
        (bounds.max[0] - bounds.min[0])
            * (bounds.max[1] - bounds.min[1])
            * (bounds.max[2] - bounds.min[2]),
        6.0,
        "bounding volume of the 1x2x3 box",
    );
    assert!(
        (bounds.min[0] - 0.0).abs() < 1e-6 && (bounds.max[2] - 3.0).abs() < 1e-6,
        "box spans the parameter space"
    );
}

#[test]
fn to_document_maps_canonical_model() {
    let bytes = golden_bytes();
    let model = jt::parse(&bytes).unwrap();
    let doc: Document = jt::to_document(&model);

    assert_eq!(doc.format(), "oep-geo");
    assert_eq!(doc.version(), 1);
    let errors = doc.validate();
    assert!(
        errors.is_empty(),
        "canonical document validates: {errors:?}"
    );

    assert_eq!(doc.parts.len(), 1, "one part");
    assert_eq!(doc.bodies.len(), 1, "one body");
    let part = &doc.parts[0];
    assert_eq!(part.name, "golden-box-a");
    let body = &doc.bodies[0];
    let bb = body.bbox.expect("body bbox from decoded vertices");
    assert_volume_tolerance(
        (bb.max[0] - bb.min[0]) * (bb.max[1] - bb.min[1]) * (bb.max[2] - bb.min[2]),
        6.0,
        "canonical bounding volume",
    );
    assert_eq!(part.body_ids, vec![body.id.clone()]);
    let f = &doc.features[0];
    assert_eq!(f.kind, "ImportJt");
    assert_eq!(
        f.parameters.get("vertex_count"),
        Some(&serde_json::json!(8.0))
    );
    assert_eq!(
        f.parameters.get("triangle_count"),
        Some(&serde_json::json!(12.0))
    );
    assert_eq!(f.body_ids, vec![body.id.clone()]);

    // Deterministic serialization round trip.
    let json = doc.to_json_pretty().expect("serialize");
    let back: Document = Document::from_json_pretty(&json).expect("reparse");
    assert_eq!(&back, &doc);
}

#[test]
fn corrupt_corpus_returns_typed_errors_never_panics() {
    let cases: &[(&str, bool)] = &[
        ("empty.jt", true),
        ("junk-not-jt.bin", true),
        ("truncated-header.jt", true),
        ("bad-byte-order.jt", true),
        ("toc-offset-oob.jt", true),
        ("segment-overrun.jt", true),
        ("root-not-lsg.jt", true),
    ];
    let mut errs = 0;
    for (name, must_err) in cases {
        let bytes = corrupt(name);
        let r = std::panic::catch_unwind(|| jt::parse(&bytes));
        assert!(r.is_ok(), "{name}: parse must not panic");
        match r.unwrap() {
            Ok(_) => assert!(!must_err, "{name}: expected a typed error"),
            Err(e) => {
                errs += 1;
                let _: &Error = &e;
            }
        }
    }
    assert_eq!(errs, cases.len(), "every corrupt case yields a typed error");
}

#[test]
fn junk_input_returns_typed_error_without_panic() {
    assert!(matches!(
        jt::parse(b""),
        Err(Error::NotJt) | Err(Error::Truncated)
    ));
    assert!(jt::parse(b"not a jt file at all").is_err());
    assert!(
        jt::parse(b"Version 10.0").is_err(),
        "short but versioned is an error"
    );
}
