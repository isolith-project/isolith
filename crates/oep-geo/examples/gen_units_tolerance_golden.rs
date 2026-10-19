//! P6-M1-T03 golden artifact generator: writes
//! `tests/golden/units-tolerance-golden.json` from the crate's own
//! units/tolerance model.
//!
//! Two-sided verification pattern (mirrors the P6-M2 math card):
//!
//! 1. `crates/oep-geo/tests/units_tolerance.rs` locks the acceptance behavior
//!    directly and checks the golden header contract.
//! 2. `tests/test_oep_geo_units_acceptance.py` independently re-derives every
//!    figure in pure Python stdlib (exact rational factors + bound logic) and
//!    cross-checks within 1e-12 relative.
//!
//! A self-consistent but wrong implementation cannot satisfy both sides.
//!
//! Usage: `cargo run -p oep-geo --example gen_units_tolerance_golden -- <out-dir>`
//! (default `crates/oep-geo/tests/golden`).

use oep_geo::tolerance::{Tolerance, DEFAULT_ANGULAR_RAD, DEFAULT_LINEAR_ABS, DEFAULT_RELATIVE};
use oep_geo::units::LengthUnit;
use std::path::PathBuf;

/// Deterministic number formatting: shortest round-trip via `{}` (Rust Display
/// for f64 is shortest-round-trip); integral values get `.0` so JSON is
/// unambiguous between int and float.
fn fmt(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/oep-geo/tests/golden"));
    std::fs::create_dir_all(&out).expect("create golden dir");
    let path = out.join("units-tolerance-golden.json");

    let default = Tolerance::default();
    let um = default.in_units(LengthUnit::Micrometre);
    let m = default.in_units(LengthUnit::Metre);
    let inch = default.in_units(LengthUnit::Inch);

    let text = format!(
        r#"{{
  "format": "oep-geo-units-tolerance-golden",
  "version": 1,
  "cases": {{
    "to_mm_factors": {{
      "mm": {mm},
      "cm": {cm},
      "m": {m_f},
      "um": {um_f},
      "in": {in_f},
      "ft": {ft}
    }},
    "inch_to_mm": {{"src": "in", "val": 1.0, "dst": "mm", "out": {in_to_mm}}},
    "ft_to_in": {{"src": "ft", "val": 1.0, "dst": "in", "out": {ft_to_in}}},
    "mm25_to_in": {{"src": "mm", "val": 25.4, "dst": "in", "out": {mm25_to_in}}},
    "tolerance_default": {{"linear_abs": {t_lin}, "relative": {t_rel}, "angular_rad": {t_ang}}},
    "in_units_um": {{"linear_abs": {iu_um}}},
    "in_units_m": {{"linear_abs": {iu_m}}},
    "in_units_in": {{"linear_abs": {iu_in}}}
  }}
}}"#,
        mm = fmt(LengthUnit::to_mm(LengthUnit::Millimetre, 1.0)),
        cm = fmt(LengthUnit::to_mm(LengthUnit::Centimetre, 1.0)),
        m_f = fmt(LengthUnit::to_mm(LengthUnit::Metre, 1.0)),
        um_f = fmt(LengthUnit::to_mm(LengthUnit::Micrometre, 1.0)),
        in_f = fmt(LengthUnit::to_mm(LengthUnit::Inch, 1.0)),
        ft = fmt(LengthUnit::to_mm(LengthUnit::Foot, 1.0)),
        in_to_mm = fmt(LengthUnit::convert(
            LengthUnit::Inch,
            1.0,
            LengthUnit::Millimetre
        )),
        ft_to_in = fmt(LengthUnit::convert(LengthUnit::Foot, 1.0, LengthUnit::Inch)),
        mm25_to_in = fmt(LengthUnit::convert(
            LengthUnit::Millimetre,
            25.4,
            LengthUnit::Inch
        )),
        t_lin = fmt(DEFAULT_LINEAR_ABS),
        t_rel = fmt(DEFAULT_RELATIVE),
        t_ang = fmt(DEFAULT_ANGULAR_RAD),
        iu_um = fmt(um.linear_abs()),
        iu_m = fmt(m.linear_abs()),
        iu_in = fmt(inch.linear_abs()),
    );

    std::fs::write(&path, text).expect("write golden json");
    println!("wrote {}", path.display());
}
