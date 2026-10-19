#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> Wavefront OBJ (P6-M4-T01).

Produces crates/oep-geo/tests/golden/box-reference.obj from the reference
tessellation using only the published Wavefront OBJ format spec:
  * `v <x> <y> <z>` vertex records (one per position, in input order);
  * `f <i> <j> <k>` face records (1-based indices, in triangle order);
  * a `#` comment header naming the exporter.

Coordinates render with `fmt_f32` (shortest round-trip; integral values as
integer literals) matching the Rust writer's f32 Display.

Conventions (must match crates/oep-geo/src/export/obj.rs): no normals, no
texture coordinates, no material library; faces are 1-based positive indices
with no `f v/vt/vn` variant.

Usage:  ./gen_golden_box_obj.py [outfile]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES, fmt_f32  # noqa: E402


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.obj"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    lines = ["# oep-geo -- Wavefront OBJ export (P6-M4-T01)"]
    lines.append("# 8 vertices, 12 triangles (reference tessellation)")
    for (x, y, z) in POSITIONS:
        lines.append(
            "v %s %s %s" % (fmt_f32(x), fmt_f32(y), fmt_f32(z))
        )
    for (a, b, c) in TRIANGLES:
        lines.append("f %d %d %d" % (a + 1, b + 1, c + 1))
    text = "\n".join(lines) + "\n"

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out} ({len(text.encode('utf-8'))} bytes, "
          f"{len(POSITIONS)} v / {len(TRIANGLES)} f)")


if __name__ == "__main__":
    main()
