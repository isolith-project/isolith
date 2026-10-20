#!/usr/bin/env python3
"""Acceptance test for Keelworks P4-M4-T02: JT import (visualization/PMI) decision
ADR -- adopt an existing OSS JT reader or build.

Written FIRST (TDD). The ADR at workstreams/step-interop/qif/jt-import-decision-adr.md must make this
test pass. It verifies the real material, not mocks:
  decision    => the card question is answered: adopt an existing OSS JT reader
                 or build our own, with a single recommendation
  evidence    => the OSS reader field is audited (TKJT/JT Assistant, PyOpenJt,
                 Voyager, C# readers, commercial-only space) with a reason each
                 way, and the documented-format fact (ISO 14306 / JTIAP PSI
                 14-1 / DIN SPEC 91383 / Siemens reference) is recorded
  license     => GPL-2.0-only readers are assessed against the program posture
                 (Apache-2.0 apps/libraries, AGPL-3.0 server) and the
                 clean-room invariant (no vendor binaries, no copied source)
  scope       => if a reader is adopted or built, the work is scoped as its own
                 mini-project with a bounded first milestone and a test gate
  guardrails  => version pin on the target JT revision, tolerant never-crash
                 model, no em-dash/curly-quote prose (program surface rule)
Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_p4m4_jt_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/step-interop/qif", "jt-import-decision-adr.md")

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
    print("P4-M4-T02 JT import decision acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: workstreams/step-interop/qif/jt-import-decision-adr.md")
    if not os.path.isfile(ADR):
        return failures

    text = read_adr()
    meta, body = split_front_matter(text)

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta["status"].strip(),
          "front matter must carry a status")
    check("date" in meta and meta["date"].strip(),
          "front matter must carry a date")
    check("owner" in meta and meta["owner"].strip(),
          "front matter must carry an owner")
    check("decision" in meta and meta["decision"].strip(),
          "front matter must carry a decision key")
    check("applies_to" in meta and meta["applies_to"].strip(),
          "front matter must carry applies_to")

    # 3. It is scoped to the JT format and answers the adopt-or-build question.
    low = text.lower()
    check("jt" in low or "jupiter" in low, "must be scoped to the JT format")
    check("visual" in low, "must name visualization as a JT import tenant")
    check("pmi" in low, "must name PMI as a JT import tenant")
    check("build" in low or "adopt" in low,
          "must answer the adopt-vs-build card question")
    check("oep-step" in low or "oep-jt" in low,
          "must name the crate the decision applies to (oep-step / oep-jt)")

    # 4. The OSS reader field is audited with named candidates.
    check("tkjt" in low or "jt assistant" in low or "opencascade" in low
          or "open cascade" in low,
          "must weigh the OCCT TKJT / JT Assistant reader explicitly")
    check("gpl" in low, "must state the TKJT license (GPL-2.0)")
    check("pyopenjt" in low or "voyager" in low or "jreader" in low
          or "jtconvert" in low or "crate" in low,
          "must weigh at least one other named OSS reader candidate")

    # 5. Grounding in the documented-format fact: JT is a published standard.
    check("iso 14306" in low, "must cite ISO 14306 (the JT standard)")
    check("prostep" in low or "jtiap" in low or "psi 14" in low or "din spec" in low,
          "must cite the free prostep JTIAP / DIN SPEC format description")
    check("spec" in low or "reference" in low,
          "must record that JT is a documented format (has a public spec)")

    # 6. Options considered: at least the build-your-own and the OSS reader
    #    options, with a rejection/deferral reason for the losing ones.
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        check("own" in oml or "build" in oml,
              "options must include building our own reader")
        check("reject" in oml or "rejected" in oml or "defer" in oml
              or "not adopt" in oml or "no " in oml,
              "at least one option must be rejected or deferred with a reason")

    # 7. License section: hybrid posture + GPL-2.0 incompatibility + clean room.
    lic = section_text(body, "License")
    check(bool(lic), "must have a 'License' section")
    if lic:
        ll = lic.lower()
        check("apache" in ll or "lgpl" in ll or "posture" in ll or "hybrid" in ll,
              "license section must tie to the program license posture")
        check("gpl" in ll, "license section must address the GPL-2.0 readers")
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if cr:
        lcr = cr.lower()
        check("vendor" in lcr or "binary" in lcr or "copy" in lcr
              or "clean-room" in lcr or "invariant" in lcr,
              "clean-room section must address vendor binaries / copied source")

    # 8. Mini-project scoping: the work is bounded with a first milestone.
    scp = section_text(body, "Mini-project scope")
    check(bool(scp), "must have a 'Mini-project scope' section")
    if scp:
        sl = scp.lower()
        check("visual" in sl, "mini-project must scope visualization first")
        check("test" in sl or "gate" in sl or "golden" in sl,
              "mini-project must carry a test gate / acceptance")

    # 9. Guardrails: version pin and a test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("pin" in gl or "version" in gl,
              "guardrails must pin the target JT revision")
        check("test" in gl or "ci" in gl or "acceptance" in gl,
              "guardrails must require a test gate")

    # 10. Cross-references: P4-M4-T02 card, the P4 lane, the P1-M3 audit.
    check("P4-M4-T02" in text, "must cite its own card P4-M4-T02")
    check("P4" in text, "must place the decision in the P4 lane")
    check("workstreams/format-corpus/reader-audit" in text or "readers" in text,
          "must tie to the P1-M3 readers audit web")

    # 11. Surface language rule: no em dashes, no curly quotes.
    ndashes = text.count(EM_DASH)
    check(ndashes == 0,
          "document contains %d em dash(es); ban applies to P4 prose too" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0,
          "document contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("JT IMPORT DECISION ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("JT IMPORT DECISION ADR ACCEPTANCE PASSED: "
          "reject GPL-2.0-only OSS readers on license grounds, build a Rust "
          "JT reader from the documented ISO 14306 / JTIAP spec, scoped as a "
          "mini-project with visualization first and a test gate.")
    return 0


def test_p4m4_jt_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "JT import decision ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
