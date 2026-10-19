//! Acceptance integration test: a 2-part fixture assembles and constrains
//! through the OEP assembly workbench, verified against a real oep-geo model
//! file (the fixture document is a canonical `format = "oep-geo"` document).
//!
//! This is the P7-M3-T01 acceptance behavior: load a real oep-geo assembly
//! model, run it through the workbench with a mate set, solve with the
//! adopted solver, write the solved placements back, and independently
//! measure that every constraint is satisfied.

use oep_assem::transform_math::{transform_apply, transform_rotate, V3};
use oep_assem::workbench::{self, SolveRequest};

const MODEL: &str = include_str!("golden/two-part-fixture.oepgeo.json");
const CONSTRAINTS: &str = include_str!("golden/two-part-fixture.constraints.json");

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
fn fixture_is_a_real_canonical_oepgeo_document() {
    // The fixture file must load and pass the canonical validator before any
    // workbench step, and the constraint set must round-trip.
    let doc = workbench::load_document(MODEL).unwrap();
    assert!(
        doc.validate().is_empty(),
        "fixture must be a valid oep-geo doc"
    );
    let cset = oep_assem::ConstraintSet::from_json_pretty(CONSTRAINTS).unwrap();
    assert_eq!(cset.format, oep_assem::ConstraintSet::FORMAT);
    assert_eq!(cset.mates.len(), 4);

    // The workbench lists the assembly model exactly as the document stores it.
    let infos = workbench::list_instances(&doc, "oep-assembly-fixture").expect("assembly present");
    assert_eq!(infos.len(), 2);
    let base = infos.iter().find(|i| i.id == "base").unwrap();
    assert_eq!(base.source_id, "oep-part-base");
    // cap starts deliberately mis-placed: this is what the solve fixes.
    let cap = infos.iter().find(|i| i.id == "cap").unwrap();
    assert_eq!(cap.transform[12].round(), 28.0);
    assert_eq!(cap.transform[14].round(), 20.0);
}

#[test]
fn two_part_fixture_assembles_and_constrains_through_the_workbench() {
    let mut doc = workbench::load_document(MODEL).unwrap();
    let cset = oep_assem::ConstraintSet::from_json_pretty(CONSTRAINTS).unwrap();

    let req = SolveRequest::new(
        "oep-assembly-fixture".to_string(),
        cset.mates.clone(),
        vec!["cap".to_string()],
    );
    let report = workbench::solve(&mut doc, &req).expect("solve must succeed");

    // The adopted solver reports success (okay or redundant-okay).
    assert!(
        report.status.is_success(),
        "solver status: {:?}",
        report.status
    );
    assert!(report.dof >= 0, "dof must not be negative");

    // Independent measurement: every mate satisfied within 1e-6.
    assert!(
        report.max_distance_error < 1e-6,
        "max distance residual too large: {:.3e}",
        report.max_distance_error
    );
    assert!(
        report.max_angle_error < 1e-6,
        "max angle residual too large: {:.3e}",
        report.max_angle_error
    );
    assert_eq!(report.moved.len(), 1, "only the cap moves");
    assert_eq!(report.moved[0].0, "cap");

    // The solved cap pose: origin at (36, 18, 10), +Z axis upright.
    let cap = instance_doc(&doc, "oep-assembly-fixture", "cap");
    let origin = transform_apply(&cap.transform, V3::new(0.0, 0.0, 0.0));
    assert!((origin.x - 36.0).abs() < 1e-6, "cap x = {origin:?}");
    assert!((origin.y - 18.0).abs() < 1e-6, "cap y = {origin:?}");
    assert!((origin.z - 10.0).abs() < 1e-6, "cap z = {origin:?}");
    let up = transform_rotate(&cap.transform, V3::new(0.0, 0.0, 1.0)).normalized();
    assert!(
        up.dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9,
        "cap +Z must be upright, got {up:?}"
    );

    // The base is fixed: it must not have moved.
    let base = instance_doc(&doc, "oep-assembly-fixture", "base");
    assert_eq!(
        base.transform.components(),
        oep_geo::Transform::identity().components()
    );

    // Written-back document must stay canonical.
    assert!(doc.validate().is_empty());

    // Serialize and reload: the solved model is still a real oep-geo document.
    let json = workbench::save_document(&doc).unwrap();
    let reloaded = workbench::load_document(&json).unwrap();
    assert!(reloaded.validate().is_empty());
    let cap2 = instance_doc(&reloaded, "oep-assembly-fixture", "cap");
    assert_eq!(cap2.transform, cap.transform);
}

#[test]
fn one_part_fixed_and_part_moving_split_is_enforced() {
    let mut doc = workbench::load_document(MODEL).unwrap();
    // Requesting to move the base while a Fixed mate fixes it must error.
    let cset = oep_assem::ConstraintSet::new("mm", "oep-assembly-fixture").with_mate(
        oep_assem::Mate::Fixed {
            instance: "base".to_string(),
        },
    );
    let req = SolveRequest::new(
        "oep-assembly-fixture".to_string(),
        cset.mates,
        vec!["base".to_string()],
    );
    let err = workbench::solve(&mut doc, &req).unwrap_err();
    assert!(matches!(
        err,
        oep_assem::WorkbenchError::ContradictoryFixed(_)
    ));
}

#[test]
fn partially_constrained_solve_reports_measured_residuals() {
    let mut doc = workbench::load_document(MODEL).unwrap();
    // Partially constrain the cap (planar z + x only; y stays a degree of
    // freedom). The solve succeeds and the workbench's independent
    // measurement reports the satisfied constraints at machine precision.
    let cset = oep_assem::ConstraintSet::new("mm", "oep-assembly-fixture")
        .with_mate(oep_assem::Mate::OnPlane {
            point: oep_assem::AnchorRef {
                instance: "cap".into(),
                anchor: "origin".into(),
            },
            plane: oep_assem::AnchorRef {
                instance: "base".into(),
                anchor: "top".into(),
            },
        })
        .with_mate(oep_assem::Mate::PlaneDistance {
            point: oep_assem::AnchorRef {
                instance: "cap".into(),
                anchor: "origin".into(),
            },
            plane: oep_assem::AnchorRef {
                instance: "base".into(),
                anchor: "inX".into(),
            },
            distance: 36.0,
        });
    let req = SolveRequest::new(
        "oep-assembly-fixture".to_string(),
        cset.mates,
        vec!["cap".to_string()],
    );
    let report = workbench::solve(&mut doc, &req).unwrap();
    assert!(report.status.is_success());
    assert!(
        report.dof >= 1,
        "y is intentionally free: dof={}",
        report.dof
    );
    // x and z must be solved exactly; y may take any value.
    let cap = instance_doc(&doc, "oep-assembly-fixture", "cap");
    let origin = transform_apply(&cap.transform, V3::new(0.0, 0.0, 0.0));
    assert!((origin.x - 36.0).abs() < 1e-6, "cap x {origin:?}");
    assert!((origin.z - 10.0).abs() < 1e-6, "cap z {origin:?}");
    // Both distance residuals measured ~0.
    assert!(
        report.max_distance_error < 1e-6,
        "{:.3e}",
        report.max_distance_error
    );
}
