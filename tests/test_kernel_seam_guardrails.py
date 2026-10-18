#!/usr/bin/env python3
"""P6-M3-T04 / ADR v1.1.0 guardrails, enforced in CI (red-on-leak).

Three independent gates, all real-material (no mocks):

  1. ADR contract  -- P6-M3/boolean-ops-occ-adr.md is v1.1.0, Ratified, and
     actually names the delivered seam: oep-engine-occ, booleans +
     fillet/chamfer + primitives, the OCCT 8.0.x pin, the two guardrails this
     file enforces, and the license/clean-room stance.

  2. Seam denial   -- NO OCCT identifier (TopoDS_, Handle_, BRepAlgo,
     BRepFilletAPI, BRepPrimAPI, gp_) appears in ANY oep-geo public item.
     Scanned over comment-stripped `pub` declarations in crates/oep-geo/src.
     Red on leak (ADR section 9). Works even though the seam's own docs say
     which identifiers it bans.

  3. No vendored OCCT -- the repo contains no copied OCCT source or header
     tree: no directory named 'opencascade', and no committed file whose name
     belongs to the OCCT header/source families. OCCT must be fetched+built
     as a dependency (scripts/kernel/) per ADR section 9.

Plus: the typed-error surface the seam must expose (BooleanError / KernelError
with a typed result, never a panic) and the pinned no-"latest" 8.0.x manifest.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "P6-M3", "boolean-ops-occ-adr.md")
GEO_DIR = os.path.join(ROOT, "crates", "oep-geo", "src")
ENGINE_DIR = os.path.join(ROOT, "crates", "oep-engine-occ")
FETCH_SH = os.path.join(ROOT, "scripts", "kernel", "fetch-occt.sh")

# OCCT identifiers that must never leak into oep-geo public items (ADR sec 9).
OCC_TOKENS = ("TopoDS_", "Handle_", "BRepAlgo", "BRepFilletAPI", "BRepPrimAPI", "gp_")

# Tell-tale OCCT source/header file families; a committed copy is a guardrail
# violation regardless of the directory it sits in (ADR section 9, no vendored
# source). Our own C++ glue (glue.hh / glue.cc) does not match any family.
OCCT_FILE_RE = re.compile(
    r"^(Standard_|BRep_|BRepAlgo|BRepFillet|BRepPrim|TopoDS_|TopAbs|TopExp|"
    r"gp_|GProp_|TColgp_|TCollection_|NCollection_|Message_|Quantity_|Geom_|"
    r"Geom2d_|Bnd_|BRepBuilderAPI|BRepTools|BRepGProp|BRepCheck|"
    r"STEPControl|ALCAF|CDM_|AIS_|V3d_)[A-Za-z0-9_]*\.(hxx|cxx|h|cpp|cc)$"
)
CPP_SUFFIXES = (".hxx", ".cxx", ".h", ".cpp", ".cc")
SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_adr():
    with open(ADR, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
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
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def strip_comments(text):
    """Remove // and /* */ comments (keep strings) so doc-text mentions of the
    banned identifiers do not trip the seam scan; the ADR bans them in public
    ITEMS, not prose."""
    out = []
    lines = text.splitlines()
    for line in lines:
        lo = line.find("/*")
        while lo != -1:
            hi = line.find("*/", lo + 2)
            if hi == -1:
                line = line[:lo]
                break
            line = line[:lo] + " " + line[hi + 2:]
            lo = line.find("/*")
        idx = line.find("//")
        if idx != -1:
            line = line[:idx]
        out.append(line)
    return "\n".join(out)


def public_declaration_lines(text):
    """Yield tokens of each line that starts a public item declaration."""
    ids = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("pub "):
            continue
        for tok in ids.findall(stripped):
            yield line, tok


# ---------------------------------------------------------------------------
# Gate 1: ADR contract.
# ---------------------------------------------------------------------------

def check_adr():
    check(os.path.isfile(ADR), "missing: P6-M3/boolean-ops-occ-adr.md")
    if not os.path.isfile(ADR):
        return
    text = read_adr()
    meta, body = split_front_matter(text)
    low = text.lower()

    check(meta.get("version") == "1.1.0",
          "ADR must be v1.1.0 (fillet/chamfer/primitive widening); got %r"
          % meta.get("version"))
    check(meta.get("status", "").strip().lower() in ("ratified",),
          "ADR must be Ratified")
    check("decision" in meta and "opencascade" in meta["decision"].lower()
          or "occ" in meta["decision"].lower(),
          "ADR decision key must name the OCC/Kernel binding")
    check("oep-engine-occ" in text, "ADR must name the oep-engine-occ crate")
    check("oep-geo" in low, "ADR must name the adopting crate oep-geo")
    check("p6-m3-t04" in low, "ADR must cite this card (P6-M3-T04)")

    # Widened scope: booleans + fillet/chamfer + primitives.
    for token in ("union", "difference", "intersection"):
        check(token in low, "ADR must cover boolean '%s'" % token)
    check("fillet" in low, "ADR must cover fillet")
    check("chamfer" in low, "ADR must cover chamfer")
    check("box" in low and "cylinder" in low and "sphere" in low,
          "ADR must cover box/cylinder/sphere primitives")

    # Guardrails named.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "ADR must have a Guardrails section")
    if gl:
        check("pin" in gl and ("8.0" in gl or "7.9" in gl),
              "guardrails pin OCCT (8.0.x line)")
        check("latest" in gl or "no \"latest\"" in gl,
              "guardrails must reject floating 'latest'")
        check("vendored" in gl or "vendor" in gl,
              "guardrails must ban vendored OCCT source")
        check("seam" in gl and ("denial" in gl or "assert" in gl or "grep" in gl),
              "guardrails must require the seam-denial scan")
        check("test" in gl or "acceptance" in gl,
              "guardrails must require a test gate")

    # License + clean-room.
    lic = section_text(body, "License").lower()
    check(bool(lic), "ADR must have a License section")
    if lic:
        check("lgpl" in lic, "license section must name LGPL")
        check("exception" in lic, "license section must name the OCCT exception")
        check("apache" in lic, "license section must tie to the Apache posture")
    cr = section_text(body, "Clean-room").lower()
    check(bool(cr), "ADR must have a Clean-room section")
    if cr:
        check("open" in cr and ("source" in cr or "public" in cr),
              "clean-room section must note OCCT is public open source")
        check("invariant" in cr or "violat" in cr or "compatible" in cr,
              "clean-room section must address the invariant")
    check("lgpl-2.1" in low or "lgpl v2.1" in low or "lgpl version 2.1" in low,
          "ADR must state the OCCT license string")

    # Change log has the 1.1.0 widening entry.
    check("1.1.0" in text and "2026-10-18" in text,
          "ADR change log must carry the 1.1.0 (2026-10-18) widening entry")


# ---------------------------------------------------------------------------
# Gate 2: seam denial.
# ---------------------------------------------------------------------------

def check_seam_denial():
    rust_files = []
    for dirpath, _, filenames in os.walk(GEO_DIR):
        for fn in filenames:
            if fn.endswith(".rs"):
                rust_files.append(os.path.join(dirpath, fn))
    check(bool(rust_files), "oep-geo src tree not found: %s" % GEO_DIR)

    # Optional (informational): which banned docs the ADR says are ok to quote.
    banned_mentioned_in_docs = 0
    leaks = []
    for path in rust_files:
        with open(path, encoding="utf-8") as fh:
            clean = strip_comments(fh.read())
        # The scan in the ADR targets PUBLIC items; only public declarations.
        for line, tok in public_declaration_lines(clean):
            for banned in OCC_TOKENS:
                if tok.startswith(banned) or banned in line.replace(tok, ""):
                    leaks.append("%s: %s" % (os.path.relpath(path, ROOT), line.strip()))
        # (informational) banned tokens may appear in prose/strings, which is
        # legal: the restriction is on public items, not on documentation.
        for banned in OCC_TOKENS:
            if banned in clean:
                banned_mentioned_in_docs += 1
    if leaks:
        for l in leaks[:8]:
            check(False, "seam leak: " + l)
    else:
        check(True, "seam clean: no OCCT identifier in oep-geo public items")


# ---------------------------------------------------------------------------
# Gate 3: no vendored OCCT source.
# ---------------------------------------------------------------------------

def check_no_vendored_occt():
    hits = []
    opencascade_dirs = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [d for d in dirnames
                       if d not in (".git", "target", ".worktrees", "__pycache__",
                                    ".cargo-x86-home", "build", ".pytest_cache")]
        components = dirpath.replace(ROOT, "").split(os.sep)
        if "opencascade" in components:
            opencascade_dirs.append(dirpath)
        for fn in filenames:
            base = os.path.basename(fn)
            if base.endswith(CPP_SUFFIXES) and OCCT_FILE_RE.match(base):
                hits.append(os.path.relpath(os.path.join(dirpath, fn), ROOT))
    check(not opencascade_dirs,
          "vendored OCCT header tree detected under: %s" % opencascade_dirs[:3])
    check(not hits, "vendored OCCT source/header detected: %s" % hits[:5])

    # The engine's own C++ glue must exist (that is OUR code, not a vendor copy)
    # and must NOT be misnamed as an OCCT family file.
    glue = os.path.join(ENGINE_DIR, "cpp", "glue.hh")
    check(os.path.isfile(glue), "missing engine C++ glue cpp/glue.hh")
    if os.path.isfile(glue):
        check(not OCCT_FILE_RE.match("glue.hh"),
              "glue.hh must not be named like an OCCT file")


# ---------------------------------------------------------------------------
# Gate 4: pinned manifest + typed surface.
# ---------------------------------------------------------------------------

def check_manifest_and_surface():
    cargo = os.path.join(ENGINE_DIR, "Cargo.toml")
    check(os.path.isfile(cargo), "missing oep-engine-occ/Cargo.toml")

    exact = " OCCT 8.0.x"
    if os.path.isfile(cargo):
        text = open(cargo, encoding="utf-8").read()
        check("8.0" in text, "engine manifest must pin the OCCT 8.0.x line")
        check("features" in text and "build-engine" in text,
              "engine manifest must gate the engine behind the build-engine feature")

    check(os.path.isfile(FETCH_SH), "missing scripts/kernel/fetch-occt.sh")
    if os.path.isfile(FETCH_SH):
        script = open(FETCH_SH, encoding="utf-8").read()
        check("OEP_OCCT_VERSION=${OEP_OCCT_VERSION:-8.0.1}" in script
              or "8.0.1" in script,
              "fetch script must pin a concrete OCCT version")
        check("sha256" in script.lower() and re.search(r"0d6913eae4bcc09a", script),
              "fetch script must carry the pinned tarball SHA-256")
        # "latest" is fine in prose/comments; banned when used AS the version.
        banned = re.findall(r"=\s*[\"']?latest[\"']?\s*$", script, re.M)
        check(not banned, "fetch script must not float 'latest'")

    # oep-geo typed seam surface.
    if os.path.isdir(GEO_DIR):
        kernel_src = os.path.join(GEO_DIR, "kernel.rs")
        check(os.path.isfile(kernel_src), "missing oep-geo kernel seam kernel.rs")
        if os.path.isfile(kernel_src):
            src = open(kernel_src, encoding="utf-8").read()
            for name in ("pub enum BooleanError", "pub enum KernelError",
                         "pub fn solid_boolean", "pub fn solid_fillet",
                         "pub fn solid_chamfer", "pub fn make_box",
                         "pub fn make_cylinder", "pub fn make_sphere",
                         "pub struct Solid", "pub struct Tolerance"):
                check(name in src, "kernel seam must expose %r" % name)
        geo_cargo = os.path.join(ROOT, "crates", "oep-geo", "Cargo.toml")
        if os.path.isfile(geo_cargo):
            gt = open(geo_cargo, encoding="utf-8").read()
            compact = gt.replace(" ", "").replace("\n", "").replace("\t", "")
            check('kernel=["dep:oep-engine-occ"' in compact,
                  "oep-geo must enable the engine via the kernel feature")


# ---------------------------------------------------------------------------
# Document hygiene + surface language rule.
# ---------------------------------------------------------------------------

def check_prose_rules():
    text = read_adr()
    check(text.count(EM_DASH) == 0,
          "ADR contains %d em dashes (program ban)" % text.count(EM_DASH))


def run_checks():
    print("P6-M3-T04 kernel seam guardrails")
    print("adr: %s" % ADR)
    print("=" * 66)
    check_adr()
    check_seam_denial()
    check_no_vendored_occt()
    check_manifest_and_surface()
    check_prose_rules()
    return failures


def main():
    failures = run_checks()
    if failures:
        print("KERNEL SEAM GUARDRAILS FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("KERNEL SEAM GUARDRAILS PASSED: ADR v1.1.0 ratified and complete, "
          "seam denial clean, no vendored OCCT, pinned 8.0.x, typed surface present.")
    return 0


def test_kernel_seam_guardrails():
    failures = run_checks()
    assert not failures, "kernel seam guardrails failed (%d): %s" % (
        len(failures), failures[:8])


if __name__ == "__main__":
    main()
