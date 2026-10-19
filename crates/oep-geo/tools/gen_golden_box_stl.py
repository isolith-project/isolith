#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> binary STL (P6-M4-T01).

Produces crates/oep-geo/tests/golden/box-reference.bin.stl from the reference
tessellation (corpus/data/box.stl, CC0-1.0, P0-M4-T02 fixture: unit box,
8 vertices / 12 triangles) using ONLY the published STL binary spec:

    <80-byte header> <u32 LE triangle count>
    per facet: <3xf32 LE normal> <3 vertices: 9xf32 LE> <u16 LE attr>

The golden file is an INDEPENDENT reference: produced by this script (not by
the Rust writer under test), so the Rust integration test asserting byte
equality proves conformance to the spec, not to an echo of the writer.

Conventions (must match crates/oep-geo/src/export/stl.rs):
  * header: 80 zero bytes;
  * facet normal: right-hand-rule unit normal = normalize(cross(v1-v0, v2-v0))
    (all box facets yield exact axis-aligned unit normals, so no FP drift);
  * attribute byte count: 0.

Usage:  ./gen_golden_box_stl.py [outfile]
"""
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES  # noqa: E402


def cross(u, v):
    return (
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    )


def mag(v):
    return (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) ** 0.5


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.bin.stl"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    blob = bytearray(80)  # header
    blob += struct.pack("<I", len(TRIANGLES))
    for (a, b, c) in TRIANGLES:
        p0 = POSITIONS[a]
        p1 = POSITIONS[b]
        p2 = POSITIONS[c]
        u = tuple(p1[i] - p0[i] for i in range(3))
        v = tuple(p2[i] - p0[i] for i in range(3))
        n = cross(u, v)
        m = mag(n)
        n = tuple(c / m for c in n)
        b_str = struct.pack("<3f", *n)
        for p in (p0, p1, p2):
            b_str += struct.pack("<3f", *p)
        b_str += struct.pack("<H", 0)
        blob += b_str

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(bytes(blob))
    print(f"wrote {out} ({len(blob)} bytes, {len(TRIANGLES)} facets)")


if __name__ == "__main__":
    main()
