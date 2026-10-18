//! P6-M2-T01 acceptance contract for the vendored math core (RED).
//!
//! Written BEFORE `oep_geo::math` exists; against the skeleton this does not
//! compile, proving TDD RED. Locks the public API + behaviors:
//!
//! 1. Known-good geometric identities (golden, byte-compared against a committed
//!    `math-golden.json` produced BY this crate, then independently re-verified
//!    by the Python acceptance test).
//! 2. Property-test invariants over seeded random inputs (deterministic xorshift,
//!    std-only, no proptest dep): rotation length preservation, quaternion
//!    round-trips, matrix associativity, inverse-recover, translation
//!    composition, column-major [f64;16] round-trip.
//! 3. Non-panicking error model: `normalize` on zero, `from_axis_angle` on a
//!    degenerate axis, `inverse` on singular, and non-finite inputs all yield
//!    typed `MathError` values instead of panics.

use oep_geo::math::{Mat3, Mat4, MathError, Quat, Vec3};

const EPS: f64 = 1e-9;

fn assert_close(a: f64, b: f64, what: &str) {
    assert!(
        (a - b).abs() <= EPS,
        "{what}: expected {b}, got {a} (diff {})",
        (a - b).abs()
    );
}

fn assert_vec_close(a: &Vec3, b: &Vec3, what: &str) {
    let d = a.sub(b).norm();
    assert!(d <= EPS, "{what}: expected {b:?}, got {a:?} (dist {d})");
}

// ---------------------------------------------------------------------------
// Deterministic PRNG for property tests (xorshift64*). Seeded => reproducible.
// ---------------------------------------------------------------------------
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in [-1, 1).
    fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
    /// Random unit vector (normalize three Gaussians via Box-Muller).
    fn unit_vec(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.signed(), self.signed(), self.signed());
            if let Ok(u) = v.normalize() {
                return u;
            }
        }
    }
    /// Random angle in [-2pi, 2pi).
    fn angle(&mut self) -> f64 {
        self.unit() * 4.0 * std::f64::consts::PI - 2.0 * std::f64::consts::PI
    }
    /// Random non-degenerate affine Mat4 (nonsingular 3x3 linear + translation).
    fn affine_mat4(&mut self) -> Mat4 {
        // random rotation (orthonormal -> always invertible) + axis scale to vary det.
        let q = Quat::from_axis_angle(self.unit_vec(), self.angle()).unwrap();
        let r = q.to_matrix3();
        let s = Mat3::from_cols(
            Vec3::new(1.0 + self.unit(), 0.0, 0.0),
            Vec3::new(0.0, 1.0 + self.unit(), 0.0),
            Vec3::new(0.0, 0.0, 1.0 + self.unit()),
        );
        let lin = r.mul(&s);
        Mat4::from_rotation(lin).mul(&Mat4::from_translation(Vec3::new(
            self.signed() * 100.0,
            self.signed() * 100.0,
            self.signed() * 100.0,
        )))
    }
}

// ---------------------------------------------------------------------------
// Golden-file cases (the crate writes math-golden.json via tools/gen_golden.rs;
// BOTH the Rust contract below and tests/test_oep_geo_math_acceptance.py verify
// the committed bytes independently).
// ---------------------------------------------------------------------------
mod golden {
    pub const ROT_Z90_POINT: [f64; 3] = [1.0, 0.0, 0.0];
    pub const ROT_Z90_OUT: [f64; 3] = [0.0, 1.0, 0.0];
    pub const TRANS_PT: [f64; 3] = [0.0, 0.0, 0.0];
    pub const TRANS_T: [f64; 3] = [1.0, 2.0, 3.0];
}

#[test]
fn golden_rotate_x_about_z_90() {
    let m = Mat4::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
    let p = Vec3::new(
        golden::ROT_Z90_POINT[0],
        golden::ROT_Z90_POINT[1],
        golden::ROT_Z90_POINT[2],
    );
    let out = m.transform_point(&p);
    assert_vec_close(
        &out,
        &Vec3::new(
            golden::ROT_Z90_OUT[0],
            golden::ROT_Z90_OUT[1],
            golden::ROT_Z90_OUT[2],
        ),
        "90deg Z rotation of (1,0,0)",
    );
}

#[test]
fn golden_quat_rotate_x_about_z_90() {
    let q = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
    let p = Vec3::new(
        golden::ROT_Z90_POINT[0],
        golden::ROT_Z90_POINT[1],
        golden::ROT_Z90_POINT[2],
    );
    let out = q.rotate_vector(&p);
    assert_vec_close(
        &out,
        &Vec3::new(
            golden::ROT_Z90_OUT[0],
            golden::ROT_Z90_OUT[1],
            golden::ROT_Z90_OUT[2],
        ),
        "quat 90deg Z rotation of (1,0,0)",
    );
}

#[test]
fn golden_translation_compose() {
    let t1 = Mat4::from_translation(Vec3::new(
        golden::TRANS_T[0],
        golden::TRANS_T[1],
        golden::TRANS_T[2],
    ));
    let t2 = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));
    // (T(1,2,3) o T(4,5,6)) applied to origin == (5,7,9)
    let composed = t1.mul(&t2);
    let out = composed.transform_point(&Vec3::new(
        golden::TRANS_PT[0],
        golden::TRANS_PT[1],
        golden::TRANS_PT[2],
    ));
    assert_vec_close(&out, &Vec3::new(5.0, 7.0, 9.0), "translation composition");
}

#[test]
fn golden_roundtrip_array16() {
    let m = Mat4::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize().unwrap(), 0.7).unwrap();
    let arr = m.to_array16();
    let back = Mat4::from_array16(arr).unwrap();
    let back_arr = back.to_array16();
    for (i, (a, b)) in arr.iter().zip(back_arr.iter()).enumerate() {
        assert_close(*a, *b, &format!("array16[{i}] round-trip"));
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

#[test]
fn prop_rotation_isometry_mat4() {
    let mut rng = XorShift::new(0x6D61_7468); // "math"
    for _ in 0..2000 {
        let m = Mat4::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let v = rng.unit_vec().mul(10.0);
        let out = m.transform_vector(&v);
        assert_close(
            out.norm(),
            v.norm(),
            "Mat4 rotation preserves vector length",
        );
    }
}

#[test]
fn prop_quat_rotation_isometry() {
    let mut rng = XorShift::new(0x51CA_1E50);
    for _ in 0..2000 {
        let q = Quat::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let v = rng.unit_vec().mul(rng.unit() * 100.0);
        let out = q.rotate_vector(&v);
        assert_close(out.norm(), v.norm(), "quat rotation preserves length");
    }
}

#[test]
fn prop_quat_inverse_roundtrip() {
    let mut rng = XorShift::new(0x0a11_babe);
    for _ in 0..2000 {
        let q = Quat::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let v = rng.unit_vec().mul(7.0);
        let qinv = q.inverse().unwrap();
        let round = qinv.rotate_vector(&q.rotate_vector(&v));
        assert_vec_close(&round, &v, "q^-1(q(v)) == v");
    }
}

#[test]
fn prop_quat_group_morphism() {
    // rotate_vector(q1 * q2, v) == q1.rotate_vector(q2.rotate_vector(v))
    let mut rng = XorShift::new(0xD00D_FACE);
    for _ in 0..1500 {
        let q1 = Quat::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let q2 = Quat::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let v = rng.unit_vec().mul(3.0);
        let via_product = q1.mul(&q2).rotate_vector(&v);
        let via_apply = q1.rotate_vector(&q2.rotate_vector(&v));
        assert_vec_close(
            &via_product,
            &via_apply,
            "quat multiplication = composition",
        );
    }
}

#[test]
fn prop_quat_matrix_equiv() {
    let mut rng = XorShift::new(0x00C0_FFEE);
    for _ in 0..1500 {
        let q = Quat::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let v = rng.unit_vec().mul(5.0);
        let from_q = q.rotate_vector(&v);
        let from_m = q.to_matrix3().mul_vec3(&v);
        assert_vec_close(&from_q, &from_m, "quat rotation == matrix rotation");
    }
}

#[test]
fn prop_mat3_associative() {
    let mut rng = XorShift::new(0x0A55_0C1A);
    for _ in 0..1500 {
        let a = Mat3::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let b = Mat3::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let c = Mat3::from_axis_angle(rng.unit_vec(), rng.angle()).unwrap();
        let lhs = a.mul(&b).mul(&c);
        let rhs = a.mul(&b.mul(&c));
        for i in 0..3 {
            for j in 0..3 {
                assert_close(lhs.get(i, j), rhs.get(i, j), "Mat3 associativity");
            }
        }
    }
}

#[test]
fn prop_mat4_inverse_recovers() {
    let mut rng = XorShift::new(0xDEAD_BEEF);
    for _ in 0..1200 {
        let m = rng.affine_mat4();
        let inv = m.inverse().unwrap();
        let id = m.mul(&inv);
        for i in 0..16 {
            if i % 5 == 0 {
                assert_close(id.to_array16()[i], 1.0, "M*M^-1 == I (diag)");
            } else {
                assert_close(id.to_array16()[i], 0.0, "M*M^-1 == I (off-diag)");
            }
        }
    }
}

#[test]
fn prop_mat4_array16_roundtrip_random() {
    let mut rng = XorShift::new(0x0A11_CE16);
    for _ in 0..1500 {
        let m = rng.affine_mat4();
        let arr = m.to_array16();
        assert_eq!(arr.len(), 16);
        assert!(arr.iter().all(|x| x.is_finite()), "array16 entries finite");
        let back = Mat4::from_array16(arr).unwrap();
        for i in 0..16 {
            assert_close(
                m.to_array16()[i],
                back.to_array16()[i],
                &format!("array16[{i}] random round-trip"),
            );
        }
    }
}

#[test]
fn prop_translation_composition_random() {
    let mut rng = XorShift::new(0x7A4E_510E);
    for _ in 0..2000 {
        let t1 = Vec3::new(
            rng.signed() * 100.0,
            rng.signed() * 100.0,
            rng.signed() * 100.0,
        );
        let t2 = Vec3::new(
            rng.signed() * 100.0,
            rng.signed() * 100.0,
            rng.signed() * 100.0,
        );
        let p = Vec3::new(
            rng.signed() * 100.0,
            rng.signed() * 100.0,
            rng.signed() * 100.0,
        );
        let m = Mat4::from_translation(t1).mul(&Mat4::from_translation(t2));
        let out = m.transform_point(&p);
        // translating by t1 then t2 == translating by (t1 + t2)
        let expect = p.add(&t1).add(&t2);
        assert_vec_close(&out, &expect, "T(t1)o T(t2) == T(t1+t2)");
    }
}

#[test]
fn prop_vec3_dot_cross_identities() {
    let mut rng = XorShift::new(0xBE_EF0000);
    for _ in 0..3000 {
        let a = Vec3::new(rng.signed(), rng.signed(), rng.signed());
        let b = Vec3::new(rng.signed(), rng.signed(), rng.signed());
        let c = Vec3::new(rng.signed(), rng.signed(), rng.signed());
        // cross is anti-symmetric
        assert_vec_close(&a.cross(&b).neg(), &b.cross(&a), "a x b == -(b x a)");
        // a . (a x b) == 0
        assert_close(a.dot(&a.cross(&b)), 0.0, "a.(a x b) == 0");
        // distributivity
        assert_close(
            a.dot(&b.add(&c)),
            a.dot(&b) + a.dot(&c),
            "a.(b+c) == a.b + a.c",
        );
        // scalar triple product identity: a.(b x c) == b.(c x a)
        assert_close(
            a.dot(&b.cross(&c)),
            b.dot(&c.cross(&a)),
            "a.(b x c) == b.(c x a)",
        );
    }
}

#[test]
fn prop_vec3_normalize() {
    let mut rng = XorShift::new(0x0F00_D0A1);
    for _ in 0..2000 {
        let v = Vec3::new(
            rng.signed() * 100.0,
            rng.signed() * 100.0,
            rng.signed() * 100.0,
        );
        if v.norm() < 1e-12 {
            continue;
        }
        let u = v.normalize().unwrap();
        assert_close(u.norm(), 1.0, "normalize keeps unit length");
        assert!(u.dot(&v) > 0.0, "normalize points same direction as input");
    }
}

#[test]
fn errors_are_typed_not_panics() {
    // normalize(0) -> Err
    assert!(matches!(
        Vec3::new(0.0, 0.0, 0.0).normalize(),
        Err(MathError::InsufficientNorm)
    ));
    // from_axis_angle on zero axis -> Err
    assert!(matches!(
        Quat::from_axis_angle(Vec3::new(0.0, 0.0, 0.0), 1.0),
        Err(MathError::DegenerateAxis)
    ));
    assert!(matches!(
        Mat4::from_axis_angle(Vec3::new(0.0, 0.0, 0.0), 1.0),
        Err(MathError::DegenerateAxis)
    ));
    // inverse of singular matrix -> Err
    assert!(matches!(
        Mat3::zero().inverse(),
        Err(MathError::SingularMatrix)
    ));
    // non-finite fields are rejected
    assert!(matches!(
        Vec3::new(f64::NAN, 0.0, 0.0).normalize(),
        Err(MathError::NonFiniteInput)
    ));
    let nan4 = Mat4::zero();
    let arr_nan = {
        let mut a = nan4.to_array16();
        a[0] = f64::INFINITY;
        a
    };
    assert!(matches!(
        Mat4::from_array16(arr_nan),
        Err(MathError::NonFiniteInput)
    ));
}

#[test]
fn slerp_endpoints_and_midpoint() {
    let q0 = Quat::identity();
    let q1 = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
    // endpoints
    assert_close(q0.slerp(&q1, 0.0).dot(&q0), 1.0, "slerp t=0 == start");
    assert_close(q0.slerp(&q1, 1.0).dot(&q1), 1.0, "slerp t=1 == end");
    // midpoint of 90deg about Z == 45deg about Z
    let mid = q0.slerp(&q1, 0.5);
    let (_, ang) = mid.to_axis_angle();
    assert_close(
        ang.abs(),
        std::f64::consts::FRAC_PI_4,
        "slerp midpoint is 45deg",
    );
}
