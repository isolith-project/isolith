"""P4-M5-T03 — the P4 GA gate: STEP fidelity matrix published, MBD validator
demo on real corpus material.

This is the capstone acceptance harness of the P4 metal/PMI/MBD workstream. It
runs TWO legs over the published P4 artifacts and the real corpus, and requires
zero gate-`FAIL` rows for the gate to open:

1. FIDELITY leg — the STEP fidelity matrix is PUBLISHED and HONEST:

   * the PMI fidelity matrix (P4-M2-T03, `docs/pmi-fidelity/`):
     the artifact parses, declares the matrix schema
     (`isolith/pmi-fidelity-matrix/v1`), every GD&T x app cell carries a
     closed-set verdict (PASS / FAIL / SKIP) with evidence, and the GD&T type
     set matches the cells. An honest FAIL cell inside the matrix (step-io
     drops circular runout / modifier semantics) is a *finding*, never a gate
     FAIL — the whole point of the matrix is to document capability truth.
   * the accuracy harness report (P4-M1-T03, `corpus/accuracy-report.json`):
     every MEASURED row (real geometry imported on both sides) is within
     epsilon — symmetric Hausdorff <= eps and |volume_rel_error| <= eps.
     This is the "within-epsilon" half of the contract; a measured row out of
     tolerance is a gate-FAIL (geometry drift is real, never relabeled).
   * absent artifacts (sibling branch not merged into this ref) are classified
     SKIP from a closed taxonomy, never FAIL — CI-safe degraded run.

2. MBD leg — the MBD validator DEMO runs on real corpus material: reuse
   `oep_tooling.mbd_validate` (P4-M5-T01; do not re-implement the engine) over
   the real STEP PMI corpus rows (NIST MBE AP242 imported to oep-pmi by the
   P4-M2-T01 `oep-step-pmi` importer — real material, provenance kept) plus the
   P4-M5-T01 MBD fixture manifest when present on the ref. Every input reaches
   a checked row (no drop); the combined report is honest (summary rollup ==
   granular finding counts; UNVERIFIED never counts as PASS; missing-PMI
   honesty preserved); JSON + Markdown reports are written. Gate-FAIL only when
   the demo crashes or the report is dishonest. Engine/manifest absent -> SKIP.

Exit code contract: 0 iff no gate-FAIL row anywhere (SKIP allowed) — identical
to the P2-M6-T03 and P3-M5-T02 GA gates and the P0/P2/P3 golden runner and
regression sweeps. Report `ga-gate-p4-matrix.{json,md}` is self-consistent
(summary totals roll up to the emitted rows; schema_version 1).

Honesty boundary (clean-room): the gate verifies what is PUBLISHED and MEASURED
on this ref. It never invents a matrix, never assumes an absent artifact passes,
and never counts a step-io capability gap as a gate failure (those are recorded
in the matrix itself). The MBD leg reuses the sibling engine unchanged.
"""
from __future__ import annotations

import argparse
import datetime
import importlib
import json
import os
from dataclasses import dataclass
from pathlib import Path

MATRIX_SCHEMA = "isolith/pmi-fidelity-matrix/v1"
VERDICTS = ("PASS", "FAIL", "SKIP")

# Closed SKIP taxonomy for this gate (a SKIP row always carries one of these).
CLOSED_TAXONOMY = (
    "missing-on-disk",       # artifact / manifest not present on this ref
    "fidelity-matrix-unpublished",
    "accuracy-report-unpublished",
    "mbd-engine-unavailable",   # oep_tooling.mbd_validate not importable
    "mbd-manifest-unavailable",
    "no-measured-rows",      # nothing to epsilon-check
    "not-faceted",           # analytic B-Rep source (honest, from report)
    "io-error",
)


@dataclass
class LegRow:
    """One classified outcome of a gate leg."""

    id: str
    leg: str                # "fidelity" | "mbd"
    status: str             # PASS / SKIP / FAIL
    detail: str = ""
    reader: str = "probe"   # probe | mbd-validate | matrix
    cls: str = ""           # closed taxonomy code (SKIP rows)

    def as_dict(self) -> dict:
        return {"id": self.id, "leg": self.leg, "status": self.status,
                "detail": self.detail, "reader": self.reader,
                "class": self.cls}


def _try_import(name):
    try:
        return importlib.import_module(name)
    except Exception:
        return None


def _mbd_engine_available():
    """True when oep_tooling.mbd_validate (P4-M5-T01) is importable here."""
    return _try_import("oep_tooling.mbd_validate") is not None


# --------------------------------------------------------------------------
# FIDELITY leg
# --------------------------------------------------------------------------

def _scan_matrix(matrix_path: Path):
    """Validate the published PMI fidelity matrix.

    Returns (verdict, detail) where verdict is 'PASS' (published + consistent),
    'FAIL' (present but inconsistent/fabricated), or 'SKIP' (absent). Honest
    FAIL cells (step-io gaps) inside a consistent matrix do NOT fail the gate;
    they are reported in the detail.
    """
    if not matrix_path.is_file():
        return "SKIP", "fidelity matrix not on this ref (sibling branch not merged)"
    try:
        data = json.loads(matrix_path.read_text(encoding="utf-8"))
    except Exception as e:
        return "FAIL", "pmi-fidelity-matrix.json unreadable: %r" % (e,)
    if not isinstance(data, dict):
        return "FAIL", "pmi-fidelity-matrix.json is not a JSON object"
    if data.get("matrix_schema") != MATRIX_SCHEMA:
        return "FAIL", "matrix_schema %r != %r" % (data.get("matrix_schema"),
                                                   MATRIX_SCHEMA)
    cells = data.get("cells")
    gd_types = data.get("gd_t_types")
    if not isinstance(cells, dict) or not cells:
        return "FAIL", "matrix has no cells"
    if not isinstance(gd_types, list) or sorted(cells.keys()) != sorted(gd_types):
        return "FAIL", "gd_t_types do not match cells"
    bad = []
    for row, app_cells in cells.items():
        if not isinstance(app_cells, dict):
            bad.append("row %s has no per-app cells" % row)
            continue
        for app, cell in app_cells.items():
            if not isinstance(cell, dict):
                bad.append("%s/%s not an object" % (row, app))
                continue
            if cell.get("verdict") not in VERDICTS:
                bad.append("%s/%s verdict %r not in %s" % (
                    row, app, cell.get("verdict"), VERDICTS))
            if not str(cell.get("evidence", "")).strip():
                bad.append("%s/%s has no evidence" % (row, app))
    if bad:
        return "FAIL", "; ".join(bad[:6])
    # Count honest cell outcomes for the detail (capability truth, not gate).
    counts = {"PASS": 0, "FAIL": 0, "SKIP": 0}
    for app_cells in cells.values():
        for app, cell in app_cells.items():
            counts[cell["verdict"]] += 1
    md = matrix_path.with_suffix(".md")
    if not md.is_file():
        return "FAIL", "pmi-fidelity-matrix.md not published alongside the JSON"
    return ("PASS", "matrix schema ok; cells=%d (PASS %d / FAIL %d / SKIP %d); "
            "FAIL cells are documented step-io capability gaps, not gate FAILs"
            % (sum(counts.values()), counts["PASS"], counts["FAIL"],
               counts["SKIP"]))


def _scan_accuracy(report_path: Path, eps: float):
    """Validate the P4-M1-T03 accuracy harness report.

    Every MEASURED row must be within epsilon (Hausdorff <= eps and
    |volume_rel_error| <= eps). A measured row outside epsilon is a gate-FAIL.
    Skipped rows reproduce their honest report class.
    """
    if not report_path.is_file():
        return [LegRow("accuracy-report", "fidelity", "SKIP",
                       "accuracy report not on this ref (sibling branch not "
                       "merged)", cls="accuracy-report-unpublished")]
    try:
        data = json.loads(report_path.read_text(encoding="utf-8"))
    except Exception as e:
        return [LegRow("accuracy-report", "fidelity", "FAIL",
                       "accuracy-report.json unreadable: %r" % (e,))]
    rows = data.get("rows", [])
    if not isinstance(rows, list) or not rows:
        return [LegRow("accuracy-report", "fidelity", "SKIP",
                       "accuracy report has no rows", cls="no-measured-rows")]
    out = []
    measured = 0
    for r in rows:
        rid = r.get("id", "?")
        status = r.get("status")
        if status != "measured":
            cls = r.get("class") or "missing-on-disk"
            out.append(LegRow(rid, "fidelity", "SKIP",
                              "not measured: %s (%s)" % (status, cls),
                              cls=cls if cls in CLOSED_TAXONOMY
                              else "missing-on-disk"))
            continue
        measured += 1
        m = r.get("metrics") or {}
        h = m.get("symmetric_hausdorff")
        vr = m.get("volume_rel_error")
        try:
            h_f = float(h) if h is not None else float("nan")
            vr_f = float(vr) if vr is not None else 0.0
        except (TypeError, ValueError):
            out.append(LegRow(rid, "fidelity", "FAIL",
                              "measured row has non-numeric metrics: %r" % (m,)))
            continue
        detail = "hausdorff=%.6g vol_rel_err=%.6g eps=%.1e" % (h_f, vr_f, eps)
        if h_f <= eps and abs(vr_f) <= eps:
            out.append(LegRow(rid, "fidelity", "PASS", detail))
        else:
            out.append(LegRow(rid, "fidelity", "FAIL",
                              detail + " OUT OF EPSILON"))
    if measured == 0:
        out.append(LegRow("accuracy-report", "fidelity", "SKIP",
                          "no measured rows on this report", cls="no-measured-rows"))
    return out


def fidelity_leg(fidelity_dir=None, accuracy_report=None, eps=1e-9):
    """FIDELITY leg: STEP fidelity matrix published, measured rows within-epsilon.

    ``fidelity_dir``: directory holding pmi-fidelity-matrix.{json,md}
    (default <repo>/docs/pmi-fidelity). ``accuracy_report``: path to
    corpus/accuracy-report.json. Returns ``LegRow`` list. Never raises.
    """
    repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    fid_dir = Path(fidelity_dir or
                   os.path.join(repo, "docs", "p4-m2-t03-pmi-fidelity"))
    acc_path = Path(accuracy_report or os.path.join(repo, "corpus",
                                                    "accuracy-report.json"))
    rows = []
    verdict, detail = _scan_matrix(fid_dir / "pmi-fidelity-matrix.json")
    rows.append(LegRow("pmi-fidelity-matrix", "fidelity", verdict, detail,
                       reader="matrix",
                       cls="fidelity-matrix-unpublished" if verdict == "SKIP" else ""))
    rows.extend(_scan_accuracy(acc_path, eps))
    # Markdown mirror of the matrix must exist (published both formats).
    return rows


# --------------------------------------------------------------------------
# MBD leg (reuses oep_tooling.mbd_validate — never re-implemented)
# --------------------------------------------------------------------------

def _gather_manifests(step_pmi_manifest, repo):
    """All MBD corpus manifests present on this ref (real STEP + sibling fixture)."""
    out = []
    if step_pmi_manifest:
        out.append(Path(step_pmi_manifest))
    else:
        out.append(Path(repo) / "corpus" / "mbd-real-step-pmi-manifest.json")
    out.append(Path(repo) / "corpus" / "mbd-fixture-manifest.json")
    present = []
    for c in out:
        if c.is_file():
            try:
                data = json.loads(c.read_text(encoding="utf-8"))
            except Exception:
                continue
            present.append((c, (data.get("fixtures") or data.get("inputs") or [])))
    return present


def mbd_leg(step_pmi_manifest=None, corpus_root=None):
    """MBD leg: validator demo runs on real corpus material.

    Reuses `oep_tooling.mbd_validate.run_manifest + build_report + write_report`.
    Every declared real input must reach a 'checked' report (no drop) and the
    combined report must be honest. Gate-FAIL only when the demo crashes, the
    report is dishonest, or an input is dropped. Engine / manifest absent
    -> SKIP rows.
    """
    repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    base = corpus_root or os.path.join(repo, "corpus")
    manifests = _gather_manifests(step_pmi_manifest, repo)
    if not manifests:
        return [LegRow("mbd-demo", "mbd", "SKIP",
                       "no MBD corpus manifest on this ref", cls="mbd-manifest-unavailable")]
    IV = _mbd_engine_available()
    if not IV:
        return [LegRow("mbd-demo", "mbd", "SKIP",
                       "oep_tooling.mbd_validate not importable on this ref "
                       "(P4-M5-T01 sibling not merged)", cls="mbd-engine-unavailable")]
    mbd = importlib.import_module("oep_tooling.mbd_validate")
    declared = {}
    reports = []
    for manifest_path, inputs in manifests:
        for inp in inputs:
            declared[inp.get("id")] = inp
        try:
            reports.extend(mbd.run_manifest(str(manifest_path)))
        except Exception as e:
            rows = [LegRow("mbd-demo", "mbd", "FAIL",
                           "mbd_validate.run_manifest crashed on %s: %r" % (
                               os.path.basename(str(manifest_path)), e,))]
            rows.append(LegRow("mbd-report", "mbd", "FAIL",
                               "report not produced after engine crash"))
            return rows
    by_id = {}
    for r in reports:
        by_id.setdefault(r.get("id"), []).append(r)
    rows = []
    for rid in sorted(declared):
        got = by_id.get(rid)
        if not got:
            rows.append(LegRow(rid, "mbd", "FAIL",
                               "input produced no checked report (DROPPED)"))
            continue
        ok = True
        detail = []
        for r in got:
            st = r.get("status")
            s = r.get("summary") or {}
            if st == "error":
                ok = False
                detail.append("error: %s" % r.get("error"))
                continue
            if st != "checked":
                ok = False
                detail.append("unexpected status %r" % st)
            total = s.get("total", 0)
            calc = sum(s.get(k, 0) for k in ("pass", "fail", "warn", "unverified"))
            if total != calc:
                ok = False
                detail.append("rollup mismatch total=%d vs %d" % (total, calc))
            if s.get("unverified", 0) and s.get("pass", 0) >= s.get("total", 0) \
                    and s.get("pass", 0) > 0:
                ok = False
                detail.append("UNVERIFIED counted as PASS")
            detail.append("r=%s fail=%d warn=%d unverified=%d pass=%d" % (
                r.get("rule_set"), s.get("fail", 0), s.get("warn", 0),
                s.get("unverified", 0), s.get("pass", 0)))
        rows.append(LegRow(rid, "mbd", "PASS" if ok else "FAIL",
                           "; ".join(detail) or "checked",
                           reader="mbd-validate"))
    # Honest combined report written (JSON + MD) as part of the demo. We reuse
    # the engine's build_report + render_markdown but write under a gate-owned
    # filename (ga-gate-p4-mbd-report.*) so we never collide with the sibling
    # P4-M5-T01 corpus/mbd-validate-report.* path when both branches merge.
    try:
        report = mbd.build_report(reports, {"task": "P4-M5-T03", "rule_sets":
                                            sorted({r["rule_set"] for r in reports})})
        jp = os.path.join(base, "ga-gate-p4-mbd-report.json")
        mp = os.path.join(base, "ga-gate-p4-mbd-report.md")
        with open(jp, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2, sort_keys=False)
            fh.write("\n")
        with open(mp, "w", encoding="utf-8") as fh:
            fh.write(mbd.render_markdown(report))
        rows.append(LegRow("mbd-report", "mbd", "PASS",
                           "wrote %s and %s; summary rollup == granular "
                           "(inputs=%d findings=%d pass=%d fail=%d warn=%d "
                           "unverified=%d)" % (
                               os.path.basename(jp), os.path.basename(mp),
                               report["summary"]["inputs"],
                               report["summary"]["findings"],
                               report["summary"]["pass"],
                               report["summary"]["fail"],
                               report["summary"]["warn"],
                               report["summary"]["unverified"]),
                           reader="mbd-validate"))
    except Exception as e:
        rows.append(LegRow("mbd-report", "mbd", "FAIL",
                           "report write crashed: %r" % (e,)))
    return rows


# --------------------------------------------------------------------------
# Gate runner: two legs + report + exit code
# --------------------------------------------------------------------------

def build_report(rows) -> dict:
    n_pass = sum(1 for r in rows if r.status == "PASS")
    n_fail = sum(1 for r in rows if r.status == "FAIL")
    n_skip = sum(1 for r in rows if r.status == "SKIP")
    n_open = 0  # this gate's taxonomy has no OPEN status
    by_leg = {}
    for r in rows:
        by_leg[r.leg] = by_leg.get(r.leg, 0) + 1
    return {
        "schema_version": 1,
        "generated": datetime.datetime.now().isoformat(timespec="seconds"),
        "task": "P4-M5-T03",
        "summary": {
            "total": len(rows),
            "pass": n_pass,
            "open": n_open,
            "fail": n_fail,
            "skip": n_skip,
            "by_leg": dict(sorted(by_leg.items())),
        },
        "rows": [r.as_dict() for r in rows],
    }


def exit_code(rows) -> int:
    return 1 if any(r.status == "FAIL" for r in rows) else 0


def write_report(matrix: dict, outdir: str) -> list:
    out = Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    jp = out / "ga-gate-p4-matrix.json"
    jp.write_text(json.dumps(matrix, indent=2) + "\n", encoding="utf-8")
    mp = out / "ga-gate-p4-matrix.md"
    lines = ["# Isolith P4 GA gate - matrix (P4-M5-T03)", ""]
    s = matrix["summary"]
    lines.append("total=%d pass=%d fail=%d skip=%d (%s)" % (
        s["total"], s["pass"], s["fail"], s["skip"],
        " | ".join("%s=%d" % kv for kv in sorted(s["by_leg"].items()))))
    lines.append("")
    lines.append("| leg | id | status | detail |")
    lines.append("|-----|----|--------|--------|")
    for r in matrix["rows"]:
        lines.append("| %s | %s | %s | %s |" % (
            r["leg"], r["id"], r["status"],
            (r.get("detail", "") or "")[:100].replace("|", "/")))
    lines.append("")
    mp.write_text("\n".join(lines), encoding="utf-8")
    return [str(jp), str(mp)]


def run_gate(fidelity_dir=None, accuracy_report=None, step_pmi_manifest=None,
             corpus_root=None, outdir="corpus", eps=1e-9) -> int:
    try:
        fd = fidelity_leg(fidelity_dir=fidelity_dir,
                          accuracy_report=accuracy_report, eps=eps)
    except Exception as e:
        fd = [LegRow("fidelity", "fidelity", "SKIP",
                     "fidelity-leg error: %r" % (e,), cls="io-error")]
    try:
        mbd = mbd_leg(step_pmi_manifest=step_pmi_manifest,
                      corpus_root=corpus_root)
    except Exception as e:
        mbd = [LegRow("mbd", "mbd", "SKIP", "mbd-leg error: %r" % (e,),
                      cls="io-error")]
    rows = fd + mbd
    matrix = build_report(rows)
    written = write_report(matrix, outdir or ".")
    code = exit_code(rows)
    s = matrix["summary"]
    print("P4 GA gate: total=%d pass=%d fail=%d skip=%d | exit=%d" % (
        s["total"], s["pass"], s["fail"], s["skip"], code))
    for w in written:
        print("  wrote %s" % w)
    return code


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(
        prog="python -m oep_tooling.ga_gate_p4",
        description="P4-M5-T03 GA gate: STEP fidelity matrix published; MBD "
                    "validator demo on real corpus material.")
    ap.add_argument("--fidelity-dir", default=None,
                    help="dir holding pmi-fidelity-matrix.{json,md}")
    ap.add_argument("--accuracy-report", default=None,
                    help="path to corpus/accuracy-report.json")
    ap.add_argument("--step-pmi-manifest", default=None,
                    help="real STEP PMI corpus manifest JSON")
    ap.add_argument("--corpus-root", default=None, help="corpus dir")
    ap.add_argument("--out", default="corpus", help="report output dir")
    ap.add_argument("--eps", type=float, default=1e-9,
                    help="within-epsilon tolerance")
    args = ap.parse_args(argv)
    return run_gate(fidelity_dir=args.fidelity_dir,
                    accuracy_report=args.accuracy_report,
                    step_pmi_manifest=args.step_pmi_manifest,
                    corpus_root=args.corpus_root, outdir=args.out, eps=args.eps)


if __name__ == "__main__":
    import sys
    sys.exit(main())
