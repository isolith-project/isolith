#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> glTF 2.0 GLB (P6-M4-T01).

Produces crates/oep-geo/tests/golden/box-reference.glb from the reference
tessellation using ONLY the published Khronos glTF 2.0 spec (binary glTF
layout, section 5.2):

    GLB = 12-byte header
        magic    u32 LE == 0x46546C67 ("glTF")
        version  u32 LE == 2
        length   u32 LE == total file size
      + JSON chunk: u32 chunkLength, u32 chunkType == 0x4E4F534A ("JSON"),
        JSON bytes space-padded (0x20) to a multiple of 4
      + BIN chunk:  u32 chunkLength, u32 chunkType == 0x004E4942 ("BIN\\0"),
        binary buffer zero-padded to a multiple of 4

JSON describes one buffer (byteLength 168: 8 VEC3 f32 positions = 96 bytes,
then 36 u16 index values = 72 bytes), two bufferViews (targets 34962
ARRAY_BUFFER / 34963 ELEMENT_ARRAY_BUFFER), two accessors (POSITION FLOAT
VEC3 count 8 with min/max, indices UNSIGNED_SHORT count 36), one mesh
primitive (mode 4 TRIANGLES), one node, one scene.

Conventions (must match crates/oep-geo/src/export/gltf.rs):
  * JSON compact, fixed key order (asset/buffers/bufferViews/accessors/
    meshes/nodes/scenes/scene);
  * integral f32 values render as integer literals ("0", "1");
  * JSON chunk padded with ASCII spaces; BIN chunk zero-padded;
  * BIN payload = positions (LE f32 x3) then indices (LE u16).

Usage:  ./gen_golden_box_glb.py [outfile]
"""
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES, fmt_f32  # noqa: E402

TARGET_ARRAY_BUFFER = 34962
TARGET_ELEMENT_ARRAY_BUFFER = 34963
COMPONENT_FLOAT = 5126
COMPONENT_USHORT = 5123
MODE_TRIANGLES = 4


def build_json():
    npos = len(POSITIONS)
    ntri = len(TRIANGLES)
    pos_view_len = 12 * npos
    idx_view_len = 6 * ntri
    bin_len = pos_view_len + idx_view_len
    xs = [p[0] for p in POSITIONS]
    ys = [p[1] for p in POSITIONS]
    zs = [p[2] for p in POSITIONS]
    mn = ",".join(fmt_f32(v) for v in (min(xs), min(ys), min(zs)))
    mx = ",".join(fmt_f32(v) for v in (max(xs), max(ys), max(zs)))
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
        % (COMPONENT_FLOAT, npos, mn, mx, COMPONENT_USHORT, 3 * ntri)
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
    padbytes = fill * ((4 - len(b) % 4) % 4)
    return b + padbytes


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.glb"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    json_bytes = build_json().encode("ascii")
    json_padded = pad(json_bytes, b" ")
    bin_bytes = b"".join(struct.pack("<3f", *p) for p in POSITIONS)
    for t in TRIANGLES:
        for i in t:
            bin_bytes += struct.pack("<H", i)
    bin_padded = pad(bin_bytes, b"\x00")

    total = 12 + 8 + len(json_padded) + 8 + len(bin_padded)
    blob = struct.pack("<4sII", b"glTF", 2, total)
    blob += struct.pack("<I4s", len(json_padded), b"JSON") + json_padded
    blob += struct.pack("<I4s", len(bin_padded), b"BIN\0") + bin_padded

    assert len(bin_bytes) == 168, f"bin payload size: {len(bin_bytes)}"
    assert len(bin_padded) == 168

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(blob)
    print(f"wrote {out} ({len(blob)} bytes; json chunk {len(json_padded)}, "
          f"bin chunk {len(bin_padded)})")


if __name__ == "__main__":
    main()
