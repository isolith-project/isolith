//! P6-M2-T02: NURBS / B-spline curve + surface math for `oep-geo`.
//!
//! Self-contained, std-only, f64 implementation (no OCC binding, no external
//! crate  -  see `docs/p6-m2-oep-geo-nurbs.md` for the decision). Follows the
//! NURBS Book conventions (Piegl & Tiller): grounded clamped knot vectors,
//! Cox-de Boor basis, rational evaluation by homogeneous projection, and
//! derivative evaluation by the homogeneous quotient (Leibniz) rule.
//!
//! Public point type is `[f64; 3]`  -  deliberately NOT named `Vec3`, to avoid a
//! name collision with the parallel unmerged P6-M2-T01 `math::Vec3`. Points
//! are reconciled with `math::Vec3` at the P6-M4-T02 importer hub.
//!
//! Every fallible operation returns a typed [`NurbsError`]; nothing panics on
//! degenerate input. All scalars are `f64` (P6-M1-T03: double everywhere).
//!
//! Clippy note: the basis/derivative routines are faithful transcriptions of
//! the NURBS Book recurrence algorithms (A2.2, A2.3), which are inherently
//! index-recurrence loops (saved/temp chains against left/right spans). An
//! iterator rewrite would obscure the published proof; the module opts out of
//! `needless_range_loop` for those blocks only.
#![allow(clippy::needless_range_loop)]

use std::fmt;

/// Absolute safety margin for "strictly greater" checks on f64 values.
const EPS: f64 = 1e-15;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed, non-panicking NURBS construction/evaluation errors.
#[derive(Debug, Clone, PartialEq)]
pub enum NurbsError {
    /// Not enough control points for the requested degree (n < degree + 1).
    NotEnoughControlPoints,
    /// A knot value is less than the previous one (knots must be non-decreasing).
    KnotNotNonDecreasing,
    /// Knot vector length != (number of control points + degree + 1).
    KnotCountMismatch,
    /// Number of weights does not equal the number of control points.
    WeightCountMismatch,
    /// A weight is not strictly positive (rational projection needs w > 0).
    NonPositiveWeight,
    /// Parameter is outside the clamped domain [knots[degree], knots[n]].
    ParameterOutOfRange,
    /// The rational denominator vanished (weighted basis sum == 0).
    ZeroDenominator,
    /// A control point, weight or knot contains NaN or +/-Inf.
    NonFiniteInput,
    /// Derivative order exceeds the supported maximum (degree).
    DerivativeOrderTooHigh,
    /// Surface control/weight grids are inconsistent (row lengths differ).
    SurfaceGridMismatch,
}

impl fmt::Display for NurbsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::NotEnoughControlPoints => "not enough control points for the degree",
            Self::KnotNotNonDecreasing => "knots must be non-decreasing",
            Self::KnotCountMismatch => "knot count != control points + degree + 1",
            Self::WeightCountMismatch => "weight count != control point count",
            Self::NonPositiveWeight => "weights must be strictly positive",
            Self::ParameterOutOfRange => "parameter outside the knot domain",
            Self::ZeroDenominator => "rational denominator is zero",
            Self::NonFiniteInput => "input contains NaN or infinity",
            Self::DerivativeOrderTooHigh => "derivative order exceeds the degree",
            Self::SurfaceGridMismatch => "surface control grid rows are inconsistent",
        };
        write!(f, "NurbsError: {msg}")
    }
}

impl std::error::Error for NurbsError {}

fn finite(x: f64) -> bool {
    x.is_finite()
}

// ---------------------------------------------------------------------------
// Basis (NURBS Book A2.1 / A2.2 / A2.3)
// ---------------------------------------------------------------------------

/// Locate the knot span index containing `u` (Algorithm A2.1).
pub(crate) fn find_span(degree: usize, knots: &[f64], u: f64) -> usize {
    let n = knots.len() - degree - 1;
    if u >= knots[n] {
        return n - 1;
    }
    if u <= knots[degree] {
        return degree;
    }
    let mut low = degree;
    let mut high = n;
    let mut mid = (low + high) / 2;
    while u < knots[mid] || u >= knots[mid + 1] {
        if u < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

/// Evaluate the nonzero basis functions at `u` for the span (Algorithm A2.2).
/// Returns `n_plus` values indexed [0..=degree], where
/// `n_plus[j] == N_{span-degree+j, degree}(u)`.
pub(crate) fn basis_funcs(degree: usize, knots: &[f64], span: usize, u: f64) -> Vec<f64> {
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    let mut n = vec![0.0; degree + 1];
    n[0] = 1.0;
    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let temp = if den.abs() > EPS { n[r] / den } else { 0.0 };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

/// Derivatives of the nonzero basis functions at `u` (Algorithm A2.3).
/// Returns `(span, ders)` where `ders[k][j]` is the k-th derivative of
/// `N_{span-degree+j, degree}` evaluated at `u`, for `j` in 0..=degree and
/// `k` in 0..=ndu_lim (ndu_lim <= degree).
pub(crate) fn ders_basis(
    degree: usize,
    knots: &[f64],
    span: usize,
    u: f64,
    ndu_lim: usize,
) -> Vec<Vec<f64>> {
    let mut ndu = vec![vec![0.0; degree + 1]; degree + 1];
    ndu[0][0] = 1.0;
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let den = ndu[j][r];
            let temp = if den.abs() > EPS {
                ndu[r][j - 1] / den
            } else {
                0.0
            };
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let mut ders = vec![vec![0.0; degree + 1]; ndu_lim + 1];
    for j in 0..=degree {
        ders[0][j] = ndu[j][degree];
    }
    let mut a = vec![vec![0.0; degree + 1]; 2];
    for r in 0..=degree {
        let mut s1 = 0usize;
        let mut s2 = 1usize;
        a[0][0] = 1.0;
        for k in 1..=ndu_lim {
            let mut d = 0.0;
            let rk = r as isize - k as isize;
            let pk = degree as isize - k as isize;
            if r >= k {
                a[s2][0] = a[s1][0] / ndu[pk as usize + 1][rk as usize];
                d = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { -rk as usize };
            let j2 = if (r as isize - 1) <= pk {
                k - 1
            } else {
                degree - r
            };
            for j in j1..=j2 {
                a[s2][j] = (a[s1][j] - a[s1][j - 1]) / ndu[pk as usize + 1][rk as usize + j];
                d += a[s2][j] * ndu[rk as usize + j][pk as usize];
            }
            if r as isize <= pk {
                a[s2][k] = -a[s1][k - 1] / ndu[pk as usize + 1][r];
                d += a[s2][k] * ndu[r][pk as usize];
            }
            ders[k][r] = d;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    let mut r = degree as f64;
    for k in 1..=ndu_lim {
        for j in 0..=degree {
            ders[k][j] *= r;
        }
        r *= (degree as isize - k as isize) as f64;
    }
    ders
}

// ---------------------------------------------------------------------------
// B-spline curve (non-rational)
// ---------------------------------------------------------------------------

/// A B-spline curve `C(u) = sum_i N_{i,p}(u) P_i` (non-rational).
#[derive(Debug, Clone)]
pub struct BsplineCurve {
    degree: usize,
    knots: Vec<f64>,
    control_points: Vec<[f64; 3]>,
}

impl BsplineCurve {
    /// Build a clamped B-spline curve, validating the knot vector and counts.
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        control_points: Vec<[f64; 3]>,
    ) -> Result<Self, NurbsError> {
        let curve = Self {
            degree,
            knots,
            control_points,
        };
        curve.validate()?;
        Ok(curve)
    }

    fn validate(&self) -> Result<(), NurbsError> {
        let n = self.control_points.len();
        if n < self.degree + 1 {
            return Err(NurbsError::NotEnoughControlPoints);
        }
        if self.knots.len() != n + self.degree + 1 {
            return Err(NurbsError::KnotCountMismatch);
        }
        for w in self.knots.windows(2) {
            if w[1] < w[0] {
                return Err(NurbsError::KnotNotNonDecreasing);
            }
        }
        for k in &self.knots {
            if !finite(*k) {
                return Err(NurbsError::NonFiniteInput);
            }
        }
        for p in &self.control_points {
            if !p.iter().all(|c| finite(*c)) {
                return Err(NurbsError::NonFiniteInput);
            }
        }
        Ok(())
    }

    /// Degree `p`.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Clamped parameter domain `[knots[p], knots[n]]`.
    pub fn domain(&self) -> (f64, f64) {
        let n = self.knots.len() - self.degree - 1;
        (self.knots[self.degree], self.knots[n])
    }

    /// Locate the knot span for `u`.
    fn span(&self, u: f64) -> Result<usize, NurbsError> {
        let (lo, hi) = self.domain();
        if !(lo <= u && u <= hi) {
            return Err(NurbsError::ParameterOutOfRange);
        }
        Ok(find_span(self.degree, &self.knots, u))
    }

    /// Evaluate the curve at parameter `u` (Algorithm A3.1).
    pub fn point(&self, u: f64) -> Result<[f64; 3], NurbsError> {
        let span = self.span(u)?;
        let n = basis_funcs(self.degree, &self.knots, span, u);
        let base = span - self.degree;
        let mut p = [0.0; 3];
        for j in 0..=self.degree {
            let c = self.control_points[base + j];
            let nfj = n[j];
            p[0] += c[0] * nfj;
            p[1] += c[1] * nfj;
            p[2] += c[2] * nfj;
        }
        Ok(p)
    }

    /// Sum of the nonzero basis functions at `u`  -  a partition-of-unity check
    /// exposed for tests and diagnostics.
    pub fn basis_sum(&self, u: f64) -> Result<f64, NurbsError> {
        let span = self.span(u)?;
        let n = basis_funcs(self.degree, &self.knots, span, u);
        Ok(n.iter().sum())
    }

    /// k-th derivative `C^{(k)}(u)` via the derivative basis (A2.3 + A3.2).
    pub fn derivative(&self, u: f64, order: usize) -> Result<[f64; 3], NurbsError> {
        let span = self.span(u)?;
        if order > self.degree {
            return Err(NurbsError::DerivativeOrderTooHigh);
        }
        let ders = ders_basis(self.degree, &self.knots, span, u, order);
        let base = span - self.degree;
        let mut p = [0.0; 3];
        for j in 0..=self.degree {
            let c = self.control_points[base + j];
            let dj = ders[order][j];
            p[0] += c[0] * dj;
            p[1] += c[1] * dj;
            p[2] += c[2] * dj;
        }
        Ok(p)
    }
}

// ---------------------------------------------------------------------------
// NURBS curve (rational)
// ---------------------------------------------------------------------------

/// A rational NURBS curve `C(u) = A(u) / w(u)` with homogeneous numerator
/// `A(u) = sum_i N_{i,p}(u) w_i P_i`.
#[derive(Debug, Clone)]
pub struct NurbsCurve {
    curve: BsplineCurve,
    weights: Vec<f64>,
}

impl NurbsCurve {
    /// Build a clamped NURBS curve, validating knots, counts and weights.
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        control_points: Vec<[f64; 3]>,
        weights: Vec<f64>,
    ) -> Result<Self, NurbsError> {
        if weights.len() != control_points.len() {
            return Err(NurbsError::WeightCountMismatch);
        }
        for w in &weights {
            if !finite(*w) {
                return Err(NurbsError::NonFiniteInput);
            }
            if *w <= 0.0 {
                return Err(NurbsError::NonPositiveWeight);
            }
        }
        let curve = BsplineCurve::new(degree, knots, control_points)?;
        Ok(Self { curve, weights })
    }

    fn span(&self, u: f64) -> Result<usize, NurbsError> {
        self.curve.span(u)
    }

    /// Rational point evaluation via homogeneous projection: `A(u)/w(u)`.
    pub fn point(&self, u: f64) -> Result<[f64; 3], NurbsError> {
        let span = self.span(u)?;
        let n = basis_funcs(self.curve.degree(), &self.curve.knots, span, u);
        let base = span - self.curve.degree();
        let mut a = [0.0; 3];
        let mut w_sum = 0.0;
        for j in 0..=self.curve.degree() {
            let c = self.curve.control_points[base + j];
            let w_j = self.weights[base + j];
            let m = n[j] * w_j;
            a[0] += c[0] * m;
            a[1] += c[1] * m;
            a[2] += c[2] * m;
            w_sum += m;
        }
        if w_sum.abs() <= EPS {
            return Err(NurbsError::ZeroDenominator);
        }
        Ok([a[0] / w_sum, a[1] / w_sum, a[2] / w_sum])
    }

    /// k-th rational derivative via the homogeneous quotient (Leibniz) rule
    /// on the numerator `A` and weight `w` (A4.2). Supports k up to `degree`.
    pub fn derivative(&self, u: f64, order: usize) -> Result<[f64; 3], NurbsError> {
        let span = self.span(u)?;
        let deg = self.curve.degree();
        if order > deg {
            return Err(NurbsError::DerivativeOrderTooHigh);
        }
        let ders = ders_basis(deg, &self.curve.knots, span, u, order.max(1));
        let base = span - deg;
        // Homogeneous derivatives A^(k) and w^(k) for k in 0..=order.
        let mut ad = vec![[0.0; 3]; order + 1];
        let mut wd = vec![0.0; order + 1];
        for k in 0..=order {
            for j in 0..=deg {
                let dkj = ders[k][j];
                let c = self.curve.control_points[base + j];
                let w_j = self.weights[base + j];
                ad[k][0] += c[0] * dkj * w_j;
                ad[k][1] += c[1] * dkj * w_j;
                ad[k][2] += c[2] * dkj * w_j;
                wd[k] += dkj * w_j;
            }
        }
        if wd[0].abs() <= EPS {
            return Err(NurbsError::ZeroDenominator);
        }
        // C^(k) = ( A^(k) - sum_{j=1..k} C(k,j) C^(k-j) w^(j) ) / w
        let mut out = vec![[0.0; 3]; order + 1];
        for k in 0..=order {
            let mut num = ad[k];
            for j in 1..=k {
                let binom = binomial(k, j);
                let ckj = out[k - j];
                num[0] -= binom * ckj[0] * wd[j];
                num[1] -= binom * ckj[1] * wd[j];
                num[2] -= binom * ckj[2] * wd[j];
            }
            out[k] = [num[0] / wd[0], num[1] / wd[0], num[2] / wd[0]];
        }
        Ok(out[order])
    }
}

// ---------------------------------------------------------------------------
// NURBS surface (tensor product)
// ---------------------------------------------------------------------------

/// A NURBS surface `S(u,v) = A(u,v) / w(u,v)` as a tensor product of two
/// rational B-spline directions. Control grid is indexed `[iv][iu]` (v-major,
/// rows of constant `v`); a matching weight grid is required.
#[derive(Debug, Clone)]
pub struct NurbsSurface {
    degree_u: usize,
    degree_v: usize,
    knots_u: Vec<f64>,
    knots_v: Vec<f64>,
    control_points: Vec<Vec<[f64; 3]>>,
    weights: Vec<Vec<f64>>,
}

impl NurbsSurface {
    /// Build a clamped NURBS surface from its two directional curves and the
    /// tensor-product control/weight grids (`grid[v][u]`).
    pub fn new(
        degree_u: usize,
        degree_v: usize,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        control_points: Vec<Vec<[f64; 3]>>,
        weights: Vec<Vec<f64>>,
    ) -> Result<Self, NurbsError> {
        let nv = control_points.len();
        if nv == 0 {
            return Err(NurbsError::NotEnoughControlPoints);
        }
        let nu = control_points[0].len();
        if weights.len() != nv {
            return Err(NurbsError::SurfaceGridMismatch);
        }
        for row in control_points.iter().skip(1) {
            if row.len() != nu {
                return Err(NurbsError::SurfaceGridMismatch);
            }
        }
        for row in &weights {
            if row.len() != nu {
                return Err(NurbsError::SurfaceGridMismatch);
            }
        }
        // Validate each direction as an independent B-spline curve (non-rational
        // structural checks), then weight positivity over the whole grid.
        let _cu = BsplineCurve::new(
            degree_u,
            knots_u.clone(),
            vec![[0.0; 3]; nu.max(degree_u + 1)],
        )?;
        let _cv = BsplineCurve::new(
            degree_v,
            knots_v.clone(),
            vec![[0.0; 3]; nv.max(degree_v + 1)],
        )?;
        for row in &weights {
            for w in row {
                if !finite(*w) {
                    return Err(NurbsError::NonFiniteInput);
                }
                if *w <= 0.0 {
                    return Err(NurbsError::NonPositiveWeight);
                }
            }
        }
        for row in &control_points {
            for p in row {
                if !p.iter().all(|c| finite(*c)) {
                    return Err(NurbsError::NonFiniteInput);
                }
            }
        }
        Ok(Self {
            degree_u,
            degree_v,
            knots_u,
            knots_v,
            control_points,
            weights,
        })
    }

    fn domain_u(&self) -> (f64, f64) {
        let nu = self.control_points[0].len();
        let n = self.knots_u.len() - self.degree_u - 1;
        debug_assert_eq!(n, nu);
        (self.knots_u[self.degree_u], self.knots_u[n])
    }

    fn domain_v(&self) -> (f64, f64) {
        let nv = self.control_points.len();
        let n = self.knots_v.len() - self.degree_v - 1;
        debug_assert_eq!(n, nv);
        (self.knots_v[self.degree_v], self.knots_v[n])
    }

    /// Evaluate `S(u,v)` via double homogeneous projection.
    pub fn point(&self, u: f64, v: f64) -> Result<[f64; 3], NurbsError> {
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        if !(ulo <= u && u <= uhi && vlo <= v && v <= vhi) {
            return Err(NurbsError::ParameterOutOfRange);
        }
        let span_u = find_span(self.degree_u, &self.knots_u, u);
        let span_v = find_span(self.degree_v, &self.knots_v, v);
        let nu = basis_funcs(self.degree_u, &self.knots_u, span_u, u);
        let nv = basis_funcs(self.degree_v, &self.knots_v, span_v, v);
        let base_u = span_u - self.degree_u;
        let base_v = span_v - self.degree_v;
        let mut a = [0.0; 3];
        let mut w_sum = 0.0;
        for j in 0..=self.degree_v {
            let nv_j = nv[j];
            for i in 0..=self.degree_u {
                let c = self.control_points[base_v + j][base_u + i];
                let w_jj = self.weights[base_v + j][base_u + i];
                let m = nu[i] * nv_j * w_jj;
                a[0] += c[0] * m;
                a[1] += c[1] * m;
                a[2] += c[2] * m;
                w_sum += m;
            }
        }
        if w_sum.abs() <= EPS {
            return Err(NurbsError::ZeroDenominator);
        }
        Ok([a[0] / w_sum, a[1] / w_sum, a[2] / w_sum])
    }
}

/// Small exact binomial helper for the quotient-rule derivative.
fn binomial(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    let mut c = 1u128;
    for i in 0..k {
        c = c * (n - i) as u128 / (i + 1) as u128;
    }
    c as f64
}
