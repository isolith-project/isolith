//! Acceptance integration test: a 3-part fixture assembles and constrains
//! through the OEP assembly workbench.
//!
//! This is the committed 3-part assembly the interactive editor surface
//! (EDITOR-ASSEM-EDIT) solves live in the shell. It proves the same contract
//! as the 2-part fixture at one more part of complexity: a real canonical
//! oep-geo model (base plate, cap, peg), each movable (non-fixed) instance
//! constrained by the workbench mate vocabulary, solved with the adopted
//! solver, solved placements written back, and every constraint independently
//! measured as satisfied. The fixture deliberately opens mis-placed and
//! tilted so the solve has visible work to do, and the mate set covers the
//! workbench vocabulary (fixed, on-plane, plane-distance, parallel); the
//! coincident kind is exercised by the interactive surface tests.

use oep_assem::transform_math::{transform_apply, transform_rotate, V3};
use oep_assem::workbench::{self, SolveRequest};

const MODEL: &str = include_str!("golden/three-part-fixture.oepgeo.json");
const CONSTRAINTS: &str = include_str!("golden/three-part-fixture.constraints.json");

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

fn origin_of(inst: &oep_geo::Instance) -> V3 {
    transform_apply(&inst.transform, V3::new(0.0, 0.0, 0.0))
}

#[test]
fn fixture_is_a_real_canonical_oepgeo_document() {
    let doc = workbench::load_document(MODEL).unwrap();
    assert!(
        doc.validate().is_empty(),
        "fixture must be a valid oep-geo doc"
    );
    let cset = oep_assem::ConstraintSet::from_json_pretty(CONSTRAINTS).unwrap();
    assert_eq!(cset.format, oep_assem::ConstraintSet::FORMAT);
    assert_eq!(cset.mates.len(), 9);

    // Three parts and a three-instance assembly.
    let infos = workbench::list_instances(&doc, "oep-assembly-fixture3").expect("assembly present");
    assert_eq!(infos.len(), 3);
    let base = infos.iter().find(|i| i.id == "base").unwrap();
    assert_eq!(base.source_id, "oep-part-base");
    // cap and peg start deliberately mis-placed and tilted.
    let cap = infos.iter().find(|i| i.id == "cap").unwrap();
    assert_eq!(cap.transform[12].round(), 10.0);
    let peg = infos.iter().find(|i| i.id == "peg").unwrap();
    assert_eq!(peg.transform[14].round(), 30.0);
}

#[test]
fn three_part_fixture_assembles_and_constrains_through_the_workbench() {
    let mut doc = workbench::load_document(MODEL).unwrap();
    let cset = oep_assem::ConstraintSet::from_json_pretty(CONSTRAINTS).unwrap();

    let req = SolveRequest::new(
        "oep-assembly-fixture3".to_string(),
        cset.mates.clone(),
        vec!["cap".to_string(), "peg".to_string()],
    );
    let report = workbench::solve(&mut doc, &req).expect("solve must succeed");

    assert!(report.status.is_success(), "status: {:?}", report.status);
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
    assert_eq!(report.moved.len(), 2, "only cap and peg move");
    let moved_ids: Vec<&str> = report.moved.iter().map(|(id, _)| id.as_str()).collect();
    assert!(moved_ids.contains(&"cap") && moved_ids.contains(&"peg"));

    // Solved cap origin at (36, 18, 10), +Z upright.
    let cap = instance_doc(&doc, "oep-assembly-fixture3", "cap");
    let o = origin_of(cap);
    assert!((o.x - 36.0).abs() < 1e-6, "cap x = {o:?}");
    assert!((o.y - 18.0).abs() < 1e-6, "cap y = {o:?}");
    assert!((o.z - 10.0).abs() < 1e-6, "cap z = {o:?}");
    let up = transform_rotate(&cap.transform, V3::new(0.0, 0.0, 1.0)).normalized();
    assert!(
        up.dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9,
        "cap +Z must be upright, got {up:?}"
    );

    // Solved peg origin at (18, 22, 10), upright.
    let peg = instance_doc(&doc, "oep-assembly-fixture3", "peg");
    let o = origin_of(peg);
    assert!((o.x - 18.0).abs() < 1e-6, "peg x = {o:?}");
    assert!((o.y - 22.0).abs() < 1e-6, "peg y = {o:?}");
    assert!((o.z - 10.0).abs() < 1e-6, "peg z = {o:?}");
    let up = transform_rotate(&peg.transform, V3::new(0.0, 0.0, 1.0)).normalized();
    assert!(
        up.dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9,
        "peg +Z must be upright, got {up:?}"
    );

    // The base is fixed: it must not have moved.
    let base = instance_doc(&doc, "oep-assembly-fixture3", "base");
    assert_eq!(
        base.transform.components(),
        oep_geo::Transform::identity().components()
    );

    // Written-back document stays canonical, and reloading round-trips.
    assert!(doc.validate().is_empty());
    let json = workbench::save_document(&doc).unwrap();
    let reloaded = workbench::load_document(&json).unwrap();
    assert!(reloaded.validate().is_empty());
    assert_eq!(
        instance_doc(&reloaded, "oep-assembly-fixture3", "cap").transform,
        cap.transform
    );
}
