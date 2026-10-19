//! [`Pose`]: a rigid isometry (world-space position + orientation) with
//! interop to the `oep-geo` [`Transform`] and Rodrigues rotation-vector math.

use oep_geo::math::{Mat3, Quat, Vec3};
use oep_geo::Transform;

/// A rigid pose: a position and an orthonormal orientation matrix.
///
/// The orientation maps a body-local vector to the world frame; the position
/// is the world-frame origin of the body's local frame.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    /// World-space position of the body-frame origin.
    pub position: Vec3,
    /// Orientation (body-local -> world), orthonormal.
    pub orientation: Mat3,
}

impl Pose {
    /// The identity pose (world = local).
    pub fn identity() -> Self {
        Self {
            position: Vec3::zero(),
            orientation: Mat3::identity(),
        }
    }

    /// A pure translational pose.
    pub fn translation(x: f64, y: f64, z: f64) -> Self {
        Self {
            position: Vec3::new(x, y, z),
            orientation: Mat3::identity(),
        }
    }

    /// Read a pose from an `oep-geo` instance [`Transform`] (16 column-major
    /// doubles: rotation block at `[0..3],[4..7],[8..11]`, translation at
    /// `[12..14]` -- the layout contract shared by the schema `Transform` and
    /// the vendored `Mat4`).
    ///
    /// The rotation block is copied as-is (the schema guarantees a unit-length
    /// orthonormal block for valid models; this reader does not re-orthonormalize).
    pub fn from_transform(t: &Transform) -> Self {
        let c = t.components();
        Self {
            position: Vec3::new(c[12], c[13], c[14]),
            orientation: Mat3::from_cols(
                Vec3::new(c[0], c[1], c[2]),
                Vec3::new(c[4], c[5], c[6]),
                Vec3::new(c[8], c[9], c[10]),
            ),
        }
    }

    /// Encode this pose as an `oep-geo` instance [`Transform`] (identity `w`
    /// row, translation in `[12..14]`).
    pub fn to_transform(&self) -> Transform {
        let mut m = [0.0_f64; 16];
        for (i, v) in [
            self.orientation.get(0, 0),
            self.orientation.get(1, 0),
            self.orientation.get(2, 0),
            0.0,
            self.orientation.get(0, 1),
            self.orientation.get(1, 1),
            self.orientation.get(2, 1),
            0.0,
            self.orientation.get(0, 2),
            self.orientation.get(1, 2),
            self.orientation.get(2, 2),
            0.0,
            self.position.x,
            self.position.y,
            self.position.z,
            1.0,
        ]
        .iter()
        .enumerate()
        {
            m[i] = *v;
        }
        Transform(m)
    }

    /// Map a body-local point to the world frame.
    pub fn transform_point(&self, local: &Vec3) -> Vec3 {
        self.orientation.mul_vec3(local).add(&self.position)
    }

    /// Map a body-local direction (untranslated) to the world frame.
    pub fn transform_vector(&self, local: &Vec3) -> Vec3 {
        self.orientation.mul_vec3(local)
    }

    /// Compose `self` then `other`: the world pose of a frame placed at
    /// `other` inside `self`'s frame. Returns pose such that points map as
    /// `self(other(local))`.
    pub fn then(&self, other: &Pose) -> Pose {
        Pose {
            position: self.transform_point(&other.position),
            orientation: self.orientation.mul(&other.orientation),
        }
    }
}

/// Build a rotation matrix from a rotation vector (Rodrigues):
/// `r = axis * angle`, `axis = normalize(r)` when `||r|| > 0`.
pub fn rotvec_to_mat3(r: &Vec3) -> Mat3 {
    let n = r.norm();
    if n < 1e-14 {
        return Mat3::identity();
    }
    // oep-geo rejects a non-finite axis; the solver only emits finite states.
    Mat3::from_axis_angle(r.div(n), n).unwrap_or(Mat3::identity())
}

/// Extract a rotation vector from a rotation matrix (Rodrigues inverse):
/// the result satisfies `rotvec_to_mat3(&result) == m`.
pub fn mat3_to_rotvec(m: &Mat3) -> Vec3 {
    let q = Quat::from_rotation_matrix(*m);
    let (axis, angle) = q.to_axis_angle();
    // from_rotation_matrix yields `angle >= 0` with the axis direction chosen
    // to match; a zero rotation gives a zero axis.
    if angle.abs() < 1e-14 {
        Vec3::zero()
    } else {
        axis.mul(angle)
    }
}

/// Orientation residual between two orientations: a 3-vector that is zero
/// exactly when `a == b` and, for small misalignment, approximates the
/// rotation vector of `a^-1 * b`. Implemented from the quaternion vector part:
/// `2 * imag(quat(a^-1 * b))`.
pub fn orientation_residual(a: &Mat3, b: &Mat3) -> Vec3 {
    // rel = a^-1 * b -- rotation taking b's frame into a's frame.
    let rel = a.transpose().mul(b);
    let q = Quat::from_rotation_matrix(rel);
    Vec3::new(2.0 * q.x, 2.0 * q.y, 2.0 * q.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_geo::math::Vec3;

    #[test]
    fn transform_round_trip_preserves_known_layout() {
        // instance transform for a fastener at (10, 30, 0) from the real
        // example-bracket.oepgeo.json golden corpus.
        let t = Transform([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 30.0, 0.0, 1.0,
        ]);
        let p = Pose::from_transform(&t);
        assert_eq!(p.position, Vec3::new(10.0, 30.0, 0.0));
        let back = p.to_transform();
        let c = back.components();
        assert!((c[12] - 10.0).abs() < 1e-12);
        assert!((c[13] - 30.0).abs() < 1e-12);
        assert!((c[1] - 0.0).abs() < 1e-12);
        assert!((c[15] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn rotvec_round_trip() {
        for angle in [
            0.1_f64,
            1.0,
            2.8,
            std::f64::consts::PI,
            std::f64::consts::PI - 0.01,
            -1.7,
        ] {
            let r = Vec3::new(0.0, 0.0, 1.0).mul(angle);
            let m = rotvec_to_mat3(&r);
            let re = mat3_to_rotvec(&m);
            let expect = if angle.abs() > std::f64::consts::PI {
                // to_axis_angle maps to [0, pi]; large negatives fold over.
                let mut a = angle;
                while a > std::f64::consts::PI {
                    a -= 2.0 * std::f64::consts::PI;
                }
                while a < -std::f64::consts::PI {
                    a += 2.0 * std::f64::consts::PI;
                }
                a
            } else {
                angle
            };
            assert!((re.z - expect).abs() < 1e-9, "angle {angle} -> {}", re.z);
        }
    }

    #[test]
    fn orientation_residual_zero_when_aligned() {
        let a = rotvec_to_mat3(&Vec3::new(0.2, -0.1, 0.05));
        let r = orientation_residual(&a, &a);
        assert!(r.norm() < 1e-12);
    }

    #[test]
    fn compose_then() {
        let root = Pose::translation(5.0, 6.0, 7.0);
        let inner = Pose::translation(1.0, 2.0, 3.0);
        let out = root.then(&inner);
        assert_eq!(out.position, Vec3::new(6.0, 8.0, 10.0));
    }
}
