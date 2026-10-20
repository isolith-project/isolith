#!/usr/bin/env python3
"""P4-M2-T01 acceptance: STEP AP242 semantic PMI import into oep-pmi.

Independent (stdlib-only) verification of the P4-M2 semantic-PMI wedge:

  - the crate exists and is a workspace member (layout sync),
  - real NIST MBE AP242 fixtures are committed under crates/oep-step-pmi/fixtures/,
  - the P4-M2 ADR (workstreams/step-interop/pmi/pmi-import-adr.md) records the gate decision:
    wrap the OSS STEP library (step-io) behind vendor-neutral wrappers for the
    semantic PMI lane, OCCT stays the analytic-B-Rep fidelity path,
    honest absence flags, no fabricated PMI,
  - surface-language rule: no em dashes, no curly quotes in the new prose,
  - no step-io identifier leaks onto the crate's public surface (the only file
    allowed to name the OSS library is the private walk src/import.rs).

The live mapping (real fixtures -> oep_pmi document) is enforced by the Rust
integration tests in tests/pmi_import.rs on the same committed files; this
script independently verifies the fixture facts this card ships.

Run: python3 tests/test_p4m2_step_pmi_import.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(ROOT, "crates", "oep-step-pmi", "fixtures")
CRATE = os.path.join(ROOT, "crates", "oep-step-pmi")
ADR = os.path.join(ROOT, "workstreams/step-interop/pmi", "pmi-import-adr.md")
EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

# Real NIST MBE PMI Validation and Conformance Testing fixtures.
PMI_BEARING = [
    "nist_ftc_06_asme1_ap242-e2.stp",   # 28 GD&T tolerances, 10 datums (AP242e2)
    "nist_stc_06_asme1_ap242-e3.stp",   # 25 tolerances, 6 datums (AP242e3)
]
NO_PMI = [
    "nist_ftc_08_asme1_ap242-e4-tg.stp",  # tessellated geometry only, no PMI
]


def run_checks():
    failures = []

    def check(cond, msg):
        if not cond:
            failures.append(msg)

    print("P4-M2-T01 semantic PMI import acceptance")
    print("=" * 60)

    # 1. Crate exists, is a workspace member, fixtures committed.
    check(os.path.isdir(CRATE), "missing crate: crates/oep-step-pmi/")
    check(os.path.isfile(os.path.join(CRATE, "src", "import.rs")),
          "missing private walk: crates/oep-step-pmi/src/import.rs")
    with open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8") as fh:
        members = re.findall(r'"crates/[^"]+"', fh.read())
    check('"crates/oep-step-pmi"' in members, "oep-step-pmi not a workspace member")
    with open(os.path.join(ROOT, "docs", "layout.md"), encoding="utf-8") as fh:
        layout = fh.read()
    check("oep-step-pmi" in layout, "docs/layout.md missing oep-step-pmi line")

    for f in PMI_BEARING + NO_PMI:
        path = os.path.join(FIXTURES, f)
        check(os.path.isfile(path), "fixture missing: %s" % f)
        if os.path.isfile(path):
            with open(path, "rb") as fh:
                head = fh.readline()
            check(head.startswith(b"ISO-10303-21"), "%s is a real STEP part-21 file" % f)

    # 2. PMI-bearing fixtures actually carry tolerance/datum entities; the
    #    no-PMI fixture does not. (Facts observed in the committed real files.)
    def has_pmi_record(fname, needle):
        path = os.path.join(FIXTURES, fname)
        with open(path, "r", errors="replace") as fh:
            return any(needle in line for line in fh)

    check(has_pmi_record("nist_ftc_06_asme1_ap242-e2.stp", "GEOMETRIC_TOLERANCE"),
          "ftc_06 (e2) must contain GEOMETRIC_TOLERANCE records")
    check(has_pmi_record("nist_ftc_06_asme1_ap242-e2.stp", "DATUM_SYSTEM"),
          "ftc_06 (e2) must contain DATUM_SYSTEM records")
    check(has_pmi_record("nist_stc_06_asme1_ap242-e3.stp", "GEOMETRIC_TOLERANCE"),
          "stc_06 (e3) must contain GEOMETRIC_TOLERANCE records")
    with open(os.path.join(FIXTURES, "nist_ftc_08_asme1_ap242-e4-tg.stp"),
              "r", errors="replace") as fh:
        body = fh.read()
    check("GEOMETRIC_TOLERANCE" not in body, "ftc_08 must carry no geometric tolerances")
    check("DATUM" not in body and "DIMENSIONAL_SIZE" not in body,
          "ftc_08 must carry no datum/dimension PMI records")

    # 3. ADR exists, versioned, documents the gate decision.
    check(os.path.isfile(ADR), "missing workstreams/step-interop/pmi/pmi-import-adr.md")
    adr = ""
    if os.path.isfile(ADR):
        with open(ADR, encoding="utf-8") as fh:
            adr = fh.read()
    low = adr.lower()
    check("version" in low and "date" in low and "decision" in low and "status" in low,
          "ADR front matter must carry version/date/decision/status")
    check("step-io" in low, "ADR must name the wrapped OSS STEP library (step-io)")
    check("vendor-neutral" in low or "seam" in low, "ADR must commit to the vendor-neutral seam")
    check("occt" in low or "open cascade" in low, "ADR must record OCCT (analytic fidelity path)")
    check(("pmi_present" in low) or ("absent" in low) or
          ("no pmi" in low) or ("honest" in low),
          "ADR must document the honest absence/flagging contract")
    check("p4-m2" in low, "ADR must cite its own card P4-M2")

    # 4. Surface-language rule: no em dashes, no curly quotes in new prose.
    for f in [ADR, os.path.join(CRATE, "src", "lib.rs"),
              os.path.join(CRATE, "src", "import.rs"),
              os.path.join(CRATE, "src", "error.rs"),
              os.path.join(CRATE, "tests", "pmi_import.rs")]:
        if not os.path.isfile(f):
            continue
        with open(f, encoding="utf-8") as fh:
            text = fh.read()
        check(EM_DASH not in text, "em dash banned in %s" % os.path.basename(f))
        check(not any(c in text for c in CURLY), "curly quotes banned in %s" % os.path.basename(f))

    # 5. Vendor-neutral seam: the OSS library may only be named in the private
    #    walk (src/import.rs); it must not leak into the public surface files.
    for name in ["lib.rs", "error.rs"]:
        path = os.path.join(CRATE, "src", name)
        with open(path, encoding="utf-8") as fh:
            src = fh.read()
        check("step_io" not in src and "step-io" not in src,
              "OSS library identifiers must stay off %s (seam rule)" % name)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P4-M2-T01 ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P4-M2-T01 PASSED: semantic PMI import mapped onto oep-pmi, "
          "honest absence flag, real NIST fixtures, vendor-neutral seam.")
    return 0


def test_p4m2_step_pmi_import_acceptance():
    failures = run_checks()
    assert not failures, "P4-M2-T01 acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
