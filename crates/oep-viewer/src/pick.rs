//! Screen-space selection: unproject a pixel into a world ray and find the
//! nearest scene primitive it hits.
//!
//! The preview's selection model is explicitly viewport-driven: the user
//! points at a screen pixel, the camera unprojects it into a world-space ray,
//! and the nearest [`Primitive`](crate::scene::Primitive) intersected is
//! returned with its full identity (instance chain, part id, body id) plus the
//! hit distance and world point.

use crate::camera::Camera;
use crate::scene::{Aabb, Scene};
use oep_geo::math::Vec3;

/// A world-space ray (origin + unit direction).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Ray origin (camera eye).
    pub origin: Vec3,
    /// Unit direction.
    pub dir: Vec3,
}

impl Ray {
    /// Point at parameter `t` along the ray.
    pub fn at(&self, t: f64) -> Vec3 {
        self.origin.add(&self.dir.mul(t))
    }
}

/// The result of a successful pick: which primitive, how far, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// Index into [`Scene::primitives`].
    pub primitive_index: usize,
    /// The primitive's stable id.
    pub id: String,
    /// Selected part id.
    pub part_id: String,
    /// Selected body id.
    pub body_id: String,
    /// Instance chain that placed the primitive (may be empty).
    pub instance_ids: Vec<String>,
    /// Distance along the ray to the hit.
    pub distance: f64,
    /// World-space hit point.
    pub point: Vec3,
}

/// Cast `ray` against an AABB (slab method). Returns the near hit parameter
/// `t`, or `None` on a miss.
pub fn ray_aabb(ray: &Ray, bb: &Aabb) -> Option<f64> {
    let mut tmin = f64::NEG_INFINITY;
    let mut tmax = f64::INFINITY;
    let invx = 1.0 / ray.dir.x;
    let invy = 1.0 / ray.dir.y;
    let invz = 1.0 / ray.dir.z;
    // X slab
    let (t1, t2) = ray_slab(ray.origin.x, invx, bb.min.x, bb.max.x);
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);
    if tmin > tmax {
        return None;
    }
    let (t1, t2) = ray_slab(ray.origin.y, invy, bb.min.y, bb.max.y);
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);
    if tmin > tmax {
        return None;
    }
    let (t1, t2) = ray_slab(ray.origin.z, invz, bb.min.z, bb.max.z);
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);
    if tmin > tmax {
        return None;
    }
    if tmax < 0.0 {
        return None; // box entirely behind the ray
    }
    Some(tmin.max(0.0))
}

#[inline]
fn ray_slab(origin: f64, inv_dir: f64, lo: f64, hi: f64) -> (f64, f64) {
    let t1 = (lo - origin) * inv_dir;
    let t2 = (hi - origin) * inv_dir;
    if t1 < t2 {
        (t1, t2)
    } else {
        (t2, t1)
    }
}

/// Pick the nearest primitive along a world ray.
pub fn pick_ray(scene: &Scene, ray: &Ray) -> Option<Selection> {
    let mut best: Option<Selection> = None;
    for (i, prim) in scene.primitives.iter().enumerate() {
        if let Some(t) = ray_aabb(ray, &prim.world_aabb()) {
            let replace = match &best {
                None => true,
                Some(b) => t < b.distance,
            };
            if replace {
                best = Some(Selection {
                    primitive_index: i,
                    id: prim.id.clone(),
                    part_id: prim.part_id.clone(),
                    body_id: prim.body_id.clone(),
                    instance_ids: prim.instance_ids.clone(),
                    distance: t,
                    point: ray.at(t),
                });
            }
        }
    }
    best
}

/// Unproject a screen pixel to a world ray through the given camera.
/// `x`,`y` are in pixels from the top-left, `w`,`h` are viewport size.
pub fn unproject(cam: &Camera, x: f64, y: f64, w: f64, h: f64) -> Ray {
    let aspect = if h > 0.0 { w / h } else { 1.0 };
    let vp = cam.view_projection(aspect);
    let inv = vp.inverse().expect("view-projection must be invertible");
    // NDC: x in [-1,1] left->right, y in [-1,1] bottom->top.
    let ndc_x = (2.0 * x) / w - 1.0;
    let ndc_y = 1.0 - (2.0 * y) / h;
    // Unproject two clip points (near z=-1, far z=+1) to world, then take the
    // direction between them: a robust way to build the ray with the
    // perspective divide handled by hand.
    let ndc_near = Vec3::new(ndc_x, ndc_y, -1.0);
    let ndc_far = Vec3::new(ndc_x, ndc_y, 1.0);
    let p_near = unproject_point(&inv, ndc_near);
    let p_far = unproject_point(&inv, ndc_far);
    let origin = cam.eye();
    let dir = p_far
        .sub(&p_near)
        .normalize()
        .expect("camera ray degenerate");
    Ray { origin, dir }
}

/// Manually transform an NDC point (x, y, z, w=1) by a 4x4 column-major
/// matrix and divide by the resulting w (perspective divide).
fn unproject_point(m: &oep_geo::math::Mat4, ndc: Vec3) -> Vec3 {
    let a = m.to_array16();
    let x = ndc.x;
    let y = ndc.y;
    let z = ndc.z;
    let w = 1.0;
    let cx = a[0] * x + a[4] * y + a[8] * z + a[12] * w;
    let cy = a[1] * x + a[5] * y + a[9] * z + a[13] * w;
    let cz = a[2] * x + a[6] * y + a[10] * z + a[14] * w;
    let cw = a[3] * x + a[7] * y + a[11] * z + a[15] * w;
    let iw = 1.0 / cw;
    Vec3::new(cx * iw, cy * iw, cz * iw)
}

/// Pick the primitive under a screen pixel.
pub fn pick(scene: &Scene, cam: &Camera, x: f64, y: f64, w: f64, h: f64) -> Option<Selection> {
    let ray = unproject(cam, x, y, w, h);
    pick_ray(scene, &ray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{build_scene, Aabb};
    use oep_geo::{Body, BodyKind, Document, Part};

    fn lazy_doc() -> Document {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p1", "part-one").with_body("b1"));
        doc.push_part(Part::new("p2", "part-two").with_body("b2"));
        doc.push_body(Body::new("b1", "back", BodyKind::Solid).with_bbox(
            oep_geo::BoundingBox::new([-10.0, -10.0, -12.0], [10.0, 10.0, -8.0]),
        ));
        doc.push_body(Body::new("b2", "front", BodyKind::Solid).with_bbox(
            oep_geo::BoundingBox::new([-2.0, -2.0, -4.0], [2.0, 2.0, 4.0]),
        ));
        doc
    }

    #[test]
    fn ray_hits_aabb() {
        let bb = Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let ray = Ray {
            origin: Vec3::new(0.0, 0.0, 10.0),
            dir: Vec3::new(0.0, 0.0, -1.0),
        };
        let t = ray_aabb(&ray, &bb).expect("should hit");
        assert!((t - 9.0).abs() < 1e-9);
    }

    #[test]
    fn ray_misses_aabb_offset() {
        let bb = Aabb::new(Vec3::new(-1.0, 5.0, -1.0), Vec3::new(1.0, 7.0, 1.0));
        let ray = Ray {
            origin: Vec3::new(0.0, 0.0, 10.0),
            dir: Vec3::new(0.0, 0.0, -1.0),
        };
        assert!(ray_aabb(&ray, &bb).is_none());
    }

    #[test]
    fn pick_prefers_nearest() {
        let doc = lazy_doc();
        let scene = build_scene(&doc);
        // Camera looking at origin from +Z.
        let cam = Camera {
            target: Vec3::zero(),
            yaw: 0.0,
            pitch: 0.0,
            distance: 50.0,
            fov_y: 45f64.to_radians(),
            near: 0.1,
            far: 5000.0,
        };
        let sel = pick(&scene, &cam, 400.0, 300.0, 800.0, 600.0).expect("screen centre should hit");
        assert_eq!(sel.part_id, "p2"); // front box (small, centered) is nearer
        assert!(sel.distance > 0.0);
    }
}
