//! P6-M2-T03 golden artifact generator: writes `tests/golden/predicates-golden.json`
//! from the crate's own geometric predicates implementation.
//!
//! The committed golden JSON is produced BY this crate (self-consistent) and then
//! independently re-verified two ways:
//!
//! 1. `crates/oep-geo/tests/predicates_contract.rs` re-derives the same figures and
//!    asserts them directly against closed-form analytic expectations.
//! 2. `tests/test_oep_geo_predicates_acceptance.py` re-computes them independently
//!    in Python stdlib (independent formulas) and cross-checks.
//!
//! So a self-consistent but wrong implementation cannot satisfy both.
//!
//! Usage: `cargo run -p oep-geo --example gen_predicates_golden -- <out-dir>`
//! (default `crates/oep-geo/tests/golden`).

use oep_geo::predicates::{
    distance_point_line, distance_point_plane, distance_point_point, distance_point_segment,
    in_aabb, in_sphere, in_triangle_plane, on_line, on_plane, on_segment, on_sphere,
    signed_distance_point_plane,
};
use std::path::PathBuf;

/// Deterministic number formatting: shortest round-trip via `{}` (Rust's Display
/// for f64 is shortest-round-trip), integral values get `.0` so the JSON is
/// unambiguous between int and float.
fn fmt(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/oep-geo/tests/golden"));
    std::fs::create_dir_all(&out).expect("create golden dir");
    let path = out.join("predicates-golden.json");

    let mut cases: Vec<String> = Vec::new();

    // distance_point_point: (1,2,3) -> (4,6,3) is a 3-4-5 triangle => 5
    let d_pp = distance_point_point([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]);

    // signed distance to plane z=0 (point origin, normal +Z)
    let sdf_pos = signed_distance_point_plane([3.0, 4.0, 5.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let sdf_neg = signed_distance_point_plane([0.0, 0.0, -2.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let d_plane_z0 = distance_point_plane([1.0, 2.0, 7.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let d_plane_xyz = distance_point_plane([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

    // on_plane tolerance bucket around z=0
    let on_plane_in = on_plane([1.0, 2.0, 1e-10], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1e-9);
    let on_plane_out = on_plane([1.0, 2.0, 0.5], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1e-9);

    // line through (0,0,0)-(4,0,0): distances + on/off
    let a = [0.0, 0.0, 0.0];
    let b = [4.0, 0.0, 0.0];
    let d_line = distance_point_line([5.0, 12.0, 0.0], a, b);
    let on_line_yes = on_line([2.0, 0.0, 0.0], a, b, 1e-9);
    let on_line_no = on_line([2.0, 3.0, 0.0], a, b, 1e-9);

    // segment distances: clamped projection
    let d_seg_perp = distance_point_segment([2.0, 3.0, 0.0], a, b);
    let d_seg_a = distance_point_segment([-1.0, 0.0, 0.0], a, b);
    let d_seg_b = distance_point_segment([6.0, 0.0, 0.0], a, b);
    let on_seg_yes = on_segment([3.5, 0.0, 0.0], a, b, 1e-9);
    let on_seg_no = on_segment([4.5, 0.0, 0.0], a, b, 1e-9);

    // sphere: unit sphere centered at origin
    let c = [0.0, 0.0, 0.0];
    let r = 1.0;
    let h = 2.0f64.sqrt() / 2.0;
    let in_sph_in = in_sphere([0.5, 0.5, 0.5], c, r, 1e-9);
    let on_sph_yes = on_sphere([h, h, 0.0], c, r, 1e-9);
    let on_sph_no = on_sphere([2.0, 0.0, 0.0], c, r, 1e-9);

    // AABB [0,0,0]-[1,1,1]
    let lo = [0.0, 0.0, 0.0];
    let hi = [1.0, 1.0, 1.0];
    let in_box_center = in_aabb([0.5, 0.5, 0.5], lo, hi, 1e-9);
    let in_box_corner = in_aabb([1.0, 1.0, 1.0], lo, hi, 0.0);
    let in_box_out = in_aabb([1.5, 0.5, 0.5], lo, hi, 1e-9);

    // triangle (0,0,0),(1,0,0),(0,1,0) with coplanarity gate
    let t_a = [0.0, 0.0, 0.0];
    let t_b = [1.0, 0.0, 0.0];
    let t_c = [0.0, 1.0, 0.0];
    let in_tri_centroid = in_triangle_plane([1.0 / 3.0, 1.0 / 3.0, 0.0], t_a, t_b, t_c, 1e-9);
    let in_tri_edge = in_triangle_plane([0.5, 0.5, 0.0], t_a, t_b, t_c, 1e-9);
    let in_tri_out = in_triangle_plane([2.0, 2.0, 0.0], t_a, t_b, t_c, 1e-9);
    let in_tri_lifted = in_triangle_plane([1.0 / 3.0, 1.0 / 3.0, 1.0], t_a, t_b, t_c, 1e-9);

    cases.push(format!(
        r#"{{"id":"dist_point_point","value":{}}}"#,
        fmt(d_pp)
    ));
    cases.push(format!(
        r#"{{"id":"sdf_above_z0","value":{}}}"#,
        fmt(sdf_pos)
    ));
    cases.push(format!(
        r#"{{"id":"sdf_below_z0","value":{}}}"#,
        fmt(sdf_neg)
    ));
    cases.push(format!(
        r#"{{"id":"dist_plane_z0","value":{}}}"#,
        fmt(d_plane_z0)
    ));
    cases.push(format!(
        r#"{{"id":"dist_plane_xyz","value":{}}}"#,
        fmt(d_plane_xyz)
    ));
    cases.push(format!(
        r#"{{"id":"on_plane_in","value":{}}}"#,
        fmt(if on_plane_in { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"on_plane_out","value":{}}}"#,
        fmt(if on_plane_out { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"dist_line_xaxis","value":{}}}"#,
        fmt(d_line)
    ));
    cases.push(format!(
        r#"{{"id":"on_line_yes","value":{}}}"#,
        fmt(if on_line_yes { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"on_line_no","value":{}}}"#,
        fmt(if on_line_no { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"dist_seg_perp","value":{}}}"#,
        fmt(d_seg_perp)
    ));
    cases.push(format!(
        r#"{{"id":"dist_seg_clamp_a","value":{}}}"#,
        fmt(d_seg_a)
    ));
    cases.push(format!(
        r#"{{"id":"dist_seg_clamp_b","value":{}}}"#,
        fmt(d_seg_b)
    ));
    cases.push(format!(
        r#"{{"id":"on_seg_yes","value":{}}}"#,
        fmt(if on_seg_yes { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"on_seg_no","value":{}}}"#,
        fmt(if on_seg_no { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_sphere_yes","value":{}}}"#,
        fmt(if in_sph_in { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"on_sphere_yes","value":{}}}"#,
        fmt(if on_sph_yes { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"on_sphere_no","value":{}}}"#,
        fmt(if on_sph_no { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_box_center","value":{}}}"#,
        fmt(if in_box_center { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_box_corner","value":{}}}"#,
        fmt(if in_box_corner { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_box_out","value":{}}}"#,
        fmt(if in_box_out { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_tri_centroid","value":{}}}"#,
        fmt(if in_tri_centroid { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_tri_edge","value":{}}}"#,
        fmt(if in_tri_edge { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_tri_out","value":{}}}"#,
        fmt(if in_tri_out { 1.0 } else { 0.0 })
    ));
    cases.push(format!(
        r#"{{"id":"in_tri_lifted","value":{}}}"#,
        fmt(if in_tri_lifted { 1.0 } else { 0.0 })
    ));

    let text = format!(
        r#"{{"format":"oep-geo-predicates-golden","version":1,"cases":[{}]}}"#,
        cases.join(",")
    );

    std::fs::write(&path, text).expect("write golden json");
    println!("wrote {} ({} cases)", path.display(), cases.len());
}
