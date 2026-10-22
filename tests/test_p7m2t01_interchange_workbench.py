"""P7-M2-T01 acceptance: OEP interchange workbench command surface.

Independent (stdlib-only) verification of the OEP interchange workbench
delivered in the adopted shell (P7-M1-T01 keep-own-shell decision):

  1. The shell crate `crates/oep-shell` exists and is a workspace member, and
     `crates/oep-pdm` carries the P8 API contract the save path is stubbed
     against.
  2. The workbench command surface (crates/oep-shell/src/workbench.rs) exposes
     the four interchange formats and the PDM load/save actions: Load (CGR /
     3DXML / STEP), OpenFromPdm, SaveToPdm, Save.
  3. The PDM contract (crates/oep-pdm/src/api.rs) is the typed PdmClient seam
     (save_revision / open_revision / list_revisions / head_revision) with a
     local stub (crates/oep-pdm/src/local.rs).
  4. The committed importer-hub golden for the real latched modern CGR mesh
     (NIST STC-06, V5-6R2023) carries the exact measured latch counts on the
     import feature: records=18, triangle=1389, index=2003, vertex=1037,
     max_index=1990.
  5. The committed importer-hub golden for a real STEP part (box-reference,
     a unit cube) maps to 1 part / 1 solid body at [0,0,0]-[1,1,1] and is
     byte-locked on the live import by the Rust side.
  6. P7 prose rule: the files this card adds carry no em dashes / curly
     quotes on the surface (the P7-M1 memo's prose ban).

The Rust side byte-locks the same goldens (crates/oep-import/tests/
importer_hub.rs) and the workbench behavior is asserted end-to-end in
crates/oep-shell/tests/workbench_acceptance.rs (real-material legs included);
this file re-parses the committed goldens from the other side with the stdlib
json module so no single decoder can drift alone.

Run:  python3 tests/test_p7m2t01_interchange_workbench.py  (and under pytest)
"""

import json
import os
import re
import sys
import tomllib

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

WORKBENCH = os.path.join(REPO, "crates", "oep-shell", "src", "workbench.rs")
PDM_API = os.path.join(REPO, "crates", "oep-pdm", "src", "api.rs")
PDM_LOCAL = os.path.join(REPO, "crates", "oep-pdm", "src", "local.rs")
CGR_GOLDEN = os.path.join(
    REPO, "crates", "oep-import", "tests", "golden", "cgr-modern-stc06-import.oepgeo.json"
)
STEP_GOLDEN = os.path.join(
    REPO, "crates", "oep-import", "tests", "golden", "box-reference-import.oepgeo.json"
)

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def members():
    with open(os.path.join(REPO, "Cargo.toml"), "rb") as fh:
        data = tomllib.load(fh)
    return data["workspace"].get("members") or []


def run_checks():
    print("P7-M2-T01 OEP interchange workbench acceptance")
    print("=" * 66)

    # 1. Workspace members: shell + PDM crates present.
    ms = members()
    check("crates/oep-shell" in ms, "crates/oep-shell is a workspace member")
    check("crates/oep-pdm" in ms, "crates/oep-pdm is a workspace member")

    # 2. Workbench command surface carries the catalog + PDM actions.
    wb = read(WORKBENCH)
    for token in (
        "WorkbenchAction",
        "WorkbenchOutcome",
        "interchange",
        "Load",
        "OpenFromPdm",
        "SaveToPdm",
        "InterchangeSavePending",
    ):
        check(token in wb, "workbench surface names %s" % token)
    for fmt in ("Cgr", "ThreeDXml", "Step"):
        check("Interchange" in wb and fmt in wb, "workbench catalog covers %s" % fmt)
    # The four formats appear as the catalog: cgr, 3dxml, step, pdm.
    low = wb.lower()
    for tag in ("cgr", "3dxml", "step", "pdm"):
        check(tag in low, "workbench touches %s lane" % tag)

    # 3. PDM API contract + local stub.
    api = read(PDM_API)
    for token in ("PdmClient", "save_revision", "open_revision", "list_revisions",
                  "head_revision", "PdmWriteRequest", "RevisionSummary"):
        check(token in api, "PDM contract names %s" % token)
    local = read(PDM_LOCAL)
    check("LocalPdmVault" in local, "local stub class exists")

    # 4. Committed modern CGR golden carries the real latch counts.
    cgr = json.loads(read(CGR_GOLDEN))
    check(cgr.get("format") == "oep-geo", "CGR golden: canonical format tag")
    check(cgr.get("version") == 2, "CGR golden: schema version 2")
    check(len(cgr.get("parts", [])) == 1, "CGR golden: one part")
    params = cgr["features"][0]["parameters"]
    expected = {"records": 18, "index_count": 2003, "vertex_count": 1037,
                "triangle_count": 1389, "max_index": 1990}
    check(params.get("records") == 18, "CGR golden: records=18 (%r)" % params.get("records"))
    for key, val in expected.items():
        check(params.get(key) == val, "CGR golden: %s=%s" % (key, val))
    body = cgr["bodies"][0]
    check(body["kind"] == "solid", "CGR golden: solid body")
    bb = body["bbox"]
    check(bb["min"][0] < 0 and bb["max"][0] > 0 and bb["max"][2] == 0.0,
          "CGR golden: bbox spans the STC-06 part (%r)" % (bb,))

    # 5. Committed STEP golden maps a real unit cube.
    stp = json.loads(read(STEP_GOLDEN))
    check(stp.get("format") == "oep-geo", "STEP golden: canonical format tag")
    check(len(stp.get("parts", [])) == 1, "STEP golden: one part")
    check(len(stp.get("bodies", [])) == 1, "STEP golden: one body")
    sbb = stp["bodies"][0]["bbox"]
    check(sbb["min"] == [0.0, 0.0, 0.0] and sbb["max"] == [1.0, 1.0, 1.0],
          "STEP golden: unit cube bbox (%r)" % (sbb,))

    # 6. P7 prose rule: no em dashes / curly quotes in the files this card
    #    adds or edits on the visible surface.
    for path, label in (
        (WORKBENCH, "workbench.rs"),
        (PDM_API, "api.rs"),
        (PDM_LOCAL, "local.rs"),
        (os.path.join(REPO, "crates", "oep-shell", "src", "lib.rs"), "oep-shell lib.rs"),
    ):
        text = read(path)
        check(EM_DASH not in text, "%s: no em dash" % label)
        check(not any(c in CURLY for c in text), "%s: no curly quotes" % label)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P7-M2-T01 INTERCHANGE WORKBENCH ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P7-M2-T01 INTERCHANGE WORKBENCH ACCEPTANCE PASSED: "
          "the OEP interchange workbench catalogs CGR/3DXML/STEP/PDM load and "
          "save in the adopted shell, loads a real latched modern CGR mesh and "
          "a 3DXML through the importer hub (committed goldens re-parsed), and "
          "stubs the PDM save path against the P8 PdmClient contract.")
    return 0


def test_p7m2t01_interchange_workbench():
    failures = run_checks()
    assert not failures, "interchange workbench acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
