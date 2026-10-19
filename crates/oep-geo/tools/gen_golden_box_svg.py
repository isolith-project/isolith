#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> 2D SVG drawing path (P6-M4-T01).

Produces crates/oep-geo/tests/golden/box-reference.svg from the reference
tessellation. A "2D drawing" of a tessellated part is its orthographic
projection onto a plane; this encoder projects onto the XY plane (drops Z,
the default "top view") and emits the projected UNIQUE undirected edges as
vector line segments (a projected wireframe drawing), one SVG <path> per
edge inside a single <path> element.

Deterministic dedup contract (must match crates/oep-geo/src/export/svg.rs):
  * an undirected projected edge is keyed by its two projected 2D endpoints,
    normalized to a canonical order ((xa,ya),(xb,yb)) with the smaller
    endpoint first;
  * duplicate projected edges (different triangles sharing the same projected
    segment, including coincident top/bottom-face edges) collapse to one;
  * edges whose two projected endpoints coincide are dropped (zero-length in
    this projection);
  * edges are emitted in FIRST-SEEN order over the triangle list (triangle
    order, edge order (a,b),(b,c),(c,a) within a triangle);
  * viewBox covers the projected bounds inflated by a fixed 0.5 margin;
  * coordinates render with fmt_f32 (integral => integer literal).

The projection is a projected-edge wireframe, NOT a hidden-line-removed
silhouette (HLR is a later drawing milestone).

Usage:  ./gen_golden_box_svg.py [outfile]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES, fmt_f32  # noqa: E402


def project2(p):
    """Orthographic projection onto the XY plane: drop z."""
    return (p[0], p[1])


def unique_projected_edges():
    """First-seen-order unique undirected projected edges (2D endpoint keys)."""
    seen = {}
    order = []
    for (a, b, c) in TRIANGLES:
        for (u, v) in ((a, b), (b, c), (c, a)):
            pu = project2(POSITIONS[u])
            pv = project2(POSITIONS[v])
            if pu == pv:
                continue  # zero-length in this projection
            key = tuple(sorted((pu, pv)))
            if key not in seen:
                seen[key] = True
                order.append(key)
    return order


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.svg"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    edges = unique_projected_edges()
    all_pts = [p for e in edges for p in e]
    xs = [p[0] for p in all_pts]
    ys = [p[1] for p in all_pts]
    minx, maxx = min(xs), max(xs)
    miny, maxy = min(ys), max(ys)
    pad = 0.5
    vbx = fmt_f32(minx - pad)
    vby = fmt_f32(miny - pad)
    vbw = fmt_f32((maxx - minx) + 2 * pad)
    vbh = fmt_f32((maxy - miny) + 2 * pad)

    segs = []
    for ((x1, y1), (x2, y2)) in edges:
        segs.append("M %s %s L %s %s" % (
            fmt_f32(x1), fmt_f32(y1), fmt_f32(x2), fmt_f32(y2)))
    d = " ".join(segs)

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<svg xmlns="http://www.w3.org/2000/svg" '
        'viewBox="{0} {1} {2} {3}">'.format(vbx, vby, vbw, vbh),
        '  <g fill="none" stroke="black" stroke-width="0.05">',
        '    <path d="{0}"/>'.format(d),
        "  </g>",
        "</svg>",
        "",
    ]
    text = "\n".join(lines)

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out} ({len(text.encode('utf-8'))} bytes; "
          f"{len(edges)} unique projected edges)")


if __name__ == "__main__":
    main()
