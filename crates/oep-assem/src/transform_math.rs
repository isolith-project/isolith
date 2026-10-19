//! Small 3D math helpers for the assembly workbench.
//!
//! These are elementary vector/quaternion/matrix operations used to map
//! between the workbench's anchor model and the [`oep_geo::Transform`]
//! storage convention. This is NOT a constraint solver and nothing here
//! reimplements solving; the workbench delegates all solving to the adopted
//! engine (see [`crate::adopted`]).

use oep_geo::Transform;

/// A 3-vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct V3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl V3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }

    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let n = self.norm();
        if n == 0.0 {
            return Self::new(0.0, 0.0, 0.0);
        }
        Self {
            x: self.x / n,
            y: self.y / n,
            z: self.z / n,
        }
    }
}

impl std::ops::Add for V3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            x: self.x + o.x,
            y: self.y + o.y,
            z: self.z + o.z,
        }
    }
}

impl std::ops::Sub for V3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self {
            x: self.x - o.x,
            y: self.y - o.y,
            z: self.z - o.z,
        }
    }
}

impl std::ops::Mul<f64> for V3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

impl From<[f64; 3]> for V3 {
    fn from(a: [f64; 3]) -> Self {
        Self {
            x: a[0],
            y: a[1],
            z: a[2],
        }
    }
}

impl From<V3> for [f64; 3] {
    fn from(v: V3) -> Self {
        [v.x, v.y, v.z]
    }
}

/// Rotation matrix (3x3, row-major) from a quaternion `[w, x, y, z]`.
pub fn quat_to_matrix(q: [f64; 4]) -> [f64; 9] {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - w * z),
        2.0 * (x * z + w * y),
        2.0 * (x * y + w * z),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - w * x),
        2.0 * (x * z - w * y),
        2.0 * (y * z + w * x),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

/// Rotate vector `v` by rotation matrix `m` (row-major 3x3).
pub fn mat3_apply(m: [f64; 9], v: V3) -> V3 {
    V3::new(
        m[0] * v.x + m[1] * v.y + m[2] * v.z,
        m[3] * v.x + m[4] * v.y + m[5] * v.z,
        m[6] * v.x + m[7] * v.y + m[8] * v.z,
    )
}

/// Apply an [`oep_geo::Transform`] (4x4 row-major) to a point (affine).
pub fn transform_apply(t: &Transform, p: V3) -> V3 {
    let m = t.components();
    V3::new(
        m[0] * p.x + m[1] * p.y + m[2] * p.z + m[12],
        m[4] * p.x + m[5] * p.y + m[6] * p.z + m[13],
        m[8] * p.x + m[9] * p.y + m[10] * p.z + m[14],
    )
}

/// Rotate a direction by the rotation part of an [`oep_geo::Transform`].
pub fn transform_rotate(t: &Transform, v: V3) -> V3 {
    let m = t.components();
    V3::new(
        m[0] * v.x + m[1] * v.y + m[2] * v.z,
        m[4] * v.x + m[5] * v.y + m[6] * v.z,
        m[8] * v.x + m[9] * v.y + m[10] * v.z,
    )
}

/// Build an affine [`oep_geo::Transform`] from a rotation matrix (row-major
/// 3x3) and a translation. The resulting `Transform` maps local coords to
/// world: `world = R * local + t`.
pub fn transform_from_rot_trans(r: [f64; 9], t: V3) -> Transform {
    Transform([
        r[0], r[1], r[2], 0.0, r[3], r[4], r[5], 0.0, r[6], r[7], r[8], 0.0, t.x, t.y, t.z, 1.0,
    ])
}

/// A fully-defined placement deduced from the solved pose of a moving anchor:
/// given the anchor's local origin, the solved world position of that origin,
/// and the solved part rotation (quaternion), produce the instance transform.
pub fn transform_from_solved_pose(
    local_origin: [f64; 3],
    world_origin: [f64; 3],
    quat: [f64; 4],
) -> Transform {
    let r = quat_to_matrix(quat);
    let translation = V3::from(world_origin) - mat3_apply(r, V3::from(local_origin));
    transform_from_rot_trans(r, translation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_quat_is_identity_rotation() {
        let r = quat_to_matrix([1.0, 0.0, 0.0, 0.0]);
        let v = mat3_apply(r, V3::new(2.0, 3.0, 4.0));
        assert!((v.x - 2.0).abs() < 1e-12);
        assert!((v.y - 3.0).abs() < 1e-12);
        assert!((v.z - 4.0).abs() < 1e-12);
    }

    #[test]
    fn quat_90_about_z_maps_x_to_y() {
        // q = (cos45, 0, 0, sin45) rotates +X to +Y.
        let r = quat_to_matrix([
            std::f64::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            std::f64::consts::FRAC_1_SQRT_2,
        ]);
        let v = mat3_apply(r, V3::new(1.0, 0.0, 0.0));
        assert!(v.x.abs() < 1e-12);
        assert!((v.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn transform_apply_translates() {
        let t = Transform::translation(4.0, 5.0, 6.0);
        let p = transform_apply(&t, V3::new(1.0, 1.0, 1.0));
        assert_eq!((p.x, p.y, p.z), (5.0, 6.0, 7.0));
    }
}
