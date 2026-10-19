#!/usr/bin/env python3
"""P6-M3-T03 acceptance test: independent (stdlib-only) verification of the
committed oep-geo feature-graph golden artifact.

TDD: written BEFORE `oep_geo::feature_graph` existed. When the module is absent
the golden file is absent and every assertion fails (RED). When the module
lands, the golden file is produced BY the crate (example gen_feature_graph_golden)
and this script independently re-derives the same replay trace from the graph
with plain Python arithmetic (no crate imports) and cross-checks within 1e-9.

This is the "two-sided" verification pattern used by sibling cards: the crate's
own golden bytes (graph + replay trace) must agree with an independent
reference, so a self-consistent (but wrong) replay engine cannot pass.
"""
import os
import sys

GOLDEN = os.path.join(
    os.path.dirname(__file__),
    "..", "crates", "oep-geo", "tests", "golden", "bracket-feature-graph.oepfg",
)
EPS = 1e-9


def parse_doc(text):
    """Parse the `oep-feature-graph` v1 text into (nodes, steps).

    Nodes: list of dicts {id, name, kind, parent, refs, params}.
    Steps: list of dicts {id, kind, box}.
    Raises AssertionError on any structural violation (mirrors Rust strictness).
    """
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
    seen = set()
    for i in range(n):
        assert nxt() == "node %d" % i, "sequential node marker"
        node = {}
        node["id"] = nxt().split(" ", 1)[1]
        assert node["id"], "non-empty id"
        assert node["id"] not in seen, "duplicate id"
        seen.add(node["id"])
        node["name"] = nxt().split(" ", 1)[1]
        node["kind"] = nxt().split(" ", 1)[1]
        parent = nxt().split(" ", 1)[1]
        node["parent"] = None if parent == "-" else parent
        r = int(nxt().split(" ", 1)[1])
        node["refs"] = [nxt() for _ in range(r)]
        assert len(node["refs"]) == r, "ref count mismatch"
        p = int(nxt().split(" ", 1)[1])
        node["params"] = {}
        for _ in range(p):
            kv = nxt()
            k, v = kv.split("=", 1)
            assert k, "non-empty param name"
            if v.startswith("str:"):
                node["params"][k] = ("str", v[4:])
            else:
                node["params"][k] = ("num", float(v))
        assert len(node["params"]) == p, "param count mismatch"
        nodes.append(node)

    m = int(nxt().split(" ", 1)[1])
    steps = [None] * m
    for j in range(m):
        assert nxt() == "step %d" % j, "sequential step marker"
        sid = nxt().split(" ", 1)[1]
        kind = nxt().split(" ", 1)[1]
        box = [float(x) for x in nxt().split(" ", 1)[1].split(";")]
        assert len(box) == 6, "six bbox coords"
        steps[j] = {"id": sid, "kind": kind, "box": box}
    assert not list(it), "trailing junk line"
    return nodes, steps


def num(node, key, default=None):
    v = node["params"].get(key)
    if v is None:
        return default
    assert v[0] == "num", "expected numeric param %s" % key
    return v[1]


def envelope(boxes):
    assert boxes, "envelope needs at least one box"
    return [
        min(b[0] for b in boxes), min(b[1] for b in boxes), min(b[2] for b in boxes),
        max(b[3] for b in boxes), max(b[4] for b in boxes), max(b[5] for b in boxes),
    ]


def op_output(op_kind, node, inputs):
    """Independent re-derivation of a node's bbox from the documented OEP mini
    vocabulary. `inputs` are the resolved input bboxes (refs' outputs, else
    the parent's output, else the start box for roots)."""
    if op_kind == "Extrude":
        box = [0.0, 0.0, 0.0,
               num(node, "length"), num(node, "width"), num(node, "height")]
        return envelope(inputs + [box])
    if op_kind == "Offset":
        dx = num(node, "dx", 0.0)
        dy = num(node, "dy", 0.0)
        dz = num(node, "dz", 0.0)
        assert inputs, "Offset needs an input"
        b = inputs[-1]
        return [b[0] + dx, b[1] + dy, b[2] + dz, b[3] + dx, b[4] + dy, b[5] + dz]
    if op_kind == "Union":
        return envelope(inputs)
    if op_kind == "Trim":
        dz = num(node, "dz")
        assert inputs, "Trim needs an input"
        b = inputs[-1]
        return [b[0], b[1], b[2], b[3], b[4], b[5] - dz]
    raise AssertionError("unknown op kind %s" % op_kind)


def mirror_output(node, inputs):
    axis = node["params"].get("axis")
    assert axis and axis[0] == "str", "axis is a string param"
    axis = axis[1]
    assert axis in ("x", "y", "z"), "axis must be x/y/z"
    at = num(node, "at")
    assert inputs, "Mirror needs an input"
    b = inputs[-1]
    lo, hi = {"x": (0, 3), "y": (1, 4), "z": (2, 5)}[axis]
    out = list(b)
    out[lo] = 2 * at - b[hi]
    out[hi] = 2 * at - b[lo]
    return out


def main() -> int:
    if not os.path.exists(GOLDEN):
        print("FAIL: golden bracket-feature-graph.oepfg missing "
              "(crate not implemented / not generated)")
        return 1
    with open(GOLDEN) as f:
        text = f.read()
    nodes, committed = parse_doc(text)
    assert len(nodes) == 5, "five nodes in the bracket story"
    ids = [n["id"] for n in nodes]
    assert ids == [
        "oep-feature-bracket-body",
        "oep-feature-side-shelf",
        "oep-feature-body-union",
        "oep-feature-top-trim",
        "oep-feature-hub-mirror",
    ], "creation order"

    # Refs must be strictly backward (deterministic replay without topo sort).
    pos = {n["id"]: i for i, n in enumerate(nodes)}
    for n in nodes:
        for ref in n["refs"]:
            assert pos[ref] < pos[n["id"]], "forward ref: %s -> %s" % (n["id"], ref)

    # Independent replay over the graph.
    start = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    out = {}
    replay = []
    for n in nodes:
        if n["refs"]:
            inputs = [out[r] for r in n["refs"]]
        elif n["parent"]:
            inputs = [out[n["parent"]]]
        else:
            inputs = [start]
        if n["kind"] == "Mirror":
            b = mirror_output(n, inputs)
        else:
            b = op_output(n["kind"], n, inputs)
        out[n["id"]] = b
        replay.append({"id": n["id"], "kind": n["kind"], "box": b})

    # Cross-check against the trace the crate committed (graph + trace).
    assert len(committed) == len(replay) == 5, "trace length"
    ok = 0
    for want, got in zip(committed, replay):
        assert (want["id"], want["kind"]) == (got["id"], got["kind"]), "step identity"
        for a, bname in zip(want["box"], got["box"]):
            if abs(a - bname) > EPS:
                raise AssertionError(
                    "%s: expected box %s, derived %s"
                    % (want["id"], want["box"], got["box"])
                )
        ok += 1

    print("ACCEPTANCE: PASS - %d independent replay checks (graph + trace) "
          "across %d nodes" % (ok, len(nodes)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
