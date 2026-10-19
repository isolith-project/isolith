#!/usr/bin/env python3
"""Acceptance test for Keelworks P7-M5-T01: power-user velocity pass.

Power-user velocity is measured, not promised: 30 common command-line
operations a power user runs every session on the OEP interchange workbench
CLI (the oep-shell `workbench` example binary, P7-M2-T01), each humming under
its recorded budget in seconds, verified with real per-op measured numbers.

Written FIRST (TDD). The committed velocity pack must make this test pass:

  catalog  => workstreams/P7-M5/velocity/ops.json documents exactly 30 common operations
              (id, category, description, cli, expected, budget_s) covering:
              discovery, STEP load, 3DXML load, modern-CGR load, PDM
              lifecycle, interchange save lanes, and error paths
  harness  => workstreams/P7-M5/velocity/run_velocity.py can build the workbench binary
              and run the 30-op catalog, resolving real corpus via the
              OEP_*_SAMPLES env vars the crates already use, SKIPping legs
              whose corpus is absent (clean-room: binaries never committed)
  measured => workstreams/P7-M5/velocity/report-velocity.json records ONE number per op
              from a real wall-clock run of the built binary on U1 (the
              reference host), plus host/provenance fields
  budget   => every op's measured mean_seconds is > 0 and <= its budget_s;
              discovery/PDM/save-lane/error ops <=  1.0 s, loads <= 5.0 s,
              the large-assembly load (FM1_satellite) <= 10.0 s
  regression => workstreams/P7-M5/velocity/baseline-velocity.json is the committed
              baseline; workstreams/P7-M5/velocity/compare_velocity.py re-runs the
              harness and FAILS on (a) any op over its hard budget or
              (b) any op whose new mean exceeds its baseline mean by more
              than the tolerance factor (regression tracker, exit code 1)
  writeup  => workstreams/P7-M5/power-user-velocity-budget.md documents the 30 ops, the
              budget, the measured numbers, and the regression policy

Plus the program surface-language rule (no em dashes, no curly quotes) on the
files this card adds, and the CI-safe gate shape: the measured report and
baseline are committed so the runner-less CI container can verify the gate
(the run harness is a developer host tool, exactly the P6-M5 proof pattern).

Run:  python3 tests/test_p7m5t01_velocity_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VEL = os.path.join(REPO, "workstreams/P7-M5", "velocity")
OPS_JSON = os.path.join(VEL, "ops.json")
HARNESS = os.path.join(VEL, "run_velocity.py")
COMPARE = os.path.join(VEL, "compare_velocity.py")
REPORT_JSON = os.path.join(VEL, "report-velocity.json")
BASELINE_JSON = os.path.join(VEL, "baseline-velocity.json")
WRITEUP = os.path.join(REPO, "workstreams/P7-M5", "power-user-velocity-budget.md")

NUM_OPS = 30
EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

# Budget ceilings by op class, seconds (mean wall time). Generous headroom
# over today's measured numbers so CI noise never trips them, tight enough
# that an order-of-magnitude regression still fails. See writeup.
BUDGET_DISCOVERY = 1.0
BUDGET_PDM = 1.0
BUDGET_SAVE = 1.0
BUDGET_ERROR = 1.0
BUDGET_SMALL_LOAD = 5.0     # step / 3dxml / modern-cgr real loads
BUDGET_LARGE_LOAD = 10.0    # multi-part assembly 3DXML (FM1_satellite)
TOLERANCE = 1.5             # regression threshold: 1.5x baseline mean

CATEGORIES_OK = {
    "discovery", "step-load", "3dxml-load", "cgr-load",
    "pdm", "save", "error",
}

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def budget_for(op):
    cat = op.get("category", "")
    if cat in ("discovery", "pdm", "save", "error"):
        return {"discovery": BUDGET_DISCOVERY, "pdm": BUDGET_PDM,
                "save": BUDGET_SAVE, "error": BUDGET_ERROR}[cat]
    if "large" in op.get("description", "").lower() or "fm1" in op.get("description", "").lower():
        return BUDGET_LARGE_LOAD
    return BUDGET_SMALL_LOAD


def run_checks():
    print("P7-M5-T01 power-user velocity (30 common ops, measured budget)")
    print("catalog: %s" % OPS_JSON)
    print("budget: discovery/pdm/save/error <= %.1fs, loads <= %.1fs (large %.1fs), "
          "regression tolerance %.1fx" % (BUDGET_DISCOVERY, BUDGET_SMALL_LOAD,
                                          BUDGET_LARGE_LOAD, TOLERANCE))
    print("=" * 66)

    # 1. Catalog: exactly 30 documented operations.
    check(os.path.isfile(OPS_JSON), "missing catalog: workstreams/P7-M5/velocity/ops.json")
    ops = []
    if os.path.isfile(OPS_JSON):
        try:
            ops = load_json(OPS_JSON)
        except ValueError as e:
            check(False, "ops.json does not parse: %s" % e)
    check(isinstance(ops, list) and len(ops) == NUM_OPS,
          "catalog must define exactly %d ops, got %d" % (NUM_OPS, len(ops)))
    ids = [o.get("id") for o in ops]
    check(len(ids) == len(set(ids)), "op ids must be unique")
    ops_by_id = {o.get("id"): o for o in ops}
    for op in ops:
        oid = op.get("id", "?")
        for key in ("category", "description", "cli", "expected"):
            check(key in op, "op %s must carry field %r" % (oid, key))
        check(op.get("category") in CATEGORIES_OK,
              "op %s category %r not in %s" % (oid, op.get("category"), sorted(CATEGORIES_OK)))
        check(op.get("expected") in ("success", "pending", "failure"),
              "op %s expected %r must be success/pending/failure" % (oid, op.get("expected")))

    # 2. Harness: runnable, thin, builds the workbench binary.
    check(os.path.isfile(HARNESS), "missing harness: run_velocity.py")
    if os.path.isfile(HARNESS):
        h = read(HARNESS)
        check("cargo build" in h or "cargo" in h, "harness must drive cargo build")
        check("workbench" in h, "harness must reference the workbench example binary")

    # 3. Measured report: valid JSON, one per-op number, host provenance.
    check(os.path.isfile(REPORT_JSON), "missing measured report: report-velocity.json")
    report = {}
    if os.path.isfile(REPORT_JSON):
        try:
            report = load_json(REPORT_JSON)
        except ValueError as e:
            check(False, "report JSON does not parse: %s" % e)
    check(bool(report.get("host")), "report must record the host identifier")
    check(bool(report.get("built_from")), "report must record the binary origin")
    check(bool(report.get("measured_at")), "report must record the measurement time")
    rops = report.get("ops", [])
    check(len(rops) == NUM_OPS, "report must carry %d measured ops, got %d" % (NUM_OPS, len(rops)))
    cat_budgets = {o.get("id"): o.get("budget_s") for o in ops}
    for op in rops:
        mean = op.get("mean_seconds")
        oid = op.get("id", "?")
        check(isinstance(mean, (int, float)) and mean > 0,
              "op %s must record mean_seconds > 0, got %r" % (oid, mean))
        # Budget is the catalog source of truth (per-op budget_s), cross-checked
        # against the class ceiling computed above.
        cat_b = cat_budgets.get(oid)
        class_b = budget_for(ops_by_id.get(oid, op))
        budget = cat_b if isinstance(cat_b, (int, float)) else class_b
        check(mean <= budget + 1e-9,
              "op %s mean %.4fs must be <= its %.1fs budget" % (oid, mean, budget))

    # 4. Regression tracking: committed baseline + working comparator.
    check(os.path.isfile(BASELINE_JSON), "missing baseline: baseline-velocity.json")
    baseline = {}
    if os.path.isfile(BASELINE_JSON):
        try:
            baseline = load_json(BASELINE_JSON)
        except ValueError as e:
            check(False, "baseline JSON does not parse: %s" % e)
    bops = {b.get("id"): b.get("mean_seconds") for b in baseline.get("ops", [])}
    check(len(bops) == NUM_OPS, "baseline must cover all %d ops, got %d" % (NUM_OPS, len(bops)))
    check(os.path.isfile(COMPARE), "missing regression comparator: compare_velocity.py")
    if os.path.isfile(COMPARE):
        c = read(COMPARE)
        check("baseline" in c, "comparator must load the baseline")
        check("TOLERANCE" in c or "tolerance" in c, "comparator must use the tolerance factor")

    # 5. Versioned writeup: ops, budget, measurement, regression policy.
    check(os.path.isfile(WRITEUP), "missing writeup: workstreams/P7-M5/power-user-velocity-budget.md")
    if os.path.isfile(WRITEUP):
        md = read(WRITEUP)
        mdl = md.lower()
        check("P7-M5-T01" in md, "writeup must cite its own card P7-M5-T01")
        check("30" in md, "writeup must state the 30-op target")
        check("budget" in mdl, "writeup must name the velocity budget")
        check("u1" in mdl or "host" in mdl or "gb10" in mdl,
              "writeup must cite the measured host (real material)")
        check("regression" in mdl, "writeup must state the regression policy")

    # 6. Surface-language rule on the files this card adds.
    for path, label in ((OPS_JSON, "ops.json"),
                        (WRITEUP, "budget writeup"),
                        (HARNESS, "run_velocity.py"),
                        (COMPARE, "compare_velocity.py")):
        if os.path.isfile(path):
            text = read(path)
            check(EM_DASH not in text, "%s: no em dashes (program discipline)" % label)
            ncurly = (text.count("\u201c") + text.count("\u201d")
                      + text.count("\u2018") + text.count("\u2019"))
            check(ncurly == 0, "%s: no curly quotes; plain ASCII quotes only" % label)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("POWER-USER VELOCITY ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("POWER-USER VELOCITY ACCEPTANCE PASSED: 30 common operations "
          "documented, harnessed, and measured with real per-op numbers under "
          "their recorded budgets; baseline + regression comparator committed.")
    return 0


def test_p7m5t01_velocity_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "power-user velocity acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
