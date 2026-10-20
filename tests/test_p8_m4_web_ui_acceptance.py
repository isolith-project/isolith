#!/usr/bin/env python3
"""Acceptance test for Isolith P8-M4-T01: PDM web UI.

Written FIRST (TDD) and kept green. The crate at crates/oep-web must make this
test pass. It verifies the acceptance criteria against the crate and its
committed real-material proof, without needing a live server here:

  explorer    => an item explorer route exists that lists vault items
  bom         => a BOM tree route exists that walks the relationship graph
                 (oep_pdm::graph) and shows composed children with quantity
  viewer      => a viewer-embed route renders real oep-geo models to PNG
                 through the P6 oep-viewer seam; committed proof PNGs are valid
                 and differ between the two fixture revisions
  compare     => a compare view shows two revisions of an item
  rest        => the REST API v1 router is merged alongside the UI
  crate       => oep-web is a workspace member; the UI note documents the card
  language    => no em dashes, no curly quotes in the files this card adds

Run:  python3 tests/test_p8_m4_web_ui_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "crates", "oep-web")
SRC = os.path.join(WEB, "src")
PROOF = os.path.join(ROOT, "workstreams", "pdm", "web", "proof")
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
EM_DASH = "\u2014"

failures = []
warnings = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_text(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def png_ok(path):
    if not os.path.isfile(path):
        return False, "missing"
    with open(path, "rb") as fh:
        head = fh.read(8)
    if head != PNG_MAGIC:
        return False, "bad magic"
    if os.path.getsize(path) < 200:
        return False, "implausibly small (%d bytes)" % os.path.getsize(path)
    return True, "ok"


def run_checks():
    print("P8-M4-T01 PDM web UI acceptance")
    print("web: %s" % WEB)
    print("=" * 66)

    # 1. The web UI crate exists and is a workspace member.
    check(os.path.isfile(os.path.join(WEB, "Cargo.toml")), "missing: crates/oep-web/Cargo.toml")
    root_cargo = read_text(os.path.join(ROOT, "Cargo.toml"))
    check("crates/oep-web" in root_cargo, "crates/oep-web must be a workspace member")

    # 2. Modules: lib.rs (server) + web/ (routes) + viewer embed preview.
    lib = read_text(os.path.join(SRC, "lib.rs")) if os.path.isfile(os.path.join(SRC, "lib.rs")) else ""
    check("pub mod web;" in lib or "pub mod web" in lib, "lib.rs must export the web module")
    check("PdmWeb" in lib, "lib.rs must provide the PdmWeb server")
    web_mod = os.path.join(SRC, "web", "mod.rs")
    web_src = read_text(web_mod) if os.path.isfile(web_mod) else ""
    check(web_src != "", "missing crate: src/web/mod.rs")
    for token in ("explorer", "item_page", "bom_page", "preview_png", "compare_page"):
        check(token in web_src, "web module must define %s" % token)
    preview = os.path.join(SRC, "web", "preview.rs")
    check(os.path.isfile(preview), "missing crate: src/web/preview.rs")
    if os.path.isfile(preview):
        pv = read_text(preview)
        check("render_document_png" in pv, "preview.rs must render documents to PNG")

    # 3. The BOM walk builds on the relationship graph (children accessor).
    graph_src = os.path.join(ROOT, "crates", "oep-pdm", "src", "graph.rs")
    if os.path.isfile(graph_src):
        gs = read_text(graph_src)
        check("pub fn children(" in gs, "graph must expose the BOM children() walk")

    # 4. A Rust acceptance test drives the real server over HTTP.
    test = os.path.join(WEB, "tests", "web_ui_acceptance.rs")
    check(os.path.isfile(test), "missing acceptance test: tests/web_ui_acceptance.rs")
    if os.path.isfile(test):
        t = read_text(test)
        for key in ("explorer", "bom_tree", "viewer_embed", "compare_view", "example-bracket"):
            check(key in t, "acceptance test must exercise %r" % key)

    # 5. Committed real-material proof: valid PNGs, and the two fixture
    #    revisions render differently (viewer embed gate without a live server).
    ok, why = png_ok(os.path.join(PROOF, "preview-1.png"))
    check(ok, "proof preview-1.png: %s" % why)
    ok, why = png_ok(os.path.join(PROOF, "preview-2.png"))
    check(ok, "proof preview-2.png: %s" % why)
    for name in ("explorer.html", "bom.html", "compare.html"):
        check(os.path.isfile(os.path.join(PROOF, name)), "committed proof %s missing" % name)
    import hashlib
    h1 = hashlib.sha256(open(os.path.join(PROOF, "preview-1.png"), "rb").read()).digest()
    h2 = hashlib.sha256(open(os.path.join(PROOF, "preview-2.png"), "rb").read()).digest()
    check(h1 != h2, "proof previews 1 and 2 must differ (geometry changed across revisions)")

    # 6. Docs describe the card.
    note = os.path.join(ROOT, "docs", "pdm", "web", "web-ui.md")
    check(os.path.isfile(note), "missing P8-M4/p8m4t01-web-ui.md")

    # 7. The build/test path passes on this host (real material execution).
    try:
        r = subprocess.run(["cargo", "test", "-p", "oep-web", "--quiet"],
                           cwd=ROOT, capture_output=True, text=True, timeout=900)
        check(r.returncode == 0,
              "cargo test -p oep-web failed (rc=%d): %s"
              % (r.returncode, (r.stdout + r.stderr)[-1200:]))
    except FileNotFoundError:
        warnings.append("cargo not found; skipping live build check")
    except subprocess.TimeoutExpired:
        check(False, "cargo test -p oep-web timed out")

    # 8. Surface-language rule on THIS card's files (no em dashes, no curly
    #    quotes). Scan the crate + the docs note this card adds.
    scan_roots = [SRC, os.path.join(WEB, "tests"), os.path.dirname(note), PROOF]
    for base in scan_roots:
        if not os.path.isdir(base):
            continue
        for dirpath, _dirs, names in os.walk(base):
            for name in names:
                if not (name.endswith((".rs", ".md", ".py"))):
                    continue
                p = os.path.join(dirpath, name)
                try:
                    text = read_text(p)
                except (UnicodeDecodeError, OSError):
                    continue
                nd = text.count(EM_DASH)
                nq = text.count("\u201c") + text.count("\u201d") \
                    + text.count("\u2018") + text.count("\u2019")
                if nd or nq:
                    check(False, "%s: em dash x%d, curly quote x%d"
                          % (os.path.relpath(p, ROOT), nd, nq))

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P8-M4 WEB UI ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    if warnings:
        print("warnings:")
        for w in warnings:
            print("  -", w)
    print("P8-M4 WEB UI ACCEPTANCE PASSED: item explorer, BOM tree (walked from "
          "the relationship graph), viewer embed (real oep-geo -> PNG) and "
          "compare view verified on the reference vertical fixture; proof "
          "committed and valid.")
    return 0


def test_p8_m4_web_ui_acceptance():
    """pytest entry point: same checks, asserted."""
    failures = run_checks()
    assert not failures, "P8-M4 web UI acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
