#!/usr/bin/env python3
"""Acceptance test for Keelworks P5-M1-T03: alpha target ADR for the native
CATPart/CATProduct reader - B-Rep/geometry first, feature-tree semantics
deferred (documented).

Written FIRST (TDD). The ADR at workstreams/P5-M1/alpha-target-brep-first-adr.md must make
this test pass. It verifies the real decision record, not mocks:

  decision    => the card question is answered: the native reader alpha
                 targets B-Rep / exact geometry first, feature-tree (history /
                 parametric) semantics are deferred, with the default per the
                 master plan ratified
  evidence    => the decision is grounded in the observed CATPart container
                 facts (P1-M2-T03 hex notebook), the shape-record hypothesis
                 (P1-M2-T05), the independent commercial-reader capability
                 audit (P1-M3-T02), and the patent/clean-room posture
                 (P0-M2-T02), with confidence marked where structural meaning
                 is [uncertain]
  options     => at least B-Rep-first and feature-tree-first are weighed, and
                 the losing option is deferred with a reason
  sequencing  => the ADR names the P5-M3 B-Rep extraction lane and the P5-M4
                 feature-tree recoverability research lane that this decision
                 splits apart
  guardrails  => clean-room invariance, no copied vendor source, honest
                 marking, and the no-em-dash/no-curly-quote surface rule
  applies_to  => the P5 native reader lane and its feed into oep-geo via the
                 P6-M4 importer hub

Run:  python3 tests/test_p5m1t03_alpha_target_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/P5-M1", "alpha-target-brep-first-adr.md")

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
    """Text under a '## [N.] Heading', before the next heading of equal or
    higher level. Tolerates the numbered-heading style used by prior records."""
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def run_checks():
    print("P5-M1-T03 native reader alpha target acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: workstreams/P5-M1/alpha-target-brep-first-adr.md")
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

    # 3. Scoped to the native CATPart/CATProduct reader, answers alpha target.
    low = text.lower()
    check("catpart" in low, "must be scoped to the native CATPart reader")
    check("brep" in low or "b-rep" in low or "geometry" in low,
          "must name B-Rep / exact geometry as the alpha target")
    check("feature tree" in low or "feature-tree" in low
          or "parametri" in low or "history" in low,
          "must name feature-tree / parametric / history semantics")
    check("defer" in low, "must defer the feature-tree semantics explicitly")
    check("alpha" in low, "must frame the decision as the alpha target")

    # 4. Default per the master plan is ratified, not invented.
    check("P5" in text, "must place the decision in the P5 native-reader lane")
    check("default" in low, "must address the master-plan default")

    # 5. Evidence base: the observed CATPart container facts and hypothesis.
    ev = section_text(body, "Evidence")
    check(bool(ev), "must have an 'Evidence' section")
    if ev:
        el = ev.lower()
        check("p1-m2-t03" in el or "container notebook" in el,
              "evidence must cite the CATPart/CATProduct hex notebook (P1-M2-T03)")
        check("shape" in el or "rep" in el,
              "evidence must address the observed shape representation")
        check("uncertain" in el or "observed" in el,
              "evidence must mark uncertainty / observation basis explicitly")
        check("p1-m3" in el or "commercial" in el or "readers" in el
              or "audit" in el,
              "evidence must cite the commercial-reader capability audit (P1-M3-T02)")
        check("p0-m2" in el or "patent" in el,
              "evidence must cite the patent/clean-room posture memo (P0-M2-T02)")

    # 6. Options considered: at least B-Rep-first and feature-tree-first are
    #    weighed, and the losing option is deferred with a reason.
    opt = section_text(body, "Options considered")
    check(bool(opt), "must have an 'Options considered' section")
    if opt:
        oml = opt.lower()
        check("b-rep" in oml or "brep" in oml or "geometry" in oml,
              "options must include B-Rep/geometry first")
        check("feature tree" in oml or "feature-tree" in oml
              or "history" in oml or "parametric" in oml,
              "options must include feature-tree / history first")
        check("defer" in oml or "reject" in oml or "later" in oml,
              "the losing option must be deferred or rejected with a reason")

    # 7. Sequencing: the decision splits P5-M3 (B-Rep extraction) from P5-M4
    #    (feature-tree recoverability research).
    seq = section_text(body, "Sequencing")
    check(bool(seq), "must have a 'Sequencing' section")
    if seq:
        sl = seq.lower()
        check("p5-m3" in sl, "sequencing must name the P5-M3 B-Rep extraction lane")
        check("p5-m4" in sl, "sequencing must name the P5-M4 feature-tree lane")

    # 8. Clean-room section: vendor binaries / copied source / honest marking.
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if cr:
        lcr = cr.lower()
        check("vendor" in lcr or "binary" in lcr or "copy" in lcr
              or "clean-room" in lcr or "invariant" in lcr,
              "clean-room section must address vendor binaries / copied source")
        check("no " in lcr or "never" in lcr or "not" in lcr,
              "clean-room section must state a negative (what is not done)")

    # 9. Guardrails.
    grd = section_text(body, "Guardrails")
    check(bool(grd), "must have a 'Guardrails' section")
    if grd:
        gl = grd.lower()
        check("test" in gl or "acceptance" in gl or "golden" in gl,
              "guardrails must require a test gate")
        check("honest" in gl or "mark" in gl or "not" in gl,
              "guardrails must require honest capability marking")

    # 10. Cross-references.
    check("P5-M1-T03" in text, "must cite its own card P5-M1-T03")
    check("P6" in text, "must tie to the oep-geo / importer hub feed (P6)")

    # 11. Surface language rule: no em dashes, no curly quotes.
    ndashes = text.count(EM_DASH)
    check(ndashes == 0,
          "document contains %d em dash(es); ban applies to P5 prose too" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0,
          "document contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("ALPHA TARGET ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("ALPHA TARGET ADR ACCEPTANCE PASSED: "
          "B-Rep/geometry first for the native CATPart reader alpha, "
          "feature-tree semantics deferred to P5-M4 research, documented "
          "with observation-based evidence and guardrails.")
    return 0


def test_p5m1t03_alpha_target_adr():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "alpha target ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
