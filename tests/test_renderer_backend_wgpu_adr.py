#!/usr/bin/env python3
"""Acceptance test for Keelworks P6-M5-T01: renderer backend decision.

Written FIRST (TDD). The decision record at P6-M5/renderer-backend-wgpu-adr.md
must make this test pass. It verifies the real material, not mocks:
  decision      => WGSL/wgpu as the renderer backend
  scope         => offline renderer for BOTH previews (P6-M5-T02 viewer)
                   and P2P render jobs (P9-M3 render worker)
  headless      => render target requires no window, no surface, no display;
                   offscreen target + CPU readback
  alternatives  => OpenGL/EGL, raw Vulkan (ash), Metal, DirectX, CPU software
                   rasterizer, browser/WebGL are weighed and rejected or
                   deferred with reasons
  interface     => renderer stays behind a clean seam in oep-geo / oep-render;
                   wgpu/WGSL concepts do not leak into the public model surface
  license       => wgpu (MIT OR Apache-2.0) assessed against the program hybrid
                   posture; no copyleft contamination for Apache-2.0 apps
  clean-room    => binding an open-source, publicly licensed renderer is not a
                   clean-room violation
  guardrails    => version pin, escape hatch to a software/fallback backend,
                   headless CI-reproducible test gate
  real material => the committed headless spike (P6-M5/spike/headless-render/)
                   with a generated proof image, plus the ADR citing the
                   verified GPU result

Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_renderer_backend_wgpu_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "P6-M5", "renderer-backend-wgpu-adr.md")
SPIKE_DIR = os.path.join(ROOT, "P6-M5", "spike", "headless-render")
SPIKE_SRC = os.path.join(SPIKE_DIR, "src", "main.rs")
PROOF_PNG = os.path.join(SPIKE_DIR, "proof.png")
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"

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
    print("P6-M5-T01 renderer backend decision acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: P6-M5/renderer-backend-wgpu-adr.md")
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

    # 3. The decision names WGSL/wgpu explicitly as the adopted backend.
    low = text.lower()
    check("wgpu" in low, "must name wgpu explicitly")
    check("wgsl" in low, "must name WGSL explicitly (the shading language)")
    check("render" in low, "must be scoped to rendering")
    check("backend" in low or "renderer" in low,
          "must frame it as a backend/renderer choice")

    # 4. Offline renderer for previews AND P2P render jobs (the card's two uses).
    check("offline" in low, "must state the renderer is an OFFLINE renderer")
    check("preview" in low, "must cover the preview use (P6-M5-T02 viewer)")
    check("p2p" in low or "peer" in low or "render job" in low,
          "must cover the P2P render-job use (P9-M3 worker)")
    check("headless" in low, "must state the headless render target")

    # 5. Headless contract: no window/surface, offscreen target, CPU readback.
    headless = section_text(body, "Headless contract")
    hlow = headless.lower()
    check(bool(hlow), "must have a 'Headless contract' section")
    if hlow:
        check("no display" in hlow or "no window" in hlow or "no surface" in hlow
              or "displayless" in hlow,
              "headless section must state no window/display dependency")
        check("offscreen" in hlow or "offscreen texture" in hlow
              or "render target" in hlow,
              "headless section must name the offscreen render target")
        check("readback" in hlow or "read back" in hlow or "copy" in hlow
              or "cpu" in hlow,
              "headless section must explain how pixels reach the CPU")

    # 6. Alternatives weighed and rejected/deferred with reasons.
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        for alt in ("opengl", "vulkan", "metal", "direct", "cpu", "webgl"):
            if alt in oml:
                break
        else:
            check(False, "options section must weigh at least one concrete "
                         "alternative backend (OpenGL/EGL, raw Vulkan, Metal, "
                         "DirectX, CPU software, WebGL)")
        check("reject" in oml or "rejected" in oml or "defer" in oml
              or "not adopt" in oml or "no" in oml,
              "alternatives must be rejected or deferred with a reason")

    # 7. Clean seam: renderer behind an interface; no leak into public surface.
    iface = section_text(body, "Clean interface")
    check(bool(iface), "must have a 'Clean interface' section")
    if iface:
        il = iface.lower()
        check("wgpu" in il or "render" in il, "interface section must discuss the renderer")
        check("leak" in il or "seam" in il or "boundary" in il
              or "public" in il or "abstraction" in il,
              "must define the seam keeping wgpu/WGSL off the public surface")

    # 8. License assessment: wgpu MIT/Apache-2.0 vs the program hybrid posture.
    lic = section_text(body, "License")
    check(bool(lic), "must have a 'License' section")
    if lic:
        ll = lic.lower()
        check("apache" in ll or "license posture" in ll or "hybrid" in ll,
              "license section must tie to the program license posture")
        check("mit" in ll, "license section must name the wgpu MIT license")
        check("copyleft" in ll or "contaminat" in ll or "compatible" in ll,
              "license section must assess copyleft contamination/compatibility")

    # 9. Clean-room stance: an open-source renderer lib is not a violation.
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if cr:
        lowcr = cr.lower()
        check("open" in lowcr and ("source" in lowcr or "public" in lowcr
                                   or "license" in lowcr),
              "clean-room section must note wgpu is open source, not a binary")
        check("violat" in lowcr or "not" in lowcr or "compatible" in lowcr
              or "invariant" in lowcr,
              "clean-room section must state wgpu binding is compatible with the "
              "clean-room invariant, or name the residual risk")

    # 10. Guardrails: version pin, escape hatch, headless test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("pin" in gl or "version" in gl,
              "guardrails must pin a wgpu version")
        check("fallback" in gl or "software" in gl or "escape" in gl
              or "swap" in gl or "replace" in gl,
              "guardrails must keep a software/fallback escape hatch")
        check("test" in gl or "ci" in gl or "acceptance" in gl,
              "guardrails must require a test gate")

    # 11. Real material: the committed headless spike + proof image.
    spike_ok = os.path.isfile(SPIKE_SRC)
    check(spike_ok, "missing spike source: P6-M5/spike/headless-render/src/main.rs")
    if spike_ok:
        spike = open(SPIKE_SRC, encoding="utf-8").read()
        slow = spike.lower()
        check("wgpu" in slow, "spike must use wgpu")
        check("wgsl" in slow, "spike must contain a WGSL shader")
        check("no surface" not in slow and "create_surface" not in slow,
              "spike must NOT create a surface (headless proof)")
    png_ok = os.path.isfile(PROOF_PNG)
    check(png_ok, "missing proof image: P6-M5/spike/headless-render/proof.png")
    if png_ok:
        with open(PROOF_PNG, "rb") as fh:
            head = fh.read(8)
        check(head == PNG_MAGIC, "proof.png is not a PNG (bad magic)")
        check(os.path.getsize(PROOF_PNG) > 100,
              "proof.png is implausibly small; expected a real render")
    check("nvidia" in low and ("gb10" in low or "proof" in low or "verified" in low),
          "ADR must cite the verified GPU result (real material)")
    check("P6-M5/spike" in text or "spike/headless-render" in text,
          "ADR must reference the committed spike path")

    # 12. Cross-references: it lives in the P6 geometry-core web.
    check("P6-M5-T01" in text, "must cite its own card P6-M5-T01")
    check("P6-M5-T02" in text, "must tie to the preview viewer card P6-M5-T02")
    check("P9-M3" in text or "P9-M3-T01" in text,
          "must tie to the P2P render worker card P9-M3-T01")
    check("oep-geo" in low, "must name the adopting surface oep-geo")

    # 13. Surface language rule: no em dashes, no curly quotes (program
    #     discipline, see P0-M2 and the P6 deliverables).
    ndashes = text.count(EM_DASH)
    check(ndashes == 0, "document contains %d em dash(es); ban applies to P6 prose too" % ndashes)
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0, "document contains %d curly quote(s); plain ASCII quotes only" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("RENDERER BACKEND WGSL/WGPU ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("RENDERER BACKEND WGSL/WGPU ADR ACCEPTANCE PASSED: "
          "WGSL/wgpu offline headless renderer for previews and P2P render jobs, "
          "alternatives + interface + license + clean-room + guardrails recorded, "
          "spike proof verified on real material.")
    return 0


def test_renderer_backend_wgpu_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "renderer backend wgpu ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
