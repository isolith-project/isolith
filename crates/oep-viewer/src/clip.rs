//! World-space clip planes: signed half-space classification of points and
//! boxes, plus a polygon clipper (Sutherland-Hodgman) used by the reference
//! renderer to slice geometry on the kept side of every plane.
//!
//! A clip plane is `normal · p + d <= eps` (kept side is the half-space the
//! normal points into, following the common "inside above the plane" rule).
//! [`Side::Inside`] means entirely kept, [`Side::Outside`] means entirely
//! removed, [`Side::Intersecting`] spans the plane.

use crate::scene::Aabb;
use oep_geo::math::Vec3;

/// Classification of a shape vs a single clip plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Fully on the kept side.
    Inside,
    /// Fully on the removed side.
    Outside,
    /// Spans the plane.
    Intersecting,
}

/// A world-space clip plane: `normal · p + d = 0`; the kept side satisfies
/// `normal · p + d <= 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipPlane {
    /// Unit (or normalized) plane normal, pointing at the removed side.
    pub normal: Vec3,
    /// Plane offset such that `normal · p + d = 0`.
    pub d: f64,
}

impl ClipPlane {
    /// Build a plane from a point on it and a normal (`normal` is normalized
    /// internally; `p` may be any point on the plane).
    pub fn point_normal(p: Vec3, normal: Vec3) -> Self {
        let n = normal
            .normalize()
            .expect("clip plane normal must not be zero");
        let d = -n.dot(&p);
        Self { normal: n, d }
    }

    /// Build a plane from the Hessian normal form `n · x + d = 0`.
    pub fn hessian(normal: Vec3, d: f64) -> Self {
        let n = normal
            .normalize()
            .expect("clip plane normal must not be zero");
        Self { normal: n, d }
    }

    /// Signed distance of a point from the plane (positive on removed side).
    pub fn signed_distance(&self, p: &Vec3) -> f64 {
        self.normal.dot(p) + self.d
    }

    /// Classify a single point.
    pub fn classify_point(&self, p: &Vec3, eps: f64) -> Side {
        let d = self.signed_distance(p);
        if d > eps {
            Side::Outside
        } else if d < -eps {
            Side::Inside
        } else {
            Side::Intersecting
        }
    }

    /// Classify an AABB against the plane (conservative: uses all 8 corners).
    pub fn classify_aabb(&self, bb: &Aabb) -> Side {
        let mut inside = false;
        let mut outside = false;
        for c in bb.corners() {
            if self.signed_distance(&c) > 0.0 {
                outside = true;
            } else {
                inside = true;
            }
        }
        match (inside, outside) {
            (true, true) => Side::Intersecting,
            (true, false) => Side::Inside,
            (false, _) => Side::Outside,
        }
    }

    /// Test if a point is on the kept side (distance <= 0).
    pub fn keeps(&self, p: &Vec3) -> bool {
        self.signed_distance(p) <= 0.0
    }

    /// Clip a convex polygon (list of world points, CCW) against this plane,
    /// returning the polygon on the kept side (Sutherland-Hodgman).
    pub fn clip_polygon(&self, poly: &[Vec3]) -> Vec<Vec3> {
        if poly.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(poly.len() + 2);
        let n = poly.len();
        for i in 0..n {
            let cur = poly[i];
            let next = poly[(i + 1) % n];
            let cur_kept = self.keeps(&cur);
            let next_kept = self.keeps(&next);
            if cur_kept {
                out.push(cur);
            }
            if cur_kept != next_kept {
                // Emit the intersection of edge(cur,next) with the plane.
                if let Some(p) = intersect_edge(self, cur, next) {
                    out.push(p);
                }
            }
        }
        out
    }
}

/// Intersection of segment (a, b) with the plane, or `None` if parallel.
fn intersect_edge(plane: &ClipPlane, a: Vec3, b: Vec3) -> Option<Vec3> {
    let da = plane.signed_distance(&a);
    let db = plane.signed_distance(&b);
    let denom = da - db;
    if denom.abs() < 1e-15 {
        return None;
    }
    let t = da / denom;
    let t = t.clamp(0.0, 1.0);
    Some(Vec3::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_normal_plane_signed_distance() {
        // Plane x = 0, normal +X (kept side is x <= 0).
        let pl = ClipPlane::point_normal(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(pl.signed_distance(&Vec3::new(5.0, 0.0, 0.0)) > 0.0); // removed
        assert!(pl.signed_distance(&Vec3::new(-5.0, 0.0, 0.0)) < 0.0); // kept
        assert!((pl.signed_distance(&Vec3::new(0.0, 0.0, 0.0))).abs() < 1e-12);
    }

    #[test]
    fn classifies_box_sides() {
        let pl = ClipPlane::point_normal(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let all_kept = Aabb::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(-1.0, 5.0, 5.0));
        let all_removed = Aabb::new(Vec3::new(1.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0));
        let spanning = Aabb::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0));
        assert_eq!(pl.classify_aabb(&all_kept), Side::Inside);
        assert_eq!(pl.classify_aabb(&all_removed), Side::Outside);
        assert_eq!(pl.classify_aabb(&spanning), Side::Intersecting);
    }

    #[test]
    fn clips_polygon_on_kept_side() {
        let pl = ClipPlane::point_normal(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        // A square centered on origin (spans the x=0 plane).
        let poly = vec![
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ];
        let clipped = pl.clip_polygon(&poly);
        // Kept side is x <= 0: all output points must satisfy it.
        assert!(!clipped.is_empty());
        for p in &clipped {
            assert!(p.x <= 1e-9, "kept point outside plane: {:?}", p);
        }
        // A quad crossing a vertical cut yields 4 points after clipping.
        assert_eq!(clipped.len(), 4);
    }
}
