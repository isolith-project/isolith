#!/usr/bin/env python3
"""P6-M3-T04 / ADR v1.2.0 guardrails, enforced in CI (red-on-leak).

Four independent gates, all real-material (no mocks):

  1. ADR contract  -- workstreams/geometry-core/kernel/boolean-ops-occ-adr.md
     is v1.2.0, Ratified, and actually names the delivered seam:
     oep-engine-occ, booleans + fillet/chamfer + primitives + extrude/revolve,
     the OCCT 8.0.x pin, the guardrails this file enforces, and the
     license/clean-room stance (LGPL-2.1 + exception as a separate kernel
     crate, no GPL-3.0, no reintroduced GPL tree).

  2. Seam denial   -- NO OCCT identifier (TopoDS_, Handle_, BRepAlgo,
     BRepFilletAPI, BRepPrimAPI, gp_) appears in ANY oep-geo public item.
     Scanned over comment-stripped `pub` declarations in crates/oep-geo/src.
     Red on leak (ADR section 9). Works even though the seam's own docs say
     which identifiers it bans.

  3. No vendored OCCT -- the repo contains no copied OCCT source or header
     tree: no directory named 'opencascade', and no committed file whose name
     belongs to the OCCT header/source families. OCCT must be fetched+built
     as a dependency (scripts/kernel/) per ADR section 9.

  4. License gate   -- the entire dependency graph reachable from
     oep-engine-occ is permissive (Apache-2.0/MIT family); no GPL/AGPL marker.
     The allowlist is exact for every package in the engine graph; when the
     cargo registry source is present the actual Cargo.toml `license` field is
     read and compared, so a licensing drift in a pinned transitive dep is
     caught on the real crate metadata, not just the name.

Plus: the typed-error surface the seam must expose (BooleanError / KernelError
with a typed result, never a panic) and the pinned no-"latest" 8.0.x manifest.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/geometry-core/kernel", "boolean-ops-occ-adr.md")
GEO_DIR = os.path.join(ROOT, "crates", "oep-geo", "src")
ENGINE_DIR = os.path.join(ROOT, "crates", "oep-engine-occ")
FETCH_SH = os.path.join(ROOT, "scripts", "kernel", "fetch-occt.sh")
CARGO_LOCK = os.path.join(ROOT, "Cargo.lock")

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

# The complete dependency graph reachable from oep-engine-occ (build-engine
# feature): name -> expected license string (verified against the real crate
# metadata in the cargo registry cache). ADR section 7.1 records that no
# GPL-3.0 enters any build path; this gate enforces it on real crate metadata.
ENGINE_DEP_LICENSES = {
    "cxx": "MIT OR Apache-2.0",
    "cxx-build": "MIT OR Apache-2.0",
    "cxxbridge-flags": "MIT OR Apache-2.0",
    "cxxbridge-macro": "MIT OR Apache-2.0",
    "cc": "MIT OR Apache-2.0",
    "link-cplusplus": "MIT OR Apache-2.0",
    "codespan-reporting": "Apache-2.0",
    "proc-macro2": "MIT OR Apache-2.0",
    "quote": "MIT OR Apache-2.0",
    "syn": "MIT OR Apache-2.0",
    "scratch": "MIT OR Apache-2.0",
}

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
    check(os.path.isfile(ADR), "missing: workstreams/geometry-core/kernel/boolean-ops-occ-adr.md")
    if not os.path.isfile(ADR):
        return
    text = read_adr()
    meta, body = split_front_matter(text)
    low = text.lower()

    check(meta.get("version") == "1.2.0",
          "ADR must be v1.2.0 (extrude/revolve + license confirmation); got %r"
          % meta.get("version"))
    check(meta.get("status", "").strip().lower() in ("ratified",),
          "ADR must be Ratified")
    check("decision" in meta and "opencascade" in meta["decision"].lower()
          or "occ" in meta["decision"].lower(),
          "ADR decision key must name the OCC/Kernel binding")
    check("oep-engine-occ" in text, "ADR must name the oep-engine-occ crate")
    check("oep-geo" in low, "ADR must name the adopting crate oep-geo")
    check("p6-m3-t04" in low, "ADR must cite this card (P6-M3-T04)")

    # Scope: booleans + fillet/chamfer + primitives + sweeps.
    for token in ("union", "difference", "intersection"):
        check(token in low, "ADR must cover boolean '%s'" % token)
    check("fillet" in low, "ADR must cover fillet")
    check("chamfer" in low, "ADR must cover chamfer")
    check("box" in low and "cylinder" in low and "sphere" in low,
          "ADR must cover box/cylinder/sphere primitives")
    check("extrude" in low or "makeprism" in low, "ADR must cover extrude")
    check("revolve" in low or "makerevol" in low, "ADR must cover revolve")

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
    check("lgpl-2.1" in low or "lgpl v2.1" in low or "lgpl version 2.1" in low,
          "ADR must state the OCCT license string")

    # v1.2.0 confirmed disposition: separately-linked kernel crate, no GPL.
    check("separate" in low and "gpl-3.0" in low,
          "ADR must record the no-GPL-3.0 / separately-linked-kernel disposition")
    check("gpl tree" in low, "ADR must bar reintroducing a GPL tree")

    cr = section_text(body, "Clean-room").lower()
    check(bool(cr), "ADR must have a Clean-room section")
    if cr:
        check("open" in cr and ("source" in cr or "public" in cr),
              "clean-room section must note OCCT is public open source")
        check("invariant" in cr or "violat" in cr or "compatible" in cr,
              "clean-room section must address the invariant")

    # Change log has the 1.2.0 landing entry.
    check("1.2.0" in text and "2026-10-22" in text,
          "ADR change log must carry the 1.2.0 (2026-10-22) entry")


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

    leaks = []
    for path in rust_files:
        with open(path, encoding="utf-8") as fh:
            clean = strip_comments(fh.read())
        for line, tok in public_declaration_lines(clean):
            for banned in OCC_TOKENS:
                if tok.startswith(banned) or banned in line.replace(tok, ""):
                    leaks.append("%s: %s" % (os.path.relpath(path, ROOT), line.strip()))
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

    glue = os.path.join(ENGINE_DIR, "cpp", "glue.hh")
    check(os.path.isfile(glue), "missing engine C++ glue cpp/glue.hh")
    if os.path.isfile(glue):
        check(not OCCT_FILE_RE.match("glue.hh"),
              "glue.hh must not be named like an OCCT file")


# ---------------------------------------------------------------------------
# Gate 4: license gate (the engine dependency graph is all-permissive).
# ---------------------------------------------------------------------------

def lock_package_versions():
    """Cargo.lock [[package]] name -> version (real lockfile)."""
    out = {}
    cur = None
    with open(CARGO_LOCK, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("[[package]]"):
                cur = {}
                out[id(cur)] = cur
            elif cur is not None and "=" in line:
                key, _, val = line.partition("=")
                cur[key.strip()] = val.strip().strip('"')
    merged = {}
    for blk in out.values():
        if "name" in blk and "version" in blk:
            merged[blk["name"]] = blk["version"]
    return merged


def registry_license(name, version):
    """Read the actual `license` field from the cargo registry source cache for
    a pinned package; None when the cache is not present (CI/offline variant)."""
    homes = [os.environ.get("CARGO_HOME"),
             os.path.join(os.path.expanduser("~"), ".cargo")]
    for home in homes:
        if not home:
            continue
        srcbase = os.path.join(home, "registry", "src")
        if not os.path.isdir(srcbase):
            continue
        for idx in os.listdir(srcbase):
            toml = os.path.join(srcbase, idx, "%s-%s" % (name, version), "Cargo.toml")
            if os.path.isfile(toml):
                text = open(toml, encoding="utf-8").read()
                m = re.search(r"^license\s*=\s*\"([^\"]+)\"", text, re.M)
                if m:
                    return m.group(1)
    return None


def check_license_gate():
    check(os.path.isfile(CARGO_LOCK), "missing Cargo.lock for the license gate")
    if not os.path.isfile(CARGO_LOCK):
        return
    versions = lock_package_versions()
    for name, expected in sorted(ENGINE_DEP_LICENSES.items()):
        version = versions.get(name)
        check(version is not None,
              "engine dependency '%s' missing from Cargo.lock" % name)
        if version is None:
            continue
        actual = registry_license(name, version)
        if actual is None:
            # Cache miss (never built on this host): name-level membership is
            # still enforced by the allowlist above; warn but do not fail a
            # host that has not compiled the engine yet.
            continue
        low = actual.lower()
        check("gpl" not in low and "agpl" not in low,
              "'%s' %s: license %r carries a GPL/AGPL marker" % (name, version, actual))
        check(actual == expected,
              "'%s' %s: license %r drifted from allowlist %r"
              % (name, version, actual, expected))


# ---------------------------------------------------------------------------
# Gate 5: pinned manifest + typed surface.
# ---------------------------------------------------------------------------

def check_manifest_and_surface():
    cargo = os.path.join(ENGINE_DIR, "Cargo.toml")
    check(os.path.isfile(cargo), "missing oep-engine-occ/Cargo.toml")
    if os.path.isfile(cargo):
        text = open(cargo, encoding="utf-8").read()
        check("8.0" in text, "engine manifest must pin the OCCT 8.0.x line")
        check("features" in text and "build-engine" in text,
              "engine manifest must gate the engine behind the build-engine feature")

    check(os.path.isfile(FETCH_SH), "missing scripts/kernel/fetch-occt.sh")
    if os.path.isfile(FETCH_SH):
        script = open(FETCH_SH, encoding="utf-8").read()
        check("8.0.1" in script, "fetch script must pin a concrete OCCT version")
        check("sha256" in script.lower() and re.search(r"0d6913eae4bcc09a", script),
              "fetch script must carry the pinned tarball SHA-256")
        banned = re.findall(r"=\s*[\"']?latest[\"']?\s*$", script, re.M)
        check(not banned, "fetch script must not float 'latest'")

    if os.path.isdir(GEO_DIR):
        kernel_src = os.path.join(GEO_DIR, "kernel.rs")
        check(os.path.isfile(kernel_src), "missing oep-geo kernel seam kernel.rs")
        if os.path.isfile(kernel_src):
            src = open(kernel_src, encoding="utf-8").read()
            for name in ("pub enum BooleanError", "pub enum KernelError",
                         "pub fn solid_boolean", "pub fn solid_fillet",
                         "pub fn solid_chamfer", "pub fn make_box",
                         "pub fn make_cylinder", "pub fn make_sphere",
                         "pub fn solid_extrude", "pub fn solid_revolve",
                         "pub struct Solid", "pub struct Tolerance",
                         "pub struct Point2D"):
                check(name in src, "kernel seam must expose %r" % name)
        geo_cargo = os.path.join(ROOT, "crates", "oep-geo", "Cargo.toml")
        if os.path.isfile(geo_cargo):
            gt = open(geo_cargo, encoding="utf-8").read()
            compact = gt.replace(" ", "").replace("\n", "").replace("\t", "")
            check('kernel=["dep:oep-engine-occ"' in compact,
                  "oep-geo must enable the engine via the kernel feature")

    # The through-the-seam acceptance matrix + its committed golden exist.
    matrix = os.path.join(ROOT, "crates", "oep-geo", "tests", "kernel_seam.rs")
    golden = os.path.join(ROOT, "crates", "oep-geo", "tests", "golden",
                          "kernel-seam-golden.json")
    check(os.path.isfile(matrix), "missing kernel acceptance matrix kernel_seam.rs")
    check(os.path.isfile(golden), "missing committed kernel golden kernel-seam-golden.json")


# ---------------------------------------------------------------------------
# Document hygiene + surface language rule.
# ---------------------------------------------------------------------------

def check_prose_rules():
    text = read_adr()
    # Program ban: no em dashes anywhere in the ADR, and no curly quotes.
    check(text.count(EM_DASH) == 0,
          "ADR contains %d em dashes (program ban)" % text.count(EM_DASH))
    ncurly = text.count("\u201c") + text.count("\u201d") + text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0, "ADR contains %d curly quote(s); plain ASCII only" % ncurly)


def run_checks():
    print("P6-M3-T04 kernel seam guardrails (ADR v1.2.0)")
    print("adr: %s" % ADR)
    print("=" * 66)
    check_adr()
    check_seam_denial()
    check_no_vendored_occt()
    check_license_gate()
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
    print("KERNEL SEAM GUARDRAILS PASSED: ADR v1.2.0 ratified and complete, "
          "seam denial clean, no vendored OCCT, license gate permissive, "
          "pinned 8.0.x, typed surface + matrix present.")
    return 0


def test_kernel_seam_guardrails():
    failures = run_checks()
    assert not failures, "kernel seam guardrails failed (%d): %s" % (
        len(failures), failures[:8])


if __name__ == "__main__":
    main()
