#!/usr/bin/env python3
"""Independent golden-file generator for OEP-CGR glTF/GLB export (P2-M2-T03).

Produces crates/oep-cgr/tests/golden/box-reference.glb from the reference
tessellation (corpus/data/box.stl, CC0-1.0, P0-M4-T02 fixture: unit box,
8 vertices / 12 triangles) using ONLY the published Khronos glTF 2.0 spec:

    GLB = 12-byte header
        magic    u32 LE  == 0x46546C67  ("glTF")
        version  u32 LE  == 2
        length   u32 LE  == total file size
      + JSON chunk: u32 chunkLength, u32 chunkType == 0x4E4F534A ("JSON"),
        JSON bytes space-padded (0x20) to a multiple of 4
      + BIN chunk:  u32 chunkLength, u32 chunkType == 0x004E4942 ("BIN\\0"),
        binary buffer zero-padded to a multiple of 4

The JSON document describes one buffer (byteLength 168: 8 VEC3 f32 positions
= 96 bytes, then 36 u16 index values = 72 bytes), two bufferViews
(targets 34962 ARRAY_BUFFER / 34963 ELEMENT_ARRAY_BUFFER), two accessors
(POSITION FLOAT VEC3 count 8 with min/max, indices UNSIGNED_SHORT count 36),
one mesh primitive (mode 4 TRIANGLES), one node, one scene.

The golden file is an INDEPENDENT reference: produced by this script (not by
the Rust writer under test), so `tests/golden_gltf.rs` can assert the
writer's output is byte-for-byte equal to the golden without the test author
simply echoing the implementation.

Conventions (must match crates/oep-cgr/src/gltf.rs):
  * JSON is emitted COMPACT (no whitespace) with the exact key order below;
  * integral f32 values render as integers ("0", "1") — both encoders agree;
  * JSON chunk is padded with ASCII spaces to a multiple of 4;
  * BIN is positions (LE f32 x3) then indices (LE u16);
  * empty meshes still emit a (0-length) BIN chunk after the JSON chunk.

Usage:  ./gen_golden_box_glb.py [outfile]
        (default outfile: ../tests/golden/box-reference.glb)

Regenerating the golden is a deliberate act; the committed file is the
reference downstream tests lock to.
"""
import struct
import sys
import pathlib

# Reference tessellation == corpus/data/box.stl (same as gen_golden_box.py).
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

TARGET_ARRAY_BUFFER = 34962
TARGET_ELEMENT_ARRAY_BUFFER = 34963
COMPONENT_FLOAT = 5126
COMPONENT_USHORT = 5123
MODE_TRIANGLES = 4


def fmt_f32(x):
    """Render an f32 as the Rust writer does: integral -> integer literal."""
    x = float(x)
    if x == int(x):
        return str(int(x))
    return repr(x)


def build_json(pos, tri):
    """Compact glTF 2.0 JSON with the fixed key order the writer emits."""
    npos = len(pos)
    ntri = len(tri)
    bin_len = 12 * npos + 2 * 3 * ntri
    pos_view_len = 12 * npos
    idx_view_len = 6 * ntri
    xs = [p[0] for p in pos]
    ys = [p[1] for p in pos]
    zs = [p[2] for p in pos]
    mn = ",".join(fmt_f32(v) for v in (min(xs), min(ys), min(zs)))
    mx = ",".join(fmt_f32(v) for v in (max(xs), max(ys), max(zs)))
    idx_component = COMPONENT_USHORT  # box indices fit in u16 (max 7)
    s = "{"
    s += '"asset":{"version":"2.0"},'
    s += '"buffers":[{"byteLength":%d}],' % bin_len
    s += (
        '"bufferViews":['
        '{"buffer":0,"byteOffset":0,"byteLength":%d,"target":%d},'
        '{"buffer":0,"byteOffset":%d,"byteLength":%d,"target":%d}],'
        % (pos_view_len, TARGET_ARRAY_BUFFER, pos_view_len, idx_view_len,
           TARGET_ELEMENT_ARRAY_BUFFER)
    )
    s += (
        '"accessors":['
        '{"bufferView":0,"byteOffset":0,"componentType":%d,"count":%d,'
        '"type":"VEC3","min":[%s],"max":[%s]},'
        '{"bufferView":1,"byteOffset":0,"componentType":%d,"count":%d,'
        '"type":"SCALAR"}],'
        % (COMPONENT_FLOAT, npos, mn, mx, idx_component, 3 * ntri)
    )
    s += (
        '"meshes":[{"primitives":[{"attributes":{"POSITION":0},'
        '"indices":1,"mode":%d}]}],' % MODE_TRIANGLES
    )
    s += '"nodes":[{"mesh":0}],'
    s += '"scenes":[{"nodes":[0]}],'
    s += '"scene":0'
    s += "}"
    return s


def pad(b, fill):
    """Pad a byte string to a multiple of 4 with a single fill byte."""
    padbytes = fill * ((4 - len(b) % 4) % 4)
    return b + padbytes


def main():
    out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else (
        pathlib.Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.glb"
    )
    json_bytes = build_json(POSITIONS, TRIANGLES).encode("ascii")
    json_padded = pad(json_bytes, b" ")
    # BIN payload: positions (LE f32 triples) then u16 indices, zero-padded.
    bin_bytes = b"".join(struct.pack("<3f", *p) for p in POSITIONS)
    for t in TRIANGLES:
        for i in t:
            bin_bytes += struct.pack("<H", i)
    bin_padded = pad(bin_bytes, b"\x00")

    total = 12 + 8 + len(json_padded) + 8 + len(bin_padded)
    blob = struct.pack("<4sII", b"glTF", 2, total)
    blob += struct.pack("<I4s", len(json_padded), b"JSON") + json_padded
    blob += struct.pack("<I4s", len(bin_padded), b"BIN\0") + bin_padded

    assert len(bin_bytes) == 168, f"bin payload size: {len(bin_bytes)} (96 pos + 72 idx)"
    assert len(bin_padded) == 168
    assert len(json_padded) % 4 == 0, f"json padded to 4: {len(json_padded)}"
    # Sanity: unique corners == 8, matching the reference tessellation.
    corners = {tuple(POSITIONS[i]) for t in TRIANGLES for i in t}
    assert len(corners) == 8, f"expected 8 unique corners, got {len(corners)}"

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(blob)
    print(
        f"wrote {out} ({len(blob)} bytes; json chunk {len(json_padded)} "
        f"({len(json_bytes)} raw), bin chunk {len(bin_padded)}, "
        f"{len(TRIANGLES)} facets, {len(corners)} unique corners)"
    )


if __name__ == "__main__":
    main()
