#!/usr/bin/env python3
"""Acceptance test for Keelworks P1-M3-T02: audit of commercial reader
feature lists (CAD Exchanger SDK, Spatial HOOPS, TransMagic, Okino), used
strictly as a functional spec source (what entity classes the industry's
readers can handle), never as a source of implementation techniques.

TDD: this test documents the acceptance criteria BEFORE the audit report
exists, so it must FAIL (RED) against a repo with no P1-M3 commercial
deliverable, then PASS (GREEN) once the audit lands.

'Done' for P1-M3-T02 means this test passes against the real audit report
and its cited source list, and every requirement enforced here was checked
against a reachable primary source on 2026-10-15 (see
workstreams/P1-M3/commercial-readers-sources.md for the citation list).

Card scope (P1-M3-T02): audit the reader feature lists of
  CAD Exchanger SDK, Spatial HOOPS, TransMagic, Okino
purely as a functional spec source: what formats and entity classes the
industry can read, so that P1-M3-T03 can turn that into prioritized import
targets and a first parcel. Clean-room invariant: we record WHAT the industry
reads, not HOW (no implementation techniques from vendors).

Run:  python3 tests/test_commercial_readers_audit_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPORT = os.path.join(ROOT, "workstreams/P1-M3", "commercial-readers-audit.md")
SOURCES = os.path.join(ROOT, "workstreams/P1-M3", "commercial-readers-sources.md")

EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

# The four commercial reader products named on the card, and the minimum
# that each section must contain. format_claims is a subset: the section must
# mention these formats. must_record_native is the critical functional-spec
# axis: does this vendor claim to read the Keelworks native targets?
REQUIRED_READER_SECTIONS = {
    "CAD Exchanger SDK": {
        "format_claims": ["CATPart", "CATProduct", "CGR"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "Spatial HOOPS": {
        "format_claims": ["HOOPS", "Exchange"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "TransMagic": {
        "format_claims": ["CATPart", "CATProduct"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
    "Okino": {
        "format_claims": ["PolyTrans", "CAD"],
        "must_record_native": ["CATPart", "CATProduct", "CGR"],
        "must_mention_license": True,
        "must_mention_status": True,
    },
}

# The entity-class axis the card is really after ("what entities the industry
# can read"). The report must carry a capability matrix that spans these
# entity classes and states, per vendor, whether the class is claimed.
REQUIRED_ENTITY_CLASSES = [
    "B-Rep",        # boundary representation (exact solids/surfaces)
    "NURBS",        # NURBS surfaces/curves
    "tessellat",    # tessellated / faceted / mesh geometry
    "PMI",          # product and manufacturing information (GD&T, datums)
    "assembly",     # product structure / assembly instances
]

# The first-parcel bridge: the audit must tie what commercial readers claim
# to the Keelworks workstreams that will have to match it, so P1-M3-T03 can
# set priorities. These substrings must appear in the synthesis section.
REQUIRED_SYNTHESIS_MENTIONS = [
    "P2",   # oep-cgr
    "P3",   # oep-3dxml
    "P4",   # oep-step
    "first parcel",
]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_text(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def main():
    # --- 1. Deliverables exist -------------------------------------------------
    check(os.path.isfile(REPORT), "workstreams/P1-M3/commercial-readers-audit.md exists")
    check(os.path.isfile(SOURCES), "workstreams/P1-M3/commercial-readers-sources.md exists")
    if not os.path.isfile(REPORT):
        print("[FAIL] audit report missing; nothing further to verify")
        print("checks installed: %d, failed: %d" % (0, len(failures)))
        return 1

    report = read_text(REPORT)
    lower = report.lower()

    # --- 2. Version header (program rule: living docs always versioned) --------
    check(bool(re.search(r"(?mi)^version:\s*\d+\.\d+", report)),
          "report has a version header (Version: x.y)")
    check("2026-10-15" in report, "report dated 2026-10-15")

    # --- 3. Every named reader has a section with the required detail ----------
    for name, req in REQUIRED_READER_SECTIONS.items():
        # allowed heading forms: "## CAD Exchanger SDK", "## 1 CAD Exchanger SDK",
        # "## Spatial HOOPS: …", "### …" etc.
        heading_ok = (
            re.search(r"(?m)^#{2,3}\s+%s(?:\s*[:.\-–(]|$)" % re.escape(name), report)
            or re.search(r"(?m)^#{2,3}\s+[0-9.]+\s+%s" % re.escape(name), report)
        )
        check(heading_ok, "audit has a section heading for %r" % name)
        if not heading_ok:
            continue
        m = re.search(r"(?m)^#{2,3}\s*.*%s.*$" % re.escape(name.split()[0]), report)
        if m is None:
            continue
        start = m.start()
        nxt = re.search(r"\n#{2,3}\s+", report[start + 1:])
        body = report[start:] if nxt is None else report[start: start + 1 + nxt.start()]
        for fmt in req["format_claims"]:
            check(fmt.lower() in body.lower(),
                  "section %r mentions %r" % (name, fmt))
        for native in req["must_record_native"]:
            check(native.lower() in body.lower(),
                  "section %r records native-status of %r" % (name, native))
        if req["must_mention_license"]:
            check(re.search(r"(?i)license|licen[sc]e|commercial|proprietary|freemium|trial",
                            body),
                  "section %r names a licensing/commercial posture" % name)
        if req["must_mention_status"]:
            check(re.search(r"(?i)20\d\d|version|release|current|latest|maint|update",
                            body),
                  "section %r records a version/status datum" % name)
        # Program language rule inside every section.
        check(EM_DASH not in body,
              "section %r prose contains no em dash (U+2014)" % name)
        # Clean-room posture must be restated inside each vendor section.
        check(re.search(r"(?i)functional spec|entity class|what .*read|no implementation",
                        body),
              "section %r frames itself as functional-spec source (what it reads, "
              "not how)" % name)

    # --- 4. Entity-class capability matrix -------------------------------------
    check(re.search(r"(?i)matrix|capability|coverage", report),
          "report has a capability matrix/coverage table")
    for ent in REQUIRED_ENTITY_CLASSES:
        check(ent.lower() in lower,
              "entity-class %r appears in the capability discussion" % ent)

    # --- 5. Synthesis / first-parcel bridge ------------------------------------
    check(re.search(r"(?mi)^#+\s+.*synthesis|^#+\s+.*implication|^#+\s+.*first parcel",
                    report),
          "report has a synthesis or implications section")
    for mention in REQUIRED_SYNTHESIS_MENTIONS:
        check(mention.lower() in lower,
              "synthesis mentions %r (workstream bridge)" % mention)

    # --- 6. Sources file sanity -------------------------------------------------
    if os.path.isfile(SOURCES):
        src = read_text(SOURCES)
        urls = re.findall(r"(https?://[^\s)>]+)", src)
        check(len(urls) >= 12,
              "sources file lists at least 12 URLs (got %d)" % len(urls))
        check("cad" in src.lower() and "spatial" in src.lower(),
              "sources cover CAD Exchanger and Spatial")
        check("transmagic" in src.lower() and "okino" in src.lower(),
              "sources cover TransMagic and Okino")
        check(EM_DASH not in src, "sources file prose contains no em dash (U+2014)")
    else:
        check(False, "sources file missing")

    # --- 7. Whole-report language rule ------------------------------------------
    check(EM_DASH not in report, "entire report prose contains no em dash (U+2014)")

    # --- 8. Real-material verification marker ------------------------------------
    # The report must say which of its claims were checked against a live,
    # reachable primary source on 2026-10-15 (the "verify on real material" rule).
    check(re.search(r"(?i)verified|checked|primary source|reachable|hopps|spatial\.com",
                    report),
          "report carries a real-material verification statement")

    # --- summary -----------------------------------------------------------------
    for f in failures:
        print("[FAIL] " + f)
    n_checks = (
        len(REQUIRED_READER_SECTIONS) * (2 + 2 + 3)
        + len(REQUIRED_ENTITY_CLASSES)
        + len(REQUIRED_SYNTHESIS_MENTIONS)
        + 8
    )
    print("checks installed: %d, failed: %d" % (n_checks, len(failures)))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
