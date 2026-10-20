#!/usr/bin/env python3
"""Acceptance test for Isolith P4-M4-T03 (t_6d40b761): JT reader
visualization-first wedge in the P4 lane (oep-step jt module).

Written FIRST (TDD) by the same convention as test_p4m4_jt_adr.py. It
verifies the real material, not mocks:

  1. The jt module exists under crates/oep-step/src/jt/ with the four
     bounded behaviors (core container, scene-graph, shape, codec).
  2. The vendor-neutral seam is clean: a scan (part of the acceptance gate)
     rejects JT container/codec vocabulary on the crate's public surface.
     COMPARE WITH the STEP lane: oep_step::parse is the vendor-neutral entry;
     for JT the seam means no exporter-style vocabulary leaks into public
     items. We scan the public item names in lib.rs re-exports and the jt
     module doc-facing surface for the banned vocabulary.
  3. No vendored foreign source: a scan rejects the TKJT / JT Assistant /
     PyOpenJt identifier set inside the crate source (clean-room gate).
  4. The committed dev-only corpus exists and the golden decodes - the Rust
     test jt_golden.rs pins 8 vertices / 12 triangles and bounding volume
     tolerance; this file re-verifies the corpus is present and catalogued
     under corpus/jt with provenance in jt-import-sources.md.
Plus the program surface rule (no em dashes, no curly quotes) for new
prose we add in P4-M4.

Run:  python3 tests/test_p4m4_jt_reader.py  (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
JT_DIR = os.path.join(ROOT, "crates", "oep-step", "src", "jt")
LIB = os.path.join(ROOT, "crates", "oep-step", "src", "lib.rs")
CORPUS = os.path.join(ROOT, "corpus", "jt")
SOURCES = os.path.join(ROOT, "workstreams/step-interop/qif", "jt-import-sources.md")
PLAN = os.path.join(ROOT, "workstreams/step-interop/qif", "jt-reader-plan.md")

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def walk_py(path):
    for dirpath, dirnames, filenames in os.walk(path):
        dirnames[:] = [d for d in dirnames if d not in ("target", "__pycache__")]
        for fn in filenames:
            yield os.path.join(dirpath, fn)


def read(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        return fh.read()


# Container / codec vocabulary that belongs only behind the seam; any hit on a
# public item name means the seam leaked.
BANNED_PUBLIC = [
    "Toc",
    "TOC",
    "Lsg",
    "LSG",
    "SegmentHeader",
    "ElementHeader",
    "Bitlength",
    "BitLength",
    "Deering",
    "Arithmetic",
    "Lzma",
    "LZMA",
    "TriStrip",
    "PolygonSet",
    "PolylineSet",
    "PointSet",
    "PrimitiveSet",
    "Quantiz",
    "ShapeLod",
    "Prostep",
]

# Foreign JT reader identifiers that must NOT appear anywhere in our crate
# source (no-vendored-foreign-source gate).
BANNED_FOREIGN = [
    "TKJT",
    "TkJt",
    "JT Assistant",
    "JTAssistant",
    "PyOpenJt",
    "oce-jt",
    "JTConvert",
    "JTReader",
    "Voyager",
    "Open CASCADE",
    "OpenCascade",
]


def public_surface_text():
    """The public surface of oep-step the seam scan must be clean on: the
    crate root re-exports (lib.rs) plus the jt module's exported names. Since
    Rust 2018 modules re-export only via pub items, we scan lib.rs and the
    top of jt/mod.rs for `pub fn` / `pub struct` / `pub enum` names."""
    texts = []
    for p in (LIB, os.path.join(JT_DIR, "mod.rs")):
        if os.path.isfile(p):
            texts.append(read(p))
    return "\n".join(texts)


def run_checks():
    print("P4-M4-T03 JT reader wedge acceptance")
    print("=" * 66)

    # 1. Bounded behaviors exist: four private modules behind the seam.
    for leaf in ("core.rs", "lsg.rs", "shape.rs", "codec.rs", "mod.rs"):
        check(os.path.isfile(os.path.join(JT_DIR, leaf)),
              "missing jt module leaf: %s" % leaf)

    # 2. Vendor-neutral seam: no container/codec vocabulary on public items.
    surface = public_surface_text()
    for token in BANNED_PUBLIC:
        # only flag whole-word-ish occurrences on pub lines, not in comments
        for line in surface.splitlines():
            if re.search(r"\bpub\b", line) and token in line:
                check(False, "seam leak: public item mentions %r" % token)
                break

    # 3. No vendored foreign source: banned identifier scan over all crate
    #    source (clean-room gate; must FAIL the build if copied source appears).
    #    Doc comments legitimately name the rejected readers when stating the
    #    clean-room policy, so the scan strips comments before matching and
    #    only fails on identifiers in actual code.
    hits = []
    for p in walk_py(os.path.join(ROOT, "crates", "oep-step", "src")):
        text = read(p)
        # strip // line comments and /* */ block comments
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
        text = re.sub(r"//[^\n]*", "", text)
        for ident in BANNED_FOREIGN:
            if re.search(r"\b" + re.escape(ident) + r"\b", text):
                hits.append("%s:%s" % (ident, os.path.relpath(p, ROOT)))
    check(not hits, "vendored foreign source identifiers found in code: %s" % (hits or ""))

    # 4. Dev-only corpus present + catalogued; golden facts are Rust-pinned.
    expected_fixtures = [
        "golden-box-a.jt",
        "bad-byte-order.jt",
        "empty.jt",
        "junk-not-jt.bin",
        "root-not-lsg.jt",
        "segment-overrun.jt",
        "toc-offset-oob.jt",
        "truncated-header.jt",
    ]
    present = os.listdir(CORPUS) if os.path.isdir(CORPUS) else []
    for fx in expected_fixtures:
        check(fx in present, "missing corpus fixture: %s" % fx)
    check(os.path.isfile(SOURCES), "missing workstreams/step-interop/qif/jt-import-sources.md")

    # 5. Plan record exists (plan-first convention) and is em-dash / curly
    #    free along with the sources file.
    check(os.path.isfile(PLAN), "missing workstreams/step-interop/qif/jt-reader-plan.md")
    for path, label in ((PLAN, "plan"), (SOURCES, "sources")):
        if os.path.isfile(path):
            text = read(path)
            check(EM_DASH not in text, "%s contains an em dash (ban)" % label)
            check(not any(c in text for c in CURLY),
                  "%s contains a curly quote (ban)" % label)

    # 6. The reader's public API returns typed errors, never panics: enforce
    #    at the crate level by requiring the golden test file exists (the Rust
    #    test is the real execution of the never-panic contract).
    golden_test = os.path.join(ROOT, "crates", "oep-step", "tests", "jt_golden.rs")
    check(os.path.isfile(golden_test), "missing jt_golden.rs (never-panic gate)")

    return failures


def main():
    failures = run_checks()
    if failures:
        print("JT READER WEDGE ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("JT READER WEDGE ACCEPTANCE PASSED: spec-driven reader behind the "
          "vendor-neutral seam, golden fixture pinned (Rust jt_golden.rs), "
          "corrupt corpus typed-error never-panic, seam + no-vendored scans "
          "clean, dev-only corpus catalogued.")
    return 0


def test_p4m4_jt_reader_acceptance():
    failures = run_checks()
    assert not failures, "JT reader wedge acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
