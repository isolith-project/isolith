#!/usr/bin/env python3
"""Acceptance test for Keelworks P1-M3-T01: audit of existing open-source
readers for the Keelworks target formats (CGR, 3DXML, CATPart/CATProduct,
STEP AP242, and lightweight Bob-the-Builder style viewer/convert tools).

TDD: this test documents the acceptance criteria BEFORE the audit report
exists, so it must FAIL (RED) against a repo with no P1-M3 deliverable, then
PASS (GREEN) once the audit lands.

'Done' for P1-M3-T01 means this test passes against the real audit report and
its cited source list, and every claim enforced here was checked against a
reachable primary source on 2026-10-15 (see workstreams/P1-M3/oss-readers-sources.md for
the citation list).

The task's specific audit targets (from the card) are each required here:
  GLC_Player, any CGR loaders, FreeCAD importers, Open CASCADE STEP, and
  Bob-the-Builder style tools.

Plus the program surface-language rule (no em dashes, U+2014), matching the
other P0/P1 deliverables.

Run:  python3 tests/test_oss_readers_audit_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPORT = os.path.join(ROOT, "workstreams/P1-M3", "oss-readers-audit.md")
SOURCES = os.path.join(ROOT, "workstreams/P1-M3", "oss-readers-sources.md")

EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

# The five audit targets named on the card, and the gap area that must be
# recorded for the Keelworks-critical native formats.
REQUIRED_READER_SECTIONS = {
    "GLC_Player": {
        "format_claims": ["3DXML", "COLLADA"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "CGR loaders": {
        "format_claims": ["CGR"],
        "must_record_native": ["CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "FreeCAD importers": {
        "format_claims": ["STEP", "FreeCAD"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "Open CASCADE STEP": {
        "format_claims": ["STEP", "AP242"],
        "must_record_native": ["AP242"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "Bob-the-Builder style tools": {
        "format_claims": ["viewer", "convert"],
        "must_record_native": ["CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
}

# The mandatory gap table (drives P1-M3-T03 first-parcel prioritization).
REQUIRED_GAP_ROWS = {
    "cgr": "native CGR reader absent in OSS",
    "3dxml": "binary .3drep branch read gap",
    "catpart": "no OSS native CATPart reader",
    "step-pmi": "AP242 PMI fidelity partial",
}

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_text(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def main():
    # --- 1. Deliverables exist -------------------------------------------------
    check(os.path.isfile(REPORT), "workstreams/P1-M3/oss-readers-audit.md exists")
    check(os.path.isfile(SOURCES), "workstreams/P1-M3/oss-readers-sources.md exists")
    if not os.path.isfile(REPORT):
        print("[FAIL] audit report missing; nothing further to verify")
        print("FAIL count: %d" % len(failures))
        return 1

    report = read_text(REPORT)
    lower = report.lower()

    # --- 2. Version header (program rule: living docs always versioned) --------
    check(bool(re.search(r"(?mi)^version:\s*\d+\.\d+", report)),
          "report has a version header (Version: x.y)")
    check("2026-10-15" in report, "report dated 2026-10-15")

    # --- 3. Every named reader target has a section with required detail -------
    for name, req in REQUIRED_READER_SECTIONS.items():
        section = "## " + name.split()[0] if " " in name else "## " + name
        # allowed heading forms: "## GLC_Player", "## CGR loaders: ...", etc.
        heading_ok = (
            re.search(r"(?m)^##\s+%s(?:[: \t]|$)" % re.escape(name), report)
            or re.search(r"(?m)^##\s+[0-9.]+\s+%s" % re.escape(name), report)
        )
        check(
            heading_ok,
            "audit has a section heading for %r" % name,
        )
        if not heading_ok:
            continue
        # isolate the section body (up to next ##)
        m = re.search(r"(?m)^##\s+.*%s.*$" % re.escape(name.split()[0]),
                      report)
        if m is None:
            continue
        start = m.start()
        nxt = re.search(r"\n##\s+", report[start + 1:])
        body = report[start:] if nxt is None else report[start: start + 1 + nxt.start()]
        for fmt in req["format_claims"]:
            check(fmt.lower() in body.lower(),
                  "section %r mentions %r" % (name, fmt))
        for native in req["must_record_native"]:
            check(native.lower() in body.lower(),
                  "section %r records native-format status of %r" % (name, native))
        if req["must_mention_license"]:
            check(re.search(r"(?i)license|licen[sc]e|gpl|lgpl|mit|apache|bsd",
                            body),
                  "section %r names a license or licensing posture" % name)
        if req["must_mention_status"]:
            check(re.search(r"(?i)maintained|status|stale|active|abandoned|last release|inactive|beta|alpha|unmaintained",
                            body),
                  "section %r records maintenance status" % name)
        # Program language rule inside every section.
        check(EM_DASH not in body,
              "section %r prose contains no em dash (U+2014)" % name)

    # --- 4. Gap table present with the mandatory gap rows ----------------------
    gap_heading = re.search(r"(?m)^[#]+\s+.*[Gg]ap", report) is not None
    check(gap_heading, "report has a Gaps section heading")
    for key, phrase in REQUIRED_GAP_ROWS.items():
        check(phrase.lower() in lower,
              "gap table records %r (%r)" % (key, phrase))

    # --- 5. Sources file sanity -------------------------------------------------
    if os.path.isfile(SOURCES):
        src = read_text(SOURCES)
        urls = re.findall(r"(https?://[^\s)>]+)", src)
        check(len(urls) >= 8, "sources file lists at least 8 URLs (got %d)" % len(urls))
        check("http" in src.lower(), "sources file contains http(s) citations")
        check(EM_DASH not in src, "sources file prose contains no em dash (U+2014)")
    else:
        check(False, "sources file missing")

    # --- 6. Whole-report language rule ------------------------------------------
    check(EM_DASH not in report, "entire report prose contains no em dash (U+2014)")

    # --- 7. Real-material verification marker ------------------------------------
    # The report must explicitly say which of its claims were checked against a
    # live artifact/source on 2026-10-15 (the "verify on real material" rule).
    check(
        re.search(r"(?i)verified|checked|primary source|live|reachable",
                  report),
        "report carries a real-material verification statement",
    )

    # --- summary ---------------------------------------------------------------
    for f in failures:
        print("[FAIL] " + f)
    print("checks: %d installed, %d failed" % (
        # approx: heuristic count of checks
        0 + len(REQUIRED_READER_SECTIONS) * 6 + len(REQUIRED_GAP_ROWS) + 8,
        len(failures),
    ))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
