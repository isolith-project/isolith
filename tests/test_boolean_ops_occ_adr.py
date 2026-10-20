#!/usr/bin/env python3
"""Acceptance test for Keelworks P6-M3-T01: B-Rep boolean operations decision.

Written FIRST (TDD). The ADR at workstreams/geometry-core/kernel/boolean-ops-occ-adr.md must make this
test pass. It verifies the real material, not mocks:
  decision      => bind/tie Open CASCADE (OCCT) for B-Rep boolean ops
  default       => adopt OCC for parity of effort, keep a clean interface
  own kernel    => routing an own "kernel-lite" boolean implementation is an
                   explicitly weighed option and is rejected with reasons
  interface     => OCC stays behind a clean, documented seam; OCC concepts do
                   not leak into the oep-geo public surface
  license       => OCCT LGPL-2.1 + exception is assessed against the program
                   hybrid license posture (Apache-2.0 apps/libraries)
  clean-room    => binding an open-source kernel is not a clean-room violation
  guardrails    => version pin, swap/escape hatch, tests so the decision
                   survives code
Plus the program surface-language rule (no em dashes), matching the P0/P6
deliverables.

Run:  python3 tests/test_boolean_ops_occ_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/geometry-core/kernel", "boolean-ops-occ-adr.md")

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
    print("P6-M3-T01 boolean-ops OCC decision acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: workstreams/geometry-core/kernel/boolean-ops-occ-adr.md")
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

    # 3. The decision names Open CASCADE as the adopted boolean engine.
    low = text.lower()
    check("open cascade" in low or "opencascade" in low or "occt" in low,
          "must name Open CASCADE (OCCT) explicitly")
    check("boolean" in low, "must be scoped to boolean operations")
    check("brep" in low or "b-rep" in low.replace("brep ", "b-rep "),
          "must name the B-Rep representation it applies to")

    # 3b. v1.1.0 widened scope: fillet, chamfer, and primitives are at the
    #     seam alongside the three booleans (P6-M3-T04).
    check("fillet" in low, "must include fillet in the kernel seam scope")
    check("chamfer" in low, "must include chamfer in the kernel seam scope")
    check("primitive" in low or "make_box" in low or "makebox" in low,
          "must include primitive creation in the kernel seam scope")
    check("cylinder" in low and "sphere" in low,
          "primitives must name box/cylinder/sphere")
    check("1.1.0" in text, "widened scope must be recorded as version 1.1.0")

    # 4. Default stance: bind OCC for parity of effort.
    check("pority" not in low and "parity" in low,
          "must cite parity of effort as the adoption rationale")
    check("adopt" in low or "bind" in low or "tie" in low,
          "must state an adopt/bind/tie stance for OCC")
    check("default" in low or "defaults" in low,
          "must present the default posture explicitly")

    # 5. Own-kernel alternative is weighed, listed as an option, and rejected
    #    with a reason (the decision is not taken on vibes).
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        check("own" in oml and ("kernel" in oml or "kernel-lite" in oml),
              "must weigh an own kernel-lite option")
        check("reject" in oml or "rejected" in oml or "defer" in oml
              or "not adopt" in oml or "no" in oml,
              "own kernel must be rejected or deferred with a reason")

    # 6. Clean interface / seam: OCC stays behind an interface; no leak.
    iface = section_text(body, "Clean interface")
    check(bool(iface), "must have a 'Clean interface' section")
    if iface:
        il = iface.lower()
        check("occ" in il, "interface section must discuss OCC")
        check("leak" in il or "seam" in il or "boundary" in il
              or "abstraction" in il or "public" in il,
              "must define the seam keeping OCC off the public surface")

    # 7. License assessment: LGPL-2.1 + exception vs the Apache-2.0 posture.
    lic = section_text(body, "License")
    check(bool(lic), "must have a 'License' section")
    if lic:
        ll = lic.lower()
        check("lgpl" in ll, "license section must name LGPL (OCCT license)")
        check("apache" in ll or "license posture" in ll or "hybrid" in ll,
              "license section must tie to the program license posture")
        check("exception" in ll or "linking" in ll or "dynamic" in ll
              or "lgpl-2.1" in ll,
              "license section must address the OCCT exception / linking model")

    # 8. Clean-room stance: linking an open-source kernel is not a violation
    #    of the program clean-room invariant.
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if cr:
        lowcr = cr.lower()
        check("open" in lowcr and ("source" in lowcr or "public" in lowcr
                                   or "license" in lowcr),
              "clean-room section must note OCC is open source, not a binary")
        check("violat" in lowcr or "not" in lowcr or "compatible" in lowcr
              or "invariant" in lowcr,
              "clean-room section must state OCC binding is compatible with the "
              "clean-room invariant, or name the residual risk")

    # 9. Guardrails: version pin and escape hatch, and a test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("pin" in gl or "version" in gl, "guardrails must pin an OCC version")
        check("swap" in gl or "escape" in gl or "replace" in gl
              or "kernel-lite" in gl or "behind" in gl,
              "guardrails must keep an escape/swap hatch for a future own kernel")
        check("test" in gl or "ci" in gl or "acceptance" in gl,
              "guardrails must require a test gate")

    # 10. Cross-references: it must live in the P6 geometry-core web.
    check("workstreams/geometry-core/kernel" in text or "P6-M3-T01" in text, "must cite its own card P6-M3-T01")
    check("oep-geo" in low, "must name the adopting crate oep-geo")
    check("P6-M2" in low or "math" in low, "must tie to the P6 math/NURBS lane")

    # 11. Surface language rule: no em dashes, no curly quotes (program
    #     discipline, see P0-M2 and the P6 deliverables).
    ndashes = text.count(EM_DASH)
    check(ndashes == 0, "document contains %d em dash(es); ban applies to P6 prose too" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0, "document contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("BOOLEAN OPS OCC ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("BOOLEAN OPS OCC ADR ACCEPTANCE PASSED: "
          "bind Open CASCADE for B-Rep boolean ops behind a clean interface, "
          "license + clean-room + guardrails recorded on real material.")
    return 0


def test_boolean_ops_occ_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "boolean ops OCC ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
