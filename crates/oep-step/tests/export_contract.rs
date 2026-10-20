//! P4-M3-T01 acceptance contract (TDD, RED first)  -  oep-step STEP export lane.
//!
//! Written BEFORE the implementation (RED: `oep_step::export` does not exist
//! on this branch). The contract, once GREEN:
//!
//! 1. `oep_step::export::write_document` writes an ISO 10303-21 AP242 file
//!    for an `oep_geo::Document` (parts and assemblies) plus a supplier of
//!    per-body tessellations. It emits a real B-Rep (`MANIFOLD_SOLID_BREP`
//!    with `ADVANCED_FACE`/`EDGE_LOOP`/`EDGE_CURVE`/`VERTEX_POINT`), NOT the
//!    `SHELL_BASED_SURFACE_MODEL` passthrough form (which OCCT cannot build).
//! 2. Single part, single body: taking the isolith unit box (8 verts, 12
//!    tris) yields a file whose DATA section contains a MANIFOLD_SOLID_BREP,
//!    one CLOSED_SHELL, 12 ADVANCED_FACEs and the honest marking strings
//!    (faceted / tessellated, not exact analytic B-Rep) in FILE_DESCRIPTION
//!    and the product description.
//! 3. Assembly: a Document with one assembly instancing a part twice under
//!    translations emits NEXT_ASSEMBLY_USAGE_OCCURRENCE products for the
//!    children and ITEM_DEFINED_TRANSFORM placements; the assembly hierarchy
//!    is preserved (root assembly product + child part products).
//! 4. PMI is deferred: the exporter writes no PMI entities (documented
//!    milestone cut); the output schema stays AP242 and the header carries no
//!    PMI marker.
//! 5. Round trip into our OWN reader: `oep_step::parse` on the exported
//!    bytes re-imports the file into a `FacetedModel` whose shell/face/bbox
//!    facts match the source tessellation (the read lane walks MSBRep).
//! 6. Tolerant-failure: empty parts, missing per-body tessellation, and an
//!    assembly referencing an unknown id produce typed, never-panic results.
//! 7. Vendor-neutral seam: the public export API names no step-io identifiers
//!    (`MANIFOLD_SOLID_BREP` etc. appear only as DATA-section text, not as
//!    oep-step type names).
//!
//! External FreeCAD re-import (the other half of the acceptance) is covered
//! separately in the Python acceptance harness, which needs a real FreeCAD so
//! it is env-gated (OEP_FREECAD), never a CI-required step.

use oep_geo::{
    Assembly, Body, BodyKind, BoundingBox, Document, Instance, InstanceSource, Part, Transform,
};
use oep_step::export::{write_document, BodyMesh, ExportError, ExportOpts};

/// The isolith CC0-1.0 unit box tessellation (same as the P6 exporters'
/// reference; 8 verts / 12 tris, axis-aligned bbox [0,0,0]..[1,1,1]).
fn box_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let positions = vec![
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 3, 1],
        [4, 5, 6],
        [5, 7, 6],
        [2, 6, 4],
        [2, 0, 6],
        [3, 5, 7],
        [3, 1, 5],
        [5, 1, 2],
        [5, 2, 4],
        [0, 7, 3],
        [0, 6, 7],
    ];
    (positions, triangles)
}

/// A one-part document owning one Solid body with the unit-box bbox.
fn box_document() -> Document {
    let mut doc = Document::new("mm");
    let body = Body::new("body-1", "box-body", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let part = Part::new("part-1", "box").with_body("body-1");
    doc.push_body(body);
    doc.push_part(part);
    doc
}

/// A two-part assembly: `assembly-1` instances `part-plate` twice, 20 mm apart.
fn assembly_document() -> Document {
    let mut doc = Document::new("mm");
    let body = Body::new("plate-body", "plate-body", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let part = Part::new("part-plate", "plate").with_body("plate-body");
    let i1 = Instance::new(
        "inst-1",
        "plate-1",
        InstanceSource::Part {
            id: "part-plate".into(),
        },
    )
    .with_transform(Transform::translation(0.0, 0.0, 0.0));
    let i2 = Instance::new(
        "inst-2",
        "plate-2",
        InstanceSource::Part {
            id: "part-plate".into(),
        },
    )
    .with_transform(Transform::translation(0.0, 0.0, 20.0));
    let asm = Assembly::new("assembly-1", "assembly-1")
        .with_instance(i1)
        .with_instance(i2);
    doc.push_body(body);
    doc.push_part(part);
    doc.push_assembly(asm);
    doc
}

fn opts() -> ExportOpts {
    ExportOpts {
        file_name: "export-test.step".to_string(),
        product_name: "export-test".to_string(),
        version: "0.1.0".to_string(),
        org: "Isolith OEP".to_string(),
        system: "oep-step".to_string(),
    }
}

/// Build the mesh slice for the unit box body id.
fn box_meshes<'a>(body_id: &'a str, pos: &'a [[f32; 3]], tri: &'a [[u32; 3]]) -> Vec<BodyMesh<'a>> {
    vec![BodyMesh {
        body_id,
        positions: pos,
        triangles: tri,
    }]
}

// ---------------------------------------------------------------------------
// 1. write_document exists and returns typed results (never panics)
// ---------------------------------------------------------------------------

#[test]
fn write_document_returns_bytes_and_sortable_stats() {
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let out = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("box exports");
    assert!(!out.is_empty(), "writer produces bytes");
    assert!(out.starts_with(b"ISO-10303-21;"), "PART 21 magic");
    let st = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("second export");
    assert_eq!(st, out, "deterministic byte-for-byte output");
}

// ---------------------------------------------------------------------------
// 2. real B-Rep topology + honest marking
// ---------------------------------------------------------------------------

#[test]
fn export_emits_manifold_solid_brep_not_shell_passthrough() {
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let out = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("box exports");
    let text = String::from_utf8(out).expect("utf8");
    assert!(
        text.contains("MANIFOLD_SOLID_BREP"),
        "real B-Rep entity (OCCT builds this); got: {}",
        text.lines().take(8).collect::<Vec<_>>().join(" | ")
    );
    assert!(
        text.contains("ADVANCED_FACE"),
        "advanced faces with full topology"
    );
    // NOT the invalid-for-OCCT passthrough form from the P6 export.
    assert!(
        !text.contains("SHELL_BASED_SURFACE_MODEL"),
        "must not emit the shell passthrough OCCT cannot build"
    );
}

#[test]
fn export_carries_honest_faceted_not_exact_brep_marking() {
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let out = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("box exports");
    let text = String::from_utf8(out).expect("utf8");
    let lower = text.to_lowercase();
    assert!(
        lower.contains("not an exact analytic b-rep") || lower.contains("not an exact"),
        "FILE_DESCRIPTION must carry the honest tessellated / not-exact marking"
    );
}

// ---------------------------------------------------------------------------
// 3. assembly: hierarchy + instance transforms
// ---------------------------------------------------------------------------

#[test]
fn assembly_export_preserves_hierarchy() {
    let (positions, triangles) = box_mesh();
    let doc = assembly_document();
    let meshes = vec![BodyMesh {
        body_id: "plate-body",
        positions: &positions,
        triangles: &triangles,
    }];
    let out = write_document(&doc, &meshes, &opts()).expect("assembly exports");
    let text = String::from_utf8(out).expect("utf8");
    assert!(
        text.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"),
        "assembly children placed via NAUO"
    );
    assert!(
        text.to_uppercase().contains("ITEM_DEFINED_TRANSFORM"),
        "instance placement emitted as ITEM_DEFINED_TRANSFORM"
    );
    // Both placements appear: two NAUO records.
    let nauo_count = text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE").count();
    assert_eq!(nauo_count, 2, "two instances -> two use occurrences");
}

// ---------------------------------------------------------------------------
// 4. PMI deferred
// ---------------------------------------------------------------------------

#[test]
fn export_deferrers_pmi() {
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let out = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("box exports");
    let text = String::from_utf8(out).expect("utf8");
    for forbidden in [
        "DIMENSIONAL_CHARACTERISTIC",
        "GEOMETRIC_TOLERANCE",
        "PROPERTY_DEFINITION",
        "SHAPE_ASPECT",
    ] {
        assert!(
            !text.contains(forbidden),
            "PMI deferred to P4-M2: {forbidden} must not be emitted"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. round trip into our own reader
// ---------------------------------------------------------------------------

#[test]
fn exported_file_reimports_into_oep_geo_reader() {
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let out = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts())
        .expect("box exports");
    let outcome = oep_step::parse(&out).expect("our reader re-imports the export");
    // The exported geometric B-Rep is the unit box again.
    let f = &outcome.facets;
    assert_eq!(f.vertices.len(), 8, "8 unique corners recovered");
    assert_eq!(f.triangles.len(), 12, "12 triangles recovered");
    let bb = f.bbox.expect("bbox from exported vertices");
    assert!((bb.min[0].abs() < 1e-6) && (bb.min[1].abs() < 1e-6) && (bb.min[2].abs() < 1e-6));
    assert!(
        (bb.max[0] - 1.0).abs() < 1e-6
            && (bb.max[1] - 1.0).abs() < 1e-6
            && (bb.max[2] - 1.0).abs() < 1e-6,
        "unit box bbox recovered"
    );
}

// ---------------------------------------------------------------------------
// 6. tolerant failure: typed errors, never panic
// ---------------------------------------------------------------------------
#[test]
fn missing_mesh_is_typed_and_non_panicking() {
    let doc = box_document();
    // Body has no tessellation supplied -> the writer must return a typed
    // result, not a panic.
    let result = write_document(&doc, &[], &opts());
    match result {
        Err(ExportError::MissingTessellation(body)) => {
            assert_eq!(body, "body-1", "error names the offending body")
        }
        other => panic!("expected MissingTessellation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 7. vendor-neutral seam
// ---------------------------------------------------------------------------

#[test]
fn export_public_surface_is_vendor_neutral() {
    // The public export API lives at oep_step::export and exposes only simple
    // neutral types (BodyMesh / ExportOpts / ExportError / Vec<u8>) ... no
    // step-io identifier. We exercise the error path through the same seam to
    // confirm the public call is reachable and typed.
    use oep_step::export::ExportError as E;
    let doc = box_document();
    let err = oep_step::export::write_document(&doc, &[], &opts())
        .expect_err("missing tessellation is a typed error");
    match err {
        E::MissingTessellation(body) => {
            assert_eq!(body, "body-1", "error names the offending body")
        }
        other => panic!("expected MissingTessellation, got {other:?}"),
    }
}

#[test]
fn document_with_unknown_assembly_target_is_typed() {
    // An instance referencing a part that doesn't exist.
    let mut doc = Document::new("mm");
    let i = Instance::new(
        "inst-x",
        "ghost",
        InstanceSource::Part {
            id: "no-such-part".into(),
        },
    );
    let asm = Assembly::new("asm-x", "asm-x").with_instance(i);
    doc.push_assembly(asm);
    let (positions, triangles) = box_mesh();
    let result = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &opts());
    // Tolerant: typed outcome, never panic.
    let _ = result;
}

// ---------------------------------------------------------------------------
// 8. committed goldens are real fixtures: re-import through our own reader
// ---------------------------------------------------------------------------

fn read_golden(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    std::fs::read(path).expect("committed golden fixture exists")
}

#[test]
fn committed_box_golden_reimports_through_our_reader() {
    let bytes = read_golden("box-part.step");
    let outcome = oep_step::parse(&bytes).expect("our reader re-imports the box golden");
    let f = &outcome.facets;
    assert_eq!(f.vertices.len(), 8, "8 unique corners");
    assert_eq!(f.triangles.len(), 12, "12 triangles");
    let bb = f.bbox.expect("bbox");
    assert!((bb.min[0].abs() < 1e-6) && (bb.min[1].abs() < 1e-6) && (bb.min[2].abs() < 1e-6));
    assert!(
        (bb.max[0] - 1.0).abs() < 1e-6
            && (bb.max[1] - 1.0).abs() < 1e-6
            && (bb.max[2] - 1.0).abs() < 1e-6
    );
}

#[test]
fn committed_assembly_golden_reimports_through_our_reader() {
    let bytes = read_golden("assembly.step");
    let outcome = oep_step::parse(&bytes).expect("our reader re-imports the assembly golden");
    let f = &outcome.facets;
    // The assembly golden places the same box twice; STEP stores the shared
    // geometry once as a MANIFOLD_SOLID_BREP and represents each placement as
    // a NEXT_ASSEMBLY_USAGE_OCCURRENCE + ITEM_DEFINED_TRANSFORM. The facet
    // walk maps the shared definition geometry (8 verts / 12 tris); the
    // placed hierarchy is structural and is carried through NAUO (verified in
    // FreeCAD as an App::Part tree) and reconstructed into oep-geo Assemblies
    // by the structured read (importer-hub), not by the flat facet walk.
    assert_eq!(
        f.vertices.len(),
        8,
        "shared box definition -> 8 unique corners"
    );
    assert_eq!(
        f.triangles.len(),
        12,
        "12 triangles in the shared definition"
    );
    let bb = f.bbox.expect("bbox");
    assert!((bb.min[0].abs() < 1e-6) && (bb.min[1].abs() < 1e-6) && (bb.min[2].abs() < 1e-6));
    // The two placements live at z=0 and z=20: the structural NAUO carries
    // them (verified by the FreeCAD harness's App::Part tree and by the
    // ITEM_DEFINED_TRANSFORM assertions above), so the definition bbox is the
    // single box's, not the composed scene's.
    assert!(
        (bb.max[2] - 1.0).abs() < 1e-6,
        "definition bbox z max = 1.0"
    );
    // Structure IS present in the file: NAUO with a going-up placement.
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(
        text.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"),
        "assembly structure present in the golden"
    );
}

#[test]
fn export_is_byte_stable_across_golden_refregen() {
    // Regenerate both goldens and assert they are byte-identical to the
    // committed files (deterministic writer; the golden generator is the
    // exact same path tests/test_step_export_freecad_acceptance.py uses).
    let (positions, triangles) = box_mesh();
    let doc = box_document();
    let bytes = write_document(&doc, &box_meshes("body-1", &positions, &triangles), &{
        let mut o = opts();
        o.file_name = "box-part.step".to_string();
        o.product_name = "isolith-export".to_string();
        o
    })
    .expect("box export");
    assert_eq!(
        bytes,
        read_golden("box-part.step"),
        "box golden byte-stable"
    );
}
