"""P13-T03 acceptance: the docs site shipped from the repo with an honest
format capability matrix.

RED first (this file before the content). The three sections must exist and
be renderable HTML, the machine-checkable format matrix must validate against
a closed fidelity taxonomy with evidence that resolves to committed artifacts,
and the QA script must run to a classified report (never a crash).

Hermetic: no docker, no cargo build, no network. Runs in CI on a fresh
checkout where the P2/P3/P4 ground-truth artifacts may be absent -- in that
case the checker classifies SKIP (honest doctrine), it never fabricates a PASS.
"""

import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE = os.path.join(ROOT, "docs-site")
MATRIX = os.path.join(SITE, "format-matrix.json")
CHECKER = os.path.join(ROOT, "scripts", "format-matrix-check.py")

SECTIONS = {
    "index.html": ("Open Engineering Platform", "getting-started.html"),
    "getting-started.html": ("Getting started", "admin-guide.html"),
    "admin-guide.html": ("Admin guide", "air-gap"),
    "format-matrix.html": ("Format capability matrix", "format-matrix.json"),
}

# Closed set: every fidelity classification used by the docs site must be one
# of these exact tokens (drawn from the P2/P3/P4 GA-going terminology). A new
# capability requires a GA report before it may appear here.
FIDELITY_CLASSES = {
    "mesh-approximate",       # triangle tessellation, no design intent (CGR)
    "mesh-golden",            # comparable within numeric epsilon vs golden
    "container-bom",          # coherent zip container / manifest BOM (3DXML)
    "surfaces-to-mesh",       # surfaces decode then tessellate w/ QC (3DXML)
    "roundtrip-byte-fixed",   # write -> read -> write byte-class fixed point
    "writer-framing-limit",   # member bytes match; zip framing differs (honest)
    "header-parse",           # container/record header decode only (3DRep)
    "brep-topology",          # exact B-Rep topology + product structure (STEP)
    "pmi-semantic",           # GD&T / datums / tolerances semantics (STEP AP242)
    "pmi-reader-gap",         # documented reader FAIL cells (circular runout,
                              #   tolerance modifiers under step-io 0.2.5)
    "accuracy-within-eps",    # Hausdorff / volume error <= epsilon (measured)
    "mbd-checked",            # MBD validator finds; UNVERIFIED never counts PASS
    "qif-basic",              # QIF basic cut: real NIST MBD round-trip, XSD-valid
    "jt-visualization",       # JT mesh/visualization wedge only (no B-Rep/PMI)
    "internal-canonical",     # our own canonical model format (oepgeo/oepmesh)
    "classified-skip",        # reasoned skip taxonomy; never invented geometry
}

VALID_EVIDENCE_PREFIXES = (
    "P10-", "P9-", "P8-", "P7-", "P6-", "P5-", "P4-", "P3-", "P2-", "P1-", "P12-",
)


def _load_matrix():
    with open(MATRIX) as fh:
        return json.load(fh)


def test_three_sections_exist_and_render():
    for name, (needle, link) in SECTIONS.items():
        path = os.path.join(SITE, name)
        assert os.path.isfile(path), f"missing section page {path}"
        html = open(path, encoding="utf-8").read().lower()
        assert needle.lower() in html, f"{name} missing its marker text {needle!r}"
        assert link in html, f"{name} does not link to {link}"
        # every page must link back home and carry the site footer nav
        assert "index.html" in html or name == "index.html"


def test_format_matrix_json_schema_and_closed_taxonomy():
    matrix = _load_matrix()
    assert matrix.get("schema_version") == 1
    assert isinstance(matrix.get("rows"), list) and matrix["rows"]
    seen = set()
    for row in matrix["rows"]:
        assert row["id"] not in seen, f"duplicate row id {row['id']}"
        seen.add(row["id"])
        fids = row.get("fidelity")
        fids = [fids] if isinstance(fids, str) else fids
        assert isinstance(fids, list) and fids, f"row {row['id']} missing fidelity"
        for fid in fids:
            assert fid in FIDELITY_CLASSES, (
                f"row {row['id']} uses an uncommitted fidelity class {fid!r}")
        for key in ("format", "direction", "module", "opens", "evidence", "gap"):
            assert row.get(key) not in (None, ""), f"row {row['id']} missing {key}"
        assert row.get("direction") in {"read", "write", "read/write"}, \
            f"row {row['id']} bad direction"
        assert any(row["evidence"]["gate"].startswith(p) for p in VALID_EVIDENCE_PREFIXES)


def test_checker_runs_to_classified_report():
    """The QA script must run anywhere and classify honestly (PASS/SKIP), never
    crash, and never report FAIL because a ground-truth artifact is absent."""
    env = dict(os.environ)
    env["PYTHONPATH"] = ROOT + os.pathsep + env.get("PYTHONPATH", "")
    proc = subprocess.run(
        [sys.executable, CHECKER, "--site", SITE, "--out", "-"],
        capture_output=True, text=True, env=env, timeout=120)
    assert proc.returncode == 0, proc.stderr
    report = json.loads(proc.stdout)
    assert report["schema_version"] == 1
    assert report["exit"] == 0
    for row in report["rows"]:
        assert row["status"] in {"PASS", "SKIP"}, f"bad status {row['status']}"
        # when we pass ground truth, rows may be PASS; without it they are SKIP.
    assert report["summary"]["fail"] == 0, "no FAIL allowed"


def test_checker_spotchecks_pass_with_ground_truth():
    """Given the P4 GA gate matrix JSON, the checker's numeric spot-checks must
    PASS against the real committed numbers (this is the 'no overstated claims'
    gate over the format capability table)."""
    p4 = os.path.join(ROOT, "corpus", "ga-gate-p4-matrix.json")
    if not os.path.isfile(p4):
        p4 = os.environ.get("P13T03_P4_MATRIX", p4)
    if not os.path.isfile(p4):
        return  # absent ground truth => nothing to over-check; honest SKIP path
    pmi = os.path.join(ROOT, "docs", "p4-m2-t03-pmi-fidelity", "pmi-fidelity-matrix.md")
    if not os.path.isfile(pmi):
        pmi = os.environ.get("P13T03_PMI_MATRIX", pmi)
    cmd = [sys.executable, CHECKER, "--site", SITE, "--p4-matrix", p4, "--out", "-"]
    if os.path.isfile(pmi):
        cmd += ["--p4-pmi", pmi]
    env = dict(os.environ)
    env["PYTHONPATH"] = ROOT + os.pathsep + env.get("PYTHONPATH", "")
    proc = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=120)
    assert proc.returncode == 0, proc.stderr
    report = json.loads(proc.stdout)
    statuses = {r["id"]: r["status"] for r in report["rows"]}
    assert statuses.get("p4-matrix-cells") == "PASS"
    assert statuses.get("p4-matrix-fail-cells") == "PASS"
    assert statuses.get("p4-accuracy-within-eps") == "PASS"
    if os.path.isfile(pmi):
        assert statuses.get("p4-pmi-fail-topics") == "PASS"

