#!/usr/bin/env python3
"""P6-M3-T02 acceptance test: topology undo/redo stack, our-own-format
serialization, verified independently.

TDD: written BEFORE oep_geo::history existed and BEFORE the golden existed, so
against the skeleton this FAILS (golden missing) - proving RED. When the stack
lands, the crate's regen test (`OEP_HISTORY_REGEN=1`, see
crates/oep-geo/tests/history_contract.rs) writes the golden file and this
script independently re-parses it with the stdlib.

This is the "two-sided" verification pattern used by sibling cards (P6-M1 /
P6-M2 / P3-M4): the crate's own serialized bytes must agree with an
independent re-encode, so a self-consistent (but wrong) implementation cannot
pass.

The golden locks a realistic bracket topology edit session:
  init -> union-lip -> hole -> fillet -> (undo x2) -> (redo) -> countersink
where the fresh "countersink" commit drops the "fillet" state from the redo
branch (classic CAD rule: a new edit invalidates redo). The final stack holds
  4 states: init, union-lip, hole, countersink; cursor on countersink (index 3).
More on the record format in crates/oep-geo/src/history.rs and the contract
test crates/oep-geo/tests/history_contract.rs.
"""
import math
import os
import sys

GOLDEN = os.path.join(
    os.path.dirname(__file__),
    "..", "crates", "oep-geo", "tests", "golden", "history-bracket.oephist",
)

EXPECTED_HEADER = {
    "format": "oep-history",
    "version": 1,
}
BODY_KINDS = {"solid", "sheet", "wire"}
EXPECTED_STATES = ["init", "union-lip", "hole", "countersink"]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def fmt(x):
    """Deterministic number formatting matching the crate's contract: integral
    values render with one decimal so JSON-safe / unambiguous."""
    if float(x).is_integer() and abs(x) < 1e15:
        return "%.1f" % x
    return repr(float(x))


def parse_state(lines, i):
    """Parse one `state N` block starting at lines[i] (which is 'state <i>').
    Returns (state_dict, next_index). Raises on structural violation."""
    assert lines[i].startswith("state "), "expected 'state N', got %r" % lines[i]
    i += 1
    st = {"revision": None, "op": None, "bodies": [], "features": []}
    if lines[i].startswith("revision "):
        st["revision"] = int(lines[i].split(" ", 1)[1]); i += 1
    if lines[i].startswith("op "):
        st["op"] = lines[i].split(" ", 1)[1].strip(); i += 1
    # bodies
    assert lines[i].startswith("bodies "), "expected 'bodies N', got %r" % lines[i]
    nb = int(lines[i].split(" ", 1)[1]); i += 1
    for _ in range(nb):
        assert lines[i].startswith("body "), "expected body line, got %r" % lines[i]
        tok = lines[i][len("body "):].split(" ", 2)
        assert len(tok) == 3, "malformed body line %r" % lines[i]
        bid, kind, coords = tok[0], tok[1], tok[2]
        assert kind in BODY_KINDS, "unknown body kind %r" % kind
        vals = [float(v) for v in coords.split(";")]
        assert len(vals) == 6, "body bbox must have 6 coords"
        st["bodies"].append((bid, kind, vals))
        i += 1
    # features
    assert lines[i].startswith("features "), "expected 'features N', got %r" % lines[i]
    nf = int(lines[i].split(" ", 1)[1]); i += 1
    for _ in range(nf):
        assert lines[i].startswith("feature "), "expected feature line, got %r" % lines[i]
        tok = lines[i][len("feature "):].split(" ", 2)
        assert len(tok) >= 2, "malformed feature line %r" % lines[i]
        fid = tok[0]
        kind = tok[1]
        params = {}
        if len(tok) == 3:
            for pair in tok[2].split(";"):
                assert "=" in pair, "malformed param %r" % pair
                k, v = pair.split("=", 1)
                params[k] = float(v)
        st["features"].append((fid, kind, params))
        i += 1
    return st, i


def parse(lines):
    """Parse the whole oep-history v1 document. Returns a header dict + states
    list. Strict: unknown top-level lines are rejected."""
    assert lines[0] == "format oep-history", "bad format line %r" % lines[0]
    header: dict = {"format": "oep-history"}
    idx = 1
    while idx < len(lines):
        line = lines[idx]
        if line == "version 1":
            header["version"] = 1; idx += 1
        elif line.startswith("capacity "):
            header["capacity"] = int(line.split(" ", 1)[1]); idx += 1
        elif line.startswith("cursor "):
            header["cursor"] = int(line.split(" ", 1)[1]); idx += 1
        elif line.startswith("states "):
            header["declared_states"] = int(line.split(" ", 1)[1]); idx += 1
        elif line.startswith("state "):
            break
        else:
            raise AssertionError("unexpected line %r" % line)
    states = []
    while idx < len(lines):
        assert lines[idx].startswith("state "), "expected 'state N', got %r" % lines[idx]
        st, idx = parse_state(lines, idx)
        states.append(st)
    return header, states


def encode_state(st):
    """Independently re-encode one state block to the documented format."""
    out = []
    out.append("state %d" % (st["index"]))
    out.append("revision %d" % st["revision"])
    out.append("op %s" % st["op"])
    out.append("bodies %d" % len(st["bodies"]))
    for bid, kind, vals in st["bodies"]:
        out.append("body %s %s %s" % (bid, kind, ";".join(fmt(v) for v in vals)))
    out.append("features %d" % len(st["features"]))
    for fid, kind, params in st["features"]:
        param_str = ";".join("%s=%s" % (k, fmt(v)) for k, v in params.items())
        line = "feature %s %s" % (fid, kind)
        if param_str:
            line += " " + param_str
        out.append(line)
    return "\n".join(out)


def reencode(header, states):
    lines = []
    lines.append("format oep-history")
    lines.append("version 1")
    lines.append("capacity %d" % header["capacity"])
    lines.append("cursor %d" % header["cursor"])
    lines.append("states %d" % len(states))
    for i, st in enumerate(states):
        st["index"] = i
        lines.append(encode_state(st))
    return "\n".join(lines) + "\n"


def load_states():
    text = open(GOLDEN, encoding="utf-8").read()
    lines = [ln.rstrip("\n") for ln in text.splitlines()]
    header, states = parse(lines)
    return header, states


def run():
    print("P6-M3-T02 topology undo/redo stack acceptance")
    print("golden: %s" % GOLDEN)
    print("=" * 66)

    # 1. Golden exists and header is exact.
    if not os.path.isfile(GOLDEN):
        check(False, "missing golden: crates/oep-geo/tests/golden/history-bracket.oephist")
        return _finish()

    text = open(GOLDEN, encoding="utf-8").read()
    lines = [ln.rstrip("\n") for ln in text.splitlines()]
    header, states = parse(lines)
    for key, val in EXPECTED_HEADER.items():
        check(header.get(key) == val, "header %r == %r" % (key, header.get(key)))

    # 2. Stack layout invariants.
    check(header.get("declared_states") == len(states),
          "declared states matches parsed states")
    check(len(states) == 4, "golden has 4 committed states, got %d" % len(states))
    cursor = header.get("cursor")
    check(isinstance(cursor, int) and 0 <= cursor < len(states),
          "cursor %r in range" % (cursor,))
    # cursor is on the last state; can_undo True, can_redo False.
    check(cursor == len(states) - 1, "cursor points at the current (last) state")
    ops = [s["op"] for s in states]
    check(ops == EXPECTED_STATES, "edit story ops == %r, got %r" % (EXPECTED_STATES, ops))

    # 3. Record discipline: id convention, kinds, finite coords, min <= max.
    for i, st in enumerate(states):
        check(isinstance(st["revision"], int) and st["revision"] >= 1,
              "state %d has revision" % i)
        check(st["op"] and st["op"][0].isalpha(), "state %d has op label" % i)
        for bid, kind, vals in st["bodies"]:
            check(bid.startswith("oep-"), "body id %r uses oep- convention" % bid)
            check(kind in BODY_KINDS, "body kind valid: %r" % kind)
            check(all(math.isfinite(v) for v in vals), "body coords finite")
            for a in range(3):
                check(vals[a] <= vals[a + 3], "body bbox min<=max on axis %d" % a)
        for fid, kind, params in st["features"]:
            check(fid.startswith("oep-"), "feature id %r uses oep- convention" % fid)
            check(bool(kind), "feature kind nonempty")
            check(all(math.isfinite(v) for v in params.values()),
                  "feature params finite")

    # 4. Golden spot values (the bracket story).
    s0 = states[0]
    check(s0["op"] == "init" and s0["revision"] == 1, "state0 = init rev 1")
    check(len(s0["bodies"]) == 1, "state0 has exactly the bracket body")
    bracket = s0["bodies"][0]
    check(bracket[0] == "oep-body-bracket", "state0 body is the bracket")
    check([bracket[2][0], bracket[2][1], bracket[2][2]] == [0.0, 0.0, 0.0],
          "bracket bbox min == (0,0,0)")
    check([bracket[2][3], bracket[2][4], bracket[2][5]] == [80.0, 40.0, 6.0],
          "bracket bbox max == (80,40,6)")

    s1 = states[1]
    check(s1["op"] == "union-lip", "state1 = union-lip")
    check(len(s1["bodies"]) == 2, "state1 has two bodies (bracket + lip)")

    s2 = states[2]
    check(s2["op"] == "hole", "state2 = hole")
    hole = dict(s2["features"][0][2])
    check(hole.get("diameter") == 6.0 and hole.get("depth") == 6.0,
          "hole feature diameter==6 depth==6")

    s3 = states[3]
    check(s3["op"] == "countersink", "state3 = countersink")
    # Features are id-sorted: oep-feature-countersink < oep-feature-hole.
    cs = dict(s3["features"][0][2])
    check(cs.get("diameter") == 8.0 and cs.get("depth") == 2.0,
          "countersink diameter==8 depth==2")
    # The fillet state was dropped from the redo branch by the fresh commit.
    fnames = [f[1] for f in s3["features"]]
    check("Fillet" not in fnames, "fillet dropped from redo branch")
    check("CounterSink" in fnames, "countersink feature present")
    check(not any(s["op"] == "fillet" for s in states),
          "no fillet state survives in the stack")

    # 5. Byte-stable re-encode: our independent writer reproduces the exact
    #    committed bytes (P6-M4 round-trip discipline applied to the stack).
    reencoded = reencode(header, states)
    check(reencoded == text, "independent re-encode is byte-identical to golden")

    return _finish()


def _finish():
    if failures:
        print("FAILED %d check(s):" % len(failures))
        for f in failures:
            print("  - %s" % f)
        return 1
    print("ACCEPTANCE: PASS - serializable undo/redo stack verified independently")
    return 0


def test_acceptance_script():
    """pytest entry point so the quality gate actually exercises the suite
    (pytest collects test_* functions; run() is the shared body)."""
    assert run() == 0


if __name__ == "__main__":
    sys.exit(run())
