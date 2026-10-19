#!/usr/bin/env python3
"""Independent golden-file generator for OEP-CGR STL export (P2-M2-T02).

Produces crates/oep-cgr/tests/golden/box-reference.bin.stl from the reference
tessellation (corpus/data/box.stl, CC0-1.0, P0-M4-T02 fixture: unit box,
8 vertices / 12 triangles) using ONLY the published STL binary spec:

    <80-byte header> <u32 LE triangle count>
    per facet: <3xf32 LE normal> <3 vertices: 9xf32 LE> <u16 LE attr>

The golden file is an INDEPENDENT reference: it is produced by this script
(not by the Rust writer under test), so the Rust integration test can assert
its output is byte-for-byte equal to the golden without the test author
simply echoing the implementation.

Conventions (must match crates/oep-cgr/src/stl.rs):
  * header: 80 zero bytes
  * facet normal: right-hand-rule unit normal = normalize(cross(v1-v0, v2-v0))
    (all box facets yield exact axis-aligned unit normals, so no FP drift);
  * attribute byte count: 0.

Usage:  ./gen_golden_box.py [outfile]
        (default outfile: ../tests/golden/box-reference.bin.stl)

Regenerating the golden is a deliberate act; the committed file is the
reference downsteam tests lock to.
"""
import struct
import sys
import pathlib

# Reference tessellation == corpus/data/box.stl. Vertex coords are the 8 unit
# box corners; the 12 triangles preserve the corpus facet order.
POSITIONS = [
    (1.0, 0.0, 0.0),  # V0
    (0.0, 1.0, 0.0),  # V1
    (0.0, 0.0, 0.0),  # V2
    (1.0, 1.0, 0.0),  # V3
    (0.0, 0.0, 1.0),  # V4
    (0.0, 1.0, 1.0),  # V5
    (1.0, 0.0, 1.0),  # V6
    (1.0, 1.0, 1.0),  # V7
]
TRIANGLES = [
    [0, 1, 2],
    [0, 3, 1],
    [4, 5, 6],
    [5, 7, 6],
    [2, 6, 4],
    [2, 0, 6],
    [3, 5, 7],
    [3, 1, 5],
    [5, 1, 2],
    [5, 2, 4],
    [0, 7, 3],
    [0, 6, 7],
]


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def cross(u, v):
    return (
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    )


def normalize(n):
    mag = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) ** 0.5
    if mag == 0.0:
        return (0.0, 0.0, 0.0)
    return (n[0] / mag, n[1] / mag, n[2] / mag)


def encode(pos, tri):
    """Encode one facet as 50 bytes: normal(3f32) + 3 vertices(9f32) + u16(0)."""
    v0, v1, v2 = (pos[i] for i in tri)
    normal = normalize(cross(sub(v1, v0), sub(v2, v0)))
    parts = [struct.pack("<3f", *normal)]
    for v in (v0, v1, v2):
        parts.append(struct.pack("<3f", *v))
    parts.append(struct.pack("<H", 0))
    return b"".join(parts)


def main():
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else (
        pathlib.Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.bin.stl"
    )
    header = b"\x00" * 80
    body = struct.pack("<I", len(TRIANGLES)) + b"".join(encode(POSITIONS, t) for t in TRIANGLES)
    blob = header + body
    assert len(blob) == 84 + 50 * 12 == 684, f"size mismatch: {len(blob)}"
    # Sanity: unique corners == 8, matching the reference tessellation.
    corners = {tuple(POSITIONS[i]) for t in TRIANGLES for i in t}
    assert len(corners) == 8, f"expected 8 unique corners, got {len(corners)}"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(blob)
    print(f"wrote {out} ({len(blob)} bytes, {len(TRIANGLES)} facets, {len(corners)} unique corners)")


if __name__ == "__main__":
    main()
