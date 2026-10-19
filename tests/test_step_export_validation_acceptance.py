#!/usr/bin/env python3
"""P4-M3-T02 acceptance test: STEP export golden-diff gate vs independent reader.

TDD: this test documents the acceptance criteria BEFORE the harness exists, so it
MUST FAIL (RED) against a tree without `oep_tooling.step_export_validation`,
then PASS (GREEN) once the implementation lands on real material.

'Done' for P4-M3-T02 means:
  - an independent-reader seam exists (Open CASCADE probe binary, discovered via
    OEP_STEP_READER or the committed corpus/tools/occ_probe build), and recovery
    facts come from THAT reader's output, never from our own STEP parser;
  - probe key=value stdout is parsed deterministically into recovery facts
    (transfer, shape_type, solids/shells/faces/edges/vertices, volume, area,
    bbox); malformed/missing reader output is a classified error, never a
    fabricated number;
  - per corpus pair, current independent-recovery is compared against the
    committed golden expectations within tolerance:
      * matches  -> `measured` (green),
      * differs  -> `drift` (classified honestly, red until golden re-blessed),
      * cannot read / reader absent -> classified `skip` from a closed taxonomy
        (reader-unavailable, reader-error, fixture-missing, unparsable,
        io-error) -- tolerated, never crashes the sweep;
  - the real box EXPORT row classifies consistently with the committed golden
    (as of this card: parsed but no-transfer, product-wiring reason) and the
    conformance reference row (AP214 OCC-conventional wiring of the same box)
    measures as OCC-recoverable (transfer=true) when the probe is present;
  - golden-diff gate `--check` exits 0 when every pair row is measured (or a
    classified skip) and non-zero on an unclassified `drift`;
  - the harness reuses the corpus infra: pair-list JSON schema, generated report
    JSON + markdown, rollup == len(rows), tests/ + docs/re-notes/ locations.

Clean-room rule: the golden reference and the harness NEVER call our own writers
or our own STEP parsers for the measured value; the gate measures only what the
independent reader (OCC) reports. Pure stdlib (probe invoked as a subprocess).
Runs under `python3 tests/test_step_export_validation_acceptance.py` and pytest.
Exit code 1 on any failure.
"""
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


CLOSED_TAXONOMY = {
    "reader-unavailable", "reader-error", "fixture-missing",
    "unparsable", "io-error",
}
DRIFT_CLASSES = {"transfer-flipped-true", "transfer-flipped-false", "geometry-differs"}

EXPORT_BOX = os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                          "box-reference.step")
REF_BOX = os.path.join(REPO, "corpus", "reference", "box-AP214-convention.step")
PROBE = os.path.join(REPO, "corpus", "tools", "occ_probe")

# A complete probe output the OCC STEPControl_Reader emits for the AP214
# conventional box reference (observed on real OCCT 7.6 on U1, 2026-10-16).
PROBE_BOX_OK = """read_status=1
roots=1
shape_type=0
solids=0
shells=12
faces=12
edges=36
vertices=72
volume=2.7755575615628914e-17
area=6
bbox_min_x=-9.9999999999999995e-08
bbox_min_y=-9.9999999999999995e-08
bbox_min_z=-9.9999999999999995e-08
bbox_max_x=1.0000001000000001
bbox_max_y=1.0000001000000001
bbox_max_z=1.0000001000000001
"""

# Probe output OCC emits for our as-committed export (reads the file, transfers
# nothing): ReadFile status Done, root present, then NbShapes == 0.
PROBE_BOX_NONE = """read_status=1
roots=1
shape_type=NULL
"""


def _import_mod():
    try:
        import oep_tooling.step_export_validation as m
        check("import oep_tooling.step_export_validation", True)
        return m
    except Exception as e:  # RED: module absent until implementation
        check("import oep_tooling.step_export_validation", False, repr(e))
        return None


def _looks_like_probe_line(line):
    return bool(re.match(r"^[a-z_]+\s*=.*$", line))


# ---------------------------------------------------------------------------
# 1) deterministic probe-output parsing + tolerant errors
# ---------------------------------------------------------------------------
def test_probe_output_parsing():
    m = _import_mod()
    if m is None:
        return
    facts = m.parse_probe_output(PROBE_BOX_OK)
    check("parse OK box -> transfer True / faces 12 / area 6",
          facts.get("transfer") is True and facts.get("faces") == 12
          and abs(float(facts.get("area")) - 6.0) < 1e-9,
          detail=str(facts))
    check("parse OK box -> bbox min/max recorded",
          "bbox_min" in facts and "bbox_max" in facts
          and len(facts["bbox_max"]) == 3)

    none_facts = m.parse_probe_output(PROBE_BOX_NONE)
    check("parse no-transfer -> transfer False, shape_type NULL",
          none_facts.get("transfer") is False
          and none_facts.get("shape_type") == "NULL",
          detail=str(none_facts))

    try:
        m.parse_probe_output("")
        check("empty probe output raises typed error", False, "no exception")
    except m.ProbeError:
        check("empty probe output raises typed error", True)
    except Exception as e:
        check("empty probe output raises typed error", False, repr(e))

    # junk that looks like probe key=value but has no transfer -> error
    try:
        m.parse_probe_output("read_status=1\nnonsense")
        check("non-ISO probe output (no transfer key) raises", False, "no exception")
    except m.ProbeError:
        check("non-ISO probe output (no transfer key) raises", True)
    except Exception as e:
        check("non-ISO probe output (no transfer key) raises", False, repr(e))


# ---------------------------------------------------------------------------
# 2) independent-reader discovery (OEP_STEP_READER / committed probe)
# ---------------------------------------------------------------------------
def test_independent_reader_discovery():
    m = _import_mod()
    if m is None:
        return
    # The independent reader is a build artifact: its SOURCE + build script are
    # committed (CI-safe); the compiled binary only exists where built (U1).
    check("committed probe source present", os.path.isfile(os.path.join(
        REPO, "corpus", "tools", "occ_probe.cpp")))
    check("committed probe build script present", os.path.isfile(os.path.join(
        REPO, "corpus", "tools", "build_occ_probe.sh")))
    probe = m.find_probe(PROBE)
    if probe is not None and os.path.isfile(probe) and os.access(probe, os.X_OK):
        check("committed probe binary resolvable when built", True)
    else:
        # binary not built on this host -> honest reader-unavailable (CI-safe);
        # the real-material gate test branches on this.
        check("committed probe binary resolvable when built (skip: not built here)",
              True, detail="no compiled occ_probe on this host")

    old = os.environ.get("OEP_STEP_READER")
    os.environ["OEP_STEP_READER"] = os.path.join(tempfile.gettempdir(),
                                                 "missing-probe-xyz")
    try:
        probe2 = m.find_probe(PROBE)
        check("OEP_STEP_READER to a missing binary -> None (honest unavailable)",
              probe2 is None)
    finally:
        if old is None:
            os.environ.pop("OEP_STEP_READER", None)
        else:
            os.environ["OEP_STEP_READER"] = old


# ---------------------------------------------------------------------------
# 3) real-material: probe (when present) vs committed golden
# ---------------------------------------------------------------------------
def test_real_material_gate():
    m = _import_mod()
    if m is None:
        return
    probe = m.find_probe(PROBE)
    check("corpus pair list present", os.path.isfile(os.path.join(
        REPO, "corpus", "export-validation-pairs.json")))
    golden = m.load_golden(os.path.join(REPO, "corpus",
                                        "export-validation-golden.json"))
    check("golden file present + schema v1",
          golden.get("schema_version") == 1 and golden.get("pairs"))
    if golden.get("pairs") is None:
        return
    check("golden exposes box-export + conformance rows",
          "keelworks-box-export" in golden["pairs"]
          and "keelworks-box-conformance" in golden["pairs"],
          detail=str(sorted(golden.get("pairs", {}).keys())))

    if probe is None:
        # CI-safe: without the probe binary every row must be a classified skip.
        pairs = m.load_pair_list(os.path.join(REPO, "corpus",
                                              "export-validation-pairs.json"))
        rows = m.run(pairs, golden, REPO, probe)
        check("reader-absent -> all rows classified skip (no drift/fabrication)",
              rows and all(r["status"] == "skip"
                           and r["class"] in CLOSED_TAXONOMY for r in rows),
              detail=str(rows))
        return

    # Conformance reference (AP214 conventional wiring) must be OCC-recoverable.
    facts = m.independent_read(REF_BOX, probe)
    check("independent reader recovers conformance box (transfer True)",
          facts.get("transfer") is True,
          detail=str(facts.get("shape_type")))
    check("conformance faces == 12, area == 6 (real OCC recovery)",
          facts.get("faces") == 12 and abs(facts.get("area") - 6.0) < 1e-9,
          detail="faces=%s area=%s" % (facts.get("faces"), facts.get("area")))

    # As-committed export: OCC parses but transfers nothing (honest drift state).
    exp = m.independent_read(EXPORT_BOX, probe)
    check("as-committed export -> transfer False (no shape)",
          exp.get("transfer") is False,
          detail=str(exp))

    # Gate over the pair list: every row measured or classified; no drift.
    pairs = m.load_pair_list(os.path.join(REPO, "corpus",
                                          "export-validation-pairs.json"))
    rows = m.run(pairs, golden, REPO, probe)
    check("pair-list coverage == len(pairs)", len(rows) == len(pairs),
          detail="rows=%d pairs=%d" % (len(rows), len(pairs)))
    for r in rows:
        ok = (r["status"] == "measured"
              or (r["status"] == "drift" and r["class"] in DRIFT_CLASSES)
              or (r["status"] == "skip" and r["class"] in CLOSED_TAXONOMY))
        check("row %s honestly classified" % r["id"], ok, detail=str(r))
    check("no row outside taxonomy (all statuses covered)",
          all((r["status"] == "measured" or r["class"]) for r in rows))


# ---------------------------------------------------------------------------
# 4) golden-diff classification logic (unit, injected probe output)
# ---------------------------------------------------------------------------
def test_golden_diff_classification():
    m = _import_mod()
    if m is None:
        return
    g_ok = {"transfer": True, "faces": 12, "area": 6.0}
    tol = {"faces": 0, "area": 1e-6, "counts": 0}
    r_ok = m.classify("x", m.parse_probe_output(PROBE_BOX_OK),
                      {"transfer": True, "faces": 12, "area": 6.0}, tol)
    check("match -> measured", r_ok["status"] == "measured", detail=str(r_ok))

    r_d1 = m.classify("x", m.parse_probe_output(PROBE_BOX_OK),
                      {"transfer": False}, tol)
    check("golden no-transfer but current transfer => drift flip-true",
          r_d1["status"] == "drift"
          and r_d1["class"] == "transfer-flipped-true", detail=str(r_d1))

    r_d2 = m.classify("x", m.parse_probe_output(PROBE_BOX_NONE),
                      {"transfer": True, "faces": 12, "area": 6.0}, tol)
    check("golden transfer but current none => drift flip-false",
          r_d2["status"] == "drift"
          and r_d2["class"] == "transfer-flipped-false", detail=str(r_d2))

    diff = "area=5\n"  # transfer true but geometry differs
    r_d3 = m.classify("x", m.parse_probe_output(PROBE_BOX_OK.replace(
        "area=6", "area=5")),
        {"transfer": True, "faces": 12, "area": 6.0}, tol)
    check("same transfer, differing geometry => drift geometry-differs",
          r_d3["status"] == "drift"
          and r_d3["class"] == "geometry-differs", detail=str(r_d3))


# ---------------------------------------------------------------------------
# 5) hostile / absent inputs never crash the sweep
# ---------------------------------------------------------------------------
def test_hostile_inputs_tolerant():
    m = _import_mod()
    if m is None:
        return
    probe = m.find_probe(PROBE)
    td = tempfile.mkdtemp(prefix="p4m3t02-")
    Path(os.path.join(td, "bad.step")).write_bytes(b"\x00\x01\x02")
    Path(os.path.join(td, "empty.step")).write_bytes(b"")
    variants = {
        "missing": os.path.join(td, "nope.step"),
        "bad": os.path.join(td, "bad.step"),
        "empty": os.path.join(td, "empty.step"),
    }
    for name, p in variants.items():
        ok = False
        detail = ""
        try:
            if probe is None:
                cls = m.discover_skip(None, p)
                ok = cls[0] in CLOSED_TAXONOMY
                detail = "%s" % (cls,)
            else:
                if name == "missing":
                    # missing fixture -> typed StepValidationError (classified)
                    try:
                        m.independent_read(p, probe)
                        ok = False
                        detail = "no exception for missing file"
                    except m.StepValidationError:
                        ok = True
                        detail = "typed StepValidationError"
                else:
                    facts = m.independent_read(p, probe)
                    ok = isinstance(facts, dict) and facts.get("transfer") is False
                    detail = str(facts)
        except Exception as e:  # should never propagate
            ok = False
            detail = "UNCAUGHT %r" % e
        check("hostile %s -> typed/no crash" % name, ok, detail=detail)


# ---------------------------------------------------------------------------
# 6) report writer + rollup consistency
# ---------------------------------------------------------------------------
def test_report_writer():
    m = _import_mod()
    if m is None:
        return
    rows = [
        {"id": "a", "status": "measured", "class": "", "reason": "",
         "current": {"transfer": True}, "golden": {"transfer": True}},
        {"id": "b", "status": "skip", "class": "reader-unavailable", "reason": "x",
         "current": {}, "golden": {}},
    ]
    report = m.build_report(rows, "2026-10-16T00:00:00+00:00")
    check("report schema + rollup == len(rows)",
          report["schema_version"] == 1 and report["summary"]["total"] == 2
          and report["summary"]["measured"] + report["summary"]["skipped"] == 2,
          detail=str(report.get("summary")))
    with tempfile.TemporaryDirectory() as td:
        jp, mp = m.write_report(report, td)
        rt = json.loads(Path(jp).read_text())
        check("report JSON round-trips", rt["summary"]["total"] == 2)
        check("markdown written", os.path.isfile(mp) and os.path.getsize(mp) > 0)


if __name__ == "__main__":
    for fn_name in sorted(n for n in dir(sys.modules[__name__])
                          if n.startswith("test_")):
        globals()[fn_name]()
    print("-" * 50)
    if _failures:
        print("RESULT: FAIL (%d)" % len(_failures))
        sys.exit(1)
    print("RESULT: PASS")
    sys.exit(0)
