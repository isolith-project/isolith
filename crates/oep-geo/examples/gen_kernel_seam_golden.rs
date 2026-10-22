//! One-shot generator for the P6-M3-T04 kernel-seam golden.
//!
//! Mirrors the P6-M1/P6-M2/P6-M3-T03 regen convention:
//! `cargo run -p oep-geo --features kernel --example gen_kernel_seam_golden -- crates/oep-geo/tests/golden`
//!
//! Runs the FULL P6-M3-T04 capability matrix against the real OCCT 8.0.x
//! kernel behind the oep-geo seam (booleans, fillet, chamfer, primitives,
//! extrude, revolve) and writes the kernel-measured values to
//! `<output-dir>/kernel-seam-golden.json`. Every measurement is cross-checked
//! here against its closed-form analytic volume to 1e-6 relative; a mismatch
//! fails the generator loudly, so the committed golden cannot drift from the
//! real kernel. The acceptance test (`tests/kernel_seam.rs`) then asserts the
//! same matrix against BOTH the closed form and this committed golden.
//!
//! Source shapes mirror the committed corpus `box-reference` solid (unit cube,
//! corner (0,0,0)-(1,1,1), see corpus/reference + tests/golden/box-reference.step):
//! every footprint in the matrix is that box or a shape derived from it.
//!
//! Requires the `kernel` feature (links the real OCCT engine). Run from the
//! repo root; verify the committed golden is byte-stable across regenerations.

use oep_geo::{
    make_box, make_cylinder, make_sphere, solid_boolean, solid_chamfer, solid_edges, solid_extrude,
    solid_fillet, solid_revolve, sys_drop, sys_new, BooleanOp, Point2D, SolidMeasures, Tolerance,
};
use std::path::PathBuf;
use std::process::ExitCode;

const PI: f64 = std::f64::consts::PI;

fn tol() -> Tolerance {
    Tolerance::new(1e-6).expect("valid tolerance")
}

/// Assert relative volume error within 1e-6 (the ADR test-gate bound).
fn assert_volume(actual: f64, expected: f64, what: &str) {
    let rel = (actual - expected).abs() / expected.max(1e-300);
    assert!(
        rel < 1e-6,
        "{what}: volume {actual} not within 1e-6 of {expected} (rel {rel:e})"
    );
}

fn main() -> ExitCode {
    let out_dir = std::env::args()
        .nth(1)
        .expect("usage: gen_kernel_seam_golden <output-dir>");
    let sys = sys_new().expect("kernel session");

    // ---- capability matrix (source shapes from the corpus box-reference) ----
    let mut measurements = serde_json::Map::new();

    // Primitives: the box-reference solid is the unit cube (0,0,0)-(1,1,1);
    // the cylinder and sphere use radii/height derived from that footprint.
    let boxed = make_box(&sys, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, tol()).expect("box");
    assert_volume(boxed.measures.volume, 1.0, "box");
    measurements.insert("box".into(), measures_json(&boxed.measures));

    let cyl = make_cylinder(&sys, 0.5, 1.0, tol()).expect("cylinder");
    assert_volume(cyl.measures.volume, PI * 0.25 * 1.0, "cylinder");
    measurements.insert("cylinder".into(), measures_json(&cyl.measures));

    let sphere = make_sphere(&sys, 0.5, tol()).expect("sphere");
    assert_volume(sphere.measures.volume, 4.0 / 3.0 * PI * 0.125, "sphere");
    measurements.insert("sphere".into(), measures_json(&sphere.measures));

    // Booleans: two unit boxes offset 0.5 in x (half overlap).
    let a = make_box(&sys, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, tol()).expect("box a");
    let b = make_box(&sys, 0.5, 0.0, 0.0, 1.5, 1.0, 1.0, tol()).expect("box b");

    let u = solid_boolean(BooleanOp::Union, &sys, &[a.clone(), b.clone()], tol()).expect("union");
    assert_volume(u.measures.volume, 1.5, "union (two overlapping unit boxes)");
    measurements.insert("union".into(), measures_json(&u.measures));

    let d =
        solid_boolean(BooleanOp::Difference, &sys, &[a.clone(), b.clone()], tol()).expect("diff");
    assert_volume(d.measures.volume, 0.5, "difference (a minus the overlap)");
    measurements.insert("difference".into(), measures_json(&d.measures));

    let i = solid_boolean(
        BooleanOp::Intersection,
        &sys,
        &[a.clone(), b.clone()],
        tol(),
    )
    .expect("intersection");
    assert_volume(i.measures.volume, 0.5, "intersection (shared half-box)");
    measurements.insert("intersection".into(), measures_json(&i.measures));

    // Fillet / chamfer: radius/distance 0.2 on one edge of the unit box. The
    // edge list is the kernel's own TopExp order (24 per-face occurrences on
    // a box, OCCT 8.0.1); edges[0] is deterministic within the pinned build.
    let edges = solid_edges(&sys, &boxed, tol()).expect("edges");

    let fillet_removed = 1.0 * (0.2f64 * 0.2 - PI * 0.2 * 0.2 / 4.0);
    let f = solid_fillet(&sys, &[edges[0]], 0.2, tol()).expect("fillet");
    assert_volume(
        f.measures.volume,
        1.0 - fillet_removed,
        "fillet r0.2 box edge",
    );
    measurements.insert("fillet".into(), measures_json(&f.measures));

    let chamfer_removed = 1.0 * (0.2f64 * 0.2 / 2.0);
    let c = solid_chamfer(&sys, &[edges[0]], 0.2, tol()).expect("chamfer");
    assert_volume(
        c.measures.volume,
        1.0 - chamfer_removed,
        "chamfer d0.2 box edge",
    );
    measurements.insert("chamfer".into(), measures_json(&c.measures));

    // Extrude / revolve: planar profiles derived from the box footprint.
    // Extruded square matches the box-reference exactly (volume 1.0).
    let square = [
        Point2D::new(0.0, 0.0),
        Point2D::new(1.0, 0.0),
        Point2D::new(1.0, 1.0),
        Point2D::new(0.0, 1.0),
    ];
    let ex = solid_extrude(&sys, &square, 1.0, tol()).expect("extrude");
    assert_volume(
        ex.measures.volume,
        1.0,
        "extruded unit square (== box-reference)",
    );
    measurements.insert("extrude".into(), measures_json(&ex.measures));

    // Revolve: annulus meridian (r in [0.4, 1.0], height 1.0) a full turn.
    let meridian = [
        Point2D::new(0.4, 0.0),
        Point2D::new(1.0, 0.0),
        Point2D::new(1.0, 1.0),
        Point2D::new(0.4, 1.0),
    ];
    let rv = solid_revolve(&sys, &meridian, 2.0 * PI, tol()).expect("revolve");
    let annulus_volume = PI * (1.0 - 0.4 * 0.4) * 1.0;
    assert_volume(
        rv.measures.volume,
        annulus_volume,
        "revolved annulus (full turn)",
    );
    measurements.insert("revolve".into(), measures_json(&rv.measures));

    sys_drop(&sys);

    let doc = serde_json::json!({
        "format": "oep-kernel-seam-golden",
        "version": 1,
        "generator": "gen_kernel_seam_golden",
        "occt_line": "8.0.x",
        "source": "corpus/reference box-reference (unit cube 0..1)",
        "measurements": measurements,
    });

    let path = PathBuf::from(&out_dir).join("kernel-seam-golden.json");
    let text = serde_json::to_string_pretty(&doc).expect("serialize golden");
    std::fs::write(&path, text.as_bytes()).expect("write golden");
    println!("wrote {}", path.display());
    println!("{}", text);
    ExitCode::SUCCESS
}

fn measures_json(m: &SolidMeasures) -> serde_json::Value {
    serde_json::json!({
        "volume": m.volume,
        "surface_area": m.surface_area,
        "face_count": m.face_count,
        "edge_count": m.edge_count,
        "vertex_count": m.vertex_count,
    })
}
