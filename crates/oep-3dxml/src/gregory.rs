//! Bicubic Gregory patch and its exact decode to a rational NURBS surface.
//!
//! A bicubic Gregory patch is a rational surface built from two tensor-product
//! bicubic Bezier surfaces that share the same 12 boundary/edge control points
//! but carry different 2x2 interior ("twist") control points:
//!
//! ```text
//! S(u,v) = ( u(1-v) * S1(u,v) + (1-u)*v * S2(u,v) ) / ( u(1-v) + (1-u)v )
//! ```
//!
//! S1 uses the "(uv)" twist set, S2 the "(vu)" twist set. The rational blend
//! guarantees the patch meets its neighbours smoothly at shared cubic Bezier
//! edges without forcing mixed partials to match (Gregory's original
//! motivation). Boundaries of the patch are exactly the cubic Bezier boundary
//! curves; the blend ratios vanish to 0/0 at the two corners (0,0) and (1,1)
//! where the limit is the corresponding corner point.
//!
//! [`GregoryPatch::to_nurbs`] performs an EXACT decode of this rational form
//! into a single rational biquartic NURBS surface (degrees 4x4, one clampspan,
//! i.e. a rational Bezier patch), using Bernstein-basis multiplication of the
//! numerator `u(1-v)*S1 + (1-u)v*S2` and the denominator `u(1-v)+(1-u)v`. The
//! mapping is exact: evaluating the NURBS equals evaluating the Gregory patch
//! to within floating-point rounding at every parameter pair where the
//! denominator is non-zero.

use crate::vec3::{Vec3, ZERO};

/// Binomial coefficient `C(n, k)` as `f64`.
fn binom(n: usize, k: usize) -> f64 {
    let k = k.min(n - k);
    let mut c: f64 = 1.0;
    for i in 1..=k {
        c = c * (n - k + i) as f64 / i as f64;
    }
    c
}

/// Bernstein basis polynomial `B_k^n(t) = C(n,k) t^k (1-t)^(n-k)`.
fn bern(n: usize, k: usize, t: f64) -> f64 {
    binom(n, k) * t.powi(k as i32) * (1.0 - t).powi((n - k) as i32)
}

/// Evaluate a tensor-product Bezier surface (bicubic control grid) at (u, v).
fn eval_bezier2d(g: &[[Vec3; 4]; 4], u: f64, v: f64) -> Vec3 {
    let mut acc = ZERO;
    for (i, row) in g.iter().enumerate() {
        let bu = bern(3, i, u);
        for (j, p) in row.iter().enumerate() {
            acc += *p * (bu * bern(3, j, v));
        }
    }
    acc
}

/// Scalar Bernstein coefficient for the 2-D product `P (deg n) * Q (deg m)`.
///
/// For basis functions `B_i^n(u) B_j^n(v)` and `B_k^m(u) B_l^m(v)` the product
/// coefficient at output index `(r, s) = (i+k, j+l)` picks up the factor
/// `C(n,i)C(m,k)/C(n+m,r)` per axis.
fn product_factor(n: usize, i: usize, m: usize, k: usize, s: usize) -> f64 {
    binom(n, i) * binom(m, k) / binom(n + m, s)
}

/// Multiply a bicubic Bezier control grid (4x4 of points) by a bilinear
/// (1,1)-degree scalar grid (2x2 of coefficients), producing the 5x5 biquartic
/// Bernstein control points of the product surface.
fn bary_mul_bilinear(g: &[[Vec3; 4]; 4], q: &[[f64; 2]; 2]) -> [[Vec3; 5]; 5] {
    // g has degree (3,3); q has degree (1,1); product has degree (4,4).
    let (n, m) = (3usize, 1usize);
    let mut out = [[ZERO; 5]; 5];
    for (i, grid_row) in g.iter().enumerate() {
        for (j, gv) in grid_row.iter().enumerate() {
            for (k, q_row) in q.iter().enumerate() {
                for (l, qv) in q_row.iter().enumerate() {
                    let fu = product_factor(n, i, m, k, i + k);
                    let fv = product_factor(n, j, m, l, j + l);
                    out[i + k][j + l] += *gv * (*qv * fu * fv);
                }
            }
        }
    }
    out
}

/// Degree-elevate a 1-D Bernstein coefficient list to `target_deg` (>= n).
fn elevate_1d(a: &[f64], target_deg: usize) -> Vec<f64> {
    let mut cur = a.to_vec();
    while cur.len() - 1 < target_deg {
        let n = cur.len() - 1;
        let mut nx = vec![0.0; n + 2];
        nx[0] = cur[0];
        nx[n + 1] = cur[n];
        for k in 1..=n {
            nx[k] = (k as f64 * cur[k - 1] + (n + 1 - k) as f64 * cur[k]) / (n + 1) as f64;
        }
        cur = nx;
    }
    cur
}

/// Elevate a (1,1)-degree scalar grid to one with `target_deg` per axis.
fn elevate_grid_to(q: &[[f64; 2]; 2], target_deg: usize) -> Vec<Vec<f64>> {
    // q[i][j] where i is the u slot and j is the v slot. Elevate along u
    // first: transpose so each "row" is a v column [q[0][v], q[1][v]].
    let transposed = [[q[0][0], q[1][0]], [q[0][1], q[1][1]]];
    let v_rows: Vec<Vec<f64>> = transposed
        .iter()
        .map(|col| elevate_1d(col, target_deg))
        .collect();
    let mut out = vec![vec![0.0; target_deg + 1]; target_deg + 1];
    // Elevate along the v axis (each u column).
    for (i, out_row) in out.iter_mut().enumerate() {
        let col: Vec<f64> = v_rows.iter().map(|r| r[i]).collect();
        let col = elevate_1d(&col, target_deg);
        out_row.copy_from_slice(&col);
    }
    out
}

/// A bicubic Gregory patch (20 control points).
///
/// Parameterisation: `u` runs 0..1, `v` runs 0..1; corners are indexed
/// [`corners`](Self::corners) as `(u,v) = (0,0), (1,0), (1,1), (0,1)`.
/// Edge runs and twist sets follow the standard bicubic Gregory layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GregoryPatch {
    /// Corner points, order: (0,0), (1,0), (1,1), (0,1).
    pub corners: [Vec3; 4],
    /// Interior boundary control points on the edge `v = 0` (u-slots 1 and 2).
    pub edge_uv0: [Vec3; 2],
    /// Interior boundary control points on the edge `v = 1` (u-slots 1 and 2).
    pub edge_uv1: [Vec3; 2],
    /// Interior boundary control points on the edge `u = 0` (v-slots 1 and 2).
    pub edge_vu0: [Vec3; 2],
    /// Interior boundary control points on the edge `u = 1` (v-slots 1 and 2).
    pub edge_vu1: [Vec3; 2],
    /// "(uv)" interior twist control points, indexed [u-slot-1][v-slot-1].
    pub twist_uv: [[Vec3; 2]; 2],
    /// "(vu)" interior twist control points, indexed [u-slot-1][v-slot-1].
    pub twist_vu: [[Vec3; 2]; 2],
}

/// Number of control points of a bicubic Gregory patch.
pub const CONTROL_POINT_COUNT: usize = 20;

impl GregoryPatch {
    /// Build a bicubic Gregory patch whose interior twist pairs coincide
    /// (a "twist-free" patch: the two blend surfaces are identical).
    pub fn twist_free(
        corners: [Vec3; 4],
        edge_uv0: [Vec3; 2],
        edge_uv1: [Vec3; 2],
        edge_vu0: [Vec3; 2],
        edge_vu1: [Vec3; 2],
        interior: [[Vec3; 2]; 2],
    ) -> Self {
        Self {
            corners,
            edge_uv0,
            edge_uv1,
            edge_vu0,
            edge_vu1,
            twist_uv: interior,
            twist_vu: interior,
        }
    }

    /// Materialise the full 4x4 bicubic Bezier control grid for one blend
    /// surface (shared boundary + the given interior twist set).
    fn bezier_grid(&self, twist: &[[Vec3; 2]; 2]) -> [[Vec3; 4]; 4] {
        let mut g = [[ZERO; 4]; 4];
        // Corners.
        g[0][0] = self.corners[0];
        g[3][0] = self.corners[1];
        g[3][3] = self.corners[2];
        g[0][3] = self.corners[3];
        // v=0 edge (u=1,2), v=1 edge (u=1,2).
        g[1][0] = self.edge_uv0[0];
        g[2][0] = self.edge_uv0[1];
        g[1][3] = self.edge_uv1[0];
        g[2][3] = self.edge_uv1[1];
        // u=0 edge (v=1,2), u=1 edge (v=1,2).
        g[0][1] = self.edge_vu0[0];
        g[0][2] = self.edge_vu0[1];
        g[3][1] = self.edge_vu1[0];
        g[3][2] = self.edge_vu1[1];
        // Interior twist set.
        for iv in 0..2 {
            for iu in 0..2 {
                g[iu + 1][iv + 1] = twist[iu][iv];
            }
        }
        g
    }

    /// Evaluate the Gregory patch at parameter pair (u, v).
    ///
    /// At the two blend-singular corners `(0,0)` and `(1,1)` the rational
    /// expression is 0/0; the limit (the corner point itself) is returned.
    pub fn eval(&self, u: f64, v: f64) -> Vec3 {
        if u <= f64::EPSILON && v <= f64::EPSILON {
            return self.corners[0];
        }
        if (1.0 - u).abs() <= f64::EPSILON && (1.0 - v).abs() <= f64::EPSILON {
            return self.corners[2];
        }
        let s1 = eval_bezier2d(&self.bezier_grid(&self.twist_uv), u, v);
        let s2 = eval_bezier2d(&self.bezier_grid(&self.twist_vu), u, v);
        let g = u * (1.0 - v);
        let h = (1.0 - u) * v;
        (s1 * g + s2 * h) / (g + h)
    }

    /// Decode this Gregory patch into an EXACT single rational NURBS patch.
    ///
    /// The result is a rational biquartic Bezier patch (degree 4 in each
    /// direction, clamped single-span knots — a NURBS with weights). Decoding
    /// is exact: the returned [`NurbsSurface::eval`] reproduces
    /// [`Self::eval`] at every non-singular parameter pair.
    pub fn to_nurbs(&self) -> NurbsSurface {
        let s1 = self.bezier_grid(&self.twist_uv);
        let s2 = self.bezier_grid(&self.twist_vu);
        // g = u(1-v) -> (1,1)-Bernstein coefficient at (u=1, v=0).
        let gco = [[0.0, 0.0], [1.0, 0.0]];
        // h = (1-u)v -> coefficient at (u=0, v=1).
        let hco = [[0.0, 1.0], [0.0, 0.0]];
        // Numerator N = g*S1 + h*S2, biquartic (5x5).
        let n_g = bary_mul_bilinear(&s1, &gco);
        let n_h = bary_mul_bilinear(&s2, &hco);
        let mut num = [[ZERO; 5]; 5];
        for i in 0..5 {
            for j in 0..5 {
                num[i][j] = n_g[i][j] + n_h[i][j];
            }
        }
        // Denominator D = g + h degree-elevated to biquartic.
        let d_g = elevate_grid_to(&gco, 4);
        let d_h = elevate_grid_to(&hco, 4);
        let mut den = [[0.0_f64; 5]; 5];
        for i in 0..5 {
            for j in 0..5 {
                den[i][j] = d_g[i][j] + d_h[i][j];
            }
        }
        // Rational control points: weight = denominator coefficient,
        // position = numerator coefficient / weight.
        let mut cps = Vec::with_capacity(25);
        let mut weights = Vec::with_capacity(25);
        for j in 0..5 {
            for i in 0..5 {
                let w = den[i][j];
                let p = if w.abs() < 1e-300 {
                    ZERO
                } else {
                    num[i][j] / w
                };
                cps.push(p);
                weights.push(w);
            }
        }
        NurbsSurface {
            degree_u: 4,
            degree_v: 4,
            control_points: cps,
            weights,
        }
    }

    /// Return the cubic Bezier boundary curve at the given edge.
    ///
    /// Edge ids encode the fixed parameter axis and the coordinate value of
    /// the opposite axis: 0 = `v=0` (u runs), 1 = `v=1` (u runs),
    /// 2 = `u=0` (v runs), 3 = `u=1` (v runs). The curve is oriented so its
    /// `t=0` end is the `(0,0)` / `(1,1)` corner pair consistent with the
    /// patch corners array (see [`corners`](Self::corners)).
    pub fn boundary_curve(&self, edge: u8) -> BezierCurve3 {
        match edge {
            0 => BezierCurve3::new([
                self.corners[0],
                self.edge_uv0[0],
                self.edge_uv0[1],
                self.corners[1],
            ]),
            1 => BezierCurve3::new([
                self.corners[3],
                self.edge_uv1[0],
                self.edge_uv1[1],
                self.corners[2],
            ]),
            2 => BezierCurve3::new([
                self.corners[0],
                self.edge_vu0[0],
                self.edge_vu0[1],
                self.corners[3],
            ]),
            3 => BezierCurve3::new([
                self.corners[1],
                self.edge_vu1[0],
                self.edge_vu1[1],
                self.corners[2],
            ]),
            _ => unreachable!("edge id must be 0..=3"),
        }
    }
}

/// A cubic Bezier curve in 3-D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierCurve3 {
    /// The four control points.
    pub control: [Vec3; 4],
}

impl BezierCurve3 {
    /// Build a cubic Bezier curve from its control points.
    pub fn new(control: [Vec3; 4]) -> Self {
        Self { control }
    }

    /// Evaluate the curve at parameter `t` in 0..=1.
    pub fn eval(&self, t: f64) -> Vec3 {
        let mut acc = ZERO;
        for i in 0..4 {
            acc += self.control[i] * bern(3, i, t);
        }
        acc
    }

    /// Evaluate the first derivative (velocity vector) at `t` in 0..=1.
    pub fn deriv(&self, t: f64) -> Vec3 {
        // d/dt of cubic Bezier = 3 * quadratic with deltas between cps.
        let quad = [
            (self.control[1] - self.control[0]) * 3.0,
            (self.control[2] - self.control[1]) * 3.0,
            (self.control[3] - self.control[2]) * 3.0,
        ];
        let mut acc = ZERO;
        for (i, q) in quad.iter().enumerate() {
            acc += *q * bern(2, i, t);
        }
        acc
    }
}

/// A tensor-product rational NURBS surface (Bezier = single clamped span).
///
/// `control_points` are stored row-major with the `u`-index varying slowest:
/// index `j*(du+1)+i` holds the control point at `(u_i, v_j)`. The patch is
/// evaluated as a rational tensor Bezier; at parameter pairs where the weight
/// denominator vanishes (the two blend-singular corners of a decoded Gregory
/// patch) the result of [`Self::eval`] is `NaN`, matching the underlying
/// rational singularity — callers decode corner limits via the Gregory form.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    /// Degree in `u` (4 for a decoded Gregory patch).
    pub degree_u: usize,
    /// Degree in `v` (4 for a decoded Gregory patch).
    pub degree_v: usize,
    /// Euclidean control points, length `(degree_u+1)*(degree_v+1)`,
    /// row-major `[u][v]`.
    pub control_points: Vec<Vec3>,
    /// Non-negative weights, length equal to [`Self::control_points`].
    pub weights: Vec<f64>,
}

impl NurbsSurface {
    /// Number of control points per direction (grid edges).
    pub fn grid_shape(&self) -> (usize, usize) {
        (self.degree_u + 1, self.degree_v + 1)
    }

    /// Set the control point at grid slot `(iu, iv)` (both 0-based).
    pub fn set(&mut self, iu: usize, iv: usize, p: Vec3, w: f64) {
        let (nu, _) = self.grid_shape();
        let idx = iv * nu + iu;
        self.control_points[idx] = p;
        self.weights[idx] = w;
    }

    /// Get the (position, weight) at grid slot `(iu, iv)`.
    pub fn get(&self, iu: usize, iv: usize) -> (Vec3, f64) {
        let (nu, _) = self.grid_shape();
        let idx = iv * nu + iu;
        (self.control_points[idx], self.weights[idx])
    }

    /// Rational evaluation of the NURBS patch at parameter pair (u, v).
    ///
    /// Returns `NaN` where the weight denominator vanishes (corners (0,0) and
    /// (1,1) of a decoded Gregory patch). Prefer [`GregoryPatch::eval`] for
    /// corner-limit evaluation.
    pub fn eval(&self, u: f64, v: f64) -> Vec3 {
        let (nu, nv) = self.grid_shape();
        let mut num = ZERO;
        let mut den = 0.0;
        for j in 0..nv {
            let bv = bern(nv - 1, j, v);
            for i in 0..nu {
                let w = self.weights[j * nu + i];
                let bu = bern(nu - 1, i, u);
                let m = w * bu * bv;
                num += self.control_points[j * nu + i] * m;
                den += m;
            }
        }
        num / den
    }

    /// Rational evaluation returning both point and denominator, for callers
    /// that want to detect the singular corner explicitly.
    pub fn eval_homogeneous(&self, u: f64, v: f64) -> (Vec3, f64) {
        let (nu, nv) = self.grid_shape();
        let mut num = ZERO;
        let mut den = 0.0;
        for j in 0..nv {
            let bv = bern(nv - 1, j, v);
            for i in 0..nu {
                let w = self.weights[j * nu + i];
                let bu = bern(nu - 1, i, u);
                let m = w * bu * bv;
                num += self.control_points[j * nu + i] * m;
                den += m;
            }
        }
        (num / den, den)
    }
}
