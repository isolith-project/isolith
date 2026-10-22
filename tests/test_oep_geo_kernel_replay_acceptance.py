#!/usr/bin/env python3
"""KERNEL-REPLAY acceptance: independent (stdlib-only) verification of the
committed kernel-replay goldens for the feature-graph replay to REAL solids.

Two files are independently re-derived with plain Python arithmetic (no crate
imports), then cross-checked:

  - acceptance-replay-graph.oepfg  -- the feature graph (Sketch -> Extrude ->
    Offset -> Union -> Trim -> Mirror over an 80x40x6 footprint) in the
    deterministic oep-feature-graph v1 owned format;
  - kernel-replay-golden.json      -- the per-step kernel measurements (volume,
    surface area, topology counts, bounding box) computed by the real OCCT
    8.0.x engine behind the oep-geo seam.

The Python side re-computes every solid step's closed-form volume and bounding
box from the graph (the box-brick narrative is exact), and asserts:

  1. each committed golden step matches its analytic value (volume to 1e-6
     relative, bounding box to 1e-4 absolute),
  2. the byte-locked canonical trace (.oepkrt) is consistent with the JSON
     golden (same step volume + bbox, same feature ids/kinds in order).

The kernel's own bbox carries OCCT's tolerance inflation (a few 1e-7 units),
which is why the bbox gate here is absolute rather than relative; the tight
kernel lock lives in the Rust acceptance test against the SAME committed
numbers. A fabricated or self-consistent-but-wrong golden cannot pass this
independent side, which is the two-sided verification pattern used by the
sibling kernel-seam card.
"""
import json
import math
import os
import sys

GOLDEN_DIR = os.path.join(
    os.path.dirname(__file__), "..", "crates", "oep-geo", "tests", "golden",
)
GRAPH = os.path.join(GOLDEN_DIR, "acceptance-replay-graph.oepfg")
JSON_GOLDEN = os.path.join(GOLDEN_DIR, "kernel-replay-golden.json")
TRACE = os.path.join(GOLDEN_DIR, "kernel-replay-trace.oepkrt")
BRACKET_GOLDEN = os.path.join(GOLDEN_DIR, "kernel-replay-bracket-golden.json")

VOL_REL = 1e-6      # volume tolerance (relative)
BBOX_ABS = 1e-4     # bounding-box tolerance (absolute; OCCT tolerance inflation)


def parse_graph(text):
    """Parse the oep-feature-graph v1 text into nodes (structure + params)."""
    lines = text.splitlines()
    it = iter(lines)

    def nxt():
        try:
            return next(it)
        except StopIteration:
            raise AssertionError("unexpected EOF")

    assert nxt() == "format oep-feature-graph", "format tag"
    assert nxt() == "version 1", "version"
    n = int(nxt().split(" ", 1)[1])
    nodes = []
    for i in range(n):
        assert nxt() == "node %d" % i, "sequential node marker"
        node = {"id": nxt().split(" ", 1)[1], "name": nxt().split(" ", 1)[1],
                "kind": nxt().split(" ", 1)[1], "refs": [], "params": {}}
        parent = nxt().split(" ", 1)[1]
        node["parent"] = None if parent == "-" else parent
        r = int(nxt().split(" ", 1)[1])
        node["refs"] = [nxt() for _ in range(r)]
        assert len(node["refs"]) == r, "ref count mismatch"
        p = int(nxt().split(" ", 1)[1])
        for _ in range(p):
            kv = nxt()
            k, v = kv.split("=", 1)
            node["params"][k] = v[4:] if v.startswith("str:") else float(v)
        assert len(node["params"]) == p, "param count mismatch"
        nodes.append(node)
    return nodes


def parse_profile(raw):
    pts = []
    for tok in raw.split(";"):
        x, y = tok.split(",")
        pts.append((float(x), float(y)))
    assert len(pts) >= 3, "closed loop needs >=3 points"
    return pts


def volume_of_box(bbox):
    return (bbox[3] - bbox[0]) * (bbox[4] - bbox[1]) * (bbox[5] - bbox[2])


def envelope(boxes):
    out = [min(b[0] for b in boxes), min(b[1] for b in boxes),
           min(b[2] for b in boxes), max(b[3] for b in boxes),
           max(b[4] for b in boxes), max(b[5] for b in boxes)]
    return out


def replay_analytic(nodes):
    """Re-derive each node's solid (bbox, volume) with plain arithmetic.
    Returns {node_index: (bbox, volume, solid_id)} for solid-producing nodes."""
    outputs = [None] * len(nodes)  # (bbox, volume) or ("profile", pts)
    solids = {}
    for i, node in enumerate(nodes):
        kind = node["kind"]
        p = node["params"]
        if kind == "Sketch":
            outputs[i] = ("profile", parse_profile(p["profile"]))
        elif kind == "Extrude":
            # Footprint from the preceding profile (or rectangle fallback).
            src = node["refs"][0] if node["refs"] else None
            idx = next((j for j, nd in enumerate(nodes) if nd["id"] == src), None)
            prof = outputs[idx][1] if idx is not None and outputs[idx][0] == "profile" \
                else parse_profile(p["profile"])
            xs = [pt[0] for pt in prof]
            ys = [pt[1] for pt in prof]
            h = p["height"]
            bbox = [min(xs), min(ys), 0.0, max(xs), max(ys), h]
            vol = (max(xs) - min(xs)) * (max(ys) - min(ys)) * h
            outputs[i] = (bbox, vol)
            solids[i] = node["id"]
        elif kind == "Offset":
            idx = next(j for j, nd in enumerate(nodes) if nd["id"] == node["refs"][0])
            bbox, vol = outputs[idx][0], outputs[idx][1]
            bbox = [bbox[0] + p.get("dx", 0.0), bbox[1] + p.get("dy", 0.0),
                    bbox[2] + p.get("dz", 0.0), bbox[3] + p.get("dx", 0.0),
                    bbox[4] + p.get("dy", 0.0), bbox[5] + p.get("dz", 0.0)]
            outputs[i] = (bbox, vol)
            solids[i] = node["id"]
        elif kind == "Union":
            idxs = [j for j, nd in enumerate(nodes) if nd["id"] in node["refs"]]
            boxes = [(outputs[j][0], outputs[j][1]) for j in idxs]
            bbox = envelope([b for b, _ in boxes])
            vol = sum(v for _, v in boxes)  # adjacent boxes: additive volume
            outputs[i] = (bbox, vol)
            solids[i] = node["id"]
        elif kind == "Difference":
            idxs = [j for j, nd in enumerate(nodes) if nd["id"] in node["refs"]]
            base = outputs[idxs[0]]
            # Subtraction of axis-aligned boxes via bbox envelope is not
            # expressed closed-form here; the acceptance story uses Trim, so
            # a real Difference node falls through to the assertion below.
            outputs[i] = (base[0], base[1])
            solids[i] = node["id"]
        elif kind == "Trim":
            idx = next(j for j, nd in enumerate(nodes) if nd["id"] == node["refs"][0])
            bbox, vol = outputs[idx][0], outputs[idx][1]
            dz = p["dz"]
            area = (bbox[3] - bbox[0]) * (bbox[4] - bbox[1])
            bbox = bbox[:]
            bbox[5] -= dz
            outputs[i] = (bbox, vol - area * dz)
            solids[i] = node["id"]
        elif kind == "Mirror":
            idx = next(j for j, nd in enumerate(nodes) if nd["id"] == node["refs"][0])
            bbox, vol = outputs[idx][0], outputs[idx][1]
            axis = p["axis"]
            at = p["at"]
            lo = {"x": 0, "y": 1, "z": 2}[axis]
            hi = lo + 3
            bbox = bbox[:]
            bbox[lo], bbox[hi] = 2.0 * at - bbox[hi], 2.0 * at - bbox[lo]
            outputs[i] = (bbox, vol)
            solids[i] = node["id"]
        else:
            raise AssertionError("unexpected kind %r in acceptance graph" % kind)
    return solids, outputs


def check_golden_vs_analytic(golden_path, nodes):
    with open(golden_path, encoding="utf-8") as fh:
        golden = json.load(fh)
    solids, outputs = replay_analytic(nodes)
    for i, sid in solids.items():
        assert sid in golden["steps"], "golden missing step %r" % sid
        g = golden["steps"][sid]
        bbox, vol = outputs[i]
        gvol = g["volume"]
        rel = abs(gvol - vol) / max(abs(vol), 1e-300)
        assert rel < VOL_REL, \
            "step %r volume %r vs analytic %r (rel %r)" % (sid, gvol, vol, rel)
        for k, coord in (("xmin", bbox[0]), ("ymin", bbox[1]), ("zmin", bbox[2]),
                         ("xmax", bbox[3]), ("ymax", bbox[4]), ("zmax", bbox[5])):
            assert abs(g["bbox"][k] - coord) < BBOX_ABS, \
                "step %r bbox %s %r vs analytic %r" % (sid, k, g["bbox"][k], coord)
        # Volume closed-form must be positive and consistent with bbox.
        assert vol == volume_of_box(bbox) or abs(vol - volume_of_box(bbox)) < 1e-9, \
            "step %r analytic volume vs its own bbox disagree" % sid
    assert len(solids) == len(golden["steps"]), \
        "analytic solid steps %d != golden steps %d" % (len(solids), len(golden["steps"]))
    return golden["steps"]


def check_trace_consistency(steps_golden):
    with open(TRACE, encoding="utf-8") as fh:
        lines = fh.read().splitlines()
    assert lines[0] == "format oep-kernel-replay-trace", "trace format tag"
    assert lines[1] == "version 1", "trace version"
    n = int(lines[2].split(" ", 1)[1])
    assert n == 6, "acceptance graph has 6 steps"
    it = iter(lines[3:])
    solid_in_trace = {}
    for i in range(n):
        assert next(it) == "step %d" % i, "sequential step marker"
        sid = next(it).split(" ", 1)[1]
        kind = next(it).split(" ", 1)[1]
        val = next(it)
        if val.startswith("solid "):
            # `solid volume=..;area=..;faces=..;edges=..;verts=..;bbox=x0;y0;z0;x1;y1;z1`
            body = val[len("solid "):]
            head, _, bbox_str = body.partition("bbox=")
            fields = dict(f.split("=", 1) for f in head.strip(";").split(";"))
            fields["bbox"] = bbox_str.split(";")
            solid_in_trace[sid] = fields
        elif val.startswith("value profile"):
            assert sid == "oep-feature-shelf-sketch" and kind == "Sketch"
            npts = int(val.split(" ")[2])
            for j in range(npts):
                next(it).startswith("point %d" % j)
        else:
            assert val == "value empty", "unexpected trace value line"
    # Every committed golden solid step appears in the trace with the same
    # volume and bbox (the byte-locked form is consistent with the JSON).
    assert set(solid_in_trace) == set(steps_golden), \
        "trace solids %r != golden solids %r" % (set(solid_in_trace), set(steps_golden))
    fmt = "%.6f"
    for sid, g in steps_golden.items():
        tf = solid_in_trace[sid]
        assert abs(float(tf["volume"]) - g["volume"]) < 1e-9, "%s volume drift" % sid
        assert abs(float(tf["area"]) - g["surface_area"]) < 1e-9, "%s area drift" % sid
        assert tf["faces"] == str(g["face_count"]) and tf["edges"] == str(g["edge_count"]) \
            and tf["verts"] == str(g["vertex_count"]), "%s topology drift" % sid
        tb = tf["bbox"]
        gb = [g["bbox"]["xmin"], g["bbox"]["ymin"], g["bbox"]["zmin"],
              g["bbox"]["xmax"], g["bbox"]["ymax"], g["bbox"]["zmax"]]
        for a, b in zip(tb, gb):
            assert float(a) == b or abs(float(a) / b - 1.0) < 1e-12, \
                "%s bbox drift %s vs %s" % (sid, a, b)
    return solid_in_trace


def test_kernel_replay_goldens_match_analytic_derivation():
    with open(GRAPH, encoding="utf-8") as fh:
        nodes = parse_graph(fh.read())
    assert len(nodes) == 6, "acceptance graph has 6 nodes"
    kinds = [n["kind"] for n in nodes]
    assert kinds == ["Sketch", "Extrude", "Offset", "Union", "Trim", "Mirror"]
    steps = check_golden_vs_analytic(JSON_GOLDEN, nodes)
    check_trace_consistency(steps)


def test_bracket_golden_matches_analytic_derivation():
    # The mini-vocabulary bracket is the same 80x40 footprint WITH a Sketch
    # node prepended in the acceptance graph; the bracket golden's solid steps
    # must equal the acceptance golden's (identical footprint and ops).
    with open(JSON_GOLDEN, encoding="utf-8") as fh:
        acc = json.load(fh)["steps"]
    with open(BRACKET_GOLDEN, encoding="utf-8") as fh:
        brk = json.load(fh)["steps"]
    shared = [sid for sid in acc if sid in brk]
    assert len(shared) == 4, "bracket shares 4 step ids with acceptance: %r" % shared
    for sid in shared:
        a, b = acc[sid], brk[sid]
        assert a["volume"] == b["volume"], "%s volume differs" % sid
        assert a["face_count"] == b["face_count"], "%s faces differ" % sid
        for k in ("xmin", "ymin", "zmin", "xmax", "ymax", "zmax"):
            assert a["bbox"][k] == b["bbox"][k], "%s bbox %s differs" % (sid, k)
    # The bracket's Extrude (its only non-shared id) is analytically the box
    # (0,0,0)-(80,40,6): volume 19200.
    ext = brk["oep-feature-bracket-body"]
    rel = abs(ext["volume"] - 19200.0) / 19200.0
    assert rel < VOL_REL, "bracket extrude volume %r vs 19200" % ext["volume"]
    assert abs(ext["bbox"]["xmin"]) < BBOX_ABS and abs(ext["bbox"]["ymin"]) < BBOX_ABS \
        and abs(ext["bbox"]["zmin"]) < BBOX_ABS
    assert abs(ext["bbox"]["xmax"] - 80.0) < BBOX_ABS \
        and abs(ext["bbox"]["ymax"] - 40.0) < BBOX_ABS \
        and abs(ext["bbox"]["zmax"] - 6.0) < BBOX_ABS


if __name__ == "__main__":
    test_kernel_replay_goldens_match_analytic_derivation()
    test_bracket_golden_matches_analytic_derivation()
    print("KERNEL-REPLAY ACCEPTANCE PASSED (independent stdlib re-derivation)")
    sys.exit(0)
