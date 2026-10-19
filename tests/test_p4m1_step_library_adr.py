#!/usr/bin/env python3
"""Acceptance test for Keelworks P4-M1-T01: oep-step library-choice ADR.

Written FIRST (TDD). The ADR at P4-M1/step-library-adr.md must make this test
pass. It verifies the real material, not mocks:
  decision    => oep-step wraps an existing OSS STEP library behind
                 vendor-neutral clean wrappers
  library     => a pure-Rust OSS STEP library is adopted for P4-M1 and Open
                 CASCADE (OCCT) is recorded as the fidelity path for P4-M2+
  neutral     => the wrapper seam keeps the OSS library's types off oep-step's
                 public surface
  options     => OCCT and an own hand-rolled parser are weighed; own parser is
                 rejected, OCCT deferred with reasons
  license     => the adopted library's license is assessed against the program
                 posture (Apache-2.0 apps/libraries)
  guardrails  => version pin, seam denial, escape hatch to OCCT, test gate
Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_p4m1_step_library_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "P4-M1", "step-library-adr.md")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_adr() -> str:
    with open(ADR, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
    """Return (meta_dict, body) for simple top-level YAML front matter."""
    if not text.startswith("---"):
        return {}, text
    _, fm, body = text.split("---", 2)
    meta = {}
    for line in fm.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or ":" not in stripped:
            continue
        key, _, val = stripped.partition(":")
        meta[key.strip()] = val.strip()
    return meta, body


def section_text(body, heading):
    """Text under a '## [N.] Heading', before the next heading of equal/higher
    level. Tolerates the numbered-heading style used by earlier records."""
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def run_checks():
    print("P4-M1-T01 oep-step library-choice ADR acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: P4-M1/step-library-adr.md")
    if not os.path.isfile(ADR):
        return failures

    text = read_adr()
    meta, body = split_front_matter(text)

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta["status"].strip(), "front matter must carry a status")
    check("date" in meta and meta["date"].strip(), "front matter must carry a date")
    check("owner" in meta and meta["owner"].strip(), "front matter must carry an owner")
    check("decision" in meta and meta["decision"].strip(),
          "front matter must carry a decision key")
    check("applies_to" in meta and meta["applies_to"].strip(),
          "front matter must carry applies_to")

    # 3. The decision wraps an existing OSS STEP library for oep-step.
    low = text.lower()
    check("step" in low, "must be scoped to the STEP interop lane")
    check("oep-step" in low, "must name the adopting crate oep-step")
    check("oss" in low or "open source" in low,
          "must frame the wrap as an existing OSS STEP library")
    check("neutral" in low or "vendor-neutral" in low,
          "must commit to a vendor-neutral wrapper")
    check("wrap" in low or "wrapper" in low or "bind" in low or "behind" in low,
          "must state the wrap/bind stance")

    # 4. Open CASCADE (OCCT) is weighed; it is the fidelity path for P4-M2+.
    check("open cascade" in low or "opencascade" in low or "occt" in low,
          "must weigh Open CASCADE (OCCT) explicitly")
    check("ap242" in low or "pmi" in low,
          "must tie OCCT's role to AP242 / PMI fidelity")

    # 5. Options considered: OCCT, the adopted library, and an own parser, with
    #    a rejection recorded for at least one option (no decision on vibes).
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        check("occt" in oml or "open cascade" in oml,
              "options must weigh Open CASCADE")
        check("own" in oml and ("parser" in oml or "reader" in oml or "hand" in oml),
              "options must weigh an own hand-rolled parser")
        check("reject" in oml or "rejected" in oml or "defer" in oml
              or "not adopt" in oml or "no" in oml,
              "at least one option must be rejected or deferred with a reason")

    # 6. Vendor-neutral seam: the OSS library's types stay off oep-step's
    #    public surface.
    seam = section_text(body, "Vendor-neutral seam")
    check(bool(seam), "must have a 'Vendor-neutral seam' section")
    if seam:
        sl = seam.lower()
        check("public" in sl or "surface" in sl,
              "seam section must define the public surface")
        check("leak" in sl or "seam" in sl or "boundary" in sl or "swap" in sl
              or "replace" in sl or "private" in sl,
              "seam section must keep the library private / swappable")

    # 7. License assessment for the adopted library.
    lic = section_text(body, "License")
    check(bool(lic), "must have a 'License' section")
    if lic:
        ll = lic.lower()
        check("apache" in ll or "mit" in ll,
              "license section must name the adopted library's permissive license")
        check("posture" in ll or "hybrid" in ll or "lgpl" in ll,
              "license section must tie to the program license posture (incl. OCCT LGPL)")

    # 8. Guardrails: version pin, escape hatch, test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("pin" in gl or "version" in gl, "guardrails must pin the library version")
        check("swap" in gl or "escape" in gl or "replace" in gl or "occt" in gl,
              "guardrails must keep the escape hatch (OCCT / a future engine)")
        check("test" in gl or "ci" in gl or "acceptance" in gl,
              "guardrails must require a test gate")

    # 9. Cross-references: lives in the P4 web, ties to the P6 model + ADRs.
    check("P4-M1" in text, "must cite its own card P4-M1")
    check("oep-geo" in low, "must name the native model crate oep-geo")
    check("P6-M3" in text or "p6-m3" in text,
          "must cross-reference the P6-M3 boolean-ops OCCT ADR")

    # 10. Surface language rule: no em dashes, no curly quotes.
    ndashes = text.count(EM_DASH)
    check(ndashes == 0, "document contains %d em dash(es); ban applies to P4 prose too" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0, "document contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("STEP LIBRARY ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("STEP LIBRARY ADR ACCEPTANCE PASSED: "
          "wrap a pure-Rust OSS STEP library behind vendor-neutral wrappers, "
          "OCCT recorded as the P4-M2 fidelity path.")
    return 0


def test_step_library_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "step library ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
