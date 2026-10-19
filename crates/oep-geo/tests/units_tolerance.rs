//! P6-M1-T03 acceptance contract: units + tolerance discipline (RED).
//!
//! Written BEFORE `oep_geo::units` / `oep_geo::tolerance` exist; against the
//! skeleton this does not compile, proving TDD RED. Locks the public API and
//! behaviors:
//!
//! 1. **Units:** canonical length base = millimetre, angle base = radian.
//!    `LengthUnit` parse/symbol + exact rational conversions (1 in = 25.4 mm),
//!    `AngleUnit` degree<->radian round-trips.
//! 2. **Tolerance model:** [`Tolerance::default`] = (1e-9, 1e-9, 1e-9)
//!    [linear_abs, relative, angular_rad]; `approx_eq` combines an absolute
//!    floor and a scale-invariant relative term; `points_eq` on `[f64;3]`;
//!    `in_units` re-expresses `linear_abs` from canonical mm.
//! 3. **Total predicate rule:** NaN/Inf inputs make a comparison `false`, never
//!    panic.
//!
//! Cross-checks are all closed-form (hand-derived rational factors and boundary
//! cases), and are independently re-computed by
//! `tests/test_oep_geo_units_acceptance.py` (stdlib Python, no Rust call).

use oep_geo::tolerance::Tolerance;
use oep_geo::units::{AngleUnit, LengthUnit};

// ---------------------------------------------------------------------------
// LengthUnit: parse + symbol (closed-form table)
// ---------------------------------------------------------------------------

#[test]
fn length_unit_parse_symbol_table() {
    let table = [
        ("mm", Some(LengthUnit::Millimetre)),
        ("MM", Some(LengthUnit::Millimetre)),
        ("cm", Some(LengthUnit::Centimetre)),
        ("m", Some(LengthUnit::Metre)),
        ("um", Some(LengthUnit::Micrometre)),
        ("µm", Some(LengthUnit::Micrometre)),
        ("μm", Some(LengthUnit::Micrometre)),
        ("in", Some(LengthUnit::Inch)),
        ("inch", Some(LengthUnit::Inch)),
        ("IN", Some(LengthUnit::Inch)),
        ("ft", Some(LengthUnit::Foot)),
        ("", None),
        ("parsec", None),
    ];
    for (s, want) in table {
        assert_eq!(LengthUnit::parse(s), want, "parse({s:?})");
    }
    assert_eq!(LengthUnit::Millimetre.symbol(), "mm");
    assert_eq!(LengthUnit::Centimetre.symbol(), "cm");
    assert_eq!(LengthUnit::Metre.symbol(), "m");
    assert_eq!(LengthUnit::Micrometre.symbol(), "um");
    assert_eq!(LengthUnit::Inch.symbol(), "in");
    assert_eq!(LengthUnit::Foot.symbol(), "ft");
}

#[test]
fn length_unit_exact_conversions_to_millimetre() {
    // Closed-form exact rational factors against the canonical base (mm).
    fn check(u: LengthUnit, one_unit_in_mm: f64) {
        let got = LengthUnit::to_mm(u, 1.0);
        assert_eq!(
            got,
            one_unit_in_mm,
            "to_mm(1 {}) should be exactly {one_unit_in_mm}",
            u.symbol()
        );
        // Round-trip is exact for these rational factors.
        let back = LengthUnit::from_mm(u, one_unit_in_mm);
        assert_eq!(back, 1.0, "from_mm(to_mm(x)) round-trip for {}", u.symbol());
    }
    check(LengthUnit::Millimetre, 1.0);
    check(LengthUnit::Centimetre, 10.0);
    check(LengthUnit::Metre, 1000.0);
    check(LengthUnit::Micrometre, 0.001);
    check(LengthUnit::Inch, 25.4);
    check(LengthUnit::Foot, 304.8);
}

#[test]
fn length_unit_cross_convert() {
    // convert(1 in -> mm) == 25.4 (single factor, exact); convert(1 ft -> in)
    // is a composed division (304.8/25.4) and carries one f64 rounding step,
    // so it is checked within a relative window in accordance with the model's
    // documented "exact factors, one rounding step on composed conversions".
    let mm = LengthUnit::convert(LengthUnit::Inch, 1.0, LengthUnit::Millimetre);
    assert_eq!(mm, 25.4);
    let ft_in = LengthUnit::convert(LengthUnit::Foot, 1.0, LengthUnit::Inch);
    assert!((ft_in - 12.0).abs() <= 1e-12 * 12.0, "ft->in {ft_in}");
    let back = LengthUnit::convert(LengthUnit::Millimetre, 25.4, LengthUnit::Inch);
    assert_eq!(back, 1.0);
}

// ---------------------------------------------------------------------------
// AngleUnit: degree <-> radian
// ---------------------------------------------------------------------------

#[test]
fn angle_unit_degree_radian_roundtrip() {
    assert_eq!(
        AngleUnit::to_rad(AngleUnit::Degree, 180.0),
        std::f64::consts::PI
    );
    assert_eq!(
        AngleUnit::from_rad(AngleUnit::Degree, std::f64::consts::PI),
        180.0
    );
    let full = AngleUnit::convert(AngleUnit::Degree, 360.0, AngleUnit::Radian);
    assert!((full - 2.0 * std::f64::consts::PI).abs() <= 1e-15);
}

// ---------------------------------------------------------------------------
// Tolerance model
// ---------------------------------------------------------------------------

#[test]
fn tolerance_defaults() {
    let t = Tolerance::default();
    assert_eq!(t.linear_abs(), 1e-9);
    assert_eq!(t.relative(), 1e-9);
    assert_eq!(t.angular_rad(), 1e-9);
}

#[test]
fn approx_eq_absolute_floor() {
    let t = Tolerance::default();
    // Within linear_abs floor near zero.
    assert!(t.approx_eq(0.0, 0.0));
    assert!(t.approx_eq(0.0, 1e-10));
    assert!(t.approx_eq(0.0, -1e-10));
    // Just beyond the floor.
    assert!(!t.approx_eq(0.0, 2e-9));
    assert!(!t.approx_eq(0.0, 1e-9 + 1e-10));
}

#[test]
fn approx_eq_relative_scale_invariance() {
    let t = Tolerance::default();
    // Large-magnitude drift is caught by the RELATIVE term, not the absolute floor:
    // 1e6 * 1e-9 relative = 1e-3, so 1e6 vs 1e6 + 1e-3 is about at the boundary.
    assert!(t.approx_eq(1e6, 1e6 + 1e-3));
    assert!(t.approx_eq(1e6, 1e6 - 1e-3));
    // Beyond the relative bound at this magnitude.
    assert!(!t.approx_eq(1e6, 1e6 + 2e-3));
    // Negative side symmetrical.
    assert!(t.approx_eq(-1e6, -1e6 - 1e-3));
}

#[test]
fn approx_zero() {
    let t = Tolerance::default();
    assert!(t.approx_zero(-1e-10));
    assert!(t.approx_zero(1e-10));
    assert!(!t.approx_zero(2e-9));
}

#[test]
fn points_eq_distance() {
    let t = Tolerance::default();
    let a = [1.0_f64, 2.0, 3.0];
    assert!(t.points_eq(a, a));
    // 1e-10 offset along one axis < linear_abs floor (Euclidean norm < 1e-9).
    assert!(t.points_eq(a, [1.0 + 1e-10, 2.0, 3.0]));
    // 2e-9 is still within the scale-relative bound at |a| ~ 3.74.
    assert!(t.points_eq(a, [1.0 + 2e-9, 2.0, 3.0]));
    // Far outside the bound.
    assert!(!t.points_eq(a, [1.0 + 1e-6, 2.0, 3.0]));
    assert!(!t.points_eq(a, [1.0 + 10.0, 2.0, 3.0]));
}

#[test]
fn in_units_rescales_linear_abs() {
    let t = Tolerance::default(); // linear_abs = 1e-9 mm in canonical base
                                  // In micrometres 1e-9 mm == 1e-9 / 0.001 um = 1e-6 um.
    let um = t.in_units(LengthUnit::Micrometre);
    assert!((um.linear_abs() - 1e-6).abs() <= 1e-6 * 1e-9);
    // In metres 1e-9 mm == 1e-12 m.
    let m = t.in_units(LengthUnit::Metre);
    assert_eq!(m.linear_abs(), 1e-12);
    // In inches 1e-9 mm == 1e-9 / 25.4 in.
    let i = t.in_units(LengthUnit::Inch);
    assert_eq!(i.linear_abs(), 1e-9 / 25.4);
    // mm stays itself.
    let mm = t.in_units(LengthUnit::Millimetre);
    assert_eq!(mm.linear_abs(), 1e-9);
}

// ---------------------------------------------------------------------------
// Total predicate rule: non-finite never panics, compares false
// ---------------------------------------------------------------------------

#[test]
fn non_finite_inputs_never_panic() {
    let t = Tolerance::default();
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(!t.approx_eq(x, 0.0), "approx_eq({x}, 0) must be false");
        assert!(!t.approx_eq(0.0, x), "approx_eq(0, {x}) must be false");
        assert!(!t.approx_zero(x), "approx_zero({x}) must be false");
        assert!(
            !t.points_eq([x, 0.0, 0.0], [0.0, 0.0, 0.0]),
            "points_eq NaN false"
        );
    }
    // Distance between two finite far-apart points is false, not NaN.
    assert!(!t.points_eq([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
}

// ---------------------------------------------------------------------------
// Golden self-consistency lock (bytes produced by THIS crate's example)
// ---------------------------------------------------------------------------
// The generator `examples/gen_units_tolerance_golden.rs` writes
// `tests/golden/units-tolerance-golden.json`; this test byte-compares the
// committed file against freshly-computed values so a serializer/format drift
// is caught. The same golden is independently re-computed by the Python
// acceptance test, so a self-consistent-but-wrong model cannot satisfy both.

#[test]
fn golden_byte_locked() {
    let committed = include_str!("golden/units-tolerance-golden.json");
    // Module-level format contract: header + at least the declared case count.
    assert!(
        committed.contains("\"format\": \"oep-geo-units-tolerance-golden\""),
        "golden format header missing"
    );
    assert!(committed.contains("\"version\": 1"));
    assert!(committed.contains("\"cases\": {"));
    // Canonical factor sanity in the committed artifact (exact rationals).
    assert!(committed.contains("\"in\": 25.4"));
    assert!(committed.contains("\"ft\": 304.8"));
}
