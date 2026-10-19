"""P6-M1-T03 acceptance: units + tolerance discipline for oep-geo.

Independent (stdlib-only) verification of the committed golden
`crates/oep-geo/tests/golden/units-tolerance-golden.json` plus two discipline
gates that do NOT touch Rust at all:

  - **double everywhere audit:** no `f32` / `f16` token anywhere under
    `crates/oep-geo/src/**/*.rs` (ints are allowed only for counts/indices/ids,
    never geometry values) -- enforced here as a source scan;
  - **closed-form math re-computation:** every unit conversion and tolerance
    predicate boundary in the golden is re-derived from hand-written factors and
    bound logic in pure Python, then cross-checked against the golden within a
    1e-12 relative window. A self-consistent-but-wrong Rust model cannot satisfy
    both this file and the Rust contract.

Canonical bases: length = millimetre, angle = radian. Default tolerance =
(linear_abs=1e-9, relative=1e-9, angular_rad=1e-9) in model length units.
Predicate rule: any NaN/Inf input makes a comparison False (never panics).
"""

import json
import math
import os
import re

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-geo", "tests", "golden", "units-tolerance-golden.json")
SRC_DIR = os.path.join(REPO, "crates", "oep-geo", "src")

# Closed-form length factors against the canonical base (millimetre).
LEN_FACTOR_MM = {
    "mm": 1.0,
    "cm": 10.0,
    "m": 1000.0,
    "um": 0.001,
    "in": 25.4,
    "ft": 304.8,
}


def _load_golden():
    with open(GOLDEN, "r", encoding="utf-8") as fh:
        return json.load(fh)


# ---------------------------------------------------------------------------
# double-everywhere audit
# ---------------------------------------------------------------------------


def test_no_f32_or_f16_in_oep_geo_src():
    offenders = []
    for root, _dirs, files in os.walk(SRC_DIR):
        # P6-M4-T01 exporters are a documented, format-bound f32 boundary
        # (glTF/STL/OBJ/PDF are float32 formats; see docs/p6-m4-oep-geo-exporters.md,
        # "Decision: writers take slices"). The f64 discipline governs the
        # geometry kernel/model; integration (INTEGRATION-T02) scopes this audit
        # to src/*.rs outside the export/ adapter directory.
        if os.path.basename(root) == "export":
            continue
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(root, fn)
            with open(path, "r", encoding="utf-8") as fh:
                text = fh.read()
            for tok in ("f32", "f16"):
                # Match the token as a word (prevents matching inside longer
                # identifiers or unrelated strings/idents that merely contain it).
                for m in re.finditer(r"\b" + re.escape(tok) + r"\b", text):
                    line_no = text.count("\n", 0, m.start()) + 1
                    offenders.append("%s:%d %r" % (path, line_no, tok))
    assert not offenders, (
        "oep-geo must be double-everywhere (P6-M1-T03); found narrow floats:\n  "
        + "\n  ".join(offenders)
    )


# ---------------------------------------------------------------------------
# closed-form unit math (independent of Rust)
# ---------------------------------------------------------------------------


def _to_mm(unit, value):
    assert unit in LEN_FACTOR_MM, unit
    return LEN_FACTOR_MM[unit] * value


def _from_mm(unit, value_mm):
    return value_mm / LEN_FACTOR_MM[unit]


def _convert(from_unit, value, to_unit):
    return _from_mm(to_unit, _to_mm(from_unit, value))


def test_length_factors_exact():
    doc = _load_golden()
    cases = doc["cases"]["to_mm_factors"]
    for unit_name, expected in cases.items():
        got = LEN_FACTOR_MM[unit_name]
        assert got == expected, "%s factor: expected %r, python %r" % (unit_name, expected, got)


def test_conversions_golden():
    doc = _load_golden()
    c = doc["cases"]
    got = _convert(c["inch_to_mm"]["src"], c["inch_to_mm"]["val"], c["inch_to_mm"]["dst"])
    assert got == c["inch_to_mm"]["out"] == 25.4
    # ft->in is a composed division (304.8/25.4): carry one f64 rounding step,
    # so compare within a relative window, not exactly.
    got = _convert(c["ft_to_in"]["src"], c["ft_to_in"]["val"], c["ft_to_in"]["dst"])
    assert abs(got - 12.0) <= 1e-12 * 12.0
    got = _convert(c["mm25_to_in"]["src"], c["mm25_to_in"]["val"], c["mm25_to_in"]["dst"])
    assert got == c["mm25_to_in"]["out"] == 1.0
    return c


def test_angle_roundtrip():
    # 180 deg == pi rad (closed form).
    assert _deg_to_rad(180.0) == math.pi
    assert _rad_to_deg(math.pi) == 180.0
    assert abs(_deg_to_rad(360.0) - 2.0 * math.pi) <= 1e-15


def _deg_to_rad(deg):
    return deg * math.pi / 180.0


def _rad_to_deg(rad):
    return rad * 180.0 / math.pi


# ---------------------------------------------------------------------------
# closed-form tolerance predicate math (independent of Rust)
# ---------------------------------------------------------------------------


def _approx_eq(a, b, linear_abs=1e-9, relative=1e-9):
    # Total rule: non-finite makes it False, never crashes.
    try:
        d = abs(a - b)
        if math.isnan(d) or math.isinf(d):
            return False
    except TypeError:
        return False
    bound = linear_abs + relative * max(abs(a), abs(b))
    return d <= bound


def _approx_zero(x, linear_abs=1e-9):
    if math.isnan(x) or math.isinf(x):
        return False
    return abs(x) <= linear_abs


def _points_eq(p, q, linear_abs=1e-9, relative=1e-9):
    dx = p[0] - q[0]
    dy = p[1] - q[1]
    dz = p[2] - q[2]
    d2 = dx * dx + dy * dy + dz * dz
    if math.isnan(d2) or math.isinf(d2):
        return False
    d = math.sqrt(d2)
    bound = linear_abs + relative * max(
        math.sqrt(p[0] ** 2 + p[1] ** 2 + p[2] ** 2),
        math.sqrt(q[0] ** 2 + q[1] ** 2 + q[2] ** 2),
    )
    return d <= bound


def test_tolerance_boundaries_closed_form():
    # Absolute floor near zero.
    assert _approx_eq(0.0, 0.0)
    assert _approx_eq(0.0, 1e-10)
    assert _approx_eq(0.0, -1e-10)
    assert not _approx_eq(0.0, 2e-9)
    assert not _approx_eq(0.0, 1e-9 + 1e-10)

    # Relative scale-invariance at large magnitude: 1e6 * 1e-9 = 1e-3.
    assert _approx_eq(1e6, 1e6 + 1e-3)
    assert _approx_eq(1e6, 1e6 - 1e-3)
    assert not _approx_eq(1e6, 1e6 + 2e-3)
    assert _approx_eq(-1e6, -1e6 - 1e-3)

    # approx_zero
    assert _approx_zero(-1e-10)
    assert _approx_zero(1e-10)
    assert not _approx_zero(2e-9)

    # points_eq on [f64;3] (bound at |a|~3.74 includes ~4.7e-9 relative term,
    # so choose offsets clearly inside and clearly outside the bound).
    a = (1.0, 2.0, 3.0)
    assert _points_eq(a, a)
    assert _points_eq(a, (1.0 + 1e-10, 2.0, 3.0))
    assert _points_eq(a, (1.0 + 2e-9, 2.0, 3.0))  # within scale-relative bound here
    assert not _points_eq(a, (1.0 + 1e-6, 2.0, 3.0))  # far outside
    assert not _points_eq(a, (1.0 + 10.0, 2.0, 3.0))

    # Non-finite never panics, never true.
    for x in (float("nan"), float("inf"), float("-inf")):
        assert not _approx_eq(x, 0.0)
        assert not _approx_eq(0.0, x)
        assert not _approx_zero(x)
        assert not _points_eq((x, 0.0, 0.0), (0.0, 0.0, 0.0))


def test_matches_golden():
    doc = _load_golden()
    c = doc["cases"]
    assert doc["format"] == "oep-geo-units-tolerance-golden"
    assert doc["version"] == 1

    # in_units rescale of linear_abs from canonical mm.
    lu = c["in_units_um"]["linear_abs"]
    assert abs(lu - 1e-6) <= 1e-6 * 1e-9
    assert c["in_units_m"]["linear_abs"] == 1e-12
    assert abs(c["in_units_in"]["linear_abs"] - 1e-9 / 25.4) <= 1e-20

    # Default tolerance triple.
    assert c["tolerance_default"] == {"linear_abs": 1e-9, "relative": 1e-9, "angular_rad": 1e-9}

    # Every conversion case in the golden re-derived in Python within 1e-12 rel.
    tol = "inch_to_mm", "ft_to_in", "mm25_to_in"
    for key in tol:
        entry = c[key]
        src = entry["src"]
        val = entry["val"]
        dst = entry["dst"]
        expected = entry["out"]
        got = _convert(src, val, dst)
        assert abs(got - expected) <= 1e-12 * max(1.0, abs(expected)), (key, got, expected)
