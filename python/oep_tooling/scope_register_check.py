#!/usr/bin/env python3
"""Isolith scope-register QA / drift guard (FEATURE-BASELINE, card t_a686d2b9;
PRACTITIONER-INPUT-CONTRACT extension, card t_5b00868e).

Verifies the CATIA/SolidWorks EDITOR PARITY BASELINE register:

1. SCHEMA: docs/scope-register.json is well-formed - schema_version in the
   supported set (1 or 2; 2 adds the practitioner_input_contract section),
   closed status/cost-class/vendor token sets, the 14 required capabilities
   present exactly once, and the human rendering (docs/scope-register.md)
   stays in sync with the JSON (row id + status in the same table line).

2. PRACTITIONER-INPUT-CONTRACT (schema >= 2): the editor input and behaviour
   contract (decision #19 bar: same-or-greater editor functionality than
   CATIA/SolidWorks; LTT practitioner test as the free specification, Don
   directive 2026-10-27). Every behaviour row carries the same closed tokens,
   an explicit test hook, and must obey the SAME status ceiling as the
   capability rows: building/shippable require a green accepted test in the
   tree, planned requires a recorded decision. What proves parity is the
   input contract an experienced user carries in their hands - feature lists
   do not prove that, green tests of the behaviour do.

3. HONEST-CLAIMS (the "no overstated claims" gate over both sections): no row
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
import sys

SCHEMA_VERSION = 2
# Tolerant migration: a schema-1 register (no practitioner section) still
# validates; a schema-2 register must carry the practitioner section and its
# rows obey the same honest-claims ceiling as the capability rows.
SUPPORTED_SCHEMA_VERSIONS = (1, 2)

PRACTITIONER_SECTION = "practitioner_input_contract"

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


def _validate_rows(rows, label, require_test_hook=False, root=ROOT):
    """Shared closed-schema + honest-claims validation for a row list.

    Returns (failures, rows_report). `label` names the section in failures.
    `require_test_hook` tightens the row contract (practitioner rows must each
    name the seam/test hook that will exercise the behaviour).
    """
    failures = []
    rows_report = []

    ids = [r.get("id") for r in rows]
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    for d in dupes:
        failures.append("%s: duplicate row id %r" % (label, d))

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
        if require_test_hook and not (r.get("test_hook") or "").strip():
            ok = False
            row_failures.append("missing explicit test_hook")

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
            failures.extend("%s %s: %s" % (label, rid, m) for m in row_failures)

        rows_report.append({
            "id": rid,
            "status": status,
            "evidence_kind": kind,
            "ok": ok,
            "detail": "; ".join(row_failures) or "ok",
        })

    return failures, rows_report


def validate_register(register, markdown, root=ROOT):
    """Return (failures, rows_report, practitioner_report). Each failure is a
    human message."""
    failures = []
    rows_report = []
    practitioner_report = []

    if not isinstance(register, dict):
        return ["register root is not an object"], [], []

    if register.get("schema_version") not in SUPPORTED_SCHEMA_VERSIONS:
        failures.append(
            "schema_version must be one of %s (found %r)"
            % (sorted(SUPPORTED_SCHEMA_VERSIONS), register.get("schema_version")))

    policy = register.get("policy") or {}
    required = list(policy.get("required_capabilities") or [])
    if not required:
        failures.append("policy.required_capabilities is empty")

    rows = register.get("rows")
    if not isinstance(rows, list) or not rows:
        failures.append("register.rows is empty or missing")
        rows = []

    ids = [r.get("id") for r in rows]
    for want in required:
        if ids.count(want) != 1:
            failures.append(
                "required capability %r must appear exactly once (found %d)"
                % (want, ids.count(want)))

    row_fails, rows_report = _validate_rows(
        rows, label="capability", require_test_hook=False, root=root)
    failures.extend(row_fails)

    # ---- practitioner input contract (schema >= 2) ----
    schema_v = register.get("schema_version")
    practitioner = register.get(PRACTITIONER_SECTION)
    practitioner_required = list(policy.get("practitioner_required_ids") or [])

    if schema_v is not None and schema_v >= 2:
        if not practitioner_required:
            failures.append("policy.practitioner_required_ids is empty "
                            "(schema 2 requires the practitioner section)")
        if not isinstance(practitioner, dict) or not practitioner.get("rows"):
            failures.append(
                "register.%s is missing or empty (required at schema 2)"
                % PRACTITIONER_SECTION)
            practitioner_rows = []
        else:
            practitioner_rows = practitioner["rows"]
        p_ids = [r.get("id") for r in practitioner_rows]
        for want in practitioner_required:
            if p_ids.count(want) != 1:
                failures.append(
                    "practitioner behaviour %r must appear exactly once "
                    "(found %d)" % (want, p_ids.count(want)))
        p_fails, practitioner_report = _validate_rows(
            practitioner_rows, label="practitioner", require_test_hook=True,
            root=root)
        failures.extend(p_fails)
    else:
        # schema 1 (tolerant migration): the section may be absent; when it is
        # present anyway, still enforce the honest-claims ceiling.
        practitioner_rows = []
        if isinstance(practitioner, dict) and practitioner.get("rows"):
            practitioner_rows = practitioner["rows"]
        p_fails, practitioner_report = _validate_rows(
            practitioner_rows, label="practitioner", require_test_hook=True,
            root=root)
        failures.extend(p_fails)

    # markdown sync: every row's id and status must appear in its table line
    if markdown:
        for r in rows:
            rid = r.get("id")
            if not rid:
                continue
            _check_md_line(failures, markdown, rid, r.get("status"))
        for r in practitioner_rows:
            rid = r.get("id")
            if not rid:
                continue
            _check_md_line(failures, markdown, rid, r.get("status"))

    return failures, rows_report, practitioner_report


def _check_md_line(failures, markdown, rid, status):
    line = _row_table_line(markdown, rid)
    if line is None:
        failures.append(
            "%s: no table line in docs/scope-register.md "
            "(register doc out of sync)" % rid)
    elif "| %s |" % status not in line:
        failures.append(
            "%s: docs/scope-register.md table line does not carry "
            "status %r (register doc out of sync)" % (rid, status))


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

    failures, rows_report, practitioner_report = validate_register(
        register, markdown)

    all_rows = rows_report + practitioner_report
    report = {
        "schema_version": SCHEMA_VERSION,
        "task": "FEATURE-BASELINE scope register (t_a686d2b9) + "
                "practitioner input contract (t_5b00868e)",
        "register": args.register,
        "exit": 0 if not failures else 1,
        "rows": rows_report,
        "practitioner_rows": practitioner_report,
        "summary": {
            "rows": len(rows_report),
            "practitioner_rows": len(practitioner_report),
            "pass": sum(1 for r in all_rows if r["ok"]),
            "fail": sum(1 for r in all_rows if not r["ok"]),
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
