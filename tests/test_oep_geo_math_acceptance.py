#!/usr/bin/env python3
"""P6-M2-T01 acceptance test: independent (stdlib-only) verification of the
committed oep-geo math golden artifact.

TDD: written BEFORE oep_geo::math existed. When the crate is absent the golden
file is absent and every assertion fails (RED). When math.rs lands, the golden
file is produced BY the crate and this script independently re-computes the same
figures with Python's math module (closed-form) and cross-checks within 1e-9.

This is the "two-sided" verification pattern used by sibling cards (P3-M1/P3-M4):
the crate's own golden bytes must agree with an independent reference, so a
self-consistent (but wrong) math implementation cannot pass.
"""
import json
import math
import os
import sys

GOLDEN = os.path.join(
    os.path.dirname(__file__),
    "..", "crates", "oep-geo", "tests", "golden", "math-golden.json",
)

EPS = 1e-9


def close(a, b, what):
    if abs(a - b) > EPS:
        raise AssertionError(f"{what}: expected {b}, got {a} (diff {abs(a-b)})")


def main() -> int:
    if not os.path.exists(GOLDEN):
        print("FAIL: golden math-golden.json missing (crate not implemented / not generated)")
        return 1
    with open(GOLDEN) as f:
        g = json.load(f)
    assert g["format"] == "oep-geo-math-golden", "golden format tag"
    assert g["version"] == 1, "golden version"

    cases = g["cases"]
    ok = 0

    # --- Case 1: rotate (1,0,0) by 90deg about +Z. Closed form: (0,1,0) ---
    c = cases["rot_z90_point"]
    x, y, z = c["out"]
    close(x, 0.0, "rot_z90_point.x")
    close(y, 1.0, "rot_z90_point.y")
    close(z, 0.0, "rot_z90_point.z")
    ok += 1

    # --- Case 2: quaternion rotate (1,0,0) by 90deg about +Z = (0,1,0) ---
    c = cases["quat_z90_point"]
    close(c["out"][0], 0.0, "quat_z90_point.x")
    close(c["out"][1], 1.0, "quat_z90_point.y")
    close(c["out"][2], 0.0, "quat_z90_point.z")
    ok += 1

    # --- Case 3: translation composition T(1,2,3) then T(4,5,6) of origin = (5,7,9) ---
    c = cases["translation_compose"]
    close(c["out"][0], 5.0, "translation_compose.x")
    close(c["out"][1], 7.0, "translation_compose.y")
    close(c["out"][2], 9.0, "translation_compose.z")
    ok += 1

    # --- Case 4: independent quaternion rotation via closed-form Rodrigues ---
    # q = axis-angle(1,2,3 normalized, 0.7 rad); rotate v=(1,-2,0.5) with the
    # independent Python implementation below and compare to crate output.
    c = cases["quat_rotate_rodrigues"]
    axis = c["axis"]
    ang = c["angle"]
    p = c["point"]
    out = c["out"]

    n = math.sqrt(sum(a * a for a in axis))
    ux, uy, uz = (a / n for a in axis)
    s = math.sin(ang)
    cth = math.cos(ang)
    C = 1.0 - cth
    # rotation matrix via Rodrigues -> apply to p
    R = [
        [
            cth + ux * ux * C,
            ux * uy * C - uz * s,
            ux * uz * C + uy * s,
        ],
        [
            uy * ux * C + uz * s,
            cth + uy * uy * C,
            uy * uz * C - ux * s,
        ],
        [
            uz * ux * C - uy * s,
            uz * uy * C + ux * s,
            cth + uz * uz * C,
        ],
    ]
    px, py, pz = p
    rx = R[0][0] * px + R[0][1] * py + R[0][2] * pz
    ry = R[1][0] * px + R[1][1] * py + R[1][2] * pz
    rz = R[2][0] * px + R[2][1] * py + R[2][2] * pz
    close(out[0], rx, "quat_rotate_rodrigues.x")
    close(out[1], ry, "quat_rotate_rodrigues.y")
    close(out[2], rz, "quat_rotate_rodrigues.z")
    ok += 1

    # --- Case 5: inverse recovers original point: M^-1 applied to M*v == v ---
    c = cases["inverse_recovers"]
    v = c["point"]
    roundtrip = c["out"]
    close(roundtrip[0], v[0], "inverse_recovers.x")
    close(roundtrip[1], v[1], "inverse_recovers.y")
    close(roundtrip[2], v[2], "inverse_recovers.z")
    ok += 1

    # --- Case 6: Mat4 column-major [f64;16] consistency for a pure rotation ---
    # A pure-rotation Mat4 must have orthonormal columns: col0.col1 == 0, |col0|==1 etc.
    c = cases["rotation_mat4_array16"]
    arr = c["array16"]
    assert len(arr) == 16, "array16 has 16 entries"
    # column-major: element (row r, col k) is arr[k*4 + r]
    col = [[arr[k * 4 + r] for r in range(3)] for k in range(3)]  # top-left 3x3
    for k in range(3):
        nk = math.sqrt(sum(col[k][r] * col[k][r] for r in range(3)))
        close(nk, 1.0, f"rotation rotation col {k} unit")
    close(sum(col[0][r] * col[1][r] for r in range(3)), 0.0, "col0.col1 == 0")
    # pure rotation: translation part zero (last column xyz == 0)
    close(arr[12], 0.0, "translation.x == 0")
    close(arr[13], 0.0, "translation.y == 0")
    close(arr[14], 0.0, "translation.z == 0")
    ok += 1

    print(f"ACCEPTANCE: PASS - {ok} independent golden checks across {len(cases)} cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())
