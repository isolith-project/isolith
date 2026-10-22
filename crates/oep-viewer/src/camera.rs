//! Orbit camera over `oep_geo::math`: look-at view, perspective projection.
//!
//! The camera orbits a target point by yaw/pitch at a distance, producing a
//! right-handed view matrix and a matching perspective projection. Picking
//! reuses the same matrices to unproject a screen point into a world-space
//! [`Ray`](crate::pick::Ray).

use crate::scene::Aabb;
use oep_geo::math::{Mat4, Vec3};

/// An orbiting preview camera. All angles in radians, distances in document
/// units (mm for the golden model).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Orbit target (look-at point).
    pub target: Vec3,
    /// Azimuth around the world +Y axis, radians.
    pub yaw: f64,
    /// Elevation above the XZ plane, radians.
    pub pitch: f64,
    /// Distance from eye to target.
    pub distance: f64,
    /// Vertical field of view, radians.
    pub fov_y: f64,
    /// Near clip plane distance.
    pub near: f64,
    /// Far clip plane distance.
    pub far: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::zero(),
            yaw: 0.7,
            pitch: 0.5,
            distance: 120.0,
            fov_y: 45f64.to_radians(),
            near: 0.1,
            far: 5000.0,
        }
    }
}

impl Camera {
    /// Build a camera framing the given world bounds (fit-to-scene).
    pub fn framed(bounds: &Aabb) -> Self {
        let centroid = bounds.centroid();
        let radius = bounds.sphere_radius().max(1e-6);
        // Fit the bounding sphere in the vertical fov with a 20% margin.
        let distance = (radius / (45f64.to_radians() / 2.0).sin()) * 1.2;
        Self {
            target: centroid,
            distance,
            near: (distance - radius * 2.0).max(1e-3),
            far: (distance + radius * 4.0).max((distance - radius * 2.0).max(1e-3) * 10.0),
            ..Self::default()
        }
    }

    /// Eye position derived from target/yaw/pitch/distance.
    pub fn eye(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        let offset = Vec3::new(cp * sy, sp, cp * cy).mul(self.distance);
        self.target.add(&offset)
    }

    /// Right-handed look-at view matrix (world -> view).
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.eye();
        let f = self
            .target
            .sub(&eye)
            .normalize()
            .expect("degenerate camera");
        let up = Vec3::new(0.0, 1.0, 0.0);
        let s = f.cross(&up).normalize().expect("camera pointing along Y");
        let u = s.cross(&f);
        // Column-major openGL-style lookAt: rows become columns.
        let m03 = -s.dot(&eye);
        let m13 = -u.dot(&eye);
        let m23 = f.dot(&eye);
        let arr = [
            s.x, u.x, -f.x, 0.0, // col 0
            s.y, u.y, -f.y, 0.0, // col 1
            s.z, u.z, -f.z, 0.0, // col 2
            m03, m13, m23, 1.0, // col 3
        ];
        Mat4::from_array16(arr).expect("view matrix is finite")
    }

    /// Right-handed perspective projection (view -> clip).
    /// Column-major, matching `Mat4` storage.
    pub fn projection(&self, aspect: f64) -> Mat4 {
        let f = 1.0 / (self.fov_y / 2.0).tan();
        let nf = 1.0 / (self.near - self.far);
        let arr = [
            f / aspect,
            0.0,
            0.0,
            0.0, // col 0
            0.0,
            f,
            0.0,
            0.0, // col 1
            0.0,
            0.0,
            (self.far + self.near) * nf,
            -1.0, // col 2
            0.0,
            0.0,
            2.0 * self.far * self.near * nf,
            0.0, // col 3
        ];
        Mat4::from_array16(arr).expect("projection matrix is finite")
    }

    /// View-Projection matrix (world -> clip).
    pub fn view_projection(&self, aspect: f64) -> Mat4 {
        self.projection(aspect).mul(&self.view_matrix())
    }

    /// Orbit by adding deltas to yaw/pitch.
    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-1.55, 1.55);
    }

    /// Zoom by a multiplicative factor (< 1 moves closer).
    pub fn zoom(&mut self, factor: f64) {
        self.distance *= factor;
        self.distance = self.distance.max(self.near * 2.0);
    }

    /// Pan by moving the orbit target along the camera's right/up screen axes
    /// by `dx` / `dy` world units (the viewport pan gesture). Deterministic
    /// and headless-callable like every other camera operation.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let eye = self.eye();
        let f = self
            .target
            .sub(&eye)
            .normalize()
            .expect("degenerate camera");
        let up = Vec3::new(0.0, 1.0, 0.0);
        let s = f.cross(&up).normalize().expect("camera pointing along Y");
        let u = s.cross(&f);
        self.target = self.target.add(&s.mul(-dx)).add(&u.mul(dy));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_camera_looks_at_centroid() {
        let b = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(80.0, 40.0, 20.0));
        let cam = Camera::framed(&b);
        assert_eq!(cam.target, Vec3::new(40.0, 20.0, 10.0));
        assert!(cam.eye().distance_to(&cam.target) > 1.0);
        // Looking straight at the target: the target projects to the view
        // direction (camera-space z negative, on the -Z axis).
        let v = cam.view_matrix().transform_point(&cam.target);
        assert!(v.z < 0.0);
    }
}
