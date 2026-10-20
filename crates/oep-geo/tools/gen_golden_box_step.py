#!/usr/bin/env python3
"""Independent golden encoder: internal mesh -> STEP faceted passthrough.

Produces crates/oep-geo/tests/golden/box-reference.step. A mesh -> STEP
faceted B-Rep pass-through, honestly marked TESSELLATED (NOT an exact
analytic B-Rep), following the P2-M5-T02 / P4-lane ISO 10303-21 contract:
AP214 (automotive_design) PART 21 file with a SHELL_BASED_SURFACE_MODEL of
planar FACETED faces (one PLANE per triangle bounded by a POLY_LOOP of its
three CARTESIAN_POINTs), grouped into one CLOSED_SHELL / OPEN_SHELL per
connected component.

The emission is deterministic and re-implemented HERE independently from the
ISO 10303-21 + AP214 facts (NOT by calling the Rust writer), so the Rust
golden test locking its output to this file proves conformance.

Numeric rule (must match crates/oep-geo/src/export/step.rs): coordinates are
emitted in shortest f32 round-trip form with a decimal point forced
("1." / "0." / "0.5") so every token is a PART 21 REAL.

This encoder computes connected components with the same BFS over shared
edges the writer uses, so shell membership matches observable structure.

Usage:  ./gen_golden_box_step.py [outfile]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from box_ref import POSITIONS, TRIANGLES, to_f32  # noqa: E402

VERSION = "0.1.0"
PRODUCT = "box"


def fmt_real(v):
    f = to_f32(v)
    s = ("%d" % int(f)) if f == int(f) and abs(f) < 1e16 else ("%r" % float(f))
    # Rust f32 Display: shortest round trip; here values are exact bin fractions
    return s if ("." in s or "e" in s or "E" in s) else s + "."


def cross(u, v):
    return (
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    )


def mag(v):
    return (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) ** 0.5


def unit(v):
    m = mag(v)
    return None if m <= 1e-12 else tuple(c / m for c in v)


def main():
    default = Path(__file__).resolve().parent / ".." / "tests" / "golden" / "box-reference.step"
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else default

    # --- validate + classify triangles (usable vs degenerate) ---
    metas = []       # (tri_idx, normal, x_axis)
    degenerate = 0
    for idx, (a, b, c) in enumerate(TRIANGLES):
        p0 = POSITIONS[a]; p1 = POSITIONS[b]; p2 = POSITIONS[c]
        u = tuple(p1[i] - p0[i] for i in range(3))
        v = tuple(p2[i] - p0[i] for i in range(3))
        n = unit(cross(u, v))
        xa = unit(u)
        if n is None or xa is None:
            degenerate += 1
        else:
            metas.append((idx, n, xa))
    assert len(metas) == len(TRIANGLES), "all box facets must be usable"

    # --- undirected edges for component detection ---
    edge_to_tris = {}
    tri_edges = []
    for mi, (tidx, _n, _x) in enumerate(metas):
        a, b, c = TRIANGLES[tidx]
        edges = []
        for (u, v2) in ((a, b), (b, c), (c, a)):
            key = (u, v2) if u < v2 else (v2, u)
            edges.append(key)
            edge_to_tris.setdefault(key, []).append(mi)
        tri_edges.append(edges)
    n = len(metas)
    comp_of = [-1] * n
    components = []
    for start in range(n):
        if comp_of[start] != -1:
            continue
        cid = len(components)
        comp_of[start] = cid
        stack = [start]
        members = []
        while stack:
            t = stack.pop()
            members.append(t)
            for e in tri_edges[t]:
                for nb in edge_to_tris.get(e, []):
                    if comp_of[nb] == -1:
                        comp_of[nb] = cid
                        stack.append(nb)
        components.append(members)

    # --- emitter ---
    next_id = [0]

    def nid():
        next_id[0] += 1
        return next_id[0]

    def line(i, body):
        return "#%d=%s;\n" % (i, body)

    out_lines = []
    out_lines.append("ISO-10303-21;\n")
    out_lines.append("HEADER;\n")
    out_lines.append(
        "FILE_DESCRIPTION(('oep-geo: tessellated mesh passthrough. Faceted "
        "(planar) B-Rep derived from source tessellation. This is a "
        "TESSELLATED representation and is NOT an exact analytic B-Rep.'),"
        "'2;1');\n"
    )
    out_lines.append(
        "FILE_NAME('box-reference.step','',('Isolith OEP'),(''),'oep-geo "
        + VERSION + "','Isolith OEP','');\n"
    )
    out_lines.append("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\n")
    out_lines.append("ENDSEC;\n")
    out_lines.append("DATA;\n")

    L = out_lines.append
    # Product cluster.
    app_ctx = nid(); L(line(app_ctx, "APPLICATION_CONTEXT('geometric model')"))
    apd = nid(); L(line(apd, "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',1999,#%d)" % app_ctx))
    prod_ctx = nid(); L(line(prod_ctx, "PRODUCT_CONTEXT('',#%d,'mechanical')" % apd))
    product = nid()
    L(line(product, "PRODUCT('%s','%s','oep-geo: tessellated faceted B-Rep passthrough. NOT an exact analytic B-Rep.',(#%d))" % (PRODUCT, PRODUCT, prod_ctx)))
    pdf2 = nid(); L(line(pdf2, "PRODUCT_DEFINITION_FORMATION('','',#%d)" % product))
    pdc = nid(); L(line(pdc, "PRODUCT_DEFINITION_CONTEXT('part definition',#%d,'design')" % apd))
    pd = nid(); L(line(pd, "PRODUCT_DEFINITION('design','',#%d,#%d)" % (pdf2, pdc)))
    pds = nid(); L(line(pds, "PRODUCT_DEFINITION_SHAPE('','tessellated mesh',#%d)" % pd))

    # One planar face per usable triangle.
    tri_face_id = [0] * n
    for k, (tidx, normal, x_axis) in enumerate(metas):
        a, b, c = TRIANGLES[tidx]
        p0, p1, p2 = POSITIONS[a], POSITIONS[b], POSITIONS[c]
        cp = [nid() for _ in range(3)]
        L(line(cp[0], "CARTESIAN_POINT('',(%s,%s,%s))" % (fmt_real(p0[0]), fmt_real(p0[1]), fmt_real(p0[2]))))
        L(line(cp[1], "CARTESIAN_POINT('',(%s,%s,%s))" % (fmt_real(p1[0]), fmt_real(p1[1]), fmt_real(p1[2]))))
        L(line(cp[2], "CARTESIAN_POINT('',(%s,%s,%s))" % (fmt_real(p2[0]), fmt_real(p2[1]), fmt_real(p2[2]))))
        zid = nid(); L(line(zid, "DIRECTION('',(%s,%s,%s))" % (fmt_real(normal[0]), fmt_real(normal[1]), fmt_real(normal[2]))))
        xid = nid(); L(line(xid, "DIRECTION('',(%s,%s,%s))" % (fmt_real(x_axis[0]), fmt_real(x_axis[1]), fmt_real(x_axis[2]))))
        place = nid(); L(line(place, "AXIS2_PLACEMENT_3D('',#%d,#%d,#%d)" % (cp[0], zid, xid)))
        plane = nid(); L(line(plane, "PLANE('',#%d)" % place))
        ploop = nid(); L(line(ploop, "POLY_LOOP('',(#%d,#%d,#%d))" % (cp[0], cp[1], cp[2])))
        fob = nid(); L(line(fob, "FACE_OUTER_BOUND('',#%d,.T.)" % ploop))
        face = nid(); L(line(face, "FACE_SURFACE('',(#%d),#%d,.T.)" % (fob, plane)))
        tri_face_id[k] = face

    # One shell per component.
    shell_ids = []
    closed_count = 0
    for members in components:
        closed = all(len(edge_to_tris[e]) == 2
                     for m in members for e in tri_edges[m])
        face_list = [tri_face_id[m] for m in members]
        kind = "CLOSED_SHELL" if closed else "OPEN_SHELL"
        s = nid()
        L(line(s, "%s('',(%s))" % (kind, ",".join("#%d" % f for f in face_list))))
        shell_ids.append(s)
        if closed:
            closed_count += 1

    sbsm = nid()
    L(line(sbsm, "SHELL_BASED_SURFACE_MODEL('oep-geo tessellated mesh',(%s))"
          % ",".join("#%d" % s for s in shell_ids)))

    # Units + context.
    len_unit = nid(); L(line(len_unit, "(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))"))
    ang_unit = nid(); L(line(ang_unit, "(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))"))
    ang_solid = nid(); L(line(ang_solid, "(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())"))
    ctx = nid()
    L(line(ctx, "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#%d,#%d,#%d))REPRESENTATION_CONTEXT('',''))" % (len_unit, ang_unit, ang_solid)))
    shape_rep = nid()
    L(line(shape_rep, "SHAPE_REPRESENTATION('tessellated faceted B-Rep (NOT exact)','ShellBasedSurfaceModel',(#%d),#%d)" % (sbsm, ctx)))
    rel = nid()
    L(line(rel, "SHAPE_REPRESENTATION_RELATIONSHIP('','',#%d,#%d)" % (pds, shape_rep)))

    out_lines.append("ENDSEC;\n")
    out_lines.append("END-ISO-10303-21;\n")

    text = "".join(out_lines)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out} ({len(text.encode('utf-8'))} bytes; {n} faces, "
          f"{len(components)} shells, {closed_count} closed)")


if __name__ == "__main__":
    main()
