//! P6-M2-T01 golden artifact generator: writes `tests/golden/math-golden.json`
//! from the crate's own math implementation.
//!
//! The committed golden JSON is produced BY this crate (self-consistent) and then
//! independently re-verified two ways:
//!
//! 1. `crates/oep-geo/tests/math_contract.rs` re-derives the same figures and
//!    asserts them directly against closed-form expectations.
//! 2. `tests/test_oep_geo_math_acceptance.py` re-computes them independently
//!    in Python stdlib (Rodrigues / orthonormality) and cross-checks within 1e-9.
//!
//! So a self-consistent but wrong implementation cannot satisfy both.
//!
//! Usage: `cargo run -p oep-geo --example gen_math_golden -- <out-dir>` (default
//! `crates/oep-geo/tests/golden`).

use oep_geo::math::{Mat4, Quat, Vec3};
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

fn arr3(v: &Vec3) -> String {
    format!("[{}, {}, {}]", fmt(v.x), fmt(v.y), fmt(v.z))
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/oep-geo/tests/golden"));
    std::fs::create_dir_all(&out).expect("create golden dir");
    let path = out.join("math-golden.json");

    let z90 = Mat4::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
    let rot_z90_point = z90.transform_point(&Vec3::new(1.0, 0.0, 0.0));

    let qz90 =
        Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
    let quat_z90_point = qz90.rotate_vector(&Vec3::new(1.0, 0.0, 0.0));

    let t1 = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let t2 = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));
    let translation_compose = t1.mul(&t2).transform_point(&Vec3::new(0.0, 0.0, 0.0));

    // Independent-verification case: quaternion rotation via Rodrigues closed form
    // (recomputed in Python). Axis (1,2,3) normalized, angle 0.7 rad.
    let axis = Vec3::new(1.0, 2.0, 3.0);
    let angle = 0.7;
    let q = Quat::from_axis_angle(axis, angle).unwrap();
    let point = Vec3::new(1.0, -2.0, 0.5);
    let quat_rotate_rodrigues = q.rotate_vector(&point);

    // Inverse-recover case
    let m_inv = Mat4::from_axis_angle(Vec3::new(0.4, -1.1, 2.0).normalize().unwrap(), 1.3)
        .unwrap()
        .mul(&Mat4::from_translation(Vec3::new(3.0, -2.0, 1.5)));
    let pt = Vec3::new(0.5, 0.25, -1.75);
    let inverse_recovers = m_inv
        .inverse()
        .unwrap()
        .transform_point(&m_inv.transform_point(&pt));

    // Pure rotation Mat4 -> column-major [f64;16], orthonormal top-left, zero translation
    let rot = Mat4::from_axis_angle(Vec3::new(0.2, -0.3, 0.4).normalize().unwrap(), 2.1).unwrap();
    let array16: Vec<String> = rot.to_array16().iter().map(|x| fmt(*x)).collect();

    let text = format!(
        r#"{{"format":"oep-geo-math-golden","version":1,"cases":{{
"rot_z90_point":{{"out":{}}},
"quat_z90_point":{{"out":{}}},
"translation_compose":{{"out":{}}},
"quat_rotate_rodrigues":{{"axis":{},"angle":{},"point":{},"out":{}}},
"inverse_recovers":{{"point":{},"out":{}}},
"rotation_mat4_array16":{{"array16":[{}]}}
}}}}"#,
        arr3(&rot_z90_point),
        arr3(&quat_z90_point),
        arr3(&translation_compose),
        arr3(&axis),
        fmt(angle),
        arr3(&point),
        arr3(&quat_rotate_rodrigues),
        arr3(&pt),
        arr3(&inverse_recovers),
        array16.join(","),
    );

    std::fs::write(&path, text).expect("write golden json");
    println!("wrote {}", path.display());
}
