//! P6-M2-T03 acceptance contract for the geometric predicates module (RED).
//!
//! Written BEFORE `oep_geo::predicates` exists; against the skeleton this does
//! not compile, proving TDD RED. Locks the public API + behaviors:
//!
//! 1. Closed-form golden expectations on exact analytic geometry (real material):
//!    points known on/off a plane, line, segment, sphere; known point-to-primitive
//!    distances; known containment inside/outside an AABB, sphere and a planar
//!    triangle (including coplanarity gating).
//! 2. Property tests over seeded random inputs (deterministic xorshift*, std-only,
//!    no proptest dep) comparing the module against an algebraically independent
//!    REFERENCE implementation in this file (brute-force numeric minimizer for
//!    line/segment distance, half-plane sign test for triangle containment,
//!    projection-distance for plane). "Fuzz vs reference" — the module and the
//!    reference share no code path, so a self-consistent-but-wrong
//!    implementation is caught.
//! 3. Consistency properties: `on_X(p, tol)` <=> `distance_X(p) <= tol`,
//!    `in_sphere` <=> point-to-center distance <= r+tol, `in_aabb` <=> distance
//!    to the box <= tol, sphere/triangle on-boundary semantics.
//! 4. Non-panicking error model: NaN/Inf inputs -> booleans false, distances NaN;
//!    degenerate (zero-length segment/line, zero normal, degenerate triangle)
//!    degrade to documented values instead of panicking.

use oep_geo::predicates::{
    distance_point_line, distance_point_plane, distance_point_point, distance_point_segment,
    in_aabb, in_sphere, in_triangle_plane, on_line, on_plane, on_segment, on_sphere,
    signed_distance_point_plane, Point,
};

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64, what: &str) {
    assert!(
        (a - b).abs() <= EPS,
        "{what}: expected {b}, got {a} (diff {})",
        (a - b).abs()
    );
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
    /// Uniform in [-half, half)^3.
    fn point(&mut self, half: f64) -> Point {
        [
            self.signed() * half,
            self.signed() * half,
            self.signed() * half,
        ]
    }
}

// ---------------------------------------------------------------------------
// Small vector helpers used by the REFERENCE implementations only (kept apart
// from the module's internals on purpose).
// ---------------------------------------------------------------------------
fn ref_sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn ref_dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn ref_norm(a: Point) -> f64 {
    ref_dot(a, a).sqrt()
}

/// Reference 1D numeric minimizer: minimize |p - (a + t*d)| over t in [lo, hi]
/// by golden-section search on the convex parabola. Genuinely independent of
/// the closed-form projection in the module (no dot-product ratio reused).
fn ref_minimize_on_line(p: Point, a: Point, b: Point, lo: f64, hi: f64) -> (f64, f64) {
    let d = ref_sub(b, a);
    let f = |t: f64| -> f64 {
        let q = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
        let w = ref_sub(p, q);
        ref_dot(w, w)
    };
    let gr = (5.0f64.sqrt() + 1.0) / 2.0; // golden ratio phi ~ 1.618
    let mut x0 = lo;
    let mut x3 = hi;
    let mut x1 = x3 - (x3 - x0) / gr;
    let mut x2 = x0 + (x3 - x0) / gr;
    let mut f1 = f(x1);
    let mut f2 = f(x2);
    for _ in 0..120 {
        if f1 < f2 {
            x3 = x2;
            x2 = x1;
            f2 = f1;
            x1 = x3 - (x3 - x0) / gr;
            f1 = f(x1);
        } else {
            x0 = x1;
            x1 = x2;
            f1 = f2;
            x2 = x0 + (x3 - x0) / gr;
            f2 = f(x2);
        }
    }
    ((x0 + x3) / 2.0, f((x0 + x3) / 2.0))
}

/// Reference point-to-segment distance: numeric minimization over the closed
/// interval [0,1], plus exact endpoint checks. Independent of the module's
/// clamped-projection formula.
fn ref_distance_point_segment(p: Point, a: Point, b: Point) -> f64 {
    // degenerate segment -> distance to the single point a
    if ref_norm(ref_sub(b, a)) < 1e-12 {
        return ref_norm(ref_sub(p, a));
    }
    let (_, f) = ref_minimize_on_line(p, a, b, 0.0, 1.0);
    f.sqrt()
}

/// Reference point-to-line distance: numeric minimization over a wide interval.
fn ref_distance_point_line(p: Point, a: Point, b: Point) -> f64 {
    if ref_norm(ref_sub(b, a)) < 1e-12 {
        return ref_norm(ref_sub(p, a));
    }
    let (_, f) = ref_minimize_on_line(p, a, b, -1e6, 1e6);
    f.sqrt()
}

/// Reference signed distance to a plane (point + normal), via explicit
/// normalization then dot (module uses the unnormalized quotient route).
fn ref_signed_distance_point_plane(p: Point, plane_point: Point, normal: Point) -> f64 {
    let n = ref_norm(normal);
    if n < 1e-12 {
        return f64::NAN;
    }
    let u = [normal[0] / n, normal[1] / n, normal[2] / n];
    ref_dot(ref_sub(p, plane_point), u)
}

/// Reference triangle containment: coplanarity gate + half-plane sign tests
/// against the geometric normal. Independent of the module's barycentric route.
fn ref_in_triangle_plane(p: Point, a: Point, b: Point, c: Point, tol: f64) -> bool {
    let ab = ref_sub(b, a);
    let ac = ref_sub(c, a);
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let nn = ref_norm(n);
    if nn < 1e-12 {
        return false; // degenerate triangle
    }
    // coplanarity gate
    let ap = ref_sub(p, a);
    if ref_dot(ap, n).abs() / nn > tol {
        return false;
    }
    // three half-plane tests using consistently oriented edge cross products
    let ok_edge = |e1: Point, e2: Point| -> bool {
        let e = ref_sub(e2, e1);
        let v = ref_sub(p, e1);
        let c = [
            e[1] * v[2] - e[2] * v[1],
            e[2] * v[0] - e[0] * v[2],
            e[0] * v[1] - e[1] * v[0],
        ];
        // dot with triangle normal must be >= -tol
        ref_dot(c, n) >= -tol
    };
    ok_edge(a, b) && ok_edge(b, c) && ok_edge(c, a)
}

// ---------------------------------------------------------------------------
// Golden closed-form cases (real material: exact analytic geometry).
// ---------------------------------------------------------------------------
mod golden {
    pub const PLANE_Z0_POINT: [f64; 3] = [0.0, 0.0, 0.0];
    pub const PLANE_Z0_NORMAL: [f64; 3] = [0.0, 0.0, 1.0];
    pub const ON_PLANE_PT: [f64; 3] = [1.0, 2.0, 1e-10]; // |d| = 1e-10
    pub const OFF_PLANE_PT: [f64; 3] = [1.0, 2.0, 0.5]; //  |d| = 0.5
    pub const SDF_POS: [f64; 3] = [3.0, 4.0, 5.0]; // +5
    pub const SDF_NEG: [f64; 3] = [0.0, 0.0, -2.0]; // -2
}

#[test]
fn golden_point_point_distance() {
    // (1,2,3) -> (4,6,3): 3-4-5 triangle in the xy plane => 5
    close(
        distance_point_point([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]),
        5.0,
        "point-point distance",
    );
}

#[test]
fn golden_plane_signed_and_unsigned_distance() {
    use golden::*;
    close(
        signed_distance_point_plane(SDF_POS, PLANE_Z0_POINT, PLANE_Z0_NORMAL),
        5.0,
        "signed dist above plane",
    );
    close(
        signed_distance_point_plane(SDF_NEG, PLANE_Z0_POINT, PLANE_Z0_NORMAL),
        -2.0,
        "signed dist below plane",
    );
    close(
        distance_point_plane([1.0, 2.0, 7.0], PLANE_Z0_POINT, PLANE_Z0_NORMAL),
        7.0,
        "unsigned dist to plane",
    );
    // non-axis-aligned plane x+y+z=1 (point (1,0,0), normal (1,1,1)): origin is 1/sqrt(3) away
    close(
        distance_point_plane([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        1.0 / 3.0f64.sqrt(),
        "dist to x+y+z=1",
    );
}

#[test]
fn golden_on_plane_tolerance_bucket() {
    use golden::*;
    // on_plane: |signed| <= tol
    assert!(on_plane(ON_PLANE_PT, PLANE_Z0_POINT, PLANE_Z0_NORMAL, 1e-9));
    assert!(!on_plane(
        ON_PLANE_PT,
        PLANE_Z0_POINT,
        PLANE_Z0_NORMAL,
        1e-11
    ));
    assert!(!on_plane(
        OFF_PLANE_PT,
        PLANE_Z0_POINT,
        PLANE_Z0_NORMAL,
        1e-9
    ));
    assert!(on_plane(OFF_PLANE_PT, PLANE_Z0_POINT, PLANE_Z0_NORMAL, 0.5));
}

#[test]
fn golden_line_distance_and_on() {
    // x-axis line through (0,0,0) and (4,0,0)
    let a = [0.0, 0.0, 0.0];
    let b = [4.0, 0.0, 0.0];
    close(
        distance_point_line([5.0, 12.0, 0.0], a, b),
        12.0,
        "line dist",
    );
    // diagonal line direction (1,1,0): point (2,3,4) -> distance = |z|=... use exact:
    // line through origin along (1,1,0); p=(1,5,0): distance = |cross(d,p)|/|d| =
    // |(0,0,-4)|/sqrt(2) = 4/sqrt(2)
    close(
        distance_point_line([1.0, 5.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 0.0]),
        4.0 / 2.0f64.sqrt(),
        "diagonal line dist",
    );
    // on_line: (2,0,0) is on the x-axis within tolerance
    assert!(on_line([2.0, 0.0, 0.0], a, b, 1e-9));
    // (2, 1e-8, 0): within tol 1e-6 but not tol 1e-12
    assert!(on_line([2.0, 1e-8, 0.0], a, b, 1e-6));
    assert!(!on_line([2.0, 1e-8, 0.0], a, b, 1e-12));
    assert!(!on_line([2.0, 3.0, 0.0], a, b, 1e-9));
}

#[test]
fn golden_segment_distance_and_on() {
    let a = [0.0, 0.0, 0.0];
    let b = [4.0, 0.0, 0.0];
    // perpendicular projection falls inside the segment
    close(
        distance_point_segment([2.0, 3.0, 0.0], a, b),
        3.0,
        "segment dist perpendicular",
    );
    // projection beyond A -> clamped to A
    close(
        distance_point_segment([-1.0, 0.0, 0.0], a, b),
        1.0,
        "segment dist clamp to A",
    );
    // projection beyond B -> clamped to B
    close(
        distance_point_segment([6.0, 0.0, 0.0], a, b),
        2.0,
        "segment dist clamp to B",
    );
    // on the segment => 0
    close(
        distance_point_segment([2.0, 0.0, 0.0], a, b),
        0.0,
        "segment dist on",
    );
    assert!(on_segment([3.5, 0.0, 0.0], a, b, 1e-9));
    assert!(!on_segment([4.5, 0.0, 0.0], a, b, 1e-9));
}

#[test]
fn golden_sphere_distance_containment() {
    let c = [0.0, 0.0, 0.0];
    let r = 1.0;
    // on_sphere: |dist - r| <= tol
    assert!(on_sphere([1.0, 0.0, 0.0], c, r, 1e-9));
    // (sqrt(2)/2, sqrt(2)/2, 0) is exactly on the unit circle
    let h = 2.0f64.sqrt() / 2.0;
    assert!(on_sphere([h, h, 0.0], c, r, 1e-9));
    assert!(!on_sphere([2.0, 0.0, 0.0], c, r, 1e-9));
    // in_sphere: dist <= r + tol
    assert!(in_sphere([0.5, 0.5, 0.5], c, r, 1e-9)); // dist ~0.866 <= 1
    assert!(in_sphere([1.0, 0.0, 0.0], c, r, 0.0)); // boundary closed
    assert!(!in_sphere([1.0 + 1e-6, 0.0, 0.0], c, r, 1e-9));
    assert!(in_sphere([1.0 + 1e-6, 0.0, 0.0], c, r, 1e-5));
}

#[test]
fn golden_aabb_containment() {
    let lo = [0.0, 0.0, 0.0];
    let hi = [1.0, 1.0, 1.0];
    assert!(in_aabb([0.5, 0.5, 0.5], lo, hi, 1e-9));
    assert!(in_aabb([1.0, 1.0, 1.0], lo, hi, 0.0)); // closed at hi corner
    assert!(in_aabb([0.0, 0.0, 0.0], lo, hi, 0.0)); // closed at lo corner
    assert!(!in_aabb([1.5, 0.5, 0.5], lo, hi, 1e-9));
    assert!(!in_aabb([-0.5, 0.5, 0.5], lo, hi, 1e-9));
    // tolerance extends the box outward
    assert!(in_aabb([1.0 + 1e-6, 0.5, 0.5], lo, hi, 1e-5));
    assert!(!in_aabb([1.0 + 1e-6, 0.5, 0.5], lo, hi, 1e-9));
}

#[test]
fn golden_triangle_containment_with_coplanarity_gate() {
    let a = [0.0, 0.0, 0.0];
    let b = [1.0, 0.0, 0.0];
    let c = [0.0, 1.0, 0.0];
    // centroid (1/3,1/3,0) is inside
    assert!(in_triangle_plane(
        [1.0 / 3.0, 1.0 / 3.0, 0.0],
        a,
        b,
        c,
        1e-9
    ));
    // on the hypotenuse edge (x+y=1) -> boundary closed
    assert!(in_triangle_plane([0.5, 0.5, 0.0], a, b, c, 1e-9));
    // on a vertex / edge midpoint
    assert!(in_triangle_plane([0.0, 0.5, 0.0], a, b, c, 1e-9));
    // outside the triangle on the same plane
    assert!(!in_triangle_plane([2.0, 2.0, 0.0], a, b, c, 1e-9));
    assert!(!in_triangle_plane([-0.1, 0.0, 0.0], a, b, c, 1e-9));
    // coplanarity gate: points lifted off the plane are NOT inside, even if
    // their xy projection is the centroid
    assert!(!in_triangle_plane(
        [1.0 / 3.0, 1.0 / 3.0, 1.0],
        a,
        b,
        c,
        1e-9
    ));
}

// ---------------------------------------------------------------------------
// Property tests: fuzz vs reference + consistency properties.
// ---------------------------------------------------------------------------

#[test]
fn prop_segment_distance_vs_numeric_reference() {
    let mut rng = XorShift::new(0x5E67_0001);
    for _ in 0..1500 {
        let a = rng.point(10.0);
        let b = rng.point(10.0);
        let p = rng.point(10.0);
        let got = distance_point_segment(p, a, b);
        let want = ref_distance_point_segment(p, a, b);
        assert!(
            (got - want).abs() <= 1e-6,
            "segment dist mismatch: got {got}, ref {want}, p={p:?} a={a:?} b={b:?}"
        );
    }
}

#[test]
fn prop_line_distance_vs_numeric_reference() {
    let mut rng = XorShift::new(0x1A1E_0002);
    for _ in 0..1500 {
        let a = rng.point(10.0);
        let b = rng.point(10.0);
        let p = rng.point(100.0);
        let got = distance_point_line(p, a, b);
        let want = ref_distance_point_line(p, a, b);
        assert!(
            (got - want).abs() <= 1e-5,
            "line dist mismatch: got {got}, ref {want}, p={p:?} a={a:?} b={b:?}"
        );
    }
}

#[test]
fn prop_plane_signed_vs_reference_and_unsigned_consistency() {
    let mut rng = XorShift::new(0x9A1E_0003);
    for _ in 0..1500 {
        let q = rng.point(10.0);
        let n = rng.point(3.0);
        if ref_norm(n) < 1e-3 {
            continue;
        }
        let p = rng.point(100.0);
        let got = signed_distance_point_plane(p, q, n);
        let want = ref_signed_distance_point_plane(p, q, n);
        assert!(
            (got - want).abs() <= 1e-8,
            "signed plane dist mismatch: got {got}, ref {want}"
        );
        assert!(
            (got.abs() - distance_point_plane(p, q, n)).abs() <= 1e-8,
            "|signed| == unsigned"
        );
        // on_plane consistency: true iff |signed| <= tol
        let tol = rng.unit() * 2.0;
        assert_eq!(
            on_plane(p, q, n, tol),
            got.abs() <= tol,
            "on_plane <=> |d|<=tol"
        );
    }
}

#[test]
fn prop_on_off_line_segment_consistency() {
    let mut rng = XorShift::new(0xBE_EF0004);
    for _ in 0..1500 {
        let a = rng.point(5.0);
        let b = rng.point(5.0);
        let p = rng.point(20.0);
        let tol = rng.unit() * 2.0;
        // on_line iff distance_point_line <= tol
        assert_eq!(
            on_line(p, a, b, tol),
            distance_point_line(p, a, b) <= tol,
            "on_line <=> dist<=tol"
        );
        // on_segment iff distance_point_segment <= tol
        assert_eq!(
            on_segment(p, a, b, tol),
            distance_point_segment(p, a, b) <= tol,
            "on_segment <=> dist<=tol"
        );
        // a point exactly on the line (constructed) is "on" for a generous tol
        if ref_norm(ref_sub(b, a)) > 1e-3 {
            let d = ref_sub(b, a);
            let t = rng.signed() * 10.0;
            let q = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
            assert!(on_line(q, a, b, 1e-6), "constructed on-line point is on");
        }
    }
}

#[test]
fn prop_sphere_containment_and_boundary() {
    let mut rng = XorShift::new(0x5F3E_0005);
    for _ in 0..1500 {
        let c = rng.point(10.0);
        let r = rng.unit() * 5.0 + 0.1;
        let p = rng.point(20.0);
        let tol = rng.unit() * 2.0;
        let d = distance_point_point(p, c);
        assert_eq!(
            in_sphere(p, c, r, tol),
            d <= r + tol,
            "in_sphere <=> dist<=r+tol"
        );
        assert_eq!(
            on_sphere(p, c, r, tol),
            (d - r).abs() <= tol,
            "on_sphere <=> |d-r|<=tol"
        );
        // on_sphere implies in_sphere
        if on_sphere(p, c, r, tol) {
            assert!(in_sphere(p, c, r, tol), "on_sphere => in_sphere");
        }
    }
}

#[test]
fn prop_aabb_containment_reference() {
    let mut rng = XorShift::new(0xAABB_0006);
    for _ in 0..1500 {
        let lo = rng.point(10.0);
        let hi = [
            lo[0] + rng.unit() * 5.0,
            lo[1] + rng.unit() * 5.0,
            lo[2] + rng.unit() * 5.0,
        ];
        let p = rng.point(15.0);
        let tol = rng.unit() * 2.0;
        // reference: each coordinate within [lo_i - tol, hi_i + tol]
        let want = (0..3).all(|i| p[i] >= lo[i] - tol && p[i] <= hi[i] + tol);
        assert_eq!(in_aabb(p, lo, hi, tol), want, "in_aabb reference check");
    }
}

#[test]
fn prop_triangle_containment_vs_halfplane_reference() {
    let mut rng = XorShift::new(0xA811_0007);
    for _ in 0..1500 {
        let a = rng.point(10.0);
        let b = rng.point(10.0);
        let c = rng.point(10.0);
        let ap = ref_sub(b, a);
        let ac = ref_sub(c, a);
        let n = [
            ap[1] * ac[2] - ap[2] * ac[1],
            ap[2] * ac[0] - ap[0] * ac[2],
            ap[0] * ac[1] - ap[1] * ac[0],
        ];
        if ref_norm(n) < 1e-3 {
            continue; // near-degenerate triangle: skip, covered by unit tests
        }
        let p = rng.point(15.0);
        let tol = rng.unit() * 1.0;
        let want = ref_in_triangle_plane(p, a, b, c, tol);
        let got = in_triangle_plane(p, a, b, c, tol);
        assert_eq!(
            got, want,
            "triangle containment mismatch: got {got}, want {want}, p={p:?}"
        );
    }
}

#[test]
fn prop_triangle_constructed_inside_outside() {
    // Sample barycentric weights; a convex-combination point (weights all >= 0)
    // must be inside; a point with one weight clearly negative (e.g. -2) must
    // be outside. Same plane by construction.
    let mut rng = XorShift::new(0xC0FF_0008);
    let a = [0.0, 0.0, 0.0];
    let b = [2.0, 0.0, 0.0];
    let c = [0.0, 2.0, 0.0];
    for _ in 0..1500 {
        let u = rng.unit();
        let v = (1.0 - u) * rng.unit();
        let w = 1.0 - u - v;
        let p = [
            u * a[0] + v * b[0] + w * c[0],
            u * a[1] + v * b[1] + w * c[1],
            0.0,
        ];
        assert!(
            in_triangle_plane(p, a, b, c, 1e-9),
            "convex combo is inside"
        );
        // negative weight => outside: (-1,0,0) is clearly outside edge x=0
        let q = [-1.0, 0.0, 0.0];
        assert!(
            !in_triangle_plane(q, a, b, c, 1e-9),
            "negative-weight point is outside"
        );
    }
}

#[test]
fn prop_point_point_symmetry() {
    let mut rng = XorShift::new(0x5EED_0009);
    for _ in 0..1500 {
        let a = rng.point(100.0);
        let b = rng.point(100.0);
        let d1 = distance_point_point(a, b);
        let d2 = distance_point_point(b, a);
        close(d1, d2, "point-point symmetric");
    }
}

#[test]
fn degenerate_and_non_finite_never_panic() {
    let nan: f64 = f64::NAN;
    let inf: f64 = f64::INFINITY;
    let zero = [0.0, 0.0, 0.0];
    let a = [1.0, 0.0, 0.0];
    let b = [1.0, 0.0, 0.0]; // zero-length segment/line

    // non-finite inputs: booleans false, distances NaN -- never a panic
    assert!(!on_plane([nan, 0.0, 0.0], zero, [0.0, 0.0, 1.0], 1.0));
    assert!(!on_line([inf, 0.0, 0.0], zero, a, 1.0));
    assert!(!in_sphere([nan, 0.0, 0.0], zero, 1.0, 1.0));
    assert!(!in_aabb([inf, 0.0, 0.0], zero, [1.0, 1.0, 1.0], 1.0));
    assert!(!in_triangle_plane(
        [nan, 0.0, 0.0],
        zero,
        a,
        [0.0, 1.0, 0.0],
        1.0
    ));
    assert!(distance_point_point([nan, 0.0, 0.0], zero).is_nan());
    assert!(signed_distance_point_plane([0.0, 0.0, 1.0], zero, [nan, 0.0, 0.0]).is_nan());
    assert!(distance_point_line([0.0, 0.0, 1.0], zero, [inf, 0.0, 0.0]).is_nan());

    // degenerate: zero-length segment/line degrades to point distance
    close(
        distance_point_segment([3.0, 4.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        4.47213595499958,
        "zero-length segment -> point dist",
    );
    close(
        distance_point_line([1.0, 0.0, 0.0], a, b),
        0.0,
        "zero-length line -> point dist",
    );
    // zero normal plane => no meaningful signed distance, on_plane false
    assert!(signed_distance_point_plane([0.0, 0.0, 1.0], zero, zero).is_nan());
    assert!(!on_plane([0.0, 0.0, 1.0], zero, zero, 1.0));
    // degenerate (collinear) triangle => not inside
    assert!(!in_triangle_plane(
        [0.5, 0.0, 0.0],
        zero,
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        1.0
    ));
    // negative tolerance is meaningless: treat as zero tolerance
    assert!(in_sphere([0.5, 0.0, 0.0], zero, 1.0, -0.1));
    assert!(!in_sphere([2.0, 0.0, 0.0], zero, 1.0, -0.1));
}
