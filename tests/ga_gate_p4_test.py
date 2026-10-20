#!/usr/bin/env python3
"""P4-M5-T03 acceptance test (TDD, RED first) — the P4 GA gate.

Written BEFORE `oep_tooling.ga_gate_p4` exists, so this file MUST FAIL (RED)
when run against main (no `oep_tooling.ga_gate_p4` module). Passes GREEN once the
gate module lands AND its legs can run (the two P4 GA criteria). On a bare main
the legs degrade to classified SKIP (CI-safe degraded run, exit 0) exactly like
the P2-M6-T03 / P3-M5-T02 gates.

P4 GA gate means BOTH PIECE of the P4 milestone contract:

1. FIDELITY LEG — the STEP fidelity matrix is published and honest: the PMI
   fidelity matrix artifact (docs/pmi-fidelity/) parses, declares the
   matrix schema, every GD&T x app cell is a closed-set verdict (PASS/FAIL/SKIP)
   with evidence, the GD&T type set matches the cells, and the rollup counts are
   self-consistent; AND the P4-M1-T03 accuracy harness report
   (corpus/accuracy-report.json) has every measured row within-epsilon
   (symmetric_hausdorff <= eps, |volume_rel_error| <= eps). Gate-FAIL only when
   an artifact is inconsistent or a measured value is out of epsilon — never for
   an honest FAIL cell in the matrix (step-io capability gaps are documented
   truth, not a fabrication).

2. MBD LEG — the MBD validator demo runs on real corpus material: reuses
   oep_tooling.mbd_validate (do not re-implement) against the real STEP PMI
   corpus rows (NIST MBE AP242 imported by P4-M2-T01) plus the P4-M5-T01 MBD
   fixture manifest; every input reaches a checked row (no drop), the report is
   honest (summary rollup == granular finding counts, UNVERIFIED never counts as
   PASS, missing-PMI honesty preserved) and writes both JSON and Markdown.
   Gate-FAIL only when the demo crashes or the report is dishonest.

Exit code contract (same as P2/P3): 0 iff no gate-FAIL row anywhere (SKIP
allowed). The gate's own report (ga-gate-p4-matrix.json / .md) must be
self-consistent (summary totals roll up to row counts, schema_version 1).
"""
import json
import os
import sys
import tempfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
sys.path.insert(0, os.path.join(REPO, "python", "oep_tooling"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    print("[%s] %s" % (status, name) + ((" -- " + detail) if detail else ""))
    if not ok:
        _failures.append(name)


CM_HEADER = {
    "schema_version": 1,
    "description": "PMI fidelity matrix (synthetic, acceptance test)",
    "matrix_schema": "isolith/pmi-fidelity-matrix/v1",
    "generated_at": "2026-10-16T00:00:00Z",
    "host": "test",
    "apps": [{"id": "stepio", "label": "step-io 0.2.5", "kind": "oss",
              "available": True, "version": "0.2.5", "evidence": "probe"}],
    "gd_t_types": ["flatness", "position", "circular-runout", "modifiers"],
    "gd_t_labels": {"flatness": "Flatness", "position": "Position",
                    "circular-runout": "Circular runout",
                    "modifiers": "Tolerance modifiers"},
}

ACC_HEADER = {
    "schema_version": 1,
    "generated": "2026-10-16T00:00:00+00:00",
    "description": "synthetic accuracy report",
    "summary": {"total": 1, "measured": 1, "skipped": 0, "skipped_by_class": {}},
}


def make_fidelity_matrix(dirpath, cells_ok=True, schema_ok=True):
    cells = {
        "flatness": {"stepio": {"verdict": "PASS", "evidence": "1 carried -> 1 surfaced",
                                "counts": {}}},
        "position": {"stepio": {"verdict": "PASS", "evidence": "2 carried -> 2 surfaced",
                                "counts": {}}},
        "circular-runout": {"stepio": {"verdict": "FAIL",
                                       "evidence": "carried but 0 surfaced (honest gap)",
                                       "counts": {}}},
        "modifiers": {"stepio": {"verdict": "FAIL",
                                 "evidence": "modifier arenas empty (honest gap)",
                                 "counts": {}}},
    }
    if not cells_ok:
        cells = {}
    m = dict(CM_HEADER)
    if not schema_ok:
        m = {"matrix_schema": "wrong/schema", "gd_t_types": []}
    m["cells"] = cells
    p = Path(dirpath) / "docs" / "p4-m2-t03-pmi-fidelity"
    p.mkdir(parents=True, exist_ok=True)
    (p / "pmi-fidelity-matrix.json").write_text(json.dumps(m, indent=2) + "\n",
                                                encoding="utf-8")
    (p / "pmi-fidelity-matrix.md").write_text("# PMI fidelity matrix\n",
                                              encoding="utf-8")
    return p


def make_accuracy_report(dirpath, hausdorff=0.0, vol_rel=0.0, measured=True):
    rows = []
    if measured:
        rows.append({
            "id": "isolith-box", "status": "measured", "class": "",
            "reason": "", "geometry_class": "prism",
            "metrics": {"symmetric_hausdorff": hausdorff,
                        "volume_rel_error": vol_rel,
                        "volume_delta": 0.0}})
    else:
        rows.append({"id": "analytic-part", "status": "skipped",
                     "class": "not-faceted", "reason": "analytic B-Rep",
                     "geometry_class": "analytic", "metrics": {}})
    r = dict(ACC_HEADER)
    r["summary"]["total"] = len(rows)
    r["summary"]["measured"] = sum(1 for x in rows if x["status"] == "measured")
    r["rows"] = rows
    p = Path(dirpath) / "corpus"
    p.mkdir(parents=True, exist_ok=True)
    (p / "accuracy-report.json").write_text(json.dumps(r, indent=2) + "\n",
                                            encoding="utf-8")
    return p


def make_step_pmi(dirpath):
    """Synthetic real-STEP-PMI bundle: oep-pmi doc with annotations + model facts."""
    p = Path(dirpath) / "corpus" / "mbd-real-step-pmi"
    p.mkdir(parents=True, exist_ok=True)
    doc = {
        "format": "oep-pmi", "version": 1, "units": "mm",
        "datums": [{"id": "datum_A", "label": "A", "feature_ids": ["f1"],
                    "body_ids": ["b1"]}],
        "frames": [{"id": "drf_a", "name": "A",
                    "refs": [{"datum_id": "datum_A", "precedence": 1}]}],
        "tolerances": [{"id": "gt_1", "characteristic": "position",
                        "value": 0.2, "frame_id": "drf_a",
                        "feature_ids": ["f1"], "body_ids": ["b1"]}],
        "notes": [],
    }
    bundle = {"schema_version": 1, "source": {"origin": "oep-step-pmi real import",
                                              "file": "synthetic.stp"},
              "pmi": doc, "pmi_extracted": True,
              "model": {"units": "mm", "feature_ids": ["f1"], "body_ids": ["b1"]},
              "model_present": True}
    (p / "synthetic.stp.oeppmi.json").write_text(
        json.dumps(bundle, indent=2) + "\n", encoding="utf-8")
    manifest = {"schema_version": 1, "task": "P4-M5-T03",
                "fixtures": [{"id": "synthetic-real-step",
                              "file": "mbd-real-step-pmi/synthetic.stp.oeppmi.json",
                              "source": "real NIST AP242 imported by oep-step-pmi"}]}
    (Path(dirpath) / "corpus" / "mbd-real-step-pmi-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return Path(dirpath) / "corpus"


# --- 1. module exists and exposes the gate API -----------------------------
def test_ga_gate_p4_module_exists():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception as e:
        check("import oep_tooling.ga_gate_p4", False, repr(e))
        return
    check("import oep_tooling.ga_gate_p4", True)
    for name in ("fidelity_leg", "mbd_leg", "run_gate", "build_report",
                 "exit_code", "write_report"):
        check("ga_gate_p4 has %s" % name, hasattr(G, name), name)


# --- 2. fidelity leg: published + honest matrix, measured rows in-epsilon ---
def test_fidelity_leg_published_and_within_epsilon():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return  # already reported in test 1 (RED)
    with tempfile.TemporaryDirectory() as td:
        fid_dir = make_fidelity_matrix(td)          # docs/pmi-fidelity
        acc_dir = make_accuracy_report(td, 0.0, 0.0)  # corpus/accuracy-report.json
        rows = G.fidelity_leg(fidelity_dir=str(fid_dir),
                              accuracy_report=str(acc_dir / "accuracy-report.json"),
                              eps=1e-9)
        fails = [r for r in rows if r.status == "FAIL"]
        passes = [r for r in rows if r.status == "PASS"]
        check("fidelity_leg returns list", isinstance(rows, list),
              repr(rows)[:120])
        check("fidelity_leg: zero gate-FAIL on consistent published material",
              len(fails) == 0, "fails=%d rows=%d" % (len(fails), len(rows)))
        check("fidelity_leg: matrix passes", any(r.id == "pmi-fidelity-matrix"
                                                 and r.status == "PASS" for r in rows),
              repr([(r.id, r.status) for r in rows])[:160])
        check("fidelity_leg: accuracy within-epsilon passes",
              any(r.id == "isolith-box" and r.status == "PASS" for r in rows),
              repr([(r.id, r.status) for r in rows])[:160])


# --- 3. within-epsilon: measured drift must be a gate FAIL (rejects) -------
def test_fidelity_leg_rejects_out_of_epsilon():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        fid_dir = make_fidelity_matrix(td)
        acc_dir = make_accuracy_report(td, hausdorff=0.5, vol_rel=0.05)
        rows = G.fidelity_leg(fidelity_dir=str(fid_dir),
                              accuracy_report=str(acc_dir / "accuracy-report.json"),
                              eps=1e-9)
        fails = [r for r in rows if r.status == "FAIL"]
        check("fidelity_leg catches geometry drift (FAIL row)",
              len(fails) >= 1, "fails=%d rows=%d" % (len(fails), len(rows)))


# --- 4. fidelity leg: absent artifacts degrade to SKIP, never FAIL/crash ----
def test_fidelity_leg_degrades_on_absent_artifacts():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return
    rows = G.fidelity_leg(fidelity_dir=str(Path("/nonexistent")),
                          accuracy_report=str(Path("/nonexistent/accuracy-report.json")))
    check("fidelity_leg with absent artifacts returns SKIP rows (no crash)",
          isinstance(rows, list) and rows
          and all(r.status in ("SKIP",) for r in rows),
          repr(rows)[:160])


# --- 5. mbd leg: real STEP PMI rows run through the validator, honest --------
def test_mbd_leg_runs_real_step_pmi():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        corpus = make_step_pmi(td)
        rows = G.mbd_leg(step_pmi_manifest=str(corpus / "mbd-real-step-pmi-manifest.json"),
                         corpus_root=str(corpus))
        check("mbd_leg returns list", isinstance(rows, list), repr(rows)[:120])
        # engine present? on bare main mbd_validate is absent -> SKIP; locally
        # with engine importable (P4-M5-T01 python on PYTHONPATH) -> PASS rows.
        engine_ok = G._mbd_engine_available()
        if engine_ok:
            check("mbd_leg: real STEP PMI row checked + honest (PASS)",
                  any(r.id == "synthetic-real-step" and r.status == "PASS"
                      for r in rows),
                  repr([(r.id, r.status) for r in rows])[:200])
        else:
            check("mbd_leg: engine absent degrades to SKIP (CI-safe)",
                  rows and all(r.status == "SKIP" for r in rows),
                  repr([(r.id, r.status) for r in rows])[:200])


# --- 6. run_gate: report + exit code; totals roll up -----------------------
def test_run_gate_report_and_exit_code():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        fid_dir = make_fidelity_matrix(td)
        acc_dir = make_accuracy_report(td, 0.0, 0.0)
        corpus = make_step_pmi(td)
        out = Path(td) / "out"
        code = G.run_gate(fidelity_dir=str(fid_dir),
                          accuracy_report=str(acc_dir / "accuracy-report.json"),
                          step_pmi_manifest=str(corpus / "mbd-real-step-pmi-manifest.json"),
                          corpus_root=str(corpus), outdir=str(out))
        jp = out / "ga-gate-p4-matrix.json"
        mp = out / "ga-gate-p4-matrix.md"
        check("run_gate exit code in {0,1}", code in (0, 1), "code=%r" % (code,))
        check("gate report json written", jp.is_file())
        check("gate report md written", mp.is_file())
        if jp.is_file():
            m = json.load(open(jp))
            summary = m["summary"]
            rows = m["rows"]
            check("report schema_version==1", m.get("schema_version") == 1)
            check("report totals roll up", summary.get("total") == len(rows),
                  "summary.total=%r rows=%d" % (summary.get("total"), len(rows)))
            # exit code contract: 0 iff no FAIL row
            has_fail = any(r["status"] == "FAIL" for r in rows)
            check("exit code matches FAIL rows", (code == 1) == has_fail,
                  "code=%r fail_rows=%r" % (code, has_fail))


# --- 7. degraded mode: bare main -> all SKIP, exit 0, no crash ------------
def test_gate_degrades_on_bare_main():
    try:
        import oep_tooling.ga_gate_p4 as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        code = G.run_gate(fidelity_dir=td, accuracy_report=str(Path(td) / "x.json"),
                          step_pmi_manifest=str(Path(td) / "x.json"),
                          corpus_root=td, outdir=str(Path(td) / "out"))
        check("bare-main degraded run exits 0 (no FAIL)", code == 0,
              "code=%r" % (code,))


# --- main: run all test_* functions, exit 1 on any failure ----------------
def main():
    tests = sorted(n for n in globals()
                   if n.startswith("test_") and callable(globals()[n]))
    for t in tests:
        try:
            globals()[t]()
        except Exception as e:
            check(t, False, "exception: %r" % (e,))
    print("--")
    if _failures:
        print("GA GATE P4 FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("GA GATE P4 GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
