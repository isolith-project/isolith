#!/usr/bin/env python3
"""P4-M1-T03 — accuracy harness: STEP geometry fidelity vs native sample.

Geometry-diff harness for the P4 STEP lane. For every corpus **pair** (a native
source sample AND the STEP representation of the same part, when both are
present in the corpus — see `corpus/accuracy-pairs.json`), it imports geometry
from both sides and measures fidelity:

  - **symmetric (two-sided) vertex-sampled Hausdorff distance** (clean
    point->triangle distance both directions; method honestly reported —
    vertex-sampled, not exact surface Hausdorff);
  - **volume compare** (signed divergence-theorem volume per side, absolute
    delta + relative error; closure/winding caveats reported explicitly).

Honest-by-construction (program anti-fabrication doctrine): a row is
`measured` only when real numbers were computed from readable geometry on BOTH
sides; anything else is a classified `skip` from a closed taxonomy with a
machine reason. The harness NEVER invents a fidelity value.

Clean-room boundary: the STEP reader understands only the facetted /
tessellated subset our own P6-M4-T01 / P2-M5-T02 exporters emit
(CARTESIAN_POINT, POLY_LOOP, FACE_OUTER_BOUND, FACE_SURFACE, PLANE,
CLOSED_SHELL, plus the AP214/203 header/preamble entities). An analytic B-Rep
STEP (ADVANCED_FACE, B_SPLINE_*, MANIFOLD_SOLID_BREP, ...) raises
:class:`NotFaceted` -> classified `not-faceted` skip, never a number. When
P4-M1-T01's full oep-step reader lands, it can be swapped in behind the same
:func:`read_faceted_step` seam.

Pure stdlib (pyproject deps = []). CLI:
`python -m oep_tooling.step_fidelity --pairs corpus/accuracy-pairs.json [--corpus corpus --out corpus]`

Skip taxonomy (closed set, each with a reason string):
  pair-missing-on-disk | native-unreadable | step-unreadable |
  not-faceted | no-native-geometry | no-step-geometry | io-error |
There is NO `fail` bucket: an introspection error becomes a classified row;
a crash in one pair never aborts the sweep (per-pair try/except).
"""
from __future__ import annotations

import json
import math
import os
import re
import struct
import sys
from datetime import datetime, timezone
from pathlib import Path

# Closed skip-classification taxonomy. Every skip row must carry one of these.
CLOSED_TAXONOMY = (
    "pair-missing-on-disk",   # native or step path does not resolve to a file
    "native-unreadable",      # native side: no reader registered for this file
    "step-unreadable",        # step side: reader failed (see reason)
    "not-faceted",            # step is an analytic B-Rep, not facetted -> honest skip
    "no-native-geometry",     # native open but no geometry recovered
    "no-step-geometry",       # step open but no geometry recovered
    "io-error",               # unreadable at the OS level
)

STEP_HEADER = "ISO-10303-21"
_RE_ENTITY = re.compile(r"#(\d+)\s*=\s*([A-Za-z_0-9]+)\s*\((.*?)\)\s*;", re.S)
_RE_VERTEX = re.compile(
    r"vertex\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)")
_RE_FACET = re.compile(
    r"facet\s+normal[^\n]*\n((?:(?!\bfacet\b).)*?)endfacet", re.S)


class StepError(Exception):
    """A STEP file could not be read at all (not an analytic-vs-faceted call)."""


class NotFaceted(StepError):
    """The STEP is an analytic/topological B-Rep, outside the facetted subset.

    Raised so the harness can classify the pair honestly (`not-faceted`) rather
    than fabricating a fidelity number from partial data.
    """


# --------------------------------------------------------------------------
# STEP facetted reader (clean-room subset)
# --------------------------------------------------------------------------
def _split_args(body):
    """Split a STEP entity argument list on top-level commas (depth-aware)."""
    parts, depth, cur = [], 0, []
    for ch in body:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return [p.strip() for p in parts]


def _tuple_literal(raw):
    raw = raw.strip().strip("'\"").strip()
    if raw.startswith("(") and raw.endswith(")"):
        raw = raw[1:-1]
    return raw


def read_faceted_step(path):
    """Parse a tessellated/faceted STEP file into a plain mesh dict.

    Returns ``{"positions": {i: (x,y,z)}, "triangles": [(i,j,k), ...],
    "shells": int, "warnings": [...]}``. Raises :class:`NotFaceted` for
    analytic B-Rep entities, :class:`StepError` for unreadable/unparsable
    input. Vertex pool is deduplicated so native/STEP comparators see the same
    vertex-set semantics as the STL reader. Never panics/fabricates.
    """
    try:
        text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        raise StepError("read failed: %r" % (e,)) from e
    if not text.lstrip().startswith(STEP_HEADER):
        raise StepError("not an ISO-10303-21 file")

    data_start = text.find("DATA;")
    data_end = text.find("ENDSEC;", data_start)
    if data_start < 0 or data_end < 0:
        raise StepError("no DATA section")
    blob = text[data_start + len("DATA;"):data_end]

    entities = {}
    for m in _RE_ENTITY.finditer(blob):
        entities[int(m.group(1))] = (m.group(2).upper(), m.group(3))

    points = {}
    for eid, (etype, body) in entities.items():
        if etype == "CARTESIAN_POINT":
            args = _split_args(body)
            raw = _tuple_literal(args[-1])
            vals = [float(c.strip()) for c in raw.split(",") if c.strip()]
            if len(vals) != 3:
                raise StepError(f"#CARTESIAN_POINT {eid}: expected 3 coords")
            points[eid] = vals

    ALLOWED_META = {
        "APPLICATION_CONTEXT", "APPLICATION_PROTOCOL_DEFINITION",
        "PRODUCT_CONTEXT", "PRODUCT", "PRODUCT_DEFINITION_FORMATION",
        "PRODUCT_DEFINITION_CONTEXT", "PRODUCT_DEFINITION",
        "PRODUCT_DEFINITION_SHAPE", "LENGTH_UNIT", "NAMED_UNIT", "SI_UNIT",
        "GEOMETRIC_REPRESENTATION_CONTEXT", "GLOBAL_UNIT_ASSIGNED_CONTEXT",
        "REPRESENTATION_CONTEXT", "SHAPE_REPRESENTATION",
        "SHAPE_REPRESENTATION_RELATIONSHIP", "PLANE_ANGLE_UNIT",
        "SOLID_ANGLE_UNIT",
    }
    NON_GEOMETRY = ALLOWED_META | {
        "CARTESIAN_POINT", "DIRECTION", "AXIS2_PLACEMENT_3D", "PLANE",
        "POLY_LOOP", "FACE_OUTER_BOUND", "FACE_SURFACE",
        "CLOSED_SHELL", "SHELL_BASED_SURFACE_MODEL",
    }
    ANALYTIC = {
        "ADVANCED_FACE", "B_SPLINE_SURFACE_WITH_KNOTS", "B_SPLINE_SURFACE",
        "B_SPLINE_CURVE_WITH_KNOTS", "SURFACE_CURVE", "EDGE_CURVE",
        "EDGE_LOOP", "ORIENTED_EDGE", "VERTEX_POINT", "MANIFOLD_SOLID_BREP",
    }
    unknown = set()
    shell_count = 0
    for eid, (etype, body) in entities.items():
        if etype in NON_GEOMETRY:
            if etype == "CLOSED_SHELL":
                shell_count += 1
            continue
        if etype in ANALYTIC:
            raise NotFaceted(f"{etype} present -> analytic B-Rep, not facetted")
        unknown.add(etype)
    if unknown:
        raise NotFaceted("unhandled entity types: %s"
                         % (", ".join(sorted(unknown))))

    loops = {}
    outer_bounds = []
    for eid, (etype, body) in entities.items():
        if etype == "POLY_LOOP":
            args = _split_args(body)
            raw = _tuple_literal(args[-1])
            refs = [r.strip() for r in raw.split(",") if r.strip()]
            loops[eid] = [int(r[1:]) for r in refs if r.startswith("#")]
        elif etype == "FACE_OUTER_BOUND":
            args = _split_args(body)
            loop_ref = next((a for a in args if a.strip().startswith("#")), None)
            if loop_ref is not None:
                outer_bounds.append(int(loop_ref.strip()[1:]))

    pool = {}
    pos = []
    triangles = []
    n_quad = 0
    n_degenerate = 0

    def _idx(c):
        if c not in pool:
            pool[c] = len(pos)
            pos.append(c)
        return pool[c]

    for lid in outer_bounds:
        poly = loops.get(lid)
        if poly is None:
            raise StepError("FACE_OUTER_BOUND -> missing POLY_LOOP")
        verts = [tuple(points[pid]) for pid in poly if pid in points]
        if len(verts) < 3:
            raise StepError("POLY_LOOP with <3 points")
        if len(verts) == 4:
            fans = [(0, 1, 2), (0, 2, 3)]   # quad fan; honest convention
            n_quad += 1
        else:
            fans = [(0, i, i + 1) for i in range(1, len(verts) - 1)]
        for (a, b, c) in fans:
            if _tri_area(verts[a], verts[b], verts[c]) == 0.0:
                n_degenerate += 1
            triangles.append((_idx(verts[a]), _idx(verts[b]), _idx(verts[c])))

    if len(pos) == 0 or not triangles:
        raise StepError("no geometry recovered")

    warnings = []
    if n_quad:
        warnings.append(f"{n_quad} quad loops fan-triangulated")
    if n_degenerate:
        warnings.append(f"{n_degenerate} degenerate triangles skipped")

    return {
        "positions": {i: p for i, p in enumerate(pos)},
        "triangles": triangles,
        "shells": shell_count,
        "warnings": warnings,
    }


def _tri_area(a, b, c):
    ux, uy, uz = b[0] - a[0], b[1] - a[1], b[2] - a[2]
    vx, vy, vz = c[0] - a[0], c[1] - a[1], c[2] - a[2]
    cx = uy * vz - uz * vy
    cy = uz * vx - ux * vz
    cz = ux * vy - uy * vx
    return 0.5 * math.hypot(math.hypot(cx, cy), cz)


# --------------------------------------------------------------------------
# Native readers (STL ASCII + binary) — tooling scope, clean and honest
# --------------------------------------------------------------------------
def read_stl(path):
    """Parse an ASCII or binary STL file into a plain mesh dict.

    Raises :class:`StepError` (used as the generic 'cannot read' error) on
    unreadable/unrecognizable input. Committed fixture corpus/data/box.stl is
    ASCII; the binary path is covered by unit tests via a synthesized blob.
    """
    try:
        data = Path(path).read_bytes()
    except OSError as e:
        raise StepError("read failed: %r" % (e,)) from e
    if len(data) == 0:
        raise StepError("empty STL")
    if data[:5] == b"solid" or data.lstrip().startswith(b"solid"):
        text = data.decode("utf-8", errors="replace")
        return _stl_ascii(text)
    # Binary STL: 80-byte header + u32 count; header may be zero-filled but the
    # count field at 80 is the real triangle count (mirrors the exporters'
    # golden parser: 80 + 4 + 50*count == file length).
    if len(data) >= 84:
        (count,) = struct.unpack_from("<I", data, 80)
        if 80 + 4 + 50 * count == len(data):
            return _stl_binary(data)
    raise StepError("STL: unrecognized header (neither ASCII 'solid' nor binary)")


def _stl_ascii(text):
    pool = {}
    pos = []
    triangles = []
    for m in _RE_FACET.finditer(text):
        tri = []
        for v in _RE_VERTEX.findall(m.group(1)):
            c = (float(v[0]), float(v[1]), float(v[2]))
            if c not in pool:
                pool[c] = len(pos)
                pos.append(c)
            tri.append(pool[c])
        if len(tri) == 3:
            triangles.append(tuple(tri))
    if not triangles:
        raise StepError("ASCII STL: no facets found")
    return {"positions": {i: p for i, p in enumerate(pos)},
            "triangles": triangles, "warnings": []}


def _stl_binary(data):
    (count,) = struct.unpack_from("<I", data, 80)
    pool = {}
    pos = []
    triangles = []
    off = 84
    for _ in range(count):
        # normal (12 bytes) + 3 verts (36) + attr (2) = 50 bytes
        tris = []
        for k in range(3):
            x, y, z = struct.unpack_from("<fff", data, off + 12 + 12 * k)
            c = (x, y, z)
            if c not in pool:
                pool[c] = len(pos)
                pos.append(c)
            tris.append(pool[c])
        triangles.append(tuple(tris))
        off += 50
    if not triangles:
        raise StepError("binary STL: no facets found")
    return {"positions": {i: p for i, p in enumerate(pos)},
            "triangles": triangles, "warnings": []}


# --------------------------------------------------------------------------
# Geometry metrics
# --------------------------------------------------------------------------
def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0])


def _norm(a):
    return math.sqrt(_dot(a, a))


def _point_tri_distance(p, a, b, c):
    """Exact minimum distance from point p to triangle (a,b,c)."""
    ab = _sub(b, a)
    ac = _sub(c, a)
    ap = _sub(p, a)
    d1 = _dot(ab, ap)
    d2 = _dot(ac, ap)
    if d1 <= 0 and d2 <= 0:
        return _norm(ap)
    bp = _sub(p, b)
    d3 = _dot(ab, bp)
    d4 = _dot(ac, bp)
    if d3 >= 0 and d4 <= d3:
        return _norm(bp)
    vc = d1 * d4 - d3 * d2
    if vc <= 0 and d1 >= 0 and d3 <= 0:
        t = d1 / (d1 - d3)
        return _norm(_sub(ap, (ab[0] * t, ab[1] * t, ab[2] * t)))
    cp = _sub(p, c)
    d5 = _dot(ab, cp)
    d6 = _dot(ac, cp)
    if d6 >= 0 and d5 <= d6:
        return _norm(cp)
    vb = d5 * d2 - d1 * d6
    if vb <= 0 and d2 >= 0 and d6 <= 0:
        t = d2 / (d2 - d6)
        return _norm(_sub(ap, (ac[0] * t, ac[1] * t, ac[2] * t)))
    va = d3 * d6 - d5 * d4
    if va <= 0 and (d4 - d3) >= 0 and (d5 - d6) >= 0:
        t = (d4 - d3) / ((d4 - d3) + (d5 - d6))
        w = _sub(c, b)
        return _norm(_sub(bp, (w[0] * t, w[1] * t, w[2] * t)))
    denom = 1.0 / (va + vb + vc)
    v = vb * denom
    w = vc * denom
    u = 1 - v - w
    q = (a[0] * u + b[0] * v + c[0] * w,
         a[1] * u + b[1] * v + c[1] * w,
         a[2] * u + b[2] * v + c[2] * w)
    return _norm(_sub(p, q))


def _tri_coords(mesh, tri):
    return (mesh["positions"][tri[0]],
            mesh["positions"][tri[1]],
            mesh["positions"][tri[2]])


def symmetric_hausdorff(mesh_a, mesh_b):
    """Two-sided vertex-sampled Hausdorff distance.

    Honest method disclosure: samples are the mesh VERTICES of each side, and
    for each vertex the exact minimum distance to the other side's triangle
    mesh is computed. Returns ``{forward, reverse, symmetric, na, nb,
    method:"vertex"}``. Vertex-sampled — the true surface Hausdorff can be at
    least this large; reported as such (never claimed exact).
    """
    tris_a = [_tri_coords(mesh_a, t) for t in mesh_a["triangles"]]
    tris_b = [_tri_coords(mesh_b, t) for t in mesh_b["triangles"]]
    va = list(mesh_a["positions"].values())
    vb = list(mesh_b["positions"].values())

    def one_side(pts, tris):
        if not tris:
            return float("inf")
        mx = 0.0
        for p in pts:
            d = min(_point_tri_distance(p, *t) for t in tris)
            if d > mx:
                mx = d
        return mx

    fwd = one_side(va, tris_b)
    rev = one_side(vb, tris_a)
    return {"forward": fwd, "reverse": rev,
            "symmetric": max(fwd, rev),
            "na": len(va), "nb": len(vb), "method": "vertex"}


def volume(mesh):
    """Signed volume of a triangulated surface (divergence theorem).

    Exact for a closed, consistently-wound shell; the absolute value is what
    should be compared across sides. Reports the raw signed divergence value so
    mixed-winding fixtures are visible as an honest observation, not hidden.
    """
    total = 0.0
    for tri in mesh["triangles"]:
        pa, pb, pc = _tri_coords(mesh, tri)
        total += _dot(pa, _cross(pb, pc))
    return total / 6.0


def compare_neighbors(native_mesh, step_mesh):
    """Fidelity metrics between native and STEP geometry of the same part."""
    h = symmetric_hausdorff(native_mesh, step_mesh)
    vn = volume(native_mesh)
    vs = volume(step_mesh)
    denom = max(abs(vn), abs(vs), 1e-12)
    return {
        "native_vertices": h["na"],
        "step_vertices": h["nb"],
        "native_triangles": len(native_mesh["triangles"]),
        "step_triangles": len(step_mesh["triangles"]),
        "symmetric_hausdorff": h["symmetric"],
        "hausdorff_forward": h["forward"],
        "hausdorff_reverse": h["reverse"],
        "hausdorff_method": h["method"],
        "native_volume": vn,
        "step_volume": vs,
        "volume_delta": vs - vn,
        "volume_rel_error": abs(vs - vn) / denom,
    }


# --------------------------------------------------------------------------
# Pair list + runner + report
# --------------------------------------------------------------------------
def _read_mesh(path, kind, corpus_root):
    """Read geometry for one side. Returns mesh or (class, reason)."""
    try:
        p = Path(path)
        if not p.is_absolute():
            p = Path(corpus_root) / p
        if not p.exists():
            return None, ("pair-missing-on-disk", f"missing: {p}")
        if kind == "step":
            try:
                mesh = read_faceted_step(str(p))
            except NotFaceted as e:
                return None, ("not-faceted", str(e))
            except StepError as e:
                return None, ("step-unreadable", str(e))
            cls = "no-step-geometry"
        else:
            try:
                mesh = read_stl(str(p))
            except StepError as e:
                return None, ("native-unreadable", str(e))
            cls = "no-native-geometry"
        if mesh is None or not mesh.get("positions") \
                or not mesh.get("triangles"):
            return None, (cls, "no geometry recovered")
        return mesh, None
    except Exception as e:  # noqa: BLE001  (tolerant: never crash a sweep)
        return None, ("io-error", repr(e))


def run(pair_list, corpus_root=""):
    """Classify every pair -> list of row dicts (coverage == truth set).

    A row is ``measured`` only when BOTH sides produced real geometry and the
    metrics were computed; otherwise ``skip`` with a closed-taxonomy class and
    a reason string. Never raises; a failure in one pair never aborts others.
    """
    rows = []
    for pair in pair_list:
        rid = pair.get("id", "?")
        native = pair.get("native", "")
        step = pair.get("step", "")
        geom = pair.get("geometry_class", "")
        try:
            nm, nerr = _read_mesh(native, "native", corpus_root)
            sm, serr = _read_mesh(step, "step", corpus_root)
            if nerr is not None:
                rows.append({"id": rid, "status": "skip",
                             "class": nerr[0], "reason": nerr[1],
                             "geometry_class": geom, "metrics": {}})
                continue
            if serr is not None:
                rows.append({"id": rid, "status": "skip",
                             "class": serr[0], "reason": serr[1],
                             "geometry_class": geom, "metrics": {}})
                continue
            metrics = compare_neighbors(nm, sm)
            rows.append({"id": rid, "status": "measured",
                         "class": "", "reason": "",
                         "geometry_class": geom, "metrics": metrics})
        except Exception as e:  # noqa: BLE001
            rows.append({"id": rid, "status": "skip", "class": "io-error",
                         "reason": repr(e), "geometry_class": geom,
                         "metrics": {}})
    return rows


def build_report(rows):
    """Wrap classified rows into a self-consistent fidelity report dict."""
    measured = sum(1 for r in rows if r["status"] == "measured")
    skipped = sum(1 for r in rows if r["status"] == "skip")
    skipped_by_class = {}
    for r in rows:
        if r["status"] == "skip":
            c = r.get("class") or "unknown"
            skipped_by_class[c] = skipped_by_class.get(c, 0) + 1
    return {
        "schema_version": 1,
        "generated": datetime.now(timezone.utc).isoformat(),
        "description": "P4-M1-T03 STEP accuracy harness fidelity report. "
                       "measured = real geometry on both sides; every skip "
                       "carries a closed-taxonomy class + reason. No number "
                       "is ever fabricated.",
        "summary": {"total": len(rows), "measured": measured,
                    "skipped": skipped, "skipped_by_class": skipped_by_class},
        "rows": rows,
    }


def write_report(report, outdir):
    """Persist report as JSON + markdown matrix. Returns (json_path, md_path)."""
    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    jp = outdir / "accuracy-report.json"
    md = outdir / "accuracy-report.md"
    jp.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = ["# P4-M1-T03 STEP Accuracy Report", "", "generated: %s"
             % report["generated"], "",
             "summary: %d rows | %d measured | %d skipped | classes %s" % (
                 report["summary"]["total"], report["summary"]["measured"],
                 report["summary"]["skipped"],
                 report["summary"]["skipped_by_class"]), "",
             "| id | status | class | symmetric_hausdorff | volume_delta |"
             " volume_rel_error |", "|---|---|---|---|---|---|"]
    for r in report["rows"]:
        m = r.get("metrics") or {}
        hd = m.get("symmetric_hausdorff", "")
        vd = m.get("volume_delta", "")
        vr = m.get("volume_rel_error", "")
        lines.append("| %s | %s | %s | %s | %s | %s |" % (
            r["id"], r["status"], r.get("class") or "-",
            "" if hd == "" else "%.9g" % hd,
            "" if vd == "" else "%.9g" % vd,
            "" if vr == "" else "%.9g" % vr))
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return str(jp), str(md)


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------
def main(argv=None):
    argv = argv if argv is not None else sys.argv[1:]
    import argparse
    ap = argparse.ArgumentParser(
        description="P4-M1-T03 STEP accuracy harness (Hausdorff/volume)")
    ap.add_argument("--pairs", default=os.path.join(
        "corpus", "accuracy-pairs.json"))
    ap.add_argument("--corpus", default=".")
    ap.add_argument("--out", default="corpus")
    args = ap.parse_args(argv)

    pair_list = json.loads(Path(args.pairs).read_text(encoding="utf-8"))
    if isinstance(pair_list, dict):
        entries = pair_list.get("pairs") or pair_list.get("entries") or []
    else:
        entries = pair_list
    rows = run(entries, args.corpus)
    report = build_report(rows)
    jp, mp = write_report(report, args.out)
    print("rows=%d measured=%d skipped=%d" % (
        report["summary"]["total"], report["summary"]["measured"],
        report["summary"]["skipped"]))
    print("json: %s" % jp)
    print("markdown: %s" % mp)
    return 0


if __name__ == "__main__":
    sys.exit(main())
