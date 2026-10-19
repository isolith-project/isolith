//! P6-M2-T03: geometric predicates for `oep-geo`.
//!
//! On/off tolerance tests, point-to-primitive distances, and containment tests
//! over [`Point`] = `[f64; 3]`.
//!
//! Design decisions (see `docs/p6-m2-oep-geo-predicates.md` and the PR body):
//!
//! - **`[f64;3]` points, NOT `math::Vec3`.** The P6-M2-T01 math branch is
//!   unmerged; consuming its `Vec3` here would drag a branch dependency into
//!   this card. This module is self-contained (its own tiny vector helpers) and
//!   the point type is reconciled with `math::Vec3` at the P6-M4-T02 importer
//!   hub. Same decision as the sibling P6-M2-T02 NURBS card.
//! - **All scalars `f64`** (P6-M1-T03: double everywhere, tolerance documented).
//! - **Explicit tolerance on every predicate, closed semantics `d <= tol`.**
//!   "On"/"in" includes the boundary. A negative `tol` is treated as `0`.
//! - **Non-panicking, total functions.** Booleans return `false` and distances
//!   return `NaN` on non-finite inputs; degenerate primitives degrade to
//!   documented values (zero-length segment/line -> point distance, zero/near
//!   zero normal -> `NaN` signed distance / `false` on-plane, degenerate
//!   triangle -> `false`). Nothing panics.
//!
//! Fuzz-vs-reference verification lives in
//! `crates/oep-geo/tests/predicates_contract.rs` (seeded random inputs compared
//! against an algebraically and numerically independent reference in the test)
//! and `tests/test_oep_geo_predicates_acceptance.py` (cross-language).

/// 3D point as three `f64` coordinates.
///
/// Chosen over `math::Vec3` because the P6-M2-T01 math branch is unmerged; this
/// module stands alone and the two are reconciled at the P6-M4-T02 importer hub.
pub type Point = [f64; 3];

/// Length threshold below which a direction/segment is treated as degenerate
/// (zero-length) so predicates degrade instead of dividing by zero.
const EPS_SQ: f64 = 1e-24; // (1e-12)^2

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: Point, s: f64) -> Point {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm2(a: Point) -> f64 {
    dot(a, a)
}
fn norm(a: Point) -> f64 {
    norm2(a).sqrt()
}

/// Treat negative tolerance as zero (closed semantics are undefined below 0).
fn sanitize_tol(tol: f64) -> f64 {
    if tol.is_finite() {
        tol.max(0.0)
    } else {
        0.0
    }
}

/// Euclidean distance between two points.
pub fn distance_point_point(a: Point, b: Point) -> f64 {
    norm(sub(a, b))
}

/// Distance from a point to the infinite line through `a` and `b`.
///
/// Formula: `|cross(b-a, p-a)| / |b-a|`. A zero-length line (`a == b`) is
/// treated as the single point `a`.
pub fn distance_point_line(p: Point, a: Point, b: Point) -> f64 {
    let d = sub(b, a);
    let len2 = norm2(d);
    if len2 <= EPS_SQ {
        return distance_point_point(p, a);
    }
    norm(cross(d, sub(p, a))) / len2.sqrt()
}

/// Distance from a point to the segment `[a, b]` (clamped projection).
///
/// The projection parameter is clamped to `[0, 1]`, so a point whose foot lies
/// past an endpoint measures to that endpoint. A zero-length segment
/// (`a == b`) is treated as the single point `a`.
pub fn distance_point_segment(p: Point, a: Point, b: Point) -> f64 {
    let d = sub(b, a);
    let len2 = norm2(d);
    if len2 <= EPS_SQ {
        return distance_point_point(p, a);
    }
    let t = dot(sub(p, a), d) / len2;
    let t = t.clamp(0.0, 1.0);
    let closest = add(a, scale(d, t));
    distance_point_point(p, closest)
}

/// Signed distance from a point to the plane through `plane_point` with
/// `plane_normal`. Positive when `p` is on the side the normal points toward.
///
/// Formula: `dot(p - plane_point, n) / |n|`. A zero/near-zero normal
/// (`|n| <= 1e-12`) yields `NaN` (the plane's orientation is undefined).
pub fn signed_distance_point_plane(p: Point, plane_point: Point, plane_normal: Point) -> f64 {
    let nn = norm(plane_normal);
    if !nn.is_finite() {
        return f64::NAN;
    }
    if nn <= 1e-12 {
        return f64::NAN;
    }
    dot(sub(p, plane_point), plane_normal) / nn
}

/// Unsigned distance from a point to a plane. See
/// [`signed_distance_point_plane`]; a degenerate normal yields `NaN`.
pub fn distance_point_plane(p: Point, plane_point: Point, plane_normal: Point) -> f64 {
    signed_distance_point_plane(p, plane_point, plane_normal).abs()
}

/// Whether a point is on a plane within `tol` (`|signed distance| <= tol`).
pub fn on_plane(p: Point, plane_point: Point, plane_normal: Point, tol: f64) -> bool {
    let t = sanitize_tol(tol);
    signed_distance_point_plane(p, plane_point, plane_normal).abs() <= t
}

/// Whether a point is on the infinite line through `a`, `b` within `tol`.
pub fn on_line(p: Point, a: Point, b: Point, tol: f64) -> bool {
    distance_point_line(p, a, b) <= sanitize_tol(tol)
}

/// Whether a point is on the segment `[a, b]` within `tol`.
pub fn on_segment(p: Point, a: Point, b: Point, tol: f64) -> bool {
    distance_point_segment(p, a, b) <= sanitize_tol(tol)
}

/// Whether a point is on the sphere (shell) centered `center`, radius `radius`,
/// within `tol` (`|distance - radius| <= tol`).
pub fn on_sphere(p: Point, center: Point, radius: f64, tol: f64) -> bool {
    let t = sanitize_tol(tol);
    let d = distance_point_point(p, center);
    d.is_finite() && (d - radius).abs() <= t
}

/// Whether a point is contained in (or on the boundary of) the axis-aligned box
/// `[min, max]`, inflated outward by `tol`.
pub fn in_aabb(p: Point, min: Point, max: Point, tol: f64) -> bool {
    let t = sanitize_tol(tol);
    (0..3).all(|i| p[i].is_finite() && p[i] >= min[i] - t && p[i] <= max[i] + t)
}

/// Whether a point is contained in (or on the boundary of) the sphere centered
/// `center`, radius `radius` (`distance <= radius + tol`).
pub fn in_sphere(p: Point, center: Point, radius: f64, tol: f64) -> bool {
    let t = sanitize_tol(tol);
    let d = distance_point_point(p, center);
    d.is_finite() && d <= radius + t
}

/// Whether a point is contained in the planar triangle `(a, b, c)`, within `tol`.
///
/// Two gates:
/// 1. **Coplanarity** — the point must lie within `tol` of the triangle's plane
///    (a point floating above the triangle is not "in" it).
/// 2. **Half-plane** — for each edge, the point must be on the inside side of
///    that edge (or within `tol` of the edge), using consistently oriented edge
///    cross products against the triangle normal.
///
/// A degenerate (collinear / zero-area) triangle yields `false`.
pub fn in_triangle_plane(p: Point, a: Point, b: Point, c: Point, tol: f64) -> bool {
    let t = sanitize_tol(tol);
    let ab = sub(b, a);
    let ac = sub(c, a);
    let n = cross(ab, ac);
    let nn = norm(n);
    if !nn.is_finite() || nn <= 1e-12 {
        return false; // degenerate triangle
    }
    // coplanarity gate
    let ap = sub(p, a);
    if dot(ap, n).abs() / nn > t {
        return false;
    }
    // half-plane tests (boundary included within tol)
    let inside_edge = |e1: Point, e2: Point| {
        let e = sub(e2, e1);
        let v = sub(p, e1);
        dot(cross(e, v), n) >= -t
    };
    inside_edge(a, b) && inside_edge(b, c) && inside_edge(c, a)
}
