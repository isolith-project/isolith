#!/usr/bin/env python3
"""P4-M2-T03 acceptance test: PMI fidelity audit runner + honest capability matrix.

TDD: this test documents the acceptance criteria BEFORE the runner exists, so it
MUST FAIL (RED) against a tree with no `oep_tooling.pmi_fidelity`, then PASS
(GREEN) once the implementation lands on real material.

'Done' for P4-M2-T03 means:

  - the runner CLI exists (`python3 -m oep_tooling.pmi_fidelity`) and emits a
    two-artifact matrix (JSON + markdown) into `--matrix-out`;
  - the matrix is a GD&T-types x reference-apps honest capability table
    (P13-T03 style): every cell verdict belongs to the closed set
    {PASS, FAIL, SKIP} and every cell carries an `evidence` string;
  - honesty gates (no claims past what is verified):
      * a PASS cell requires the reference app to have surfaced >=1 instance
        of that GD&T type on a real corpus file (evidence.surfaced > 0);
      * a GD&T type that the corpus carries (independent lexical scan of the
        DATA section) but the app never surfaces is FAIL, never PASS;
      * an unavailable reference app yields SKIP with an evidence string
        recording why (not installed / no OSS binding / policy), never PASS;
      * no value-correctness claim is made: the audit measures presence +
        classification fidelity, not tolerance-value or datum-anchoring
        correctness (that is P4-M2-T01/T02 scope, stated in the matrix);
  - real material: on the license-clean NIST MBE PMI AP242 corpus the step-io
    0.2.5 column produces verified PASS rows (position, datums, and the 13
    other characteristics carried) AND at least one honest FAIL row
    (circular runout in nist_ctc_05: 3 CIRCULAR_RUNOUT_TOLERANCE entities in
    the DATA section, zero surfaced by step-io 0.2.5, per-file evidence);
  - the matrix JSON carries runner metadata (probe binary sha256, step-io
    version, generated_at) so a rerun is reproducible and auditable.

Real-material corpus: passed via OEP_PMI_CORPUS (the NIST MBE PMI AP242 dir).
When the corpus is absent, the material legs report SKIP and the structural
contract legs still run. Pure stdlib. Runs under pytest and directly.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
sys.path.insert(0, os.path.join(REPO, "python", "oep_tooling"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def stepio_probe_available():
    """Locate a runnable pmi_probe binary: env override, built target, or cargo."""
    probe = os.environ.get("OEP_PMI_PROBE")
    if probe and os.path.exists(probe):
        return probe
    worktree_target = os.path.join(REPO, "target", "debug", "examples", "pmi_probe")
    if os.path.exists(worktree_target):
        return worktree_target
    # Try building it via cargo (present in rust CI jobs / local dev).
    try:
        r = subprocess.run(
            ["cargo", "build", "-p", "oep-step", "--example", "pmi_probe"],
            capture_output=True, text=True, timeout=600,
        )
        if r.returncode == 0 and os.path.exists(worktree_target):
            return worktree_target
    except Exception:
        pass
    return None


def load_matrix(matrix_out):
    with open(os.path.join(matrix_out, "pmi-fidelity-matrix.json")) as fh:
        return json.load(fh)


def main():
    corpus = os.environ.get("OEP_PMI_CORPUS", "")
    probe = stepio_probe_available()

    # ---- structural / contract legs (always run) ---------------------------
    try:
        import oep_tooling
        from oep_tooling import pmi_fidelity  # noqa: F401
        check("runner module importable", True)
    except Exception as e:  # noqa: BLE001
        check("runner module importable", False, str(e))
        _emit_summary()
        return 1

    with tempfile.TemporaryDirectory() as tmp:
        cmd = [sys.executable, "-m", "oep_tooling.pmi_fidelity", "--matrix-out", tmp]
        if corpus:
            cmd += ["--corpus", corpus]
        if probe:
            cmd += ["--probe", probe]
        # `-m oep_tooling.pmi_fidelity` resolves from the python/ package root.
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=900,
                           cwd=os.path.join(REPO, "python"))
        check("runner CLI exits 0", r.returncode == 0, r.stderr[-400:] if r.returncode else "")
        if r.returncode != 0:
            _emit_summary()
            return 1

        mj = os.path.join(tmp, "pmi-fidelity-matrix.json")
        mm = os.path.join(tmp, "pmi-fidelity-matrix.md")
        check("emits matrix JSON", os.path.exists(mj))
        check("emits matrix markdown", os.path.exists(mm))
        if not os.path.exists(mj):
            _emit_summary()
            return 1
        mx = load_matrix(tmp)

        check("matrix schema marker present", mx.get("matrix_schema") == "isolith/pmi-fidelity-matrix/v1")
        rows = set(mx.get("gd_t_types", []))
        want_rows = {"straightness", "flatness", "circularity", "cylindricity",
                     "profile-of-line", "profile-of-surface", "angularity",
                     "perpendicularity", "parallelism", "position",
                     "concentricity", "symmetry", "circular-runout", "total-runout",
                     "datums", "modifiers"}
        check("matrix covers the 14 ASME/ISO characteristics + datums + modifiers",
              want_rows.issubset(rows) and len(rows) == len(want_rows))

        apps = {a["id"] for a in mx.get("apps", [])}
        check("reference-app axes present",
              {"stepio", "occt", "freecad", "commercial-readers"}.issubset(apps))

        # Every cell: verdict in closed set, evidence non-empty.
        cells = mx.get("cells", {})
        bad_verdict = []
        no_evidence = []
        pass_with_zero = []
        for gdt, byapp in cells.items():
            for app, cell in byapp.items():
                if cell.get("verdict") not in {"PASS", "FAIL", "SKIP"}:
                    bad_verdict.append((gdt, app, cell.get("verdict")))
                if not cell.get("evidence"):
                    no_evidence.append((gdt, app))
                if cell.get("verdict") == "PASS":
                    c = cell.get("counts") or {}
                    surfaced = 0
                    for f, fv in c.get("files", {}).items():
                        surfaced += fv.get("surfaced", 0)
                    if surfaced == 0:
                        pass_with_zero.append((gdt, app, c))
        check("all cell verdicts in {PASS, FAIL, SKIP}", not bad_verdict,
              str(bad_verdict[:5]))
        check("every cell has evidence", not no_evidence, str(no_evidence[:5]))
        check("honesty gate: no PASS with zero surfaced", not pass_with_zero,
              str(pass_with_zero[:5]))

        # Honesty gate: type carried lexically but surfaced 0 -> FAIL for apps
        # that parsed the file. Recompute from counts to catch a PASS/FAIL swap.
        contradiction = []
        for gdt, byapp in cells.items():
            for app, cell in byapp.items():
                c = cell.get("counts") or {}
                files = c.get("files", {})
                weighted_lex = sum(fv.get("lexical", 0) for fv in files.values())
                weighted_surf = sum(fv.get("surfaced", 0) for fv in files.values())
                if app == "stepio" and weighted_lex > 0 and weighted_surf == 0:
                    if cell.get("verdict") != "FAIL":
                        contradiction.append((gdt, "carried-but-unsurfaced not FAIL"))
        check("honesty gate: carried-but-unsurfaced type is FAIL, never PASS/SKIP",
              not contradiction, str(contradiction[:5]))

        meta = mx.get("runner", {})
        check("runner metadata: probe sha256", bool(meta.get("probe_sha256", "").strip()))
        check("runner metadata: step-io version", bool(meta.get("step_io_version", "").strip()))
        check("runner metadata: generated_at", bool(meta.get("generated_at", "").strip()))
        check("markdown contains a capability table header",
              "|" in open(mm, encoding="utf-8").read())
        check("markdown documents verdict semantics",
              any(w in open(mm, encoding="utf-8").read() for w in ("PASS", "FAIL", "SKIP")))

        # ---- real-material legs (corpus + probe present only) --------------
        if corpus and probe:
            present = {p.get("file") for p in mx.get("probe_reports", []) if p.get("parse_ok")}
            check("real material: every corpus AP242 opens without crash",
                  len(present) > 0, str(len(present)) + " opened")

            stepio_row = {gdt: byapp.get("stepio", {}) for gdt, byapp in cells.items()}
            # Verified honest facts on the NIST MBE PMI corpus (AP242):
            pass_rows = ["straightness", "flatness", "circularity", "cylindricity",
                         "profile-of-line", "profile-of-surface", "angularity",
                         "perpendicularity", "parallelism", "position",
                         "concentricity", "symmetry", "total-runout", "datums"]
            n_pass = sum(1 for r in pass_rows if (r in stepio_row) and stepio_row[r].get("verdict") == "PASS")
            check("step-io column: >=13 characteristics PASS on real corpus",
                  n_pass >= 13, "%d/14 PASS" % n_pass)

            cr = stepio_row.get("circular-runout", {})
            cr_evidence = cr.get("evidence", "")
            check("step-io column: circular runout honestly FAIL (present, not surfaced)",
                  cr.get("verdict") == "FAIL", cr_evidence[:200])
            check("circular runout FAIL evidence cites nist_ctc_05 and the drop",
                  "nist_ctc_05" in cr_evidence and "3" in cr_evidence)

            pos = stepio_row.get("position", {}).get("evidence", "")
            check("position PASS evidence names a real surfaced file",
                  stepio_row.get("position", {}).get("verdict") == "PASS"
                  and "nist" in pos.lower())

            # contiguity: surfaced counts in the evidence match the per-file probes
            ok_count = True
            for gdt, byapp in cells.items():
                for app, cell in byapp.items():
                    if app != "stepio":
                        continue
                    for fname, fv in (cell.get("counts") or {}).get("files", {}).items():
                        if fv.get("surfaced", 0) > 0 and fv.get("probe_surfaced", 0) <= 0:
                            ok_count = False
            check("evidence surfaced counts consistent with probe report", ok_count)
        else:
            print("[SKIP] real-material legs (corpus=%s probe=%s)" % (bool(corpus), bool(probe)))

    _emit_summary()
    return 1 if _failures else 0


def _emit_summary():
    if _failures:
        print("\n%d FAILING: %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("\nALL ACCEPTANCE CHECKS PASSED")
    return 0


def test_pmi_fidelity_acceptance():
    """pytest entry: the acceptance main() must exit cleanly."""
    assert main() == 0


if __name__ == "__main__":
    sys.exit(main())
