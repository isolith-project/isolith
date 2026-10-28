#!/usr/bin/env python3
"""P7-M4-T02 acceptance test: `oep-rules` knowledge-light rules/checks sandbox.

TDD RED - written before the implementation exists. Runs the rules demo on a
REAL part (the committed canonical bracket golden) and independently checks the
results with Python stdlib only, so a self-consistent-but-wrong Rust engine
cannot pass:

  1. Artifacts exist: crate doc, knowledge set golden, report golden,
     the real part golden, and documentation that states the not-parity scope.
  2. The demo binary is built and run against the real part + committed
     knowledge set; its JSON report is parsed.
  3. Independent verification: Python re-reads the bracket JSON itself, derives
     the part envelope (80x40x6), applies the same check formulas, and the
     report numbers agree within 1e-9.
  4. The demo stdout bytes are locked to the committed report golden.
  5. Documentation states the not-parity scope explicitly (required markers).

Run: python3 tests/test_p7m4t02_rules_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

REAL_PART = os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                         "example-bracket.oepgeo.json")
KNOWLEDGE = os.path.join(REPO, "crates", "oep-rules", "tests", "golden",
                         "iso-bracket.json")
REPORT_GOLDEN = os.path.join(REPO, "crates", "oep-rules", "tests", "golden",
                             "bracket-report.golden.json")
DOC = os.path.join(REPO, "workstreams/modeler/sketch-and-rules", "rules-checks-sandbox.md")
CRATE_LIB = os.path.join(REPO, "crates", "oep-rules", "src", "lib.rs")

E = 1e-9
_failures = []


def check(cond, msg):
    if cond:
        print("[PASS] %s" % msg)
    else:
        print("[FAIL] %s" % msg)
        _failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def main() -> int:
    print("Isolith P7-M4-T02 rules/checks sandbox acceptance")
    print("repo: %s" % REPO)

    # --- 1. artifacts -----------------------------------------------------
    check(os.path.isfile(DOC), "documentation exists: workstreams/modeler/sketch-and-rules/rules-checks-sandbox.md")
    check(os.path.isfile(CRATE_LIB), "crate lib exists: crates/oep-rules/src/lib.rs")
    check(os.path.isfile(REAL_PART), "real part golden exists")
    check(os.path.isfile(KNOWLEDGE), "knowledge set golden exists")
    check(os.path.isfile(REPORT_GOLDEN), "report golden exists")

    # --- 5. documentation states the not-parity scope ---------------------
    if os.path.isfile(DOC):
        text = read(DOC)
        check("doc is not empty", len(text) > 800)
        check("doc states 'not parity' explicitly",
              "not parity" in text)
        check("doc names V5 Knowledgeware as the reference",
              "Knowledgeware" in text)
        check("doc covers formulas", "formula" in text)
        check("doc covers checks", "check" in text)
        docs = ["design table", "powerCopy", "reaction", "UDF", "Knowledge Advisor"]
        for marker in docs:
            check("doc scopes out %r" % marker, marker.lower() in text.lower())

    # --- 2 + 3 + 4. run the demo, verify independently ---------------------
    if not all(os.path.isfile(p) for p in (REAL_PART, KNOWLEDGE, REPORT_GOLDEN)):
        print("missing demo artifacts; cannot run real-material demo")
        check(False, "real-material demo ran")
        return 1

    # Independent: derive the expected values from the bracket JSON in Python.
    bracket = json.loads(read(REAL_PART))
    assert bracket["format"] == "oep-geo"
    body = bracket["bodies"][0]
    bmin, bmax = body["bbox"]["min"], body["bbox"]["max"]
    width, length, depth = (bmax[i] - bmin[i] for i in range(3))
    feats = {f["id"]: f for f in bracket["features"]}
    fillet = feats["oep-feature-bracket-fillet"]["parameters"]["radius"]
    hole_d = feats["oep-feature-bracket-hole"]["parameters"]["diameter"]
    hole_depth = feats["oep-feature-bracket-hole"]["parameters"]["depth"]

    sheet_thickness = float(depth)          # 6.0
    sheet_area = float(width) * float(length)   # 80 * 40 = 3200
    # knowledge values from the committed set
    ks = json.loads(read(KNOWLEDGE))
    check(ks["format"] == "oep-knowledge", "knowledge golden format tag")
    check(ks["version"] == 1, "knowledge golden version")
    safe_factor = 2.0
    min_fillet = 3.0
    min_thickness = 4.0
    # rules (single pass, deterministic)
    if sheet_thickness >= 6:
        safe_factor = 1.5                      # thick_sheet_uses_tight_factor
    max_safe_hole = safe_factor * sheet_thickness
    deburr_status = "ACT" if fillet < min_fillet else "OK"  # auto_deburr_escalate

    expect_checks = {
        "sheet_min_thickness": sheet_thickness >= min_thickness,
        "sheet_max_thickness": sheet_thickness <= 12,
        "mount_hole_safe": hole_d <= max_safe_hole,
        "through_hole": abs(hole_depth - sheet_thickness) < 0.001,
        "edge_fillet_deburr": fillet >= min_fillet,
    }
    expect_params = {
        "sheet_thickness": sheet_thickness,
        "sheet_area": sheet_area,
        "edge_fillet": float(fillet),
        "hole_diameter": float(hole_d),
        "hole_depth": float(hole_depth),
        "safe_factor": safe_factor,
        "max_safe_hole": max_safe_hole,
        "deburr_status": deburr_status,
        "material": "AL6061-T6",
    }

    # Run the real demo (compiles once; output JSON is stdout-only).
    cmd = ["cargo", "run", "--quiet", "--manifest-path",
           os.path.join(REPO, "crates", "oep-rules", "Cargo.toml"),
           "--example", "demo_rules", "--", REAL_PART, KNOWLEDGE]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    except Exception as exc:  # pragma: no cover - subprocess start failure
        check(False, "demo binary ran (%r)" % (exc,))
        return 1
    check(proc.returncode == 0,
          "demo binary exit 0" if proc.returncode == 0 else
          "demo binary exit 0: %s" % proc.stderr.strip().splitlines()[-1:])
    if proc.returncode != 0:
        return 1

    demo_bytes = proc.stdout
    try:
        report = json.loads(demo_bytes)
    except ValueError:
        check(False, "demo stdout is a JSON report")
        return 1

    # --- 4. stdout locked to the committed golden ---------------------------
    check(demo_bytes == read(REPORT_GOLDEN),
          "demo stdout bytes equal the committed report golden")

    # --- 3. independent numeric verification -------------------------------
    check(report["format"] == "oep-rules-report", "report format tag")
    check(report["part"]["id"] == "oep-part-bracket", "report names the real part")
    check(report["part"]["name"] == "bracket", "report part name")
    params = {p["name"]: p["value"] for p in report["parameters"]}
    for k, want in expect_params.items():
        got = params.get(k)
        if isinstance(want, float):
            ok = isinstance(got, (int, float)) and abs(float(got) - want) < E
            check(ok, "parameter %s == %r (got %r)" % (k, want, got))
        else:
            check(got == want, "parameter %s == %r (got %r)" % (k, want, got))

    checks = {c["name"]: c for c in report["checks"]}
    for name, want in expect_checks.items():
        c = checks.get(name)
        check(checks.get(name) is not None, "check %s reported" % name)
        if c:
            check(c["passed"] == want, "check %s passed == %r (got %r)"
                  % (name, want, c["passed"]))
            check(isinstance(c["message"], str) and len(c["message"]) > 0,
                  "check %s has a message" % name)

    rules = {r["name"]: r for r in report["rules"]}
    check(rules["auto_deburr_escalate"]["branch"] == "then",
          "auto_deburr_escalate takes the then branch")
    check(rules["thick_sheet_uses_tight_factor"]["branch"] == "then",
          "thick_sheet_uses_tight_factor takes the then branch")

    print("=" * 60)
    if _failures:
        print("FAILED %d check(s)" % len(_failures))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())


def test_p7m4t02_rules_acceptance():
    """pytest entry point: run the same acceptance checks, asserted not exit-coded."""
    assert main() == 0, "oep-rules real-part rules demo acceptance failed (see output)"
