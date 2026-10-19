//! P6-M1-T03: units model for `oep-geo`.
//!
//! Canonical bases (documented in `docs/p6-m1-units-and-tolerances.md`):
//!
//! - **Length:** canonical base is the **millimetre**. [`LengthUnit`] maps the
//!   engineering drawing units onto that base with exact rational factors
//!   (inch = 25.4 mm, foot = 304.8 mm, ...), so converting through the base
//!   carries at most one f64 rounding step.
//! - **Angle:** canonical base is the **radian**. [`AngleUnit`] converts
//!   degrees <-> radians exactly through `pi`.
//!
//! Disciplines this module encodes (P6-M1-T03):
//!
//! - **Double everywhere:** every scalar is `f64`; integer types are reserved
//!   for counts/indices/ids, never geometry values (enforced by the acceptance
//!   source audit in `tests/test_oep_geo_units_acceptance.py`).
//! - Parsing is total and non-panicking: an unrecognized symbol yields `None`,
//!   never a panic.

/// Engineering length unit, expressed against the canonical millimetre base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    /// Millimetre — the canonical base (`1 mm`).
    Millimetre,
    /// Centimetre (`10 mm`).
    Centimetre,
    /// Metre (`1000 mm`).
    Metre,
    /// Micrometre (`0.001 mm`).
    Micrometre,
    /// International inch (exactly `25.4 mm`).
    Inch,
    /// International foot (exactly `304.8 mm`).
    Foot,
}

impl LengthUnit {
    /// The canonical length unit (millimetre) all conversions route through.
    pub const CANONICAL: LengthUnit = LengthUnit::Millimetre;

    /// Parse a length-unit symbol, case-insensitively. Accepts
    /// `mm`, `cm`, `m`, `um`, `µm`, `μm`, `in`, `inch`, `ft`.
    /// Unknown or empty symbols yield `None` (never panics).
    pub fn parse(s: &str) -> Option<LengthUnit> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "mm" => Some(LengthUnit::Millimetre),
            "cm" => Some(LengthUnit::Centimetre),
            "m" => Some(LengthUnit::Metre),
            "um" | "µm" | "μm" => Some(LengthUnit::Micrometre),
            "in" | "inch" => Some(LengthUnit::Inch),
            "ft" => Some(LengthUnit::Foot),
            _ => None,
        }
    }

    /// The canonical symbol used in the model format (`mm`, `cm`, ...).
    pub fn symbol(&self) -> &'static str {
        match self {
            LengthUnit::Millimetre => "mm",
            LengthUnit::Centimetre => "cm",
            LengthUnit::Metre => "m",
            LengthUnit::Micrometre => "um",
            LengthUnit::Inch => "in",
            LengthUnit::Foot => "ft",
        }
    }

    /// Exact factor `what 1 of this unit equals in millimetres`.
    fn to_mm_factor(self) -> f64 {
        match self {
            LengthUnit::Millimetre => 1.0,
            LengthUnit::Centimetre => 10.0,
            LengthUnit::Metre => 1000.0,
            LengthUnit::Micrometre => 0.001,
            LengthUnit::Inch => 25.4,
            LengthUnit::Foot => 304.8,
        }
    }

    /// Convert a value **from this unit into millimetres** (single multiply).
    pub fn to_mm(unit: LengthUnit, value: f64) -> f64 {
        value * unit.to_mm_factor()
    }

    /// Convert a millimetre value **into this unit** (single divide).
    pub fn from_mm(unit: LengthUnit, value_mm: f64) -> f64 {
        value_mm / unit.to_mm_factor()
    }

    /// Convert `value` from `from` to `to` through the canonical millimetre
    /// base. Single-unit conversions (e.g. in -> mm) are exact factor
    /// multiplies; composed conversions (e.g. ft -> in) carry at most one f64
    /// rounding step, per the documented model.
    pub fn convert(from: LengthUnit, value: f64, to: LengthUnit) -> f64 {
        if from == to {
            return value;
        }
        LengthUnit::from_mm(to, LengthUnit::to_mm(from, value))
    }
}

/// Engineering angle unit, expressed against the canonical radian base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    /// Degree.
    Degree,
    /// Radian — the canonical base.
    Radian,
}

impl AngleUnit {
    /// The canonical angle unit (radian) all conversions route through.
    pub const CANONICAL: AngleUnit = AngleUnit::Radian;

    /// Parse an angle-unit symbol, case-insensitively: `deg`/`degree(s)` or
    /// `rad`/`radian(s)`.
    pub fn parse(s: &str) -> Option<AngleUnit> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "deg" | "degree" | "degrees" => Some(AngleUnit::Degree),
            "rad" | "radian" | "radians" => Some(AngleUnit::Radian),
            _ => None,
        }
    }

    /// The canonical symbol (`deg` or `rad`).
    pub fn symbol(&self) -> &'static str {
        match self {
            AngleUnit::Degree => "deg",
            AngleUnit::Radian => "rad",
        }
    }

    /// Convert a value **from this unit into radians**.
    pub fn to_rad(unit: AngleUnit, value: f64) -> f64 {
        match unit {
            AngleUnit::Degree => value.to_radians(),
            AngleUnit::Radian => value,
        }
    }

    /// Convert a radian value **into this unit**.
    pub fn from_rad(unit: AngleUnit, value_rad: f64) -> f64 {
        match unit {
            AngleUnit::Degree => value_rad.to_degrees(),
            AngleUnit::Radian => value_rad,
        }
    }

    /// Convert `value` from `from` to `to` through the canonical radian base.
    pub fn convert(from: AngleUnit, value: f64, to: AngleUnit) -> f64 {
        if from == to {
            return value;
        }
        AngleUnit::from_rad(to, AngleUnit::to_rad(from, value))
    }
}

#[cfg(test)]
mod tests {
    use super::{AngleUnit, LengthUnit};

    #[test]
    fn canonical_identity_length() {
        assert_eq!(LengthUnit::to_mm(LengthUnit::Millimetre, 1.0), 1.0);
        assert_eq!(LengthUnit::from_mm(LengthUnit::Millimetre, 1.0), 1.0);
    }

    #[test]
    fn canonical_identity_angle() {
        assert_eq!(AngleUnit::to_rad(AngleUnit::Radian, 1.5), 1.5);
        assert_eq!(AngleUnit::from_rad(AngleUnit::Radian, 1.5), 1.5);
    }
}
