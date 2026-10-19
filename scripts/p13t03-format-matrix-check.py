#!/usr/bin/env python3
"""P13-T03 docs-site format-matrix QA / spot-check.

Verifies two things about the committed docs-site:

1. TABLE: docs-site/format-matrix.json is well-formed - schema_version, a
   closed-set fidelity taxonomy, valid direction, gate-tagged evidence - and
   its rendered page (format-matrix.html) stays in sync with the JSON.

2. SPOT-CHECK against the committed P2/P3/P4 GA matrices (the "no overstated
   claims" gate): the truth anchors encoded in format-matrix.json must match
   the committed GA report numbers when those reports are on the ref. Absent a
   source artifact the leg classifies SKIP with a reason (honest doctrine:
   never a fabricated PASS, never a hard fail for a missing artifact).

Honest exit code contract (same as the GA gates): 0 iff no FAIL row.

Usage:
  scripts/p13t03-format-matrix-check.py \
      --site docs-site \
      [--p4-matrix corpus/ga-gate-p4-matrix.json] \
      [--p2-report docs/re-notes/p2-m6-t03-ga-gate.md] \
      [--p3-report docs/re-notes/p3-m5-t02-ga-gate.md] \
      [--out -]           # default: docs-site/proof/format-matrix-check.json
"""

import argparse
import json
import os
import re
import sys

SCHEMA_VERSION = 1
CLOSED_DIRECTIONS = {"read", "write", "read/write"}

# Overclaim tokens the honest matrix must never contain in its own wording
# unless a ground-truth GA report uses them. Kept tiny and conservative.
OVERCLAIM_TOKENS = [
    "lossless",
    "100% fidelity",
    "bit-perfect",
    "identical to",
    "full support for all",
    "reads everything",
]

ROWS_TABLE = {
    "cgr-read": "CGR",
    "cgr-export-gltf": "CGR to glTF/GLB",
    "3dxml-read": "3DXML",
    "3dxml-write": "3DXML (write)",
    "3drep-header": "3DRep",
    "step-read": "STEP AP203/AP214/AP242",
    "step-write": "STEP AP242 (write)",
    "step-accuracy": "STEP accuracy",
    "step-pmi": "STEP AP242 PMI",
    "step-mbd": "STEP MBD validation",
    "qif": "QIF",
    "jt": "JT",
    "internal-canonical": "OEP canonical (oepgeo / oepmesh)",
    "stl": "STL",
}


def load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def safe_read(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except OSError:
        return ""


def classify(rows, anchor_id, status, detail, source):
    rows.append({
        "id": anchor_id,
        "leg": "spot-check",
        "status": status,
        "detail": detail,
        "source": source,
    })
    return rows


def spot_check_p4(rows, matrix_path, pmi_path):
    """The committed P4 GA gate matrix numbers must match our truth anchors."""
    try:
        m = load_json(matrix_path)
    except (OSError, ValueError) as e:
        return classify(rows, "p4-matrix-cells", "SKIP",
                        f"p4 matrix source absent/not parseable: {e}", matrix_path)

    # Spot-check 1: the fidelity matrix row reproduces the committed totals.
    detail_0 = ""
    for row in m.get("rows", []):
        if row.get("id") == "pmi-fidelity-matrix":
            detail_0 = (row.get("detail") or "") + " | " + (row.get("status") or "")
    ok_cells = all(tok in detail_0 for tok in ["cells=64", "PASS 14", "FAIL 2", "SKIP 48"])
    classify(rows, "p4-matrix-cells",
             "PASS" if ok_cells else "FAIL",
             f"fidelity-matrix row detail {'matches' if ok_cells else 'MISMATCHES'} committed totals (cells=64, PASS 14, FAIL 2, SKIP 48): {detail_0[:160]}"
             if ok_cells else f"fidelity-matrix detail diverges from committed totals: {detail_0[:160]}",
             matrix_path)

    # Spot-check 2: the 2 FAIL cells are the documented step-io gaps (as the
    # committed GA matrix states; the exact topics are audited separately from
    # the PMI fidelity matrix source).
    lower = detail_0.lower()
    ok_gaps = "documented step-io capability gaps" in lower
    classify(rows, "p4-matrix-fail-cells",
             "PASS" if ok_gaps else "FAIL",
             "FAIL cells documented as step-io capability gaps in the GA matrix"
             if ok_gaps else f"GA matrix does not commit the step-io-gap wording: {detail_0[:160]}",
             matrix_path)

    # Spot-check 2b: the two FAIL topics (circular runout, tolerance modifiers)
    # are recorded in the committed PMI fidelity matrix. Absent on this ref:
    # classified SKIP, never a fabricated pass.
    pmi_text = safe_read(pmi_path)
    if not pmi_text:
        classify(rows, "p4-pmi-fail-topics", "SKIP",
                f"PMI fidelity matrix absent on this ref: {pmi_path}", pmi_path)
    else:
        lower = pmi_text.lower()
        ok_topics = ("circular-runout" in lower and "circular runout" in lower
                     and "modifiers" in lower and "fail" in lower)
        classify(rows, "p4-pmi-fail-topics",
                 "PASS" if ok_topics else "FAIL",
                 "PMI fidelity matrix records circular-runout and modifiers FAIL cells"
                 if ok_topics else f"PMI fidelity matrix does not record both FAIL topics (or not FAIL): {pmi_path}",
                 pmi_path)

    # Spot-check 3: the within-epsilon accuracy row is PASS as committed.
    box = next((r for r in m.get("rows", []) if r.get("id") == "keelworks-box"), None)
    if box is None:
        return classify(rows, "p4-accuracy-within-eps", "FAIL",
                        "keelworks-box row missing from p4 matrix", matrix_path)
    bd = (box.get("detail") or "").lower()
    ok_acc = box.get("status") == "PASS" and "hausdorff=0" in bd and \
             "vol_rel_err=0" in bd and "1.0e-09" in bd
    classify(rows, "p4-accuracy-within-eps",
             "PASS" if ok_acc else "FAIL",
             f"keelworks-box PASS hausdorff=0 vol_rel_err=0 eps=1.0e-09"
             if ok_acc else f"keelworks-box diverges from committed accuracy row: {box}",
             matrix_path)
    return rows


def spot_check_gate_report(rows, anchor_id, path, expected_line):
    text = safe_read(path)
    if not text:
        return classify(rows, anchor_id, "SKIP",
                        f"GA report source absent on this ref: {path}", path)
    norm = re.sub(r"\s+", " ", text)
    ok = expected_line in norm
    classify(rows, anchor_id,
             "PASS" if ok else "FAIL",
             f"summary line matches committed GA report"
             if ok else f"expected summary line not found in {path}: {expected_line}",
             path)
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--site", default="docs-site")
    ap.add_argument("--p4-matrix", default=None)
    ap.add_argument("--p4-pmi", default=None)
    ap.add_argument("--p2-report", default=None)
    ap.add_argument("--p3-report", default=None)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    here = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(here)
    site = args.site if os.path.isabs(args.site) else os.path.join(repo_root, args.site)
    p4 = args.p4_matrix or os.path.join(repo_root, "corpus", "ga-gate-p4-matrix.json")
    p2 = args.p2_report or os.path.join(repo_root, "docs", "re-notes", "p2-m6-t03-ga-gate.md")
    p3 = args.p3_report or os.path.join(repo_root, "docs", "re-notes", "p3-m5-t02-ga-gate.md")

    rows = []

    # ---- TABLE checks (always run; a violation here is a real defect) ----
    matrix_path = os.path.join(site, "format-matrix.json")
    if not os.path.isfile(matrix_path):
        rows.append({"id": "table-present", "leg": "table", "status": "FAIL",
                     "detail": f"format-matrix.json missing at {matrix_path}", "source": matrix_path})
        print(f"P13-T03 format-matrix check: total=1 pass=0 fail=1 skip=0 | exit=1")
        return 1

    matrix = load_json(matrix_path)
    taxonomy = set(matrix.get("fidelity_classes", []))
    if matrix.get("schema_version") != SCHEMA_VERSION:
        rows.append({"id": "table-schema", "leg": "table", "status": "FAIL",
                     "detail": f"schema_version {matrix.get('schema_version')} != {SCHEMA_VERSION}",
                     "source": matrix_path})
    else:
        rows.append({"id": "table-schema", "leg": "table", "status": "PASS",
                     "detail": f"schema_version {SCHEMA_VERSION}", "source": matrix_path})

    bad = []
    seen = set()
    declared_formats = {r["id"]: r.get("format", "") for r in matrix.get("rows", [])}
    for r in matrix.get("rows", []):
        rid = r.get("id")
        if not rid or rid in seen:
            bad.append(f"row id missing/duplicate: {rid!r}")
            continue
        seen.add(rid)
        if r.get("direction") not in CLOSED_DIRECTIONS:
            bad.append(f"{rid}: direction {r.get('direction')!r} not closed set")
        for tok in r.get("fidelity", []):
            if tok not in taxonomy:
                bad.append(f"{rid}: fidelity {tok!r} not in closed taxonomy")
        gate = r.get("evidence", {}).get("gate", "")
        if not re.match(r"^P\d+", gate):
            bad.append(f"{rid}: evidence.gate {gate!r} not a GA report id")
        # overclaim guard
        blob = " ".join(str(r.get(k, "")) for k in ("opens", "gap", "module")).lower()
        for tok in OVERCLAIM_TOKENS:
            if tok in blob:
                bad.append(f"{rid}: overclaim candidate token {tok!r} in honest matrix")
    # Rendered page in sync with the JSON rows (row-anchor ids must exist, and
    # the format label must appear once formatting tags, whitespace and entity
    # differences are normalized: lowercase, tags+whitespace stripped,
    # "→" == "to").
    html_path = os.path.join(site, "format-matrix.html")
    html = safe_read(html_path)
    html_no_tags = re.sub(r"<[^>]+>", "", html)
    norm_html = re.sub(r"\s+", "", html_no_tags.lower()).replace("→", "to")
    for rid, fmt in declared_formats.items():
        norm_fmt = re.sub(r"\s+", "", str(fmt).lower()).replace("→", "to")
        if str(fmt).strip() and norm_fmt not in norm_html:
            bad.append(f"format-matrix.html missing row for {fmt!r} ({rid})")
        anchor = f'id="{rid}"'
        if anchor not in html:
            bad.append(f"format-matrix.html missing anchor {anchor}")

    if bad:
        rows.append({"id": "table-rows", "leg": "table", "status": "FAIL",
                     "detail": "; ".join(bad[:6]), "source": matrix_path})
    else:
        rows.append({"id": "table-rows", "leg": "table", "status": "PASS",
                     "detail": f"{len(seen)} rows valid, closed taxonomy, page in sync",
                     "source": matrix_path})

    # ---- SPOT-CHECK legs (SKIP when ground truth absent on the ref) ----
    pmi_base = args.p4_pmi or os.path.join(
        repo_root, "docs", "p4-m2-t03-pmi-fidelity", "pmi-fidelity-matrix.md")
    spot_check_p4(rows, p4, pmi_base)
    spot_check_gate_report(
        rows, "p2-gate-summary", p2,
        "GA gate: total=24 pass=1 open=11 fail=0 skip=12 | exit=0")
    spot_check_gate_report(
        rows, "p3-gate-summary", p3,
        "P3 GA gate: total=18 pass=9 open=7 fail=0 skip=2 | exit=0")

    # ---- rollup + exit contract (0 iff no FAIL) ----
    states = {}
    for r in rows:
        states.setdefault(r["status"], []).append(r["id"])
    fail_rows = states.get("FAIL", [])
    summary = {
        "total": len(rows),
        "pass": len(states.get("PASS", [])),
        "fail": len(fail_rows),
        "skip": len(states.get("SKIP", [])),
    }
    report = {
        "schema_version": 1,
        "generated": None,
        "task": "workstreams/P13-T03",
        "exit": 0 if not fail_rows else 1,
        "summary": summary,
        "rows": rows,
    }
    out_path = args.out
    if out_path == "-":
        json.dump(report, sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        if not out_path:
            out_dir = os.path.join(site, "proof")
            os.makedirs(out_dir, exist_ok=True)
            out_path = os.path.join(out_dir, "format-matrix-check.json")
        parent = os.path.dirname(os.path.abspath(out_path))
        os.makedirs(parent, exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
        print(f"P13-T03 format-matrix check: total={summary['total']} "
              f"pass={summary['pass']} fail={summary['fail']} skip={summary['skip']} | "
              f"exit={report['exit']}")
        print(f"report: {out_path}")
    return report["exit"]


if __name__ == "__main__":
    sys.exit(main())
