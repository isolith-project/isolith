//! P3-M2-T02 — parametric surface tessellation with quality control and a
//! golden-compare basis.
//!
//! Converts any parametric surface `S(u,v) -> [f64;3]`, `u,v in [0,1]^2`,
//! into a crack-free triangle mesh whose deviation from the TRUE surface is
//! bounded by a caller-supplied tolerance (`max_sag`). Quality control is
//! measured independently of the meshing choice: after a candidate grid is
//! built, the true surface is sampled inside every cell and the distance from
//! each sample to the piecewise-linear mesh is computed; the grid is refined
//! (uniform doubling, capped at `max_uv`) until every sample is within
//! `max_sag`, then the worst measured distance is reported. The tessellator
//! never panics: degenerate (zero-area) and non-finite geometry return typed
//! [`TessellateError`] values.
//!
//! ## The P3-M2-T01 seam
//!
//! Gregory-patch decode (P3-M2-T01) produces single-span rational Bezier
//! NURBS surfaces (weighted control grids). This module's
//! [`RationalBezierSurface`] is exactly that representation, so a decoded
//! surface plugs in directly; [`RationalBezierSurface::parse`] also reads a
//! plain-text descriptor so external corpus surfaces (env
//! `OEP_3DXML_SURFACES`) can be exercised end to end.
//!
//! ## Mesh convention
//!
//! A uniform grid of `(N+1) x (N+1)` positions is meshed with two triangles
//! per cell, each cell split along one chosen diagonal. A plain
//! [`Mesh`] (positions + triangle indices) keeps this independent of any
//! cross-crate mesh type (docs/layout.md: no cross-crate path deps until the
//! P6 importer hub), while `positions()` / `triangles()` expose the same
//! plain slices the ASCII writer (P3-M4-T01) consumes.
//!
//! Clean-room: the only maths used are public-domain CAGD results
//! (Bernstein bases, rational Bezier evaluation, point-triangle distance)
//! implemented from scratch; no vendor geometry library is linked.

use std::fmt;

/// Bernstein basis polynomial `B_i^n(t) = C(n,i) t^i (1-t)^(n-i)`.
fn bern(n: usize, i: usize, t: f64) -> f64 {
    binom(n, i) * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

/// Binomial coefficient `C(n, k)` as `f64`.
fn binom(n: usize, k: usize) -> f64 {
    let k = k.min(n - k);
    let mut c: f64 = 1.0;
    for i in 1..=k {
        c = c * (n - k + i) as f64 / i as f64;
    }
    c
}

/// Distance from a point to a triangle (3-D), used by the quality-control
/// measure. Standard closest-point-on-triangle algorithm.
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
    let t = vb * denom;
    let w = vc * denom;
    let q = [
        a[0] + t * ab[0] + w * ac[0],
        a[1] + t * ab[1] + w * ac[1],
        a[2] + t * ab[2] + w * ac[2],
    ];
    dot(sub(p, q), sub(p, q)).sqrt()
}

/// A parametric surface evaluator over the unit parameter square.
///
/// Implementors map `(u, v)` in `[0,1] x [0,1]` to a 3-D point. Exact
/// analytic surfaces (planes, spheres, rational Bezier patches) implement
/// this trait directly; the P3-M2-T01 NURBS surfaces are representable via
/// [`RationalBezierSurface`].
pub trait SurfaceEval {
    /// Evaluate the surface at parameter pair `(u, v)` in `[0,1]^2`.
    fn eval(&self, u: f64, v: f64) -> [f64; 3];
}

/// Types of failure the tessellator can report. It never panics.
#[derive(Debug, Clone, PartialEq)]
pub enum TessellateError {
    /// Every sample evaluates to the same point: no triangle with positive
    /// area can be produced (zero-area / collapsed surface).
    Degenerate,
    /// A surface sample is not finite (NaN/Inf at a parameter pair).
    NonFiniteSurface,
    /// A rational-Bezier descriptor or control grid has invalid dimensions or
    /// content (wrong control-point/weight count, bad tokens).
    InvalidSurface(String),
}

impl fmt::Display for TessellateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TessellateError::Degenerate => write!(f, "surface collapsed to zero area"),
            TessellateError::NonFiniteSurface => write!(f, "surface produced a non-finite point"),
            TessellateError::InvalidSurface(detail) => {
                write!(f, "invalid rational-Bezier surface: {detail}")
            }
        }
    }
}

impl std::error::Error for TessellateError {}

/// A triangle mesh: positions plus triangle indices into `positions`.
///
/// Lightweight and dependency-free (plain `f64` positions, `u32` triangles);
/// `positions()` / `triangles()` return the exact slices the 3DXML ASCII
/// writer and exporters consume.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    positions: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// Build a mesh from plain slices (used by the golden-compare reference
    /// grids in the acceptance test).
    pub fn new(positions: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>) -> Self {
        Self {
            positions,
            triangles,
        }
    }

    /// Positions of the mesh (plain slice).
    pub fn positions(&self) -> &[[f64; 3]] {
        &self.positions
    }

    /// Triangle index triples (plain slice).
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Invariant check: non-empty triangle list, in-range indices, finite
    /// positions, no zero-area triangles. `Ok(())` when structurally valid.
    pub fn validate(&self) -> Result<(), TessellateError> {
        if self.triangles.is_empty() {
            return Err(TessellateError::Degenerate);
        }
        for t in &self.triangles {
            for &i in t {
                if i as usize >= self.positions.len() {
                    return Err(TessellateError::Degenerate); // index out of range
                }
            }
            let [a, b, c] = *t;
            let pa = self.positions[a as usize];
            let pb = self.positions[b as usize];
            let pc = self.positions[c as usize];
            for p in [pa, pb, pc] {
                if !p.iter().all(|x| x.is_finite()) {
                    return Err(TessellateError::NonFiniteSurface);
                }
            }
            // Cross-product area; zero on any collinear/degenerate triangle.
            let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
            let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
            let cx = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let area_sq = cx[0] * cx[0] + cx[1] * cx[1] + cx[2] * cx[2];
            if area_sq <= f64::EPSILON * f64::EPSILON {
                return Err(TessellateError::Degenerate);
            }
        }
        Ok(())
    }
}

/// Tessellation parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessellateOpts {
    /// Quality-control bound: every sampled distance from the true surface to
    /// the piecewise-linear mesh must not exceed this. Smaller = finer mesh.
    pub max_sag: f64,
    /// Initial grid resolution (grid lines per axis). Larger = finer start.
    pub base_uv: usize,
    /// Hard cap on grid lines per axis. The tessellator honours this without
    /// overflow even on pathological curvature; if the bound is not met by
    /// the cap it reports the achieved (best) deviation honestly.
    pub max_uv: usize,
}

/// Relative area threshold below which a triangle is treated as a degenerate
/// zero-area sliver and NOT emitted. A rational parameter singularity that
/// collapses a row of cells (e.g. a sphere pole where the whole top edge
/// maps to one point) then yields a clean fan instead of zero-area noise;
/// a fully collapsed surface produces NO triangles and is reported as
/// [`TessellateError::Degenerate`].
const SLIVER_AREA_REL: f64 = 1e-14;

impl Default for TessellateOpts {
    fn default() -> Self {
        Self {
            max_sag: 1e-3,
            base_uv: 8,
            max_uv: 1024,
        }
    }
}

/// Quality report produced alongside the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessellateReport {
    /// The requested bound (`opts.max_sag`).
    pub max_sag: f64,
    /// Independently measured worst deviation of the true surface from the
    /// mesh. This is the quality-control number the acceptance test checks
    /// against `max_sag`.
    pub max_deviation: f64,
    /// Grid resolution used (`N` lines per axis -> `(N+1)^2` positions).
    pub grid_uv: usize,
    /// Number of triangles.
    pub triangle_count: usize,
    /// True when the achieved deviation stayed within `max_sag`; false when
    /// the grid hit `max_uv` first (the bound is then not guaranteed).
    pub within_tolerance: bool,
}

/// A rational Bezier surface (single-span NURBS): a weighted control grid
/// `(u,v) -> (sum_i w_i P_i B_i^du(u) B_i^dv(v)) / (sum_i w_i B_i B_i)`.
///
/// This is exactly the surface form P3-M2-T01's Gregory decode emits
/// (rational biquartic single-span patches), so decoded patches plug straight
/// in. `control_points` are row-major with the `u`-index varying slowest,
/// mirroring the sibling module's `NurbsSurface` layout.
#[derive(Debug, Clone, PartialEq)]
pub struct RationalBezierSurface {
    degree_u: usize,
    degree_v: usize,
    control_points: Vec<[f64; 3]>,
    weights: Vec<f64>,
}

impl RationalBezierSurface {
    /// Build from degrees and a weighted control grid. Returns
    /// [`TessellateError::InvalidSurface`] when the number of control points /
    /// weights does not match `(degree_u+1) * (degree_v+1)` or contains
    /// non-finite values.
    pub fn new(
        degree_u: usize,
        degree_v: usize,
        control_points: Vec<[f64; 3]>,
        weights: Vec<f64>,
    ) -> Result<Self, TessellateError> {
        let expected = (degree_u + 1) * (degree_v + 1);
        if control_points.len() != expected || weights.len() != expected {
            return Err(TessellateError::InvalidSurface(format!(
                "expected {expected} control points/weights, got {} cps / {} weights",
                control_points.len(),
                weights.len()
            )));
        }
        if control_points
            .iter()
            .any(|p| !p.iter().all(|x| x.is_finite()))
            || weights.iter().any(|w| !w.is_finite())
        {
            return Err(TessellateError::InvalidSurface(
                "non-finite control point or weight".into(),
            ));
        }
        Ok(Self {
            degree_u,
            degree_v,
            control_points,
            weights,
        })
    }

    /// Parse a plain-text descriptor. Format (one surface per file):
    /// first line `du dv`, then `(du+1)*(dv+1)` weights and thereafter the
    /// control points as x y z triples — whitespace/newline separated.
    /// Returns [`TessellateError::InvalidSurface`] on any parse problem.
    pub fn parse(text: &str) -> Result<Self, TessellateError> {
        let mut it = text.split_whitespace();
        let du: usize = it
            .next()
            .ok_or_else(|| TessellateError::InvalidSurface("missing degree_u".into()))?
            .parse()
            .map_err(|_| TessellateError::InvalidSurface("degree_u not an integer".into()))?;
        let dv: usize = it
            .next()
            .ok_or_else(|| TessellateError::InvalidSurface("missing degree_v".into()))?
            .parse()
            .map_err(|_| TessellateError::InvalidSurface("degree_v not an integer".into()))?;
        let n = (du + 1) * (dv + 1);
        let mut weights: Vec<f64> = Vec::with_capacity(n);
        for _ in 0..n {
            let w = it
                .next()
                .ok_or_else(|| TessellateError::InvalidSurface("not enough weights".into()))?
                .parse()
                .map_err(|_| TessellateError::InvalidSurface("weight not a number".into()))?;
            weights.push(w);
        }
        let mut cps: Vec<[f64; 3]> = Vec::with_capacity(n);
        for _ in 0..n {
            let x: f64 = it
                .next()
                .ok_or_else(|| TessellateError::InvalidSurface("not enough x".into()))?
                .parse()
                .map_err(|_| TessellateError::InvalidSurface("x not a number".into()))?;
            let y: f64 = it
                .next()
                .ok_or_else(|| TessellateError::InvalidSurface("not enough y".into()))?
                .parse()
                .map_err(|_| TessellateError::InvalidSurface("y not a number".into()))?;
            let z: f64 = it
                .next()
                .ok_or_else(|| TessellateError::InvalidSurface("not enough z".into()))?
                .parse()
                .map_err(|_| TessellateError::InvalidSurface("z not a number".into()))?;
            cps.push([x, y, z]);
        }
        if it.next().is_some() {
            return Err(TessellateError::InvalidSurface("trailing tokens".into()));
        }
        Self::new(du, dv, cps, weights)
    }

    /// Degrees per axis.
    pub fn degrees(&self) -> (usize, usize) {
        (self.degree_u, self.degree_v)
    }

    /// Control points (row-major).
    pub fn control_points(&self) -> &[[f64; 3]] {
        &self.control_points
    }

    /// Weights (one per control point).
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
}

impl SurfaceEval for RationalBezierSurface {
    fn eval(&self, u: f64, v: f64) -> [f64; 3] {
        let (du, dv) = (self.degree_u, self.degree_v);
        // Precompute per-axis Bernstein values.
        let bu: Vec<f64> = (0..=du).map(|i| bern(du, i, u)).collect();
        let bv: Vec<f64> = (0..=dv).map(|j| bern(dv, j, v)).collect();
        let mut num = [0.0_f64; 3];
        let mut den = 0.0_f64;
        for (j, &bvj) in bv.iter().enumerate() {
            for (i, &bui) in bu.iter().enumerate() {
                let idx = j * (du + 1) + i;
                let w = self.weights[idx] * bui * bvj;
                let p = self.control_points[idx];
                num[0] += w * p[0];
                num[1] += w * p[1];
                num[2] += w * p[2];
                den += w;
            }
        }
        if den.abs() < f64::EPSILON {
            // Rational singularity (e.g. a decoded Gregory corner): delegate
            // this to the caller's tolerance model; return NaN so the
            // non-finite path flags it rather than inventing a point.
            return [f64::NAN; 3];
        }
        [num[0] / den, num[1] / den, num[2] / den]
    }
}

/// Tessellate a parametric surface into a triangle mesh with quality control.
///
/// Strategy: start from a `base_uv x base_uv` uniform grid; triangulate each
/// cell with two triangles along one diagonal; then MEASURE the true surface
/// (samples per cell at the cell centre) against the piecewise-linear mesh.
/// While any sample deviates by more than `max_sag` and the grid has not hit
/// `max_uv`, double the resolution and repeat. The final report carries the
/// worst measured deviation — the acceptance quality-control number.
///
/// Never panics. Degenerate (zero-area) and non-finite surfaces return typed
/// errors.
pub fn tessellate<S: SurfaceEval + ?Sized>(
    surface: &S,
    opts: &TessellateOpts,
) -> Result<(Mesh, TessellateReport), TessellateError> {
    let base = opts.base_uv.max(1);
    let cap = opts.max_uv.max(base);
    let mut n = base;
    let mut best: Option<(Mesh, f64)> = None;

    // Verify the surface produces a finite, non-degenerate cell before the
    // refinement loop: a fully collapsed surface fails fast with a typed
    // error (no triangle emitted anywhere), never a panic.
    {
        let (probe, _cells) = build_mesh(surface, n)?;
        probe.validate()?;
    }

    loop {
        let (mesh, cells) = build_mesh(surface, n)?;
        mesh.validate()?;
        let measured = measure(surface, &mesh, &cells, n);
        let within = measured <= opts.max_sag;
        // Track the best (finest) mesh seen even if the cap stops us early.
        if best.as_ref().is_none_or(|(_, d)| measured < *d) {
            best = Some((mesh, measured));
        }
        if within || n >= cap {
            let (mesh, measured) = best.expect("at least one mesh built");
            let triangle_count = mesh.triangles().len();
            let grid_uv = n;
            return Ok((
                mesh,
                TessellateReport {
                    max_sag: opts.max_sag,
                    max_deviation: measured,
                    grid_uv,
                    triangle_count,
                    within_tolerance: within,
                },
            ));
        }
        n = (n.saturating_mul(2)).min(cap);
    }
}

/// Per-cell emitted-triangle index table: `[v_index][u_index]` -> indices
/// into the mesh's triangle list (empty where the cell's triangles were
/// dropped as zero-area slivers). Returned alongside a built grid so the QC
/// measure always touches the exact triangles the mesh carries.
type CellTriangles = Vec<Vec<Vec<u32>>>;

/// Build a uniform `N x N` grid mesh of the surface: `(N+1)^2` positions
/// (row-major `[u][v]`) and two triangles per cell split along the `b-c`
/// diagonal (the direction OPPOSITE the acceptance test's independent
/// reference grid, so the two golden paths differ). Zero-area slivers
/// (parameter singularities like a sphere pole) are not emitted. Alongside
/// the mesh, the returned cell table maps each cell to the world-space
/// triangle indices actually emitted for it (empty when the cell's two
/// triangles were both slivers), so the QC measure touches the SAME mesh.
fn build_mesh<S: SurfaceEval + ?Sized>(
    surface: &S,
    n: usize,
) -> Result<(Mesh, CellTriangles), TessellateError> {
    let mut positions: Vec<[f64; 3]> = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        let v = j as f64 / n as f64;
        for i in 0..=n {
            let u = i as f64 / n as f64;
            let p = surface.eval(u, v);
            if !p.iter().all(|x| x.is_finite()) {
                return Err(TessellateError::NonFiniteSurface);
            }
            positions.push(p);
        }
    }
    let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    let mut triangles: Vec<[u32; 3]> = Vec::with_capacity(2 * n * n);
    let mut cell_triangles: Vec<Vec<Vec<u32>>> = vec![vec![Vec::new(); n]; n];
    // Reference scale for the sliver test: mean grid edge length squared.
    let mut scale_sq = 0.0_f64;
    if n > 0 {
        let p00 = positions[0];
        let p01 = positions[1];
        let mut s = 0.0_f64;
        for d in 0..3 {
            let e = p01[d] - p00[d];
            s += e * e;
        }
        scale_sq = s;
    }
    let push_tri = |a: u32, b: u32, c: u32, tri_ref: &mut Vec<[u32; 3]>, cell: &mut Vec<u32>| {
        let pa = positions[a as usize];
        let pb = positions[b as usize];
        let pc = positions[c as usize];
        let u_ = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let v_ = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let cx = [
            u_[1] * v_[2] - u_[2] * v_[1],
            u_[2] * v_[0] - u_[0] * v_[2],
            u_[0] * v_[1] - u_[1] * v_[0],
        ];
        let area_sq = cx[0] * cx[0] + cx[1] * cx[1] + cx[2] * cx[2];
        let threshold = SLIVER_AREA_REL * SLIVER_AREA_REL * scale_sq;
        if area_sq <= threshold {
            return; // zero-area sliver (pole fan / collapsed cell)
        }
        let t = [a, b, c];
        tri_ref.push(t);
        cell.push((tri_ref.len() - 1) as u32);
    };
    for (j, row) in cell_triangles.iter_mut().enumerate() {
        for (i, cell) in row.iter_mut().enumerate() {
            let a = idx(i, j);
            let b = idx(i + 1, j);
            let c = idx(i, j + 1);
            let d = idx(i + 1, j + 1);
            // One diagonal per cell (b-c here, opposite the reference grid's
            // a-d): two triangles tile the cell exactly; a collapsed edge at a
            // singularity is dropped rather than shipped as zero-area noise.
            push_tri(a, b, c, &mut triangles, cell);
            push_tri(b, d, c, &mut triangles, cell);
        }
    }
    Ok((Mesh::new(positions, triangles), cell_triangles))
}

/// Measure the worst deviation of the true surface from the mesh by sampling
/// the surface at several points per cell (centre + the four edge midpoints,
/// in cell-local parameter space) and computing the distance to that cell's
/// emitted triangles (the same ones the mesh actually carries; sliver-less).
/// Returns the global maximum over all cells.
fn measure<S: SurfaceEval + ?Sized>(
    surface: &S,
    mesh: &Mesh,
    cells: &[Vec<Vec<u32>>],
    n: usize,
) -> f64 {
    let positions = mesh.positions();
    let triangles = mesh.triangles();
    // Cell-local sample fractions (centre + edge midpoints). Each is fed to
    // the TRUE surface and compared against the cell's linear triangles, so
    // the reported deviation reflects the whole cell, not just its centre.
    const SAMPLES: [[f64; 2]; 5] = [
        [0.50, 0.50],
        [0.50, 0.00],
        [0.50, 1.00],
        [0.00, 0.50],
        [1.00, 0.50],
    ];
    let mut worst = 0.0_f64;
    for (j, row) in cells.iter().enumerate() {
        for (i, cell) in row.iter().enumerate() {
            let base_u = i as f64 / n as f64;
            let base_v = j as f64 / n as f64;
            for [fu, fv] in SAMPLES {
                let cu = base_u + fu / n as f64;
                let cv = base_v + fv / n as f64;
                let p = surface.eval(cu, cv);
                if !p.iter().all(|x| x.is_finite()) {
                    // A non-finite probe is a NaN region; the build path
                    // already rejects non-finite vertices, so treat a mid-cell
                    // NaN as the worst possible deviation (the mesh cannot
                    // represent it).
                    return f64::INFINITY;
                }
                // Distance from the surface sample to the MESH: the minimum
                // over the cell's triangles (a point on a shared edge or
                // corner is exactly on the mesh via the owning triangle).
                let mut best = f64::INFINITY;
                for &ti in cell {
                    let t = triangles[ti as usize];
                    let pa = positions[t[0] as usize];
                    let pb = positions[t[1] as usize];
                    let pc = positions[t[2] as usize];
                    let d = dist_pt_tri(p, pa, pb, pc);
                    if d < best {
                        best = d;
                    }
                }
                if best > worst {
                    worst = best;
                }
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_qc_measure_is_exactly_zero() {
        // Flat plane: any sample stays z=0 and inside its cell's footprint,
        // so the QC measure must be ~0 at every cell and every sample slot.
        struct Plane;
        impl SurfaceEval for Plane {
            fn eval(&self, u: f64, v: f64) -> [f64; 3] {
                [u, v, 0.0]
            }
        }
        for n in [4usize, 8, 16] {
            let (mesh, cells) = build_mesh(&Plane, n).expect("plane builds");
            let m = measure(&Plane, &mesh, &cells, n);
            assert!(m <= 1e-12, "plane QC measure at n={n} is ~0, got {m:.3e}");
        }
    }

    #[test]
    fn dist_pt_tri_on_plane_edge_is_zero() {
        let a = [0.0_f64, 0.0, 0.0];
        let b = [1.0_f64, 0.0, 0.0];
        let c = [0.0_f64, 1.0, 0.0];
        for p in [
            [0.5, 0.5, 0.0], // straight centroid/interior
            [0.5, 0.0, 0.0], // on edge ab
            [0.0, 0.5, 0.0], // on edge ac
            [0.25, 0.75, 0.0],
        ] {
            let d = dist_pt_tri(p, a, b, c);
            assert!(d <= 1e-12, "point {p:?} on plane: d={d:.3e}");
        }
    }
}
