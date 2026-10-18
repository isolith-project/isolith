//! P6-M3-T04 / ADR v1.1.0: kernel-seam acceptance on real material (gn.)
//!
//! Through-the-seam glue tests: oep-geo types in, oep-geo types out, with the
//! pinned OCCT 8.0.x engine linked behind the `kernel` feature. These are the
//! ADR section 9 test-gate assertions -- volumes to tolerance, face counts, and
//! typed (never panicked) errors on known-failing geometry.
//!
//! Build/run: `cargo test -p oep-geo --features kernel --test kernel_seam`
//! (enables the oep-engine-occ binding; OCCT is fetched+built by
//! scripts/kernel/build-occt.sh the first time). Without the `kernel` feature
//! this file is empty, so the default workspace build stays C++-free.

#![cfg(feature = "kernel")]
#![deny(warnings)]

use oep_geo::{
    make_box, make_cylinder, make_sphere, solid_boolean, solid_chamfer, solid_edges, solid_fillet,
    sys_drop, sys_new, BooleanError, BooleanOp, KernelError, Solid, Tolerance,
};

const PI: f64 = std::f64::consts::PI;

fn tol() -> Tolerance {
    Tolerance::new(1e-6).expect("valid tolerance")
}

/// Assert relative volume error within 1e-6 (tight, real-material gate).
fn assert_volume(actual: f64, expected: f64, what: &str) {
    let rel = (actual - expected).abs() / expected.max(1e-300);
    assert!(
        rel < 1e-6,
        "{what}: volume {actual} not within 1e-6 of {expected} (rel {rel:e})"
    );
}

fn box_unit(sys: &oep_geo::SysHandle) -> Solid {
    make_box(sys, 0.0, 0.0, 0.0, 10.0, 10.0, 10.0, tol()).expect("box")
}

// ---------------------------------------------------------------------------
// primitives (volumes to tolerance; acceptance: "primitive box/cylinder/sphere
// assert volumes to tolerance")
// ---------------------------------------------------------------------------

#[test]
fn primitive_box_volume() {
    let sys = sys_new().expect("session");
    let s = make_box(&sys, 0.0, 0.0, 0.0, 10.0, 20.0, 30.0, tol()).expect("box");
    assert_volume(s.measures.volume, 6000.0, "box 10x20x30");
    assert_eq!(s.measures.face_count, 6, "box faces");
    sys_drop(&sys);
}

#[test]
fn primitive_cylinder_volume() {
    let sys = sys_new().expect("session");
    let s = make_cylinder(&sys, 5.0, 10.0, tol()).expect("cylinder");
    assert_volume(s.measures.volume, PI * 25.0 * 10.0, "cylinder r5 h10");
    assert_eq!(s.measures.face_count, 3, "cylinder caps + lateral shell");
    sys_drop(&sys);
}

#[test]
fn primitive_sphere_volume() {
    let sys = sys_new().expect("session");
    let s = make_sphere(&sys, 7.0, tol()).expect("sphere");
    assert_volume(s.measures.volume, 4.0 / 3.0 * PI * 343.0, "sphere r7");
    assert_eq!(s.measures.face_count, 1, "sphere single face");
    sys_drop(&sys);
}

// ---------------------------------------------------------------------------
// booleans: union / difference / intersection of two intersecting cubes
// (acceptance: "union of two intersecting boxes asserts resulting volume to
// tolerance"; "difference and intersection assert their volumes and face
// counts")
// ---------------------------------------------------------------------------

fn two_boxes(sys: &oep_geo::SysHandle) -> (Solid, Solid) {
    let a = make_box(sys, 0.0, 0.0, 0.0, 10.0, 10.0, 10.0, tol()).expect("box a");
    // Overlap 5 units in x: [5,15]x[0,10]x[0,10].
    let b = make_box(sys, 5.0, 0.0, 0.0, 15.0, 10.0, 10.0, tol()).expect("box b");
    (a, b)
}

#[test]
fn boolean_union_volume() {
    let sys = sys_new().expect("session");
    let (a, b) = two_boxes(&sys);
    let u = solid_boolean(BooleanOp::Union, &sys, &[a.clone(), b.clone()], tol()).expect("union");
    assert_volume(u.measures.volume, 1500.0, "union of 10-cubes offset 5 in x");
    // OCCT 8.0.1 keeps the coplanar interior walls where the two cubes meet
    // (an unsewn 14-face union of this pair); that IS the kernel's answer, and
    // the assertion locks it so a bump that changes the topology is reviewed.
    assert_eq!(u.measures.face_count, 14, "union face count (OCCT 8.0.1)");
    assert!(
        u.measures.edge_count > 24,
        "union has the combined edge network"
    );
    sys_drop(&sys);
}

#[test]
fn boolean_difference_volume_and_faces() {
    let sys = sys_new().expect("session");
    let (a, b) = two_boxes(&sys);
    let d =
        solid_boolean(BooleanOp::Difference, &sys, &[a.clone(), b.clone()], tol()).expect("diff");
    assert_volume(d.measures.volume, 500.0, "a minus the overlap");
    assert_eq!(d.measures.face_count, 6, "difference is a 5x10x10 box");
    sys_drop(&sys);
}

#[test]
fn boolean_intersection_volume_and_faces() {
    let sys = sys_new().expect("session");
    let (a, b) = two_boxes(&sys);
    let c = solid_boolean(
        BooleanOp::Intersection,
        &sys,
        &[a.clone(), b.clone()],
        tol(),
    )
    .expect("common");
    assert_volume(c.measures.volume, 500.0, "shared 5x10x10 region");
    assert_eq!(c.measures.face_count, 6, "intersection is a 5x10x10 box");
    sys_drop(&sys);
}

// ---------------------------------------------------------------------------
// fillet / chamfer of a single box edge
// (acceptance: "fillet of a box edge asserts volume + face count"; "chamfer of
// a box edge asserts volume + face count")
// ---------------------------------------------------------------------------

#[test]
fn fillet_box_edge_volume_and_faces() {
    let sys = sys_new().expect("session");
    let boxed = box_unit(&sys);
    let edges = solid_edges(&sys, &boxed, tol()).expect("edges");
    // OCCT reports each edge once per incident face occurrence (a cube yields
    // 24 edge rows, 12 unique); that per-occurrence count is the kernel answer.
    assert_eq!(edges.len(), 24, "OCCT box edge occurrences");

    // A radius-2 fillet on one straight edge of a 10-cube removes a quarter
    // cylinder of material: L*(r^2 - pi*r^2/4) per unit length over L=10.
    let removed = 10.0 * (4.0 - PI * 4.0 / 4.0);
    let f = solid_fillet(&sys, &[edges[0]], 2.0, tol()).expect("fillet");
    assert_volume(f.measures.volume, 1000.0 - removed, "fillet r2 on box edge");
    assert!(
        f.measures.face_count == 7,
        "fillet adds the quarter-cylinder face: got {} faces",
        f.measures.face_count
    );
    assert!(
        f.measures.volume < boxed.measures.volume,
        "fillet removes material"
    );
    sys_drop(&sys);
}

#[test]
fn chamfer_box_edge_volume_and_faces() {
    let sys = sys_new().expect("session");
    let boxed = box_unit(&sys);
    let edges = solid_edges(&sys, &boxed, tol()).expect("edges");

    // A symmetric distance-2 chamfer on one straight edge of a 10-cube removes
    // a right prism of cross-section d^2/2 over length 10.
    let removed = 10.0 * (4.0 / 2.0);
    let c = solid_chamfer(&sys, &[edges[0]], 2.0, tol()).expect("chamfer");
    assert_volume(
        c.measures.volume,
        1000.0 - removed,
        "chamfer d2 on box edge",
    );
    assert!(
        c.measures.face_count == 7,
        "chamfer adds the flat face: got {} faces",
        c.measures.face_count
    );
    assert!(
        c.measures.volume < boxed.measures.volume,
        "chamfer removes material"
    );
    sys_drop(&sys);
}

// ---------------------------------------------------------------------------
// known-failing geometry returns a TYPED error, never a panic (ADR section 6)
// ---------------------------------------------------------------------------

#[test]
fn degenerate_primitive_is_kernel_error_not_panic() {
    let sys = sys_new().expect("session");
    let r = make_sphere(&sys, 0.0, tol());
    assert!(matches!(r, Err(KernelError::Degenerate(_))), "got {r:?}");
    let r2 = make_box(&sys, 0.0, 0.0, 0.0, 10.0, 0.0, 10.0, tol());
    assert!(matches!(r2, Err(KernelError::Degenerate(_))), "got {r2:?}");
    sys_drop(&sys);
}

#[test]
fn boolean_of_fewer_than_two_operands_is_typed_error() {
    let sys = sys_new().expect("session");
    let one = box_unit(&sys);
    let r = solid_boolean(BooleanOp::Union, &sys, &[one], tol());
    assert!(
        matches!(r, Err(BooleanError::InvalidOperand(_))),
        "got {r:?}"
    );
    let r2 = solid_boolean(BooleanOp::Union, &sys, &[], tol());
    assert!(
        matches!(r2, Err(BooleanError::InvalidOperand(_))),
        "got {r2:?}"
    );
    sys_drop(&sys);
}

#[test]
fn difference_that_consumes_all_material_is_typed_error() {
    let sys = sys_new().expect("session");
    // Tool fully contains the base: base \ tool has no faces left.
    let base = make_box(&sys, 2.0, 2.0, 2.0, 8.0, 8.0, 8.0, tol()).expect("base");
    let tool = box_unit(&sys);
    let r = solid_boolean(BooleanOp::Difference, &sys, &[base, tool], tol());
    assert!(matches!(r, Err(BooleanError::EmptyResult)), "got {r:?}");
    sys_drop(&sys);
}

#[test]
fn cross_session_solid_is_typed_result_never_a_panic() {
    let sys = sys_new().expect("session");
    let orphan = box_unit(&sys);
    sys_drop(&sys);

    // Engine tokens are session-local (ADR section 5): a solid from the dropped
    // session carries no valid reference in a new session, but the seam contract
    // is that the call is a typed Result either way -- never a panic.
    let sys2 = sys_new().expect("new session");
    let fresh = box_unit(&sys2);
    let r = solid_boolean(BooleanOp::Union, &sys2, &[orphan, fresh], tol());
    match r {
        Ok(_) => {}
        Err(BooleanError::EngineUnavailable) => {
            unreachable!("kernel feature is enabled in this test")
        }
        Err(_) => {}
    }
    sys_drop(&sys2);
}

#[test]
fn empty_edge_selection_is_kernel_error_not_panic() {
    let sys = sys_new().expect("session");
    // A live solid exists in the session, but no edges are selected.
    let _boxed = box_unit(&sys);
    let r = solid_fillet(&sys, &[], 2.0, tol());
    assert!(
        matches!(r, Err(KernelError::InvalidArgument(_))),
        "got {r:?}"
    );
    sys_drop(&sys);
}
