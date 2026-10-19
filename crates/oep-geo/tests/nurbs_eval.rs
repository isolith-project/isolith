//! P6-M2-T02 acceptance tests: NURBS/B-spline curve + surface evaluation.
//!
//! Written FIRST (TDD): before the implementation existed, the API referenced
//! here (`oep_geo::nurbs::...`) did not compile (RED). Now GREEN on real
//! material committed at `tests/golden/nurbs-circle.oepgeo.txt` and
//! `tests/golden/nurbs-cylinder.oepgeo.txt` (our own documented golden format).
//!
//! The goldens were generated from the ANALYTIC constructions (a classic
//! rational B-spline unit circle and a ruled rational cylinder), NOT from the
//! Rust implementation under test, and are independently re-verified against
//! cos/sin by `tests/test_oep_geo_nurbs_acceptance.py` (stdlib Python, its
//! own Cox-de Boor). This file verifies the Rust implementation against the
//! same goldens AND against analytic properties (radius == 1, tangent
//! orthogonal to radius, partition of unity, Bezier reduction, derivative
//! finite-difference consistency).

use oep_geo::nurbs::{BsplineCurve, NurbsCurve, NurbsSurface};

use std::fs;
use std::path::PathBuf;

/// Absolute tolerance for analytic-geometry checks on unit-scale fixtures.
const TOL: f64 = 1e-9;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

// ---------------------------------------------------------------------------
// Tiny parser for the OEP-NURBS-GOLDEN v1 text format (whitespace tokens only;
// see the goldens' own header comment for the contract).
// ---------------------------------------------------------------------------
type CurveGolden = (
    usize,
    Vec<f64>,
    Vec<[f64; 3]>,
    Vec<f64>,
    Vec<(f64, [f64; 3])>,
);
type SurfaceGolden = (
    usize,
    usize,
    Vec<f64>,
    Vec<f64>,
    Vec<Vec<[f64; 3]>>,
    Vec<Vec<f64>>,
    Vec<(f64, f64, [f64; 3])>,
);

fn parse_curve_golden(path: &PathBuf) -> CurveGolden {
    let text = fs::read_to_string(path).expect("golden file readable");
    let mut degree = 0usize;
    let mut knots: Vec<f64> = Vec::new();
    let mut ctrl: Vec<[f64; 3]> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();
    let mut spots: Vec<(f64, [f64; 3])> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("OEP-NURBS") {
            continue;
        }
        let toks: Vec<&str> = t.split_whitespace().collect();
        match toks[0] {
            "degree" => degree = toks[1].parse().unwrap(),
            "kind" => {}
            "knots" => knots = toks[1..].iter().map(|s| s.parse().unwrap()).collect(),
            "ncontrol" => {}
            "ctrl" => {
                let it = toks[1..].iter().map(|s| s.parse::<f64>().unwrap());
                let v: Vec<f64> = it.collect();
                assert_eq!(v.len() % 3, 0);
                ctrl = v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            "weights" => weights = toks[1..].iter().map(|s| s.parse().unwrap()).collect(),
            "spot" => {
                let u: f64 = toks[1].parse().unwrap();
                let p: [f64; 3] = [
                    toks[2].parse().unwrap(),
                    toks[3].parse().unwrap(),
                    toks[4].parse().unwrap(),
                ];
                spots.push((u, p));
            }
            _ => panic!("unexpected golden token: {toks:?}"),
        }
    }
    (degree, knots, ctrl, weights, spots)
}

fn parse_surface_golden(path: &PathBuf) -> SurfaceGolden {
    let text = fs::read_to_string(path).expect("golden file readable");
    let mut du = 0usize;
    let mut dv = 0usize;
    let mut ku: Vec<f64> = Vec::new();
    let mut kv: Vec<f64> = Vec::new();
    let mut cps: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut ws: Vec<Vec<f64>> = Vec::new();
    let mut spots = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("OEP-NURBS") {
            continue;
        }
        let toks: Vec<&str> = t.split_whitespace().collect();
        match toks[0] {
            "degree_u" => du = toks[1].parse().unwrap(),
            "degree_v" => dv = toks[1].parse().unwrap(),
            "kind" => {}
            "knots_u" => ku = toks[1..].iter().map(|s| s.parse().unwrap()).collect(),
            "knots_v" => kv = toks[1..].iter().map(|s| s.parse().unwrap()).collect(),
            "nu" | "nv" => {}
            "ctrl_v0" => {
                let v: Vec<f64> = toks[1..].iter().map(|s| s.parse().unwrap()).collect();
                cps.push(v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect());
            }
            "ctrl_v1" => {
                let v: Vec<f64> = toks[1..].iter().map(|s| s.parse().unwrap()).collect();
                cps.push(v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect());
            }
            "weights_v0" => ws.push(toks[1..].iter().map(|s| s.parse().unwrap()).collect()),
            "weights_v1" => ws.push(toks[1..].iter().map(|s| s.parse().unwrap()).collect()),
            "spot" => {
                let u: f64 = toks[1].parse().unwrap();
                let v: f64 = toks[2].parse().unwrap();
                let p: [f64; 3] = [
                    toks[3].parse().unwrap(),
                    toks[4].parse().unwrap(),
                    toks[5].parse().unwrap(),
                ];
                spots.push((u, v, p));
            }
            _ => panic!("unexpected surface golden token: {toks:?}"),
        }
    }
    (du, dv, ku, kv, cps, ws, spots)
}

fn close(a: [f64; 3], b: [f64; 3], tol: f64, what: &str) {
    let d = (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]);
    assert!(
        d <= tol,
        "{what}: |got-want|={d:.3e} > {tol:.1e}\n  got  {a:?}\n  want {b:?}"
    );
}

// ---------------------------------------------------------------------------
// Curve: exact rational unit circle (the golden).
// ---------------------------------------------------------------------------

#[test]
fn circle_curve_matches_golden_spots() {
    let (degree, knots, ctrl, weights, spots) =
        parse_curve_golden(&golden_dir().join("nurbs-circle.oepgeo.txt"));
    let curve = NurbsCurve::new(degree, knots, ctrl, weights).expect("construct circle NURBS");
    for (u, want) in &spots {
        let got = curve.point(*u).expect("eval point");
        close(got, *want, TOL, &format!("circle spot u={u}"));
    }
}

#[test]
fn circle_curve_radius_is_exactly_one() {
    let (degree, knots, ctrl, weights, _) =
        parse_curve_golden(&golden_dir().join("nurbs-circle.oepgeo.txt"));
    let curve = NurbsCurve::new(degree, knots, ctrl, weights).expect("construct circle NURBS");
    for i in 0..=40 {
        let u = i as f64 / 40.0;
        let p = curve.point(u).expect("eval point");
        let r = p[0].hypot(p[1]);
        assert!((r - 1.0).abs() <= TOL, "radius at u={u}: {r:.3e} != 1");
    }
}

#[test]
fn circle_curve_tangent_orthogonal_to_radius() {
    let (degree, knots, ctrl, weights, _) =
        parse_curve_golden(&golden_dir().join("nurbs-circle.oepgeo.txt"));
    let curve = NurbsCurve::new(degree, knots, ctrl, weights).expect("construct circle NURBS");
    for i in 1..40 {
        let u = i as f64 / 40.0;
        let p = curve.point(u).expect("point");
        let d = curve.derivative(u, 1).expect("first derivative");
        let dot = p[0] * d[0] + p[1] * d[1] + p[2] * d[2];
        assert!(
            dot.abs() <= 1e-6,
            "tangent not orthogonal to radius at u={u}: dot={dot:.3e}"
        );
    }
}

#[test]
fn bspline_basis_sum_to_one() {
    // Partition of unity on the circle knot vector (strict interior params).
    let (degree, knots, ctrl, _weights, _) =
        parse_curve_golden(&golden_dir().join("nurbs-circle.oepgeo.txt"));
    let curve = BsplineCurve::new(degree, knots, ctrl).expect("construct B-spline curve");
    for i in 1..40 {
        let u = i as f64 / 40.0;
        let sum = curve.basis_sum(u).expect("basis sum");
        assert!(
            (sum - 1.0).abs() <= 1e-9,
            "partition of unity failed at u={u}: {sum}"
        );
    }
}

// ---------------------------------------------------------------------------
// Curve: degree-1 (linear) B-spline reproduces the control polygon.
// ---------------------------------------------------------------------------

#[test]
fn linear_bspline_reproduces_polygon() {
    let ctrl = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 1.0, 0.0],
    ];
    let knots = vec![0.0, 0.0, 0.33, 0.66, 1.0, 1.0];
    let curve = BsplineCurve::new(1, knots, ctrl.to_vec()).expect("linear curve");
    // Endpoints interpolate the first/last control point exactly.
    let p0 = curve.point(0.0).expect("p0");
    let p1 = curve.point(1.0).expect("p1");
    close(p0, [0.0, 0.0, 0.0], TOL, "linear start");
    close(p1, [2.0, 1.0, 0.0], TOL, "linear end");
    // Midpoint of the first segment: u=0.165 lies on [0.0,0.33], halfway between p0 and p1.
    let pmid = curve.point(0.165).expect("mid");
    close(pmid, [0.5, 0.0, 0.0], 1e-6, "linear first-mid");
}

// ---------------------------------------------------------------------------
// Curve: degree-3 Bezier (single clamped span) reduces to Bernstein form.
// ---------------------------------------------------------------------------

#[test]
fn cubic_bezier_matches_bernstein() {
    let ctrl = [
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [2.0, -1.0, 0.0],
        [3.0, 0.0, 0.0],
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let curve = BsplineCurve::new(3, knots, ctrl.to_vec()).expect("bezier");
    let bern = |t: f64| -> [f64; 3] {
        let b0 = (1.0 - t).powi(3);
        let b1 = 3.0 * t * (1.0 - t).powi(2);
        let b2 = 3.0 * t * t * (1.0 - t);
        let b3 = t.powi(3);
        [
            b0 * ctrl[0][0] + b1 * ctrl[1][0] + b2 * ctrl[2][0] + b3 * ctrl[3][0],
            b0 * ctrl[0][1] + b1 * ctrl[1][1] + b2 * ctrl[2][1] + b3 * ctrl[3][1],
            b0 * ctrl[0][2] + b1 * ctrl[1][2] + b2 * ctrl[2][2] + b3 * ctrl[3][2],
        ]
    };
    for i in 0..=20 {
        let t = i as f64 / 20.0;
        let got = curve.point(t).expect("bezier point");
        let want = bern(t);
        close(got, want, TOL, &format!("bezier t={t}"));
    }
}

// ---------------------------------------------------------------------------
// Curve: derivative finite-difference consistency on a non-rational curve.
// ---------------------------------------------------------------------------

#[test]
fn derivative_agrees_with_finite_difference() {
    let ctrl = [
        [0.0, 0.0, 0.0],
        [0.5, 1.5, 0.2],
        [1.2, 0.5, -0.3],
        [1.8, 2.0, 0.1],
        [2.5, 0.5, -0.2],
        [3.0, 0.0, 0.0],
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.2, 0.5, 0.8, 1.0, 1.0, 1.0];
    let curve = BsplineCurve::new(2, knots, ctrl.to_vec()).expect("curve");
    let h = 1e-6;
    for i in 3..17 {
        let u = i as f64 / 20.0;
        let d = curve.derivative(u, 1).expect("deriv");
        let fp = curve.point(u + h).expect("fp");
        let fm = curve.point(u - h).expect("fm");
        let fd = [
            (fp[0] - fm[0]) / (2.0 * h),
            (fp[1] - fm[1]) / (2.0 * h),
            (fp[2] - fm[2]) / (2.0 * h),
        ];
        let err = (d[0] - fd[0])
            .abs()
            .max((d[1] - fd[1]).abs())
            .max((d[2] - fd[2]).abs());
        assert!(err <= 1e-4, "derivative mismatch at u={u}: {err:.3e}");
    }
}

// ---------------------------------------------------------------------------
// Surface: exact ruled rational cylinder (the golden).
// ---------------------------------------------------------------------------

#[test]
fn cylinder_surface_matches_golden_spots() {
    let (du, dv, ku, kv, cps, ws, spots) =
        parse_surface_golden(&golden_dir().join("nurbs-cylinder.oepgeo.txt"));
    let surf =
        NurbsSurface::new(du, dv, ku, kv, cps, ws).expect("construct cylinder NURBS surface");
    for (u, v, want) in &spots {
        let got = surf.point(*u, *v).expect("surface point");
        close(got, *want, TOL, &format!("cylinder spot ({u},{v})"));
    }
}

#[test]
fn cylinder_surface_radius_is_exactly_one() {
    let (du, dv, ku, kv, cps, ws, _) =
        parse_surface_golden(&golden_dir().join("nurbs-cylinder.oepgeo.txt"));
    let surf =
        NurbsSurface::new(du, dv, ku, kv, cps, ws).expect("construct cylinder NURBS surface");
    for i in 0..=10 {
        let u = i as f64 / 10.0;
        for j in 0..=6 {
            let v = j as f64 / 6.0;
            let p = surf.point(u, v).expect("surface point");
            let r = p[0].hypot(p[1]);
            assert!(
                (r - 1.0).abs() <= TOL,
                "cylinder radius at ({u},{v}): {r:.3e} != 1"
            );
            // Ruled in z linearly: z == H*v where H = 2 (golden contract).
            assert!(
                (p[2] - 2.0 * v).abs() <= TOL,
                "cylinder z at ({u},{v}): {:.3e}",
                p[2]
            );
        }
    }
}
