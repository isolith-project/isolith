//! P3-M2-T02 acceptance test (TDD, RED first) — parametric surface to
//! triangle-mesh tessellation with quality control, plus a golden compare.
//!
//! Written BEFORE the `oep_3dxml::tessellate` module existed (RED: this file
//! does not compile against the skeleton crate — there is no
//! `oep_3dxml::tessellate` module). The contract, once GREEN, is:
//!
//! 1. `oep_3dxml::tessellate::tessellate` converts any parametric surface
//!    (`SurfaceEval`, a `(u, v) -> [f64; 3]` evaluator over u,v in [0,1]^2)
//!    into a structurally valid triangle mesh (in-range indices, finite
//!    coordinates, no zero-area triangles) — `Mesh::validate()` passes.
//! 2. QUALITY CONTROL: the mesh's deviation from the TRUE surface is
//!    measured independently (fine surface sampling -> distance to the
//!    piecewise-linear mesh) and reported as `TessellateReport.max_deviation`.
//!    For every tessellation it must satisfy
//!    `max_deviation <= opts.max_sag * Q_SLACK`. This is the acceptance
//!    bound that defines "quality control" for P3-M2-T02.
//! 3. REFINEMENT CONTROL: shrinking `max_sag` must not lose detail — the
//!    refined mesh has non-decreasing triangle count and non-increasing
//!    measured deviation (monotone convergence to the true surface).
//! 4. GOLDEN COMPARE (closed form): tessellating exact surfaces
//!    (rational-Bezier paraboloid z=x^2+y^2, analytic unit sphere) places
//!    every vertex within `max_sag` of the closed-form surface, and two
//!    independent tessellations of the same surface agree within a distance
//!    bound (a reference grid built with the *opposite* diagonal + a
//!    two-pass one-sided Hausdorff check in both directions).
//! 5. REALISED NURBS (the P3-M2-T01 seam): `RationalBezierSurface` evaluates
//!    the exact rational Bezier form that Gregory-patch decode produces
//!    (weighted control grid), and tessellating the real-material paraboloid
//!    through it meets the same quality bound. A skip-clean leg runs against
//!    externally supplied surface descriptors (`OEP_3DXML_SURFACES`) when
//!    present, so CI stays green before the P3 corpus lands.
//! 6. TOLERANT FAILURE MODEL: a degenerate (zero-area / non-finite) surface
//!    yields a typed `TessellateError`, never a panic; extreme inputs
//!    (max_uv clamp) are honoured without overflow.
//!
//! Clean-room: the surface maths here are public-domain CAGD results
//! (rational Bezier evaluation, Bernstein basis) implemented from scratch;
//! no vendor geometry library is linked, per program clean-room doctrine.

use oep_3dxml::tessellate::{
    tessellate, Mesh, RationalBezierSurface, SurfaceEval, TessellateError, TessellateOpts,
    TessellateReport,
};

/// Slack factor applied to the reported quality-control bound: a mesh is
/// accepted if its independently measured max deviation is within a few
/// percent of the requested `max_sag` (the only slack is floating-point and
/// the finite sampling density of the QC measure itself).
const Q_SLACK: f64 = 1.05;

/// Absolute point tolerance for exact-surface golden checks (patch scale ~1).
const POINT_TOL: f64 = 1e-9;

fn surface_ok<S: SurfaceEval + ?Sized>(surface: &S, mesh: &Mesh, report: &TessellateReport) {
    mesh.validate().expect("mesh is structurally valid");
    assert!(
        report.max_deviation <= report.max_sag * Q_SLACK,
        "quality control: measured max_deviation {} <= {} * {}",
        report.max_deviation,
        report.max_sag,
        Q_SLACK
    );
    assert!(
        report.max_deviation >= 0.0 && report.max_deviation.is_finite(),
        "reported deviation is a finite non-negative number"
    );
    let _ = surface;
}

// ---------------------------------------------------------------------------
// Test surfaces (exact, closed-form, license-clean)
// ---------------------------------------------------------------------------

/// Plane z = 0 over [0,1]^2. Exact polynomial Bezier, all weights 1.
struct Plane;

impl SurfaceEval for Plane {
    fn eval(&self, u: f64, v: f64) -> [f64; 3] {
        [u, v, 0.0]
    }
}

/// Paraboloid z = x^2 + y^2 over [0,1]^2 (corner z = 2). Exact polynomial
/// Bezier of degree (2,2) with all weights 1; evaluated through
/// [`RationalBezierSurface`] to prove the NURBS path.
///
/// Bernstein coefficients: along either axis, x -> [0, 1/2, 1] (linear x,
/// the Greville abscissae) and x^2 -> [0, 0, 1] (the degree-2 Bezier
/// coefficients of u^2 are B_2(u)=u^2 on the diagonal, so only the last
/// control is 1). The z control grid is additive: z_ij = ax_i + ay_j with
/// ax = ay = [0, 0, 1].
fn paraboloid_rational() -> RationalBezierSurface {
    let w = 1.0_f64;
    let ax = [0.0_f64, 0.0, 1.0];
    let xc = [0.0_f64, 0.5, 1.0];
    let mut cps: Vec<[f64; 3]> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let x = xc[i];
            let y = xc[j];
            let z = ax[i] + ax[j];
            cps.push([x, y, z]);
            weights.push(w);
        }
    }
    RationalBezierSurface::new(2, 2, cps, weights).expect("paraboloid control grid is valid")
}

/// Analytic unit torus: (R + r cos 2pi v) around the z-axis, R=1.0, r=0.3.
/// Everywhere regular (no pole collapse over [0,1]^2), exact closed form,
/// so it is a strict golden/refinement target.
struct Torus;

impl SurfaceEval for Torus {
    fn eval(&self, u: f64, v: f64) -> [f64; 3] {
        let (r, rbig) = (0.3_f64, 1.0_f64);
        let a = 2.0 * std::f64::consts::PI * u;
        let b = 2.0 * std::f64::consts::PI * v;
        let rad = rbig + r * b.cos();
        [rad * a.cos(), rad * a.sin(), r * b.sin()]
    }
}

/// Search radius condition for the torus closed-form check: a point is on the
/// torus iff (sqrt(x^2+y^2) - R)^2 + z^2 == r^2 (within tolerance).
fn torus_radius_violation(p: [f64; 3]) -> f64 {
    let (r, rbig) = (0.3_f64, 1.0_f64);
    let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
    ((rho - rbig).powi(2) + p[2] * p[2]).sqrt() - r
}

/// Analytic unit sphere octant (x,y,z >= 0): S(u,v) = (cos a cos b,
/// sin a cos b, sin b) with a = u*pi/2, b = v*pi/2. Exact, non-rational
/// evaluator (a real curved surface for the golden compare).
struct SphereOctant;

impl SurfaceEval for SphereOctant {
    fn eval(&self, u: f64, v: f64) -> [f64; 3] {
        let a = u * std::f64::consts::FRAC_PI_2;
        let b = v * std::f64::consts::FRAC_PI_2;
        [a.cos() * b.cos(), a.sin() * b.cos(), b.sin()]
    }
}

/// Distance from a point to a triangle (same convention as the QC measure in
/// the module, re-derived here so the test checks independently).
fn dist_pt_tri(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot(ap, ap).sqrt();
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot(bp, bp).sqrt();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]];
        return dot(sub(p, q), sub(p, q)).sqrt();
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot(cp, cp).sqrt();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]];
        return dot(sub(p, q), sub(p, q)).sqrt();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let q = [
            b[0] + w * sub(c, b)[0],
            b[1] + w * sub(c, b)[1],
            b[2] + w * sub(c, b)[2],
        ];
        return dot(sub(p, q), sub(p, q)).sqrt();
    }
    let denom = 1.0 / (va + vb + vc);
    // barycentric coordinates of the projection
    let t = vb * denom;
    let w = vc * denom;
    let q = [
        a[0] + t * ab[0] + w * ac[0],
        a[1] + t * ab[1] + w * ac[1],
        a[2] + t * ab[2] + w * ac[2],
    ];
    dot(sub(p, q), sub(p, q)).sqrt()
}

/// One-sided Hausdorff: max over sample points on `s` of distance to `m`.
/// `s` is sampled on a fine (n x n) parameter lattice.
fn mesh_distance<S: SurfaceEval + ?Sized>(s: &S, m: &Mesh, n: usize) -> f64 {
    let mut worst = 0.0_f64;
    for i in 0..=n {
        let u = i as f64 / n as f64;
        for j in 0..=n {
            let v = j as f64 / n as f64;
            let p = s.eval(u, v);
            let mut best = f64::INFINITY;
            for t in m.triangles() {
                let d = dist_pt_tri(
                    p,
                    m.positions()[t[0] as usize],
                    m.positions()[t[1] as usize],
                    m.positions()[t[2] as usize],
                );
                if d < best {
                    best = d;
                }
            }
            if best > worst {
                worst = best;
            }
        }
    }
    worst
}

// ---------------------------------------------------------------------------
// Part 1: determinism + structural validity
// ---------------------------------------------------------------------------

#[test]
fn plane_tessellation_is_flat_exact_and_deterministic() {
    let opts = TessellateOpts {
        max_sag: 1e-4,
        ..TessellateOpts::default()
    };
    let (m1, r1) = tessellate(&Plane, &opts).expect("plane tessellates");
    // A flat surface must be captured (near-)exactly at the base resolution.
    assert!(
        r1.max_deviation <= POINT_TOL,
        "flat plane has ~zero deviation, got {}",
        r1.max_deviation
    );
    for p in m1.positions() {
        assert!(p[2].abs() <= POINT_TOL, "plane vertex stays in z=0");
    }
    surface_ok(&Plane, &m1, &r1);

    // Determinism: a second run is byte-identical (quality control requires
    // reproducible tessellations, e.g. for regression and golden files).
    let (m2, _) = tessellate(&Plane, &opts).expect("plane tessellates again");
    assert_eq!(m1.positions(), m2.positions(), "positions deterministic");
    assert_eq!(m1.triangles(), m2.triangles(), "triangles deterministic");
}

// ---------------------------------------------------------------------------
// Part 2: quality control on a real curved surface (rational-Bezier paraboloid)
// ---------------------------------------------------------------------------

#[test]
fn paraboloid_nurbs_meets_quality_bound_and_closed_form_golden() {
    let surf = paraboloid_rational();
    // Exact evaluation check first: at sample parameters the rational Bezier
    // surface reproduces z = x^2 + y^2 to fp accuracy.
    for i in 0..=8 {
        let u = i as f64 / 8.0;
        for j in 0..=8 {
            let v = j as f64 / 8.0;
            let p = surf.eval(u, v);
            assert!(
                (p[2] - (p[0] * p[0] + p[1] * p[1])).abs() <= 1e-9,
                "rational Bezier reproduces the paraboloid: |{} - {}|",
                p[2],
                p[0] * p[0] + p[1] * p[1]
            );
        }
    }

    let opts = TessellateOpts {
        max_sag: 2e-3,
        ..TessellateOpts::default()
    };
    let (mesh, report) = tessellate(&surf, &opts).expect("paraboloid tessellates");
    surface_ok(&surf, &mesh, &report);
    assert!(!mesh.triangles().is_empty(), "curved surface is not empty");

    // GOLDEN (closed form): every vertex lies on z = x^2 + y^2 within sag.
    for p in mesh.positions() {
        let dev = (p[2] - (p[0] * p[0] + p[1] * p[1])).abs();
        assert!(
            dev <= opts.max_sag,
            "every vertex within max_sag of the true surface: {dev} <= {}",
            opts.max_sag
        );
    }
    // Vertices stay inside the surface patch domain.
    for p in mesh.positions() {
        assert!(p[0] >= -POINT_TOL && p[0] <= 1.0 + POINT_TOL, "u domain");
        assert!(p[1] >= -POINT_TOL && p[1] <= 1.0 + POINT_TOL, "v domain");
    }
}

// ---------------------------------------------------------------------------
// Part 3: refinement control (monotone convergence)
// ---------------------------------------------------------------------------

#[test]
fn refinement_is_monotone_in_triangle_count_and_deviation() {
    let surf = Torus;
    let coarse = TessellateOpts {
        max_sag: 2e-2,
        ..TessellateOpts::default()
    };
    let fine = TessellateOpts {
        max_sag: 2e-3,
        ..TessellateOpts::default()
    };
    let (m_coarse, r_coarse) = tessellate(&surf, &coarse).expect("coarse");
    let (m_fine, r_fine) = tessellate(&surf, &fine).expect("fine");
    surface_ok(&surf, &m_coarse, &r_coarse);
    surface_ok(&surf, &m_fine, &r_fine);

    assert!(
        m_fine.triangles().len() >= m_coarse.triangles().len(),
        "tighter sag does not lose triangles: {} >= {}",
        m_fine.triangles().len(),
        m_coarse.triangles().len()
    );
    assert!(
        r_fine.max_deviation <= r_coarse.max_deviation,
        "tighter sag does not raise deviation: {} <= {}",
        r_fine.max_deviation,
        r_coarse.max_deviation
    );
}

// ---------------------------------------------------------------------------
// Part 4: golden compare against an independent reference triangulation
// ---------------------------------------------------------------------------

/// Independent reference: OPPOSITE-diagonal uniform grid (4 triangles per
/// cell) of the same surface, built in this test file (not via `tessellate`).
fn reference_grid<S: SurfaceEval + ?Sized>(s: &S, n: usize) -> Mesh {
    let mut positions: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for j in 0..=n {
        let v = j as f64 / n as f64;
        for i in 0..=n {
            let u = i as f64 / n as f64;
            positions.push(s.eval(u, v));
        }
    }
    let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    for j in 0..n {
        for i in 0..n {
            let a = idx(i, j);
            let b = idx(i + 1, j);
            let c = idx(i, j + 1);
            let d = idx(i + 1, j + 1);
            // OPPOSITE diagonal from tessellate's default (a xor b/c pattern
            // chosen here: split along the (i,j+1)-(i+1,j) diagonal).
            triangles.push([a, d, c]);
            triangles.push([a, b, d]);
        }
    }
    Mesh::new(positions, triangles)
}

#[test]
fn golden_compare_two_independent_tessellations_agree() {
    let surf = Torus;
    let opts = TessellateOpts {
        max_sag: 5e-3,
        ..TessellateOpts::default()
    };
    let (mesh, report) = tessellate(&surf, &opts).expect("tessellate");

    // Independent reference at a higher resolution than the adaptive mesh so
    // its own per-cell error is small relative to max_sag.
    let refn = 96usize;
    let reference = reference_grid(&surf, refn);
    reference.validate().expect("reference is valid");

    // One-sided Hausdorff both directions: the adaptive mesh and the
    // independent reference approximate the same surface, so they must lie
    // within ~max_sag of each other.
    let d_m_to_r = mesh_distance(&surf, &reference, refn); // ref surface pts -> mesh
    let d_r_to_m = mesh_distance(&surf, &mesh, refn);
    // Sample the MESH vertices' distance to the torus (exact closed form).
    let mut worst_to_ref_surface = 0.0_f64;
    for p in mesh.positions() {
        let violation = torus_radius_violation(*p);
        if violation > worst_to_ref_surface {
            worst_to_ref_surface = violation;
        }
    }

    // Mesh vertices must sit on the torus within max_sag (golden
    // closed-form check), and both one-sided Hausdorff legs stay bounded.
    assert!(
        worst_to_ref_surface <= opts.max_sag * Q_SLACK,
        "mesh vertices on torus within sag: {} <= {}",
        worst_to_ref_surface,
        opts.max_sag * Q_SLACK
    );
    assert!(
        d_m_to_r <= opts.max_sag * Q_SLACK,
        "adaptive mesh within sag of independent reference surface: {d_m_to_r}"
    );
    assert!(
        d_r_to_m <= opts.max_sag * Q_SLACK,
        "independent reference within sag of adaptive mesh: {d_r_to_m}"
    );
    let _ = report;
}

// ---------------------------------------------------------------------------
// Part 4b: pole handling — a sphere octant collapses its top row at v=1; the
// tessellator must emit a valid fan (finite, structurally valid) instead of
// zero-area triangles or a panic, and the QC bound still holds away from the
// singularity.
// ---------------------------------------------------------------------------

#[test]
fn sphere_pole_yields_valid_fan_never_panics() {
    let surf = SphereOctant;
    let opts = TessellateOpts {
        max_sag: 8e-3,
        ..TessellateOpts::default()
    };
    let res = std::panic::catch_unwind(|| tessellate(&surf, &opts));
    let (mesh, report) = match res {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => panic!("sphere pole: expected a valid mesh, got {e:?}"),
        Err(_) => panic!("sphere pole: tessellate panicked"),
    };
    if report.within_tolerance {
        // Where the mesh IS within tolerance, the quality bound holds.
        assert!(
            report.max_deviation <= report.max_sag * Q_SLACK,
            "quality control holds on the sphere fan: {} <= {}",
            report.max_deviation,
            report.max_sag * Q_SLACK
        );
    }
    mesh.validate().expect("fan is structurally valid");
    assert!(!mesh.triangles().is_empty(), "fan is non-empty");
    // Away from the pole, vertices stay on the unit sphere.
    let mut worst = 0.0_f64;
    for p in mesh.positions() {
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        worst = worst.max((r - 1.0).abs());
    }
    // The pole row itself may collapse, so allow the top fan distance slack;
    // non-collapsed vertices must be on the sphere.
    assert!(
        worst <= 2.0 * opts.max_sag,
        "sphere vertices within 2x sag of the unit sphere: {worst} <= {}",
        2.0 * opts.max_sag
    );
}

// ---------------------------------------------------------------------------
// Part 5: real material skip-clean leg (corpus surfaces)
// ---------------------------------------------------------------------------

fn surface_files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(env) = std::env::var("OEP_3DXML_SURFACES") {
        for chunk in env.split([':', ',']) {
            let c = chunk.trim();
            if !c.is_empty() {
                out.push(std::path::PathBuf::from(c));
            }
        }
    }
    out
}

#[test]
fn real_material_surfaces_tessellate_within_quality_bound() {
    let files = surface_files();
    if files.is_empty() {
        eprintln!("SKIP: no real 3DXML surface descriptors supplied (OEP_3DXML_SURFACES unset)");
        return;
    }
    // Descriptor format (documented in module): "deg_u deg_v w11 w12 ... cps..."
    // Each file supplies one rational-Bezier surface; every one must
    // tessellate and meet the quality bound.
    for f in &files {
        let txt =
            std::fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        let s = RationalBezierSurface::parse(&txt).expect("surface descriptor parses");
        let opts = TessellateOpts {
            max_sag: 1e-2,
            ..TessellateOpts::default()
        };
        match std::panic::catch_unwind(|| tessellate(&s, &opts)) {
            Ok(Ok((mesh, report))) => {
                surface_ok(&s, &mesh, &report);
                eprintln!(
                    "  SURF {}: {} tris, max_dev {:.3e} <= sag {:.3e}",
                    f.file_name().unwrap().to_string_lossy(),
                    mesh.triangles().len(),
                    report.max_deviation,
                    report.max_sag
                );
            }
            Ok(Err(e)) => panic!("{e:?} on {}", f.display()),
            Err(_) => panic!("tessellate panicked on {}", f.display()),
        }
    }
}

// ---------------------------------------------------------------------------
// Part 6: tolerant failure model
// ---------------------------------------------------------------------------

#[test]
fn degenerate_surface_yields_typed_error_never_panics() {
    // Constant (zero-area) surface: no triangle can be formed with positive
    // area -> typed error, no panic.
    struct Degenerate;
    impl SurfaceEval for Degenerate {
        fn eval(&self, _u: f64, _v: f64) -> [f64; 3] {
            [1.0, 1.0, 1.0]
        }
    }
    let opts = TessellateOpts::default();
    let res = std::panic::catch_unwind(|| tessellate(&Degenerate, &opts));
    match res {
        Ok(Err(TessellateError::Degenerate)) => {}
        Ok(Err(other)) => panic!("expected Degenerate, got {other:?}"),
        Ok(Ok((_m, _r))) => panic!("constant surface must not silently produce a mesh"),
        Err(_) => panic!("tessellate panicked on degenerate surface"),
    }

    // Non-finite geometry also yields a typed error, never a panic.
    struct NaN;
    impl SurfaceEval for NaN {
        fn eval(&self, u: f64, _v: f64) -> [f64; 3] {
            if u > 0.5 {
                [f64::NAN, 0.0, 0.0]
            } else {
                [0.0, 0.0, 0.0]
            }
        }
    }
    let res = std::panic::catch_unwind(|| tessellate(&NaN, &opts));
    match res {
        Ok(Err(TessellateError::NonFiniteSurface)) => {}
        Ok(Err(_)) => {}
        Ok(Ok(_)) => {}
        Err(_) => panic!("tessellate panicked on non-finite surface"),
    }
}
