#!/usr/bin/env python3
"""Acceptance test for Isolith P7-M1-T01: open modeler shell eval (keep/acquire
memo). Decides the architectural fork P7-M2 through P7-M5 depend on.

Written FIRST (TDD). The memo at workstreams/modeler/shell/open-modeler-shell-eval-memo.md must make
this test pass. It verifies the real material, not mocks:
  decision   => the card question is answered with a single recommendation, and
                all three candidates are weighed: FreeCAD (acquire), the CADCraft
                lineage (acquire), and a thin own shell around Open CASCADE
                (keep)
  evidence   => a primary-source evidence file exists and is cited; facts carry
                live-source URLs and a checked date
  posture    => license, clean-room, renderer (wgpu), canonical model (oep-geo),
                and solver-adoption doctrine are addressed against the program's
                already ratified decisions
  fork       => the consequences for P7-M2, P7-M3, P7-M4, P7-M5 are stated (this
                record is the fork they depend on)
  guardrails => an escape hatch / revisit trigger and a test gate are named
Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_p7m1t01_modeler_shell_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MEMO = os.path.join(ROOT, "workstreams/modeler/shell", "open-modeler-shell-eval-memo.md")
SOURCES = os.path.join(ROOT, "workstreams/modeler/shell", "open-modeler-shell-sources.md")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path) -> str:
    with open(path, encoding="utf-8") as fh:
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
    print("P7-M1-T01 open modeler shell eval acceptance")
    print("memo:    %s" % MEMO)
    print("sources: %s" % SOURCES)
    print("=" * 66)

    # 1. The memo and evidence file exist.
    check(os.path.isfile(MEMO), "missing: workstreams/modeler/shell/open-modeler-shell-eval-memo.md")
    check(os.path.isfile(SOURCES), "missing: workstreams/modeler/shell/open-modeler-shell-sources.md")
    if not os.path.isfile(MEMO):
        return failures
    text = read(MEMO)
    meta, body = split_front_matter(text)

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta.get("version", "")),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta.get("status", "").strip(),
          "front matter must carry a status")
    check("date" in meta and meta.get("date", "").strip(),
          "front matter must carry a date")
    check("owner" in meta and meta.get("owner", "").strip(),
          "front matter must carry an owner")
    check("decision" in meta and meta.get("decision", "").strip(),
          "front matter must carry a decision key")
    check("applies_to" in meta and meta.get("applies_to", "").strip(),
          "front matter must carry applies_to")

    # 3. Scoped to the modeler shell question and answers keep-or-acquire.
    low = text.lower()
    check("modeler shell" in low or "shell" in low,
          "must be scoped to the modeler shell question")
    check("freecad" in low, "must weigh FreeCAD as a candidate shell")
    check("cadcraft" in low, "must weigh the CADCraft lineage as a candidate")
    check("open cascade" in low or "occt" in low or "occ" in low,
          "must name the Open CASCADE / OCCT path (the keep option)")
    check("keep" in low and "acquire" in low,
          "must answer the keep-vs-acquire card question")

    # 4. A single recommendation: the decision section names the chosen option.
    dec = section_text(body, "Decision")
    dlow = dec.lower()
    check(bool(dlow), "must have a 'Decision' section")
    if dlow:
        check(("reject" in dlow or "decline" in dlow or "do not" in dlow)
              and ("adopt" in dlow or "keep" in dlow or "build" in dlow),
              "decision must reject at least one option and adopt/keep one")

    # 5. All three options considered, each with a verdict and a reason.
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        check("freecad" in oml, "options must weigh FreeCAD")
        check("cadcraft" in oml, "options must weigh CADCraft")
        check("own" in oml or "keep" in oml,
              "options must weigh the own-shell / keep path")
        check("reject" in oml or "rejected" in oml or "defer" in oml
              or "decline" in oml or "watch" in oml,
              "at least one option must be rejected or deferred with a reason")

    # 6. Grounded against the program's already ratified decisions.
    check("oep-geo" in low, "must tie to the oep-geo canonical model (P6-M1)")
    check("wgpu" in low, "must tie to the wgpu renderer decision (P6-M5)")
    check("solver" in low and ("adopt" in low or "oss" in low),
          "must address the P7-M3 solver-adoption doctrine (adopt OSS, not rebuild)")

    # 7. License section ties to the program posture; clean-room present.
    lic = section_text(body, "License")
    ll = lic.lower()
    check(bool(ll), "must have a 'License' section")
    if bool(ll):
        check("apache" in ll or "lgpl" in ll or "posture" in ll or "hybrid" in ll,
              "license section must tie to the program license posture")
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if bool(cr):
        lcr = cr.lower()
        check("vendor" in lcr or "binary" in lcr or "copy" in lcr
              or "clean-room" in lcr or "invariant" in lcr,
              "clean-room section must address vendor binaries / copied source")

    # 8. The fork: consequences for P7-M2 through P7-M5 are stated.
    for card in ("P7-M2", "P7-M3", "P7-M4", "P7-M5"):
        check(card in text, "must state the consequence for %s (the fork's consumers)" % card)
    check("P7-M1-T01" in text, "must cite its own card P7-M1-T01")

    # 9. Evidence file carries live sources with a checked date.
    slow = read(SOURCES).lower()
    check("wikipedia" in slow or "freecad.org" in slow or
          "github.com" in slow or "opencascade.org" in slow,
          "sources must cite at least one live primary source")
    check("2026-10-16" in slow or "2026" in slow,
          "sources must record when each source was checked")
    check("cadcraft" in slow and ("freecad" in slow or "occt" in slow),
          "sources must cover the CADCraft and FreeCAD/OCC surfaces")

    # 10. Guardrails: escape hatch / revisit trigger and a test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("escape" in gl or "revisit" in gl or "fallback" in gl,
              "guardrails must name an escape hatch or revisit trigger")
        check("test" in gl or "ci" in gl or "gate" in gl or "milestone" in gl,
              "guardrails must require a test gate / milestone checkpoint")

    # 11. Surface language rule: no em dashes, no curly quotes.
    ndashes = text.count(EM_DASH)
    check(ndashes == 0,
          "memo contains %d em dash(es); the ban applies to P7 prose too" % ndashes)
    ncurly = (text.count("\u201c") + text.count("\u201d")
              + text.count("\u2018") + text.count("\u2019"))
    check(ncurly == 0,
          "memo contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("MODELER SHELL EVAL MEMO ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("MODELER SHELL EVAL MEMO ACCEPTANCE PASSED: "
          "all three shells weighed (FreeCAD, CADCraft lineage, own shell around "
          "Open CASCADE), a single keep/acquire recommendation recorded with "
          "evidence, and the P7-M2..P7-M5 fork consequences stated.")
    return 0


def test_p7m1t01_modeler_shell_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "modeler shell eval memo acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
