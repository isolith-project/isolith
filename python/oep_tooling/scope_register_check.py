#!/usr/bin/env python3
"""Isolith scope-register QA / drift guard (FEATURE-BASELINE, card t_a686d2b9).

Verifies the CATIA/SolidWorks EDITOR PARITY BASELINE register:

1. SCHEMA: docs/scope-register.json is well-formed - schema_version, closed
   status/cost-class/vendor token sets, the 14 required capabilities present
   exactly once, and the human rendering (docs/scope-register.md) stays in
   sync with the JSON (row id + status in the same table line).

2. HONEST-CLAIMS (the "no overstated claims" gate over the register): no row
   may claim a status higher than its evidence proves. The evidence ladder:
   `not-scoped` needs no proof (or a decision), `planned` needs a recorded
   decision/ADR/card, `building` and `shippable` each need a GREEN ACCEPTED
   TEST in the tree. A `building`/`shippable` row whose cited test file does
   not exist on disk is a FAIL (a claim without evidence). The "green" half is
   proven by the full pytest/cargo suite the register rides in - this checker
   proves the claim has a citable test at all.

Honest exit code contract (same as the GA gates and format-matrix-check):
0 iff no FAIL row. Absent a register the checker fails (the register is the
deliverable, not an optional artifact).

Usage:
  python/oep_tooling/scope_register_check.py \
      --register docs/scope-register.json \
      --markdown docs/scope-register.md \
      [--out -]
"""

import argparse
import json
import os
import re
import sys

SCHEMA_VERSION = 1

CLOSED_STATUS = {"not-scoped", "planned", "building", "shippable"}
CLOSED_COST_CLASS = {
    "class-1-additive",
    "class-2-model-extend",
    "class-3-kernel-gated",
    "class-4-different-foundation",
}
CLOSED_VENDOR_TIER = {"base", "module", "limited", "no"}

# A status may be claimed by exactly these evidence kinds (a row with a
# higher-status claim than its evidence allows is an overclaim -> FAIL).
EVIDENCE_LADDER = {
    "shippable": {"test"},
    "building": {"test"},
    "planned": {"decision", "test"},
    "not-scoped": {"none", "decision"},
}

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def _resolve_test_ref(ref, root=ROOT):
    """Resolve an evidence test ref to an existing repo file, or None."""
    if not ref:
        return None
    cands = []
    if ref.startswith("tests/") or ref.startswith("crates/") or "/" in ref:
        cands.append(os.path.normpath(os.path.join(root, ref)))
    if ref.startswith("tests/") or not os.path.sep in ref:
        # bare name: default to the tests/ dir and the scripts dir
        cands.append(os.path.join(root, "tests", os.path.basename(ref)))
    if ref.endswith((".py", ".sh", ".rs")) or ref.startswith("test"):
        cands.append(os.path.join(root, ref))
    for c in cands:
        if os.path.isfile(c):
            return c
    return None


def _row_table_line(md, row_id):
    """Return the markdown table line whose first cell is | <row_id>, or None."""
    for line in md.splitlines():
        if line.strip().startswith("| " + row_id + " "):
            return line.strip()
        if line.strip() == "| " + row_id + " |":
            return line.strip()
    return None


def validate_register(register, markdown, root=ROOT):
    """Return (failures, rows_report). Each failure is a human message."""
    failures = []
    rows_report = []

    if not isinstance(register, dict):
        return ["register root is not an object"], []

    if register.get("schema_version") != SCHEMA_VERSION:
        failures.append(
            "schema_version must be %d (found %r)"
            % (SCHEMA_VERSION, register.get("schema_version")))

    policy = register.get("policy") or {}
    required = list(policy.get("required_capabilities") or [])
    if not required:
        failures.append("policy.required_capabilities is empty")

    rows = register.get("rows")
    if not isinstance(rows, list) or not rows:
        failures.append("register.rows is empty or missing")
        rows = []

    ids = [r.get("id") for r in rows]
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    for d in dupes:
        failures.append("duplicate row id %r" % d)

    for want in required:
        if ids.count(want) != 1:
            failures.append(
                "required capability %r must appear exactly once (found %d)"
                % (want, ids.count(want)))

    for r in rows:
        rid = r.get("id") or "?"
        ok = True
        row_failures = []
        for field, closed in (
                ("status", CLOSED_STATUS),
                ("cost_class", CLOSED_COST_CLASS),
                ("catia", CLOSED_VENDOR_TIER),
                ("solidworks", CLOSED_VENDOR_TIER)):
            val = r.get(field)
            if val not in closed:
                ok = False
                row_failures.append("%s=%r not in closed set %r"
                                    % (field, val, sorted(closed)))
        if not r.get("capability"):
            ok = False
            row_failures.append("missing capability label")
        if not r.get("workstream"):
            ok = False
            row_failures.append("missing owning workstream")

        status = r.get("status")
        ev = r.get("evidence") or {}
        kind = ev.get("kind")
        ref = ev.get("ref") or ""

        if kind not in EVIDENCE_LADDER.get(status, set()):
            ok = False
            row_failures.append(
                "status %r requires evidence kind in %s (got %r)"
                % (status, sorted(EVIDENCE_LADDER.get(status, set())), kind))
        if status != "not-scoped" and not ref:
            # a claim needs a citable reason; not-scoped may be bare
            ok = False
            row_failures.append("status %r has no evidence ref" % status)

        if kind == "test":
            if not _resolve_test_ref(ref, root):
                ok = False
                row_failures.append(
                    "status %r cites test evidence %r that does not exist in "
                    "the tree (a claim with no proof)" % (status, ref))
        if not ok:
            failures.extend("%s: %s" % (rid, m) for m in row_failures)

        rows_report.append({
            "id": rid,
            "status": status,
            "evidence_kind": kind,
            "ok": ok,
            "detail": "; ".join(row_failures) or "ok",
        })

    # markdown sync: every row's id and status must appear in its table line
    if markdown:
        for r in rows:
            rid = r.get("id")
            if not rid:
                continue
            line = _row_table_line(markdown, rid)
            if line is None:
                failures.append(
                    "%s: no table line in docs/scope-register.md "
                    "(register doc out of sync)" % rid)
            elif "| %s |" % r.get("status") not in line:
                failures.append(
                    "%s: docs/scope-register.md table line does not carry "
                    "status %r (register doc out of sync)"
                    % (rid, r.get("status")))

    return failures, rows_report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--register", default="docs/scope-register.json")
    ap.add_argument("--markdown", default="docs/scope-register.md")
    ap.add_argument("--out", default="-")
    args = ap.parse_args(argv)

    reg_path = os.path.normpath(os.path.join(ROOT, args.register))
    md_path = os.path.normpath(os.path.join(ROOT, args.markdown))

    missing = [p for p in (reg_path, md_path) if not os.path.isfile(p)]
    if missing:
        print("\n".join("FAIL: missing %s" % m for m in missing))
        sys.exit(1)

    with open(reg_path, encoding="utf-8") as fh:
        register = json.load(fh)
    with open(md_path, encoding="utf-8") as fh:
        markdown = fh.read()

    failures, rows_report = validate_register(register, markdown)

    report = {
        "schema_version": SCHEMA_VERSION,
        "task": "FEATURE-BASELINE scope register (t_a686d2b9)",
        "register": args.register,
        "exit": 0 if not failures else 1,
        "rows": rows_report,
        "summary": {
            "rows": len(rows_report),
            "pass": sum(1 for r in rows_report if r["ok"]),
            "fail": sum(1 for r in rows_report if not r["ok"]),
            "failures": failures,
        },
    }
    out = args.out
    if out == "-":
        json.dump(report, sys.stdout, indent=1)
    else:
        with open(os.path.normpath(os.path.join(ROOT, out)), "w",
                  encoding="utf-8") as fh:
            json.dump(report, fh, indent=1)
        print("wrote %s (exit=%d)" % (out, report["exit"]))
    sys.exit(report["exit"])


if __name__ == "__main__":
    main()
