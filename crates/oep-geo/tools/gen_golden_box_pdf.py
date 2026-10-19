#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> PDF vector drawing (P6-M4-T01).

Produces crates/oep-geo/tests/golden/box-reference.pdf: a minimal, valid PDF
1.4 whose single page's content stream draws the SAME 2D projected-wireframe
drawing as the SVG export (P6-M4-T01 "PDF (drawing path)"). Each unique
projected edge becomes a `m`/`l`/`S` path operator.

Layout contract (byte-for-byte, must match crates/oep-geo/src/export/pdf.rs):
  * 4 objects: Catalog, Pages, Page, Content stream (no compression);
  * MediaBox [0 0 W H] where W/H = projected bounds + 1.0 unit margin, and all
    projected coordinates are translated so the view is inside [0, W]x[0, H]
    (translation offsets: -min_x + 0.5, -min_y + 0.5);
  * content stream: for each unique projected edge `x1 y1 m x2 y2 l S \\n`
    (f32 shortest form via fmt_f32);
  * xref table with byte offsets computed the same way on both sides;
  * PDF reals use the same formatter as the writer (integral => "0"/"1").

Note: the projection is an orthographic XY projection (drop Z) of unique
undirected edges; the "drawing" is a projected-edge wireframe, not a
hidden-line-removed silhouette (HLR is a later milestone).

Usage:  ./gen_golden_box_pdf.py [outfile]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES, fmt_f32  # noqa: E402


def project2(p):
    return (p[0], p[1])


def unique_edges():
    seen = {}
    order = []
    for (a, b, c) in TRIANGLES:
        for (u, v) in ((a, b), (b, c), (c, a)):
            pu = project2(POSITIONS[u])
            pv = project2(POSITIONS[v])
            if pu == pv:
                continue  # zero-length in this projection (drop)
            key = tuple(sorted((pu, pv)))
            if key not in seen:
                seen[key] = True
                order.append(key)
    return order


def content_stream(edges, pts2):
    del pts2
    out = []
    for ((x1, y1), (x2, y2)) in edges:
        out.append("%s %s m %s %s l S" % (
            fmt_f32(x1), fmt_f32(y1), fmt_f32(x2), fmt_f32(y2)))
    return "\n".join(out) + "\n"


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.pdf"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    pts2 = [project2(p) for p in POSITIONS]
    edges = unique_edges()
    xs = [p[0] for p in pts2]
    ys = [p[1] for p in pts2]
    minx, miny = min(xs), min(ys)
    w = (max(xs) - minx) + 1.0
    h = (max(ys) - miny) + 1.0
    tx = -minx + 0.5
    ty = -miny + 0.5
    pts2 = [(x + tx, y + ty) for (x, y) in pts2]
    # Translate edge endpoints into the page coordinate system the same way.
    edges = [(((x1 + tx), (y1 + ty)), ((x2 + tx), (y2 + ty))) for ((x1, y1), (x2, y2)) in edges]
    content = content_stream(edges, pts2)

    objects = []
    objects.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objects.append(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    objects.append(
        ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %s %s] /Contents 4 0 R >>"
         % (fmt_f32(w), fmt_f32(h))).encode("ascii")
    )
    objects.append(
        ("<< /Length %d >>\nstream\n%s\nendstream" % (len(content), content))
        .encode("ascii")
    )

    # Assemble: header + objects (record offsets), xref, trailer.
    header = b"%PDF-1.4\n"
    body = bytearray(header)
    offsets = []
    for i, obj in enumerate(objects, start=1):
        offsets.append(len(body))
        body += b"%d 0 obj\n" % i
        body += obj + b"\nendobj\n"

    xref_pos = len(body)
    body += b"xref\n0 %d\n" % (len(objects) + 1)
    body += b"0000000000 65535 f \n"
    for off in offsets:
        body += ("%010d 00000 n \n" % off).encode("ascii")
    body += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1, xref_pos)

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(bytes(body))
    print(f"wrote {out} ({len(body)} bytes; {len(edges)} projected edges)")


if __name__ == "__main__":
    main()
