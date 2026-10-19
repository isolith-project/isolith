#!/usr/bin/env python3
"""P6-M2-T03 acceptance test: independent (stdlib-only) verification of the
committed oep-geo geometric-predicates golden artifact.

TDD: written BEFORE `oep_geo::predicates` existed. When the crate is absent the
golden file is absent and every assertion fails (RED). When the predicates land,
the golden file is produced BY the crate and this script independently re-computes
the same figures with Python's math module (closed-form independent formulas) and
cross-checks within epsilon.

This is the "two-sided" verification pattern used by sibling cards (P6-M2-T01,
P3-M1/P3-M4): the crate's own golden bytes must agree with an independent
reference, so a self-consistent (but wrong) predicate implementation cannot pass.

The reference formulas here deliberately differ from the crate's internals:
point->line/segment distance is derived from the cross-product area formula
(|cross(b-a, p-a)| / |b-a|) and clamped projection, plane distance from the
normalized dot, triangle containment from half-plane sign tests against the
triangle normal, AABB/sphere containment from coordinate/distance bounds.
"""

import json
import math
import os
import sys

GOLDEN = os.path.join(
    os.path.dirname(__file__),
    "..", "crates", "oep-geo", "tests", "golden", "predicates-golden.json",
)

EPS = 1e-6  # golden values carry fp rounding; independent formulas differ slightly


def close(a, b, what):
    if abs(a - b) > EPS:
        raise AssertionError(f"{what}: expected {b}, got {a} (diff {abs(a - b)})")


def vsub(a, b):
    return [a[i] - b[i] for i in range(3)]


def vcross(a, b):
    return [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]


def vdot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def vnorm(a):
    return math.sqrt(vdot(a, a))


def dist_point_point(a, b):
    return vnorm(vsub(a, b))


def dist_point_line(p, a, b):
    return vnorm(vcross(vsub(b, a), vsub(p, a))) / vnorm(vsub(b, a))


def dist_point_segment(p, a, b):
    ab = vsub(b, a)
    d2 = vdot(ab, ab)
    if d2 < 1e-18:
        return vnorm(vsub(p, a))
    t = max(0.0, min(1.0, vdot(vsub(p, a), ab) / d2))
    return vnorm(vsub(p, [a[i] + t * ab[i] for i in range(3)]))


def signed_dist_plane(p, q, n):
    nn = vnorm(n)
    u = [n[i] / nn for i in range(3)]
    return vdot(vsub(p, q), u)


def dist_point_plane(p, q, n):
    return abs(signed_dist_plane(p, q, n))


def in_sphere(p, c, r, tol):
    return dist_point_point(p, c) <= r + tol


def on_sphere(p, c, r, tol):
    return abs(dist_point_point(p, c) - r) <= tol


def in_aabb(p, lo, hi, tol):
    return all(lo[i] - tol <= p[i] <= hi[i] + tol for i in range(3))


def in_triangle_plane(p, a, b, c, tol):
    ab = vsub(b, a)
    ac = vsub(c, a)
    n = vcross(ab, ac)
    nn = vnorm(n)
    if nn < 1e-12:
        return False
    # coplanarity gate
    if abs(vdot(vsub(p, a), n)) / nn > tol:
        return False
    # half-plane tests with consistently-oriented edge cross products
    for e1, e2 in ((a, b), (b, c), (c, a)):
        e = vsub(e2, e1)
        v = vsub(p, e1)
        if vdot(vcross(e, v), n) < -tol:
            return False
    return True


def on_plane(p, q, n, tol):
    return abs(signed_dist_plane(p, q, n)) <= tol


def on_line(p, a, b, tol):
    return dist_point_line(p, a, b) <= tol


def on_segment(p, a, b, tol):
    return dist_point_segment(p, a, b) <= tol


def main() -> int:
    if not os.path.exists(GOLDEN):
        print("FAIL: golden predicates-golden.json missing (crate not implemented / not generated)")
        return 1
    with open(GOLDEN) as f:
        g = json.load(f)
    assert g["format"] == "oep-geo-predicates-golden", "golden format tag"
    assert g["version"] == 1, "golden version"

    cases = {c["id"]: c["value"] for c in g["cases"]}
    ok = 0

    def val(cid, expected, norm=False):
        """Compare crate value (0/1 for booleans) to an independent expectation."""
        nonlocal ok
        got = cases[cid]
        if expected in (True, False):
            assert int(expected) == got, f"{cid}: expected bool {expected}, got {got}"
        else:
            close(got, expected, cid)
        ok += 1

    # --- distances ---
    val("dist_point_point", dist_point_point([1, 2, 3], [4, 6, 3]))  # 5
    val("sdf_above_z0", signed_dist_plane([3, 4, 5], [0, 0, 0], [0, 0, 1]))  # +5
    val("sdf_below_z0", signed_dist_plane([0, 0, -2], [0, 0, 0], [0, 0, 1]))  # -2
    val("dist_plane_z0", dist_point_plane([1, 2, 7], [0, 0, 0], [0, 0, 1]))  # 7
    # plane x+y+z=1 (normal (1,1,1), point (1,0,0)); origin distance 1/sqrt(3)
    val("dist_plane_xyz", 1.0 / math.sqrt(3))
    val("dist_line_xaxis", dist_point_line([5, 12, 0], [0, 0, 0], [4, 0, 0]))  # 12
    val("dist_seg_perp", dist_point_segment([2, 3, 0], [0, 0, 0], [4, 0, 0]))  # 3
    val("dist_seg_clamp_a", dist_point_segment([-1, 0, 0], [0, 0, 0], [4, 0, 0]))  # 1
    val("dist_seg_clamp_b", dist_point_segment([6, 0, 0], [0, 0, 0], [4, 0, 0]))  # 2

    # --- on/off tolerance ---
    val("on_plane_in", on_plane([1, 2, 1e-10], [0, 0, 0], [0, 0, 1], 1e-9))  # True
    val("on_plane_out", on_plane([1, 2, 0.5], [0, 0, 0], [0, 0, 1], 1e-9))  # False
    val("on_line_yes", on_line([2, 0, 0], [0, 0, 0], [4, 0, 0], 1e-9))  # True
    val("on_line_no", on_line([2, 3, 0], [0, 0, 0], [4, 0, 0], 1e-9))  # False
    val("on_seg_yes", on_segment([3.5, 0, 0], [0, 0, 0], [4, 0, 0], 1e-9))  # True
    val("on_seg_no", on_segment([4.5, 0, 0], [0, 0, 0], [4, 0, 0], 1e-9))  # False
    h = math.sqrt(2) / 2
    val("on_sphere_yes", on_sphere([h, h, 0], [0, 0, 0], 1.0, 1e-9))  # True
    val("on_sphere_no", on_sphere([2, 0, 0], [0, 0, 0], 1.0, 1e-9))  # False

    # --- containment ---
    val("in_sphere_yes", in_sphere([0.5, 0.5, 0.5], [0, 0, 0], 1.0, 1e-9))  # True
    val("in_box_center", in_aabb([0.5, 0.5, 0.5], [0, 0, 0], [1, 1, 1], 1e-9))  # True
    val("in_box_corner", in_aabb([1, 1, 1], [0, 0, 0], [1, 1, 1], 0.0))  # True (closed)
    val("in_box_out", in_aabb([1.5, 0.5, 0.5], [0, 0, 0], [1, 1, 1], 1e-9))  # False
    ta, tb, tc = [0, 0, 0], [1, 0, 0], [0, 1, 0]
    val("in_tri_centroid", in_triangle_plane([1 / 3, 1 / 3, 0], ta, tb, tc, 1e-9))  # True
    val("in_tri_edge", in_triangle_plane([0.5, 0.5, 0], ta, tb, tc, 1e-9))  # True (boundary)
    val("in_tri_out", in_triangle_plane([2, 2, 0], ta, tb, tc, 1e-9))  # False
    val("in_tri_lifted", in_triangle_plane([1 / 3, 1 / 3, 1], ta, tb, tc, 1e-9))  # False (coplanarity)

    print(f"ACCEPTANCE: PASS - {ok} independent golden checks across {len(cases)} cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())
