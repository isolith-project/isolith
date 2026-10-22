#!/usr/bin/env python3
"""Acceptance test for Isolith SURFACING-CLASS-A: Class-A surfacing strategy.

Written as the gate for the strategy ADR at
workstreams/geometry-core/kernel/srf-class-a-adr.md (card SURFACING-CLASS-A,
decision #19 full-send). It verifies the decision on real material, not mocks:
  open paths   => Open CASCADE's Class-A limit is stated, NURBS
                  reconstruction lanes are named, and the permissive-kernel
                  reality (no BSD trimmed-NURBS B-Rep near OCCT) is recorded
  CGM moat     => Dassault CGM / ICEM / tolerant modeling / Spatial moat
                  elements from the vault CATIA research are captured
  math model   => continuity constraints, fairing, reconstruct-from-triangle-
                  mesh, and curvature diagnostics are named as OWNED, with the
                  reuse line (OCCT seam, own NURBS math) drawn
  approach     => two-track strategy: modeling-grade surfacing on the OCCT
                  seam now, an own permissive Class-A surface core as the
                  strategy-gated long program (P15), kernel requirements
  honest pin   => NO Class-A surface inside the near-term editor train
  honest cost  => honest-cost writeup in cost-class / effort-class terms with
                  no calendar promise
  register     => the decomposition is registered (P15) and references the
                  master plan
Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_srf_class_a_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/geometry-core/kernel", "srf-class-a-adr.md")

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
    """Text under a '## [N.] Heading' or '### N.N Heading' section, before the
    next heading of equal or higher level. Tolerates the numbered-heading style
    used by other records (##, ###, and ### 5.2 Own: style)."""
    m = re.search(r"\n#{2,4}\s+[^\n]*?%s" % re.escape(heading), body,
                  re.IGNORECASE)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n#{2,4} ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def run_checks():
    print("SURFACING-CLASS-A strategy decision acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR),
          "missing: workstreams/geometry-core/kernel/srf-class-a-adr.md")
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

    low = text.lower()

    # 3. Open paths: Open CASCADE is named AND its Class-A limit is stated.
    check("open cascade" in low or "opencascade" in low or "occt" in low,
          "must name Open CASCADE (OCCT)")
    check("class-a" in low, "must be scoped to Class-A")
    check("not a class-a kernel" in low or "not deliver" in low
          or "no variational fairer" in low,
          "must state that OCCT / vanilla OCC does not deliver Class-A")
    check("g1" in low and "g2" in low, "must name G1 and G2 continuity")
    check("g3" in low, "must name G3 continuity")
    check("nurbs" in low, "must frame the problem in NURBS terms")

    # 4. The mathematical dependency model sections are present and own the
    #    four modules.
    cont = section_text(body, "Continuity constraints") or \
        section_text(body, "continuity constraints")
    fair = section_text(body, "Fairing")
    rec = section_text(body, "reconstruct-from-triangle-mesh") or \
        section_text(body, "Reconstruct-from-triangle-mesh")
    diag = section_text(body, "curvature diagnostics") or \
        section_text(body, "Curvature diagnostics")
    m = section_text(body, "Mathematical dependency model") or \
        section_text(body, "5.1")
    check(bool(m) or bool(cont), "must have a mathematical dependency model")
    if cont or m:
        cm = cont.lower() or m.lower()
        check("watertight" in cm or "twist" in cm or "g2" in cm,
              "continuity model must name watertight seams / twist / G2")
    for s, name in ((fair, "fairing"), (rec, "reconstruction"),
                    (diag, "curvature diagnostics")):
        check(bool(s), "must have a section owning %s" % name)
    if fair:
        fl = fair.lower()
        check("variational" in fl, "fairing must be variational (energy-based)")
        check("bend" in fl or "energy" in fl or "second partials" in fl,
              "fairing must name the energy functional")
    if rec:
        rl = rec.lower()
        check("parameteriz" in rl, "reconstruction must address mesh parameterization")
        check("least" in rl and ("squares" in rl or "square" in rl),
              "reconstruction must name least-squares fitting")
    if diag:
        dl = diag.lower()
        check("zebra" in dl or "reflection" in dl or "highlight" in dl
              or "isophote" in dl or "comb" in dl,
              "diagnostics must name curvature comb / zebra / highlight / isophote")

    # 5. Reuse line: the OCCT seam and own NURBS math are drawn as reuse,
    #    and the moat line is drawn.
    reuse = section_text(body, "Reuse")
    check(bool(reuse), "must have a 'Reuse' section drawing the own-vs-reuse line")
    if reuse:
        rl2 = reuse.lower()
        check("occt" in rl2 or "oep-engine-occ" in rl2,
              "reuse must name OCCT behind the seam")
        check("oep-geo" in rl2 or "p6-m2" in rl2 or "nurbs" in rl2,
              "reuse must name own NURBS math (P6-M2-T02 / oep-geo)")

    # 6. The CGM moat is captured from the vault CATIA research.
    cgm = section_text(body, "What Dassault") or section_text(body, "CGM")
    check(bool(cgm), "must document the Dassault CGM moat")
    if cgm:
        cl = cgm.lower()
        check("cgm" in cl, "must name CGM")
        check("icem" in cl, "must name the ICEM lineage")
        check("tolerant" in cl, "must name tolerant modeling")
        check("spatial" in cl, "must name Spatial as the commercial arm")
        check("zebra" in cl or "reflection" in cl or "comb" in cl
              or "isophote" in cl,
              "moat must include the Class-A diagnostics/craft")

    # 7. Chosen approach: two-track, own permissive Class-A core on the seam,
    #    strategy-gated long program.
    dec = section_text(body, "Decision")
    check(bool(dec), "must have a 'Decision' section")
    if dec:
        dl = dec.lower()
        check("two-track" in dl, "decision must be a two-track strategy")
        check("own" in dl and "permissive" in dl,
              "decision must choose an own permissive Class-A core")
        check("seam" in dl or "oep-engine-occ" in dl or "occt" in dl,
              "decision must place the core on the OCCT seam")
        check("strategy-gated" in dl or "long program" in dl or "long-horizon" in dl,
              "decision must be a strategy-gated long program")

    # 8. The honest pin: NO Class-A surface inside the near-term editor train.
    check("no class-a surface" in low,
          "must pin: no Class-A surface inside the near-term editor train")
    check("editor train" in low, "must name the editor train")

    # 9. Honest cost: cost-class / effort-class terms, no calendar promise.
    hc = section_text(body, "Honest-cost") or section_text(body, "honest-cost")
    check(bool(hc), "must have an honest-cost writeup")
    if hc:
        hl = hc.lower()
        check("cost class" in hl or "effort class" in hl or "engineer-year" in hl,
              "honest cost must be in cost-class / effort-class terms")
        check("calendar" in hl, "honest cost must explicitly disclaim calendar dates")
        check("team" in hl, "honest cost must describe a credible team shape")
        check("gate" in hl, "honest cost must phase spend behind gates")

    # 10. Decomposition registered: P15 named, master plan referenced, kernel
    #     requirements present.
    decomp = section_text(body, "Long-horizon decomposition") or \
        section_text(body, "long-horizon decomposition")
    check(bool(decomp), "must have the long-horizon decomposition")
    if decomp:
        dl = decomp.lower()
        check("p15" in dl, "decomposition must register workstream P15")
        check("gate" in dl, "decomposition must carry Go/No-Go gates")
    check("master plan" in low,
          "must state the decomposition is registered in the master plan")
    kr = section_text(body, "Kernel requirements") or \
        section_text(body, "kernel requirements")
    check(bool(kr), "must have kernel requirements")
    if kr:
        kl = kr.lower()
        check("trimmed" in kl and "brep" in kl.replace("b-rep", "brep"),
              "kernel must require trimmed NURBS B-Rep")
        check("watertight" in kl, "kernel must require watertight seams")
        check("variational" in kl, "kernel must require variational fairing")

    # 11. Cross-references: sits in the geometry-core web.
    check("boolean-ops-occ-adr" in text or "P6-M3-T04" in text,
          "must tie to the kernel seam (P6-M3-T04 / boolean-ops-occ-adr)")
    check("oep-geo" in low, "must name the adopting crate oep-geo")
    check("geometry-nurbs" in text or "P6-M2" in text,
          "must tie to the P6 NURBS math lane")
    check("blender-bsd" in text.lower() or "P7-M1-T02" in text,
          "must tie to the P7-M1-T02 Blender-BSD surfacing verdict")
    check("oep-car-hood" in low or "impeller" in low or "corpus" in low,
          "must tie to the surface corpus fixtures / fidelity bar")
    check("P1-M4-T02" in text or "surface corpus" in low,
          "must tie to the surface corpus lane (P1-M4-T02)")

    # 12. Surface language rule: no em dashes, no curly quotes.
    ndashes = text.count(EM_DASH)
    check(ndashes == 0, "document contains %d em dash(es); ban applies" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + \
        text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0, "document contains %d curly quote(s)" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("SURFACING-CLASS-A ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("SURFACING-CLASS-A ADR ACCEPTANCE PASSED: "
          "two-track strategy, own permissive Class-A core on the OCCT seam, "
          "math model + decomposition + honest cost registered on real material.")
    return 0


def test_srf_class_a_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "SURFACING-CLASS-A ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
