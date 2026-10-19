"""P6-M2-T02 acceptance: NURBS / B-spline curve + surface evaluation.

Independent (stdlib-only) verification of the committed NURBS goldens at
`crates/oep-geo/tests/golden/nurbs-circle.oepgeo.txt` and
`.../nurbs-cylinder.oepgeo.txt`. It implements its own Cox-de Boor basis and
rational evaluation in plain Python (no numpy, no Rust) and re-derives the
expected values from the ANALYTIC geometry the goldens encode, then checks the
committed spots against those analytic values.

Covered:
  - golden format sanity: header line, kind curve|surface, degree, knot
    cardinality (len(knots) == n + degree + 1), non-decreasing knots, weights
    strictly positive, control/weight counts agree
  - the CIRCLE golden is an exact rational B-spline unit circle:
      * |P(u)| == 1 to fp tolerance across a dense parameter sweep
      * committed spot points match the independent Python evaluation
      * spot at u=0.125 lies at 45 degrees: P == (sqrt2/2, sqrt2/2, 0)
      * first-derivative tangent is orthogonal to the radius (dot == 0)
      * P(0) == P(1) == (1,0,0), P(0.25) == (0,1,0), etc (clamped endpoints /
        quarter marks interpolate the on-circle control points)
  - the CYLINDER golden is an exact ruled rational cylinder radius 1, H = 2:
      * |P_xy(u,v)| == 1 at sampled (u,v)
      * z == H*v (ruling is linear in v)
      * committed spots match the independent Python evaluation
  - partition of unity: sum of nonzero basis == 1 on the interior

Written first (TDD): the committed goldens and these analytic checks are the
"real material" gate; the Rust implementation is verified independently by
crates/oep-geo/tests/nurbs_eval.rs against the same goldens.
"""

import math
import os

from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
GOLDEN_DIR = REPO / "crates" / "oep-geo" / "tests" / "golden"
CIRCLE = GOLDEN_DIR / "nurbs-circle.oepgeo.txt"
CYLINDER = GOLDEN_DIR / "nurbs-cylinder.oepgeo.txt"

# Tolerance: goldens are exact to fp; this absorbs pure rounding.
TOL = 1e-12


# ---------------------------------------------------------------------------
# Independent math (Cox-de Boor, NURBS Book conventions)
# ---------------------------------------------------------------------------

def find_span(p, knots, u):
    n = len(knots) - p - 1
    if u >= knots[n]:
        return n - 1
    if u <= knots[p]:
        return p
    lo, hi = p, n
    mid = (lo + hi) // 2
    while u < knots[mid] or u >= knots[mid + 1]:
        if u < knots[mid]:
            hi = mid
        else:
            lo = mid
        mid = (lo + hi) // 2
    return mid


def basis_funcs(p, knots, span, u):
    left = [0.0] * (p + 1)
    right = [0.0] * (p + 1)
    n = [0.0] * (p + 1)
    n[0] = 1.0
    for j in range(1, p + 1):
        left[j] = u - knots[span + 1 - j]
        right[j] = knots[span + j] - u
        saved = 0.0
        for r in range(j):
            den = right[r + 1] + left[j - r]
            temp = n[r] / den if abs(den) > 1e-15 else 0.0
            n[r] = saved + right[r + 1] * temp
            saved = left[j - r] * temp
        n[j] = saved
    return n


def curve_point(p, knots, ctrl, weights, u):
    span = find_span(p, knots, u)
    n = basis_funcs(p, knots, span, u)
    base = span - p
    num = [0.0, 0.0, 0.0]
    den = 0.0
    for j in range(p + 1):
        w = weights[base + j]
        m = n[j] * w
        num[0] += ctrl[base + j][0] * m
        num[1] += ctrl[base + j][1] * m
        num[2] += ctrl[base + j][2] * m
        den += m
    assert den > 0.0
    return [num[0] / den, num[1] / den, num[2] / den]


def surface_point(du, dv, ku, kv, ctrl, weights, u, v):
    su = find_span(du, ku, u)
    sv = find_span(dv, kv, v)
    nu_ = basis_funcs(du, ku, su, u)
    nv_ = basis_funcs(dv, kv, sv, v)
    bu = su - du
    bv = sv - dv
    num = [0.0, 0.0, 0.0]
    den = 0.0
    for jv in range(dv + 1):
        for iu in range(du + 1):
            w = weights[bv + jv][bu + iu]
            m = nu_[iu] * nv_[jv] * w
            c = ctrl[bv + jv][bu + iu]
            num[0] += c[0] * m
            num[1] += c[1] * m
            num[2] += c[2] * m
            den += m
    assert den > 0.0
    return [num[0] / den, num[1] / den, num[2] / den]


# ---------------------------------------------------------------------------
# Golden parsing (same OEP-NURBS-GOLDEN v1 whitespace format)
# ---------------------------------------------------------------------------

def parse_curve(blob):
    degree = knots = None
    ctrl = []
    weights = []
    spots = []
    for raw in blob.splitlines():
        t = raw.strip()
        if not t or t.startswith("#") or t.startswith("OEP-NURBS"):
            continue
        toks = t.split()
        tag = toks[0]
        if tag == "degree":
            degree = int(toks[1])
        elif tag == "kind":
            assert toks[1] == "curve"
        elif tag == "knots":
            knots = [float(x) for x in toks[1:]]
        elif tag == "ncontrol":
            assert int(toks[1]) == len(ctrl) or ctrl == []  # forwards declaration
        elif tag == "ctrl":
            v = [float(x) for x in toks[1:]]
            assert len(v) % 3 == 0
            ctrl = [v[i:i + 3] for i in range(0, len(v), 3)]
        elif tag == "weights":
            weights = [float(x) for x in toks[1:]]
        elif tag == "spot":
            spots.append((float(toks[1]), [float(x) for x in toks[2:5]]))
    return degree, knots, ctrl, weights, spots


def parse_surface(blob):
    du = dv = None
    ku = kv = []
    ctrl = []
    weights = []
    spots = []
    for raw in blob.splitlines():
        t = raw.strip()
        if not t or t.startswith("#") or t.startswith("OEP-NURBS"):
            continue
        toks = t.split()
        tag = toks[0]
        if tag == "kind":
            assert toks[1] == "surface"
        elif tag == "degree_u":
            du = int(toks[1])
        elif tag == "degree_v":
            dv = int(toks[1])
        elif tag == "knots_u":
            ku = [float(x) for x in toks[1:]]
        elif tag == "knots_v":
            kv = [float(x) for x in toks[1:]]
        elif tag in ("nu", "nv"):
            pass
        elif tag.startswith("ctrl_v"):
            v = [float(x) for x in toks[1:]]
            assert len(v) % 3 == 0
            ctrl.append([v[i:i + 3] for i in range(0, len(v), 3)])
        elif tag.startswith("weights_v"):
            weights.append([float(x) for x in toks[1:]])
        elif tag == "spot":
            spots.append((float(toks[1]), float(toks[2]), [float(x) for x in toks[3:6]]))
    return du, dv, ku, kv, ctrl, weights, spots


def check_structural(degree, knots, npoints, weights):
    assert degree is not None and degree >= 1
    assert len(knots) == npoints + degree + 1, f"knot cardinality {len(knots)} != {npoints + degree + 1}"
    assert all(b >= a for a, b in zip(knots, knots[1:])), "knots not non-decreasing"
    assert len(weights) == npoints
    assert all(w > 0.0 for w in weights), "weight not strictly positive"
    assert all(math.isfinite(k) for k in knots)


# ---------------------------------------------------------------------------
# Acceptance tests
# ---------------------------------------------------------------------------

def test_circle_golden_structural_and_analytic():
    blob = CIRCLE.read_text(encoding="utf-8")
    degree, knots, ctrl, weights, spots = parse_curve(blob)
    check_structural(degree, knots, len(ctrl), weights)

    # Dense radius sweep: an exact unit circle.
    for i in range(0, 1001):
        u = i / 1000.0
        p = curve_point(degree, knots, ctrl, weights, u)
        r = math.hypot(p[0], p[1])
        assert abs(r - 1.0) <= TOL, f"radius {r} at u={u}"

    # Committed spots match independent evaluation exactly (fp).
    for u, want in spots:
        got = curve_point(degree, knots, ctrl, weights, u)
        assert all(abs(g - w) <= TOL for g, w in zip(got, want)), f"spot {u}: {got} != {want}"

    # Analytic quarter marks / clamped endpoints.
    marks = {
        0.0: [1.0, 0.0, 0.0],
        0.25: [0.0, 1.0, 0.0],
        0.5: [-1.0, 0.0, 0.0],
        0.75: [0.0, -1.0, 0.0],
        1.0: [1.0, 0.0, 0.0],
    }
    for u, want in marks.items():
        got = curve_point(degree, knots, ctrl, weights, u)
        assert all(abs(g - w) <= TOL for g, w in zip(got, want)), f"mark u={u}"

    # u=0.125 is 45 degrees -> (sqrt2/2, sqrt2/2, 0).
    p45 = curve_point(degree, knots, ctrl, weights, 0.125)
    s = math.sqrt(2) / 2
    assert all(abs(g - w) <= 1e-9 for g, w in zip(p45, [s, s, 0.0])), f"45deg: {p45}"

    # Partition of unity on the interior.
    for i in range(1, 100):
        u = i / 100.0
        span = find_span(degree, knots, u)
        n = basis_funcs(degree, knots, span, u)
        assert abs(sum(n) - 1.0) <= 1e-9, f"partition of unity at u={u}"


def test_circle_tangent_orthogonal_to_radius():
    blob = CIRCLE.read_text(encoding="utf-8")
    degree, knots, ctrl, weights, _ = parse_curve(blob)
    # Numerical central-difference tangent.
    h = 1e-6
    for i in range(1, 100):
        u = i / 100.0
        p = curve_point(degree, knots, ctrl, weights, u)
        fp = curve_point(degree, knots, ctrl, weights, u + h)
        fm = curve_point(degree, knots, ctrl, weights, u - h)
        t = [(a - b) / (2 * h) for a, b in zip(fp, fm)]
        dot = p[0] * t[0] + p[1] * t[1] + p[2] * t[2]
        assert abs(dot) <= 1e-6, f"tangent not orthogonal to radius at u={u}: dot={dot}"


def test_cylinder_golden_structural_and_analytic():
    blob = CYLINDER.read_text(encoding="utf-8")
    du, dv, ku, kv, ctrl, weights, spots = parse_surface(blob)
    nv = len(ctrl)
    nu = len(ctrl[0])
    check_structural(du, ku, nu, weights[0])
    check_structural(dv, kv, nv, [w[0] for w in weights])
    assert du == 2 and dv == 1

    H = 2.0  # golden contract: top row at z=H, ruling linear in v.
    for i in range(0, 11):
        u = i / 10.0
        for j in range(0, 7):
            v = j / 6.0
            p = surface_point(du, dv, ku, kv, ctrl, weights, u, v)
            r = math.hypot(p[0], p[1])
            assert abs(r - 1.0) <= TOL, f"cylinder radius {r} at ({u},{v})"
            assert abs(p[2] - H * v) <= TOL, f"cylinder z {p[2]} vs {H*v} at ({u},{v})"

    for u, v, want in spots:
        got = surface_point(du, dv, ku, kv, ctrl, weights, u, v)
        assert all(abs(g - w) <= TOL for g, w in zip(got, want)), f"surface spot ({u},{v}): {got} != {want}"


def test_goldens_are_committed_and_readable():
    assert CIRCLE.exists(), "circle golden missing"
    assert CYLINDER.exists(), "cylinder golden missing"
    assert ("OEP-NURBS-GOLDEN 1" in CIRCLE.read_text(encoding="utf-8").splitlines()[0])
    assert ("OEP-NURBS-GOLDEN 1" in CYLINDER.read_text(encoding="utf-8").splitlines()[0])


if __name__ == "__main__":
    for fn in [test_goldens_are_committed_and_readable,
               test_circle_golden_structural_and_analytic,
               test_circle_tangent_orthogonal_to_radius,
               test_cylinder_golden_structural_and_analytic]:
        fn()
        print(f"PASS {fn.__name__}")
    print("P6-M2-T02 acceptance: all independent Python checks passed.")
