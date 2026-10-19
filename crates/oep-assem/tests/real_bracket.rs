//! Verify the assembly workbench against the REAL oep-geo golden model the
//! P6-M1-T01 schema card ships (`example-bracket.oepgeo.json`): load it,
//! read its assembly structure through the workbench, derive anchors from the
//! real parts' bounding boxes, run a real constraint on real geometry, and
//! confirm the documented sub-assembly limitation surfaces honestly.

use oep_assem::anchors::derived_anchors;
use oep_assem::transform_math::{transform_apply, transform_rotate, V3};
use oep_assem::workbench::{self, SolveRequest};

// Path from this test file back to the oep-geo crate's golden fixture that
// lives in the same workspace.
const REAL_MODEL: &str = include_str!("../../oep-geo/tests/golden/example-bracket.oepgeo.json");

fn instance_doc<'a>(doc: &'a oep_geo::Document, assembly: &str, id: &str) -> &'a oep_geo::Instance {
    doc.assemblies
        .iter()
        .find(|a| a.id == assembly)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.id == id)
        .unwrap()
}

#[test]
fn real_model_structure_reads_through_the_workbench() {
    let doc = workbench::load_document(REAL_MODEL).unwrap();
    assert!(doc.validate().is_empty(), "real model must be canonical");

    // The fastener-set sub-assembly has exactly four real fasteners.
    let infos = workbench::list_instances(&doc, "oep-assembly-fastener-set").unwrap();
    assert_eq!(infos.len(), 4);
    let f1 = infos
        .iter()
        .find(|i| i.id == "oep-inst-fastener-1")
        .unwrap();
    assert_eq!(f1.source_id, "oep-part-fastener");
    assert_eq!(f1.transform[12], 10.0); // real placement x=10 from the file
    assert_eq!(f1.transform[13], 10.0);

    // Derived anchors must match the real parts' bounding boxes.
    let fastener_anchors = derived_anchors(&doc, "oep-part-fastener");
    let bottom = fastener_anchors
        .iter()
        .find(|a| a.name == "bottom")
        .unwrap();
    assert_eq!(bottom.origin, [5.0, 5.0, 0.0]); // bbox 10x10x20
    let top = fastener_anchors.iter().find(|a| a.name == "top").unwrap();
    assert_eq!(top.origin, [5.0, 5.0, 20.0]);
    let bracket_anchors = derived_anchors(&doc, "oep-part-bracket");
    let front = bracket_anchors.iter().find(|a| a.name == "front").unwrap();
    assert_eq!(front.origin, [40.0, 0.0, 3.0]); // bbox 80x40x6
}

#[test]
fn real_model_solve_without_mates_keeps_real_placements() {
    let mut doc = workbench::load_document(REAL_MODEL).unwrap();
    // Moving fastener-1 with an empty mate set solves trivially: its real
    // placement must come back byte-identical and the document stays valid.
    let before = instance_doc(&doc, "oep-assembly-fastener-set", "oep-inst-fastener-1")
        .transform
        .clone();
    let req = SolveRequest::new(
        "oep-assembly-fastener-set".to_string(),
        Vec::new(),
        vec!["oep-inst-fastener-1".to_string()],
    );
    let report = workbench::solve(&mut doc, &req).unwrap();
    assert!(report.status.is_success());
    assert_eq!(report.moved.len(), 1);
    let after = instance_doc(&doc, "oep-assembly-fastener-set", "oep-inst-fastener-1");
    assert_eq!(
        &after.transform, &before,
        "no-mate solve must not move a real instance"
    );
    assert!(doc.validate().is_empty());
}

#[test]
fn real_model_constrains_solved_placement_measured() {
    let mut doc = workbench::load_document(REAL_MODEL).unwrap();
    // Constrain fastener-1's origin onto fastener-3's origin (real geometry,
    // real placements). The workbench must move f1 so the measured residual
    // drops to ~0 and f3 (fixed) stays put.
    let req = SolveRequest::new(
        "oep-assembly-fastener-set".to_string(),
        vec![oep_assem::Mate::Coincident {
            a: oep_assem::AnchorRef {
                instance: "oep-inst-fastener-1".into(),
                anchor: "origin".into(),
            },
            b: oep_assem::AnchorRef {
                instance: "oep-inst-fastener-3".into(),
                anchor: "origin".into(),
            },
        }],
        vec!["oep-inst-fastener-1".to_string()],
    );
    let report = workbench::solve(&mut doc, &req).unwrap();
    assert!(report.status.is_success());
    assert!(
        report.max_distance_error < 1e-6,
        "{:.3e}",
        report.max_distance_error
    );

    // f1's origin must now be at f3's real origin world position (10,30,0).
    let f1 = instance_doc(&doc, "oep-assembly-fastener-set", "oep-inst-fastener-1");
    let origin = transform_apply(&f1.transform, V3::new(0.0, 0.0, 0.0));
    assert!((origin.x - 10.0).abs() < 1e-6, "f1 x {origin:?}");
    assert!((origin.y - 30.0).abs() < 1e-6, "f1 y {origin:?}");
    assert!((origin.z - 0.0).abs() < 1e-6, "f1 z {origin:?}");
    // Orientation unchanged (identity) and still upright.
    let up = transform_rotate(&f1.transform, V3::new(0.0, 0.0, 1.0)).normalized();
    assert!(up.dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9);

    // f3 (fixed) must be untouched.
    let f3 = instance_doc(&doc, "oep-assembly-fastener-set", "oep-inst-fastener-3");
    assert_eq!(f3.transform[12], 10.0);
    assert_eq!(f3.transform[13], 30.0);
    assert!(doc.validate().is_empty());
}

#[test]
fn real_model_sub_assembly_limitation_is_surfaced_honestly() {
    // The bracket-assembly nests a sub-assembly instance. The workbench
    // documents this as unsupported (real B-Rep faces and nested product
    // structure land via P5), and must say so instead of guessing.
    let mut doc = workbench::load_document(REAL_MODEL).unwrap();
    let req = SolveRequest::new(
        "oep-assembly-bracket-assembly".to_string(),
        Vec::new(),
        vec!["oep-inst-fastener-set".to_string()],
    );
    let err = workbench::solve(&mut doc, &req).unwrap_err();
    assert!(matches!(
        err,
        oep_assem::WorkbenchError::SubAssemblyNotSupported(_)
    ));
}
