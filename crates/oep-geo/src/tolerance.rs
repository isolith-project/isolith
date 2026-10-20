//! P6-M1-T03: the documented tolerance model for `oep-geo`.
//!
//! This is the single epsilon policy that later geometry cards reuse:
//! P6-M2-T03 (geometric predicates), P6-M2-T02 (NURBS evaluation) and the
//! P6-M4 exporters. The model is documented in full in
//! `docs/units-and-tolerances.md`; the essentials:
//!
//! - **Absolute floor + scale-invariant relative term.** A comparison
//!   `a ≈ b` holds when `|a - b| <= linear_abs + relative·max(|a|,|b|)`.
//!   `linear_abs` (default `1e-9`, in the model's length units) catches
//!   near-zero and exact-coincidence; `relative` (default `1e-9`,
//!   dimensionless) carries scale so large-magnitude drift is correctly
//!   rejected far above f64 ULP.
//! - **Total predicates.** Non-finite (`NaN`, `±Inf`) inputs make a
//!   comparison `false` — never a panic, never `true`. This is a hard rule:
//!   geometry code must degrade to "not equal" on bad input, not crash.
//! - **`in_units` re-scales `linear_abs`** from the canonical millimetre base
//!   into the working unit, so a tolerance travels with a model's units.
//! - **`angular_rad`** (default `1e-9` rad) is the reserved angular
//!   comparison bound for later cards; it is carried but not yet consumed by
//!   predicates (nothing here compares angles yet — YAGNI).

use crate::units::LengthUnit;

/// Default absolute linear tolerance (in the model's length units): `1e-9`.
pub const DEFAULT_LINEAR_ABS: f64 = 1e-9;
/// Default relative (scale-invariant) tolerance: `1e-9`.
pub const DEFAULT_RELATIVE: f64 = 1e-9;
/// Default angular tolerance in radians: `1e-9`.
pub const DEFAULT_ANGULAR_RAD: f64 = 1e-9;

/// The documented comparison tolerance model.
///
/// All fields are `f64` (P6-M1-T03: double everywhere).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    linear_abs: f64,
    relative: f64,
    angular_rad: f64,
}

impl Default for Tolerance {
    /// The documented default model: `(1e-9, 1e-9, 1e-9)`.
    fn default() -> Self {
        Tolerance {
            linear_abs: DEFAULT_LINEAR_ABS,
            relative: DEFAULT_RELATIVE,
            angular_rad: DEFAULT_ANGULAR_RAD,
        }
    }
}

impl Tolerance {
    /// Construct a tolerance model with explicit bounds (all `f64`).
    pub fn new(linear_abs: f64, relative: f64, angular_rad: f64) -> Self {
        Tolerance {
            linear_abs,
            relative,
            angular_rad,
        }
    }

    /// The absolute linear floor, in the model's length units.
    pub fn linear_abs(&self) -> f64 {
        self.linear_abs
    }

    /// The scale-invariant relative bound (dimensionless).
    pub fn relative(&self) -> f64 {
        self.relative
    }

    /// The angular bound in radians.
    pub fn angular_rad(&self) -> f64 {
        self.angular_rad
    }

    /// Re-express `linear_abs` from the canonical millimetre base into `unit`.
    /// `relative` and `angular_rad` are dimensionless/unit-independent and pass
    /// through unchanged.
    pub fn in_units(&self, unit: LengthUnit) -> Tolerance {
        Tolerance {
            linear_abs: LengthUnit::from_mm(unit, self.linear_abs),
            relative: self.relative,
            angular_rad: self.angular_rad,
        }
    }

    /// Total scalar comparison: `|a - b| <= linear_abs + relative·max(|a|,|b|)`.
    ///
    /// Any non-finite input (`NaN`, `±Inf`) in either operand returns `false`
    /// (never panics, per the documented discipline).
    pub fn approx_eq(&self, a: f64, b: f64) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let bound = self.linear_abs + self.relative * a.abs().max(b.abs());
        (a - b).abs() <= bound
    }

    /// Total near-zero test: `|x| <= linear_abs`. Non-finite input is `false`.
    pub fn approx_zero(&self, x: f64) -> bool {
        if !x.is_finite() {
            return false;
        }
        x.abs() <= self.linear_abs
    }

    /// Total point comparison on `[f64;3]`: Euclidean distance between `p` and
    /// `q` within the same absolute+relative bound used by [`Tolerance::approx_eq`]
    /// (bound computed against the larger of the two point norms). Non-finite
    /// coordinates make the result `false`, never a panic.
    pub fn points_eq(&self, p: [f64; 3], q: [f64; 3]) -> bool {
        let dx = p[0] - q[0];
        let dy = p[1] - q[1];
        let dz = p[2] - q[2];
        let dist2 = dx * dx + dy * dy + dz * dz;
        if !dist2.is_finite() {
            return false;
        }
        let dist = dist2.sqrt();
        let norm_p = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let norm_q = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
        let bound = self.linear_abs + self.relative * norm_p.max(norm_q);
        dist <= bound
    }
}

#[cfg(test)]
mod tests {
    use super::Tolerance;

    #[test]
    fn default_constants_match_struct() {
        let t = Tolerance::default();
        assert_eq!(t.linear_abs(), 1e-9);
        assert_eq!(t.relative(), 1e-9);
        assert_eq!(t.angular_rad(), 1e-9);
    }

    #[test]
    fn explicit_constructor() {
        let t = Tolerance::new(1e-6, 1e-9, 1e-8);
        assert_eq!(t.linear_abs(), 1e-6);
        assert_eq!(t.relative(), 1e-9);
        assert_eq!(t.angular_rad(), 1e-8);
    }
}
