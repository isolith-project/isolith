#!/usr/bin/env python3
"""P7-M5-T01 regression tracker: re-run the velocity harness and compare.

The power-user velocity bargain is that it stays fast, so the velocity test is
also a regression gate. This script re-runs the 30-op harness (a fresh
measurement on this host) and compares each op against the committed baseline
(P7-M5/velocity/baseline-velocity.json):

  - HARD BUDGET: any op whose fresh mean_seconds exceeds its catalog budget_s
    is a regression (the op no longer makes its "under N seconds" promise).
  - TOLERANCE: any op whose fresh mean_seconds exceeds its baseline mean by
    more than TOLERANCE (1.5x) AND whose absolute delta exceeds ABS_FLOOR
    (0.05 s) is flagged as a regression. The absolute floor keeps the gate
    honest on fast ops (a 6 ms jitter on a 12 ms op is noise, not a real
    slowdown); a many-x slowdown on a fast op still crosses both.
  - SKIPPED ops (corpus absent on this host) are reported but never fail the
    gate: a leg that cannot run is honest, whereas a slow run is a regression.

Exit code 0 = all runnable ops within budget and within tolerance.
Exit code 1 = at least one regression (budget breach or > tolerance).

Usage:
  python3 P7-M5/velocity/compare_velocity.py [--ops ops.json]
         [--baseline baseline-velocity.json] [--repeats N] [--no-build]
         [--json regressions.json]
Stdlib only; run from the repo root.
"""
import argparse
import json
import os
import sys
import tempfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
VEL = os.path.join(REPO, "P7-M5", "velocity")
DEFAULT_OPS = os.path.join(VEL, "ops.json")
DEFAULT_BASELINE = os.path.join(VEL, "baseline-velocity.json")

TOLERANCE = 1.5  # regression threshold: fresh mean > baseline mean * tolerance
ABS_FLOOR = 0.05  # absolute floor in seconds: ignore sub-noise deltas on fast ops

HARNESS = os.path.join(VEL, "run_velocity.py")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ops", default=DEFAULT_OPS)
    ap.add_argument("--baseline", default=DEFAULT_BASELINE)
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--json")
    args = ap.parse_args()

    with open(args.baseline, encoding="utf-8") as fh:
        baseline = json.load(fh)
    with open(args.ops, encoding="utf-8") as fh:
        ops = json.load(fh)
    budget = {o["id"]: o.get("budget_s") for o in ops}
    baseline_mean = {o["id"]: o.get("mean_seconds") for o in baseline.get("ops", [])}
    baseline_host = baseline.get("host", "unknown")

    # Fresh measurement through the harness into a temp report.
    fd, tmp_report = tempfile.mkstemp(suffix="-velocity-report.json")
    os.close(fd)
    cmd = [sys.executable, HARNESS, "--ops", args.ops, "--out", tmp_report]
    if args.no_build:
        cmd.append("--no-build")
    cmd += ["--repeats", str(args.repeats)]
    sub = __import__("subprocess")
    sub.run(cmd, check=True)

    with open(tmp_report, encoding="utf-8") as fh:
        fresh = json.load(fh)
    os.unlink(tmp_report)

    fresh_mean = {o["id"]: o.get("mean_seconds") for o in fresh.get("ops", [])}
    fresh_status = {o["id"]: o.get("status") for o in fresh.get("ops", [])}

    regressions = []
    rows = []
    for op in ops:
        oid = op["id"]
        rows.append((oid, op.get("description", ""), budget.get(oid),
                     baseline_mean.get(oid), fresh_mean.get(oid),
                     fresh_status.get(oid)))
    rows.sort(key=lambda r: (r[3] if r[3] is not None else -1), reverse=True)

    print("P7-M5-T01 velocity regression check")
    print("baseline host: %s   tolerance: %.1fx   fresh host: %s"
          % (baseline_host, TOLERANCE, fresh.get("host", "unknown")))
    print("=" * 78)
    print("%-8s %-48s %-7s %-9s %-9s %s" % (
        "op", "description", "budget_s", "base_mean", "fresh_mean", "status"))
    for oid, desc, bud, bmean, fmean, status in rows:
        flag = ""
        if fmean is not None and bud is not None and fmean > bud + 1e-9:
            flag = "BUDGET"
            regressions.append({"op": oid, "kind": "budget",
                                "mean_seconds": fmean, "budget_s": bud})
        elif (fmean is not None and bmean and
              fmean > bmean * TOLERANCE and fmean - bmean > ABS_FLOOR):
            flag = "REGRESS"
            regressions.append({"op": oid, "kind": "tolerance",
                                "mean_seconds": fmean,
                                "baseline_mean": bmean,
                                "tolerance": TOLERANCE,
                                "abs_floor_s": ABS_FLOOR})
        status_txt = status or "skip"
        bud_txt = "%.1f" % bud if bud is not None else "n/a"
        b_txt = "%.3f" % bmean if bmean is not None else "skip"
        f_txt = "%.3f" % fmean if fmean is not None else "skip"
        print("%-8s %-48s %-7s %-9s %-9s %s%s" % (
            oid, (desc[:46] + "...") if len(desc) > 46 else desc,
            bud_txt, b_txt, f_txt, status_txt, ("  " + flag) if flag else ""))

    if regressions:
        print("\nFAIL: %d regression(s)" % len(regressions))
        if args.json:
            with open(args.json, "w", encoding="utf-8") as fh:
                json.dump(regressions, fh, indent=2)
                fh.write("\n")
            print("regressions written to %s" % args.json)
        sys.exit(1)
    print("\nPASS: every runnable op is within budget and within %.1fx of "
          "baseline." % TOLERANCE)


if __name__ == "__main__":
    main()
