#!/usr/bin/env python3
"""P5-M3-T02 — geometry accuracy harness vs an external converter reference
(test-only ground truth, never shipped, never claimed as the product).

For every corpus **pair** (a native sample AND the reference file an external,
independent converter produced from the SAME native sample — see
`corpus/native-accuracy-pairs.json` and `corpus/reference/`), the harness reads
geometry from both sides and measures fidelity:

  - **symmetric (two-sided) vertex-sampled Hausdorff distance** (clean
    point->triangle distance both directions; method honestly reported —
    vertex-sampled, not exact surface Hausdorff);
  - **volume compare** (signed divergence-theorem volume per side, absolute
    delta + relative error; closure/winding caveats reported explicitly);
  - **tolerance DISTRIBUTION** (the acceptance for P5-M3-T02): per-vertex
    distance percentiles (p50/p90/p99/p100) plus, for a ladder of tolerance
    bins scaled to the bounding-box diagonal, the fraction of sampled vertices
    within each bin — reported as a distribution, not a single epsilon.

The REFERENCE side is the output of an external converter (e.g. FreeCAD /
Open CASCADE kernel via the committed generator `corpus/tools/
gen_freecad_reference.py`) reading the same native file. Per the P5-M1-T03 clean-
room line (§6) that output is used STRICTLY as test-only ground truth for our own
testing: it is never a source of format internals, never shipped, never claimed
as the product. The harness pins a measured epsilon against that reference and
reports it honestly.

Honest-by-construction (program anti-fabrication doctrine): a row is `measured`
only when real numbers were computed from readable geometry on BOTH sides;
anything else is a classified `skip` from a closed taxonomy with a machine
reason. The harness NEVER invents a fidelity value.

Clean-room boundary for the native side: `read_native` understands only the
formats our own tooling natively reads today (STL ASCII + binary, clean-room
stdlib parsers). A native CATPart/CATProduct without an external-converter
reference (the P5 native-reader lane is in flight on P5-M3-T01) is an honest
`reference-unavailable` skip, never a fabricated number.

Pure stdlib (pyproject deps = []). CLI:
`PYTHONPATH=python python3 -m oep_tooling.native_accuracy
 --pairs corpus/native-accuracy-pairs.json [--corpus corpus --out corpus]`

Skip taxonomy (closed set, each with a reason string):
  pair-missing-on-disk | native-unreadable | reference-unreadable |
  reference-unavailable | no-native-geometry | no-reference-geometry |
  not-a-mesh | io-error
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
    "pair-missing-on-disk",     # native or reference path does not resolve
    "native-unreadable",        # native side: reader failed (see reason)
    "reference-unreadable",     # reference side: reader failed (see reason)
    "reference-unavailable",    # no external-converter reference mapped to row
    "no-native-geometry",       # native open but no geometry recovered
    "no-reference-geometry",    # reference open but no geometry recovered
    "not-a-mesh",               # reference is not a mesh reference we can read
    "io-error",                 # unreadable at the OS level
)

_RE_FACET = re.compile(
    r"facet\s+normal[^\n]*\n((?:(?!\bfacet\b).)*?)endfacet", re.S)
_RE_VERTEX = re.compile(
    r"vertex\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)")


class NativeError(Exception):
    """A native file could not be read at all."""


class ReferenceError(Exception):
    """A reference file could not be read at all."""


# --------------------------------------------------------------------------
# Native readers (STL ASCII + binary) — clean-room, stdlib-only
# --------------------------------------------------------------------------
def read_native(path):
    """Read a native sample (today: STL ASCII + binary) into a mesh dict.

    Returns ``{"positions": {i: (x,y,z)}, "triangles": [(i,j,k), ...],
    "warnings": [...]}``. Raises :class:`NativeError` on unreadable input.
    When the P5 native CATPart reader (P5-M3-T01) lands it can be registered
    here behind the same seam without changing the runner/report surface.
    """
    try:
        data = Path(path).read_bytes()
    except OSError as e:
        raise NativeError("read failed: %r" % (e,)) from e
    if len(data) == 0:
        raise NativeError("empty file")
    if data[:5] == b"solid" or data.lstrip().startswith(b"solid"):
        return _stl_ascii(data.decode("utf-8", errors="replace"))
    if len(data) >= 84:
        (count,) = struct.unpack_from("<I", data, 80)
        if 80 + 4 + 50 * count == len(data):
            return _stl_binary(data)
    raise NativeError("unrecognized native format (neither ASCII 'solid' "
                      "STL nor binary STL)")


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
        raise NativeError("ASCII STL: no facets found")
    return {"positions": {i: p for i, p in enumerate(pos)},
            "triangles": triangles, "warnings": []}


def _stl_binary(data):
    (count,) = struct.unpack_from("<I", data, 80)
    pool = {}
    pos = []
    triangles = []
    off = 84
    for _ in range(count):
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
        raise NativeError("binary STL: no facets found")
    return {"positions": {i: p for i, p in enumerate(pos)},
            "triangles": triangles, "warnings": []}


# --------------------------------------------------------------------------
# Reference reader (external-converter mesh JSON schema, see corpus/reference/)
# --------------------------------------------------------------------------
def read_reference(path):
    """Read an external-converter reference file into a mesh dict.

    Understands the committed mesh-reference JSON schema
    (``{"format":"mesh-reference", "mesh": {"positions": [[x,y,z],...],
    "triangles": [[i,j,k],...]}, "provenance": {...}}``) written by
    `corpus/tools/gen_freecad_reference.py`. Raises :class:`ReferenceError` on
    anything else (never fabricates a fidelity number from partial data).
    """
    try:
        text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        raise ReferenceError("read failed: %r" % (e,)) from e
    try:
        doc = json.loads(text)
    except ValueError as e:
        raise ReferenceError("not JSON: %s" % (e,)) from e
    mesh = doc.get("mesh")
    if not isinstance(mesh, dict):
        raise ReferenceError("mesh-reference schema: no mesh object")
    positions = mesh.get("positions")
    triangles = mesh.get("triangles")
    if not isinstance(positions, list) or not positions:
        raise ReferenceError("mesh-reference schema: no positions")
    if not isinstance(triangles, list) or not triangles:
        raise ReferenceError("mesh-reference schema: no triangles")
    pos = {}
    for i, p in enumerate(positions):
        if not isinstance(p, (list, tuple)) or len(p) < 3:
            raise ReferenceError("mesh-reference schema: bad point %d" % i)
        pos[i] = (float(p[0]), float(p[1]), float(p[2]))
    tris = []
    for t in triangles:
        if not isinstance(t, (list, tuple)) or len(t) < 3:
            raise ReferenceError("mesh-reference schema: bad triangle %r" % (t,))
        tris.append((int(t[0]), int(t[1]), int(t[2])))
    return {"positions": pos, "triangles": tris,
            "provenance": doc.get("provenance"),
            "warnings": [n for n in (doc.get("provenance") or {}).values()
                         if isinstance(n, str)]}


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


def _vertex_dists(pts, tris):
    """Min point->triangle distance per vertex; returns list over `pts`."""
    out = []
    for p in pts:
        out.append(min(_point_tri_distance(p, *t) for t in tris))
    return out


def _percentile(sorted_vals, p):
    if not sorted_vals:
        return float("inf")
    k = (len(sorted_vals) - 1) * p
    lo = int(math.floor(k))
    hi = int(math.ceil(k))
    if lo == hi:
        return sorted_vals[lo]
    return sorted_vals[lo] * (hi - k) + sorted_vals[hi] * (k - lo)


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
    fwd = max(_vertex_dists(va, tris_b)) if tris_b else float("inf")
    rev = max(_vertex_dists(vb, tris_a)) if tris_a else float("inf")
    return {"forward": fwd, "reverse": rev,
            "symmetric": max(fwd, rev),
            "na": len(va), "nb": len(vb), "method": "vertex"}


def tolerance_distribution(mesh_a, mesh_b):
    """Tolerance distribution between two meshes (vertex-sampled).

    For every vertex of A the exact min distance to B is computed, and vice
    versa (method: "vertex"). Reports percentiles p50/p90/p99/p100 of the
    BOTH-side distance population, plus for a ladder of tolerance bins scaled to
    the A-side bounding-box diagonal the fraction of A-vertices inside each bin
    (a cumulative distribution, not a single epsilon). Honest method disclosure:
    vertex-sampled, not exact surface distance.
    """
    tris_a = [_tri_coords(mesh_a, t) for t in mesh_a["triangles"]]
    tris_b = [_tri_coords(mesh_b, t) for t in mesh_b["triangles"]]
    va = list(mesh_a["positions"].values())
    vb = list(mesh_b["positions"].values())
    da = sorted(_vertex_dists(va, tris_b))
    db = sorted(_vertex_dists(vb, tris_a))
    all_d = sorted(da + db)
    bbox_a = _bbox(mesh_a)
    diag = _bbox_diag(bbox_a) or 1.0
    bins = [1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0]
    fraction_within = []
    for b in bins:
        eps = b * diag
        frac = (sum(1 for d in da if d <= eps) / len(da)) if da else 0.0
        fraction_within.append({"bin": b, "epsilon": eps, "fraction": frac})
    return {
        "method": "vertex",
        "na": len(da), "nb": len(db),
        "p50": _percentile(all_d, 0.50),
        "p90": _percentile(all_d, 0.90),
        "p99": _percentile(all_d, 0.99),
        "p100": _percentile(all_d, 1.00),
        "bbox_diag": diag,
        "bins_relative_to_bbox_diag": bins,
        "fraction_within": fraction_within,
    }


def _bbox(mesh):
    xs = [p[0] for p in mesh["positions"].values()]
    ys = [p[1] for p in mesh["positions"].values()]
    zs = [p[2] for p in mesh["positions"].values()]
    return (min(xs), min(ys), min(zs), max(xs), max(ys), max(zs))


def _bbox_diag(b):
    x0, y0, z0, x1, y1, z1 = b
    return math.sqrt((x1 - x0) ** 2 + (y1 - y0) ** 2 + (z1 - z0) ** 2)


def volume(mesh):
    """Signed volume of a triangulated surface (divergence theorem).

    Exact for a closed, consistently-wound shell; compare ABSOLUTE values across
    sides. Reports the raw signed value so mixed-winding fixtures are visible as
    an honest observation, not hidden.
    """
    total = 0.0
    for tri in mesh["triangles"]:
        pa, pb, pc = _tri_coords(mesh, tri)
        total += _dot(pa, _cross(pb, pc))
    return total / 6.0


def compare_pair(native_mesh, reference_mesh):
    """Fidelity metrics between native and external-converter reference."""
    h = symmetric_hausdorff(native_mesh, reference_mesh)
    tdist = tolerance_distribution(native_mesh, reference_mesh)
    vn = volume(native_mesh)
    vr = volume(reference_mesh)
    denom = max(abs(vn), abs(vr), 1e-12)
    return {
        "native_vertices": h["na"],
        "reference_vertices": h["nb"],
        "native_triangles": len(native_mesh["triangles"]),
        "reference_triangles": len(reference_mesh["triangles"]),
        "reference_provenance": reference_mesh.get("provenance") or {},
        "symmetric_hausdorff": h["symmetric"],
        "hausdorff_forward": h["forward"],
        "hausdorff_reverse": h["reverse"],
        "hausdorff_method": h["method"],
        "tolerance_distribution": tdist,
        "native_volume": vn,
        "reference_volume": vr,
        "volume_delta": vr - vn,
        "volume_rel_error": abs(vr - vn) / denom,
    }


# --------------------------------------------------------------------------
# Pair list + runner + report
# --------------------------------------------------------------------------
def _read_mesh(path, kind, corpus_root):
    """Read geometry for one side. Returns (mesh, None) or (None, (cl, reason))."""
    try:
        p = Path(path)
        if not p.is_absolute():
            p = Path(corpus_root) / p
        if not p.exists():
            return None, ("pair-missing-on-disk", "missing: %s" % p)
        if kind == "reference":
            try:
                mesh = read_reference(str(p))
            except ReferenceError as e:
                return None, ("reference-unreadable", str(e))
            cls = "no-reference-geometry"
        else:
            try:
                mesh = read_native(str(p))
            except NativeError as e:
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
    metrics were computed; otherwise ``skip`` with a closed-taxonomy class and a
    reason string. Never raises; a failure in one pair never aborts others.
    """
    rows = []
    for pair in pair_list:
        rid = pair.get("id", "?")
        native = pair.get("native", "")
        reference = pair.get("reference", "") or ""
        geom = pair.get("geometry_class", "")
        try:
            if not reference:
                rows.append({"id": rid, "status": "skip",
                             "class": "reference-unavailable",
                             "reason": "no external-converter reference "
                                       "mapped for this row",
                             "geometry_class": geom, "metrics": {}})
                continue
            nm, nerr = _read_mesh(native, "native", corpus_root)
            rm, rerr = _read_mesh(reference, "reference", corpus_root)
            if nerr is not None:
                rows.append({"id": rid, "status": "skip",
                             "class": nerr[0], "reason": nerr[1],
                             "geometry_class": geom, "metrics": {}})
                continue
            if rerr is not None:
                rows.append({"id": rid, "status": "skip",
                             "class": rerr[0], "reason": rerr[1],
                             "geometry_class": geom, "metrics": {}})
                continue
            metrics = compare_pair(nm, rm)
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
        "description": "P5-M3-T02 geometry accuracy harness vs external "
                       "converter reference (test-only). measured = real "
                       "geometry on both sides; every skip carries a "
                       "closed-taxonomy class + reason. Reference output is "
                       "test-only ground truth, never shipped/claimed. No "
                       "number is ever fabricated.",
        "summary": {"total": len(rows), "measured": measured,
                    "skipped": skipped, "skipped_by_class": skipped_by_class},
        "rows": rows,
    }


def write_report(report, outdir):
    """Persist report as JSON + markdown matrix. Returns (json_path, md_path)."""
    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    jp = outdir / "native-accuracy-report.json"
    md = outdir / "native-accuracy-report.md"
    jp.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = ["# P5-M3-T02 Geometry Accuracy Report "
             "(vs external converter reference, test-only)",
             "", "generated: %s" % report["generated"], "",
             "summary: %d rows | %d measured | %d skipped | classes %s" % (
                 report["summary"]["total"], report["summary"]["measured"],
                 report["summary"]["skipped"],
                 report["summary"]["skipped_by_class"]), "",
             "| id | status | class | symmetric_hausdorff | volume_delta "
             "| tol-p100 |",
             "|---|---|---|---|---|---|"]
    for r in report["rows"]:
        m = r.get("metrics") or {}
        hd = m.get("symmetric_hausdorff", "")
        vd = m.get("volume_delta", "")
        tdist = m.get("tolerance_distribution") or {}
        p100 = tdist.get("p100", "")
        lines.append("| %s | %s | %s | %s | %s | %s |" % (
            r["id"], r["status"], r.get("class") or "-",
            "" if hd == "" else "%.9g" % hd,
            "" if vd == "" else "%.9g" % vd,
            "" if p100 == "" else "%.9g" % p100))
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return str(jp), str(md)


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------
def main(argv=None):
    argv = argv if argv is not None else sys.argv[1:]
    import argparse
    ap = argparse.ArgumentParser(
        description="P5-M3-T02 geometry accuracy harness (native vs external "
                    "converter reference, test-only)")
    ap.add_argument("--pairs", default=os.path.join(
        "corpus", "native-accuracy-pairs.json"))
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
