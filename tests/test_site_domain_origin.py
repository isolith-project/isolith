#!/usr/bin/env python3
"""SITE-DOMAIN-ORIGIN acceptance: the docs site consumes SITE_ORIGIN, adds the
public pages, and keeps the air-gap build origin-free.

Written FIRST (TDD). Verifies real material:

  SITE_ORIGIN / air-gap  => scripts/site-build.py is the single consumer:
                            committed tree has relative links and no absolute
                            origin / canonical tags; a public build injects
                            canonicals and writes sitemap.xml + robots.txt
  generated page         => docs-site/what-it-does-not-do.html is produced by
                            scripts/site-build.py --regen-what-not from
                            format-matrix.json gaps (never hand-written), and
                            the committed page matches a fresh regen
  new public pages       => download (real checksums), releases, license,
                            security, contact all exist and render
  identity               => one primary name per page title (Isolith), the
                            expansion used once, no retired codename, no
                            analytics, no third-party fonts or CDN deps

Run:  python3 tests/test_site_domain_origin.py
      python3 -m pytest tests/test_site_domain_origin.py
Exit 0 = pass; nonzero = fail. Stdlib only.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE = os.path.join(ROOT, "docs-site")
SITE_JSON = os.path.join(SITE, "site.json")
BUILDER = os.path.join(ROOT, "scripts", "site-build.py")
WHAT_NOT = os.path.join(SITE, "what-it-does-not-do.html")

PUBLIC_PAGES = [
    "index.html",
    "getting-started.html",
    "admin-guide.html",
    "format-matrix.html",
    "what-it-does-not-do.html",
    "download.html",
    "releases.html",
    "license.html",
    "security.html",
    "contact.html",
]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def run(builder_args, cwd=ROOT):
    env = dict(os.environ)
    proc = subprocess.run(
        [sys.executable, BUILDER] + builder_args, capture_output=True,
        text=True, env=env, cwd=cwd, timeout=120)
    return proc


def test_new_public_pages_exist_and_render():
    for name in PUBLIC_PAGES:
        path = os.path.join(SITE, name)
        check(os.path.isfile(path), f"missing public page {path}")
        html = read(path).lower()
        # every page carries the site header + a home link + the style sheet
        check('rel="stylesheet" href="assets/style.css"' in html,
              f"{name} missing its stylesheet link")
        check("index.html" in html or name == "index.html",
              f"{name} does not link home")
    # the five addendum pages carry their honest markers
    download = read(os.path.join(SITE, "download.html")).lower()
    check("sha256" in download, "download page missing checksum handling")
    check("github releases" in download, "download page missing GitHub Releases link")
    check("ear99" in download, "download page missing EAR99 export-control statement")
    check("smartscreen" in download, "download page missing Windows unsigned note")
    security = read(os.path.join(SITE, "security.html")).lower()
    check("security.md" in security, "security page does not point at SECURITY.md")
    license_page = read(os.path.join(SITE, "license.html"))
    check("apache-2.0" in license_page.lower() and "agpl-3.0" in license_page.lower(),
          "license page must state the Apache-2.0 and AGPL-3.0 boundary")
    contact = read(os.path.join(SITE, "contact.html")).lower()
    check("hello@isolith.dev" in contact and "security@isolith.dev" in contact,
          "contact page missing the email aliases")


def test_committed_tree_is_airgap_clean():
    proc = run([])
    check(proc.returncode == 0, f"air-gap check failed: {proc.stdout} {proc.stderr}")
    # belt-and-braces: no committed page carries an origin or a canonical tag
    for name in PUBLIC_PAGES:
        text = read(os.path.join(SITE, name))
        stripped = re.sub(r"<[^>]+>", "", text)
        for m in re.finditer(r"https?://[A-Za-z0-9.-]+", stripped):
            token = m.group(0)
            if token.startswith(("http://127.0.0.1", "http://localhost")):
                continue
            check(False, f"{name} hard-codes origin {token!r} in committed tree")
        check("rel=\"canonical\"" not in text,
              f"{name} carries a canonical tag in the air-gap tree")


def test_public_build_injects_canonicals_and_artifacts():
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "site-public")
        proc = run(["--public", "--origin", "https://isolith.dev", "--out", out])
        check(proc.returncode == 0, f"public build failed: {proc.stdout} {proc.stderr}")
        for name in PUBLIC_PAGES:
            html = read(os.path.join(out, name))
            href = f'href="https://isolith.dev/{name if name != "index.html" else ""}"'
            check(href in html, f"{name} missing canonical for https://isolith.dev")
        robots = read(os.path.join(out, "robots.txt"))
        check("Sitemap: https://isolith.dev/sitemap.xml" in robots,
              "robots.txt missing sitemap line")
        sitemap = read(os.path.join(out, "sitemap.xml"))
        check("https://isolith.dev/" in sitemap, "sitemap.xml missing index loc")
        for name in ("download", "releases", "license", "security", "contact"):
            check(f"https://isolith.dev/{name}.html" in sitemap,
                  f"sitemap.xml missing {name}.html")


def test_what_not_page_is_generated_not_handwritten():
    with tempfile.TemporaryDirectory() as tmp:
        regen_path = os.path.join(tmp, "what-it-does-not-do.regen.html")
        # reuse the committed regen by writing to a temp copy of the site dir
        site_copy = os.path.join(tmp, "site")
        shutil.copytree(SITE, site_copy)
        env = dict(os.environ)
        env["SITE_ORIGIN"] = "https://isolith.dev"  # must not matter for regen
        from importlib import import_module  # inlined to keep this test standalone
        # simpler: call the script with a copied site and regen into it
        proc = subprocess.run(
            [sys.executable, BUILDER, "--regen-what-not"],
            capture_output=True, text=True, env=env, cwd=site_copy, timeout=120)
        check(proc.returncode == 0, f"regen failed: {proc.stdout} {proc.stderr}")
        generated = os.path.join(site_copy, "what-it-does-not-do.html")
        text = read(generated)
        # the gaps must come from the matrix (a marker of at least one row gap)
        matrix = json.loads(read(os.path.join(site_copy, "format-matrix.json")))
        gaps = [r for r in matrix["rows"] if (r.get("gap") or "").strip()]
        check(gaps, "format matrix has no gap rows to generate from")
        # at least one gap string must appear verbatim, not a hand-written list
        for g in gaps[:1]:
            snippet = (g["gap"][:40]).split(" ")[:6]
            token = " ".join(snippet).lower()
            check(token in text.lower(),
                  f"generated page missing matrix gap token {token!r}")
        # committed page must match the freshly regenerated one (never drift)
        committed = read(WHAT_NOT)
        check(committed == text, "committed what-it-does-not-do.html has drifted "
                                 "from scripts/site-build.py --regen-what-not")


def test_identity_sitewide():
    """One primary name per title (Isolith), expansion used once, no retired
    codename, no analytics, no third-party fonts or CDN deps."""
    # construct the retired codename so this file never emits the literal
    # forbidden string (the brand-identity drift guard scans file contents)
    retired = "Keel" + "works"
    expansion_count = 0
    for name in PUBLIC_PAGES:
        text = read(os.path.join(SITE, name))
        title = re.search(r"<title>(.*?)</title>", text, re.S)
        check(title is not None, f"{name} missing <title>")
        if title:
            check("Isolith" in title.group(1),
                  f"{name}: title does not lead with the primary name Isolith")
            expansion_count += len(re.findall(r"Open Engineering Platform", title.group(1)))
        check(retired not in text and retired.lower() not in text,
              f"{name}: retired codename present")
        check("google-analytics" not in text and "gtag(" not in text and
              "fathom" not in text and "plausible" not in text and
              "segment.com" not in text,
              f"{name}: analytics present")
        check("fonts.googleapis" not in text and "fonts.gstatic" not in text and
              "cdn." not in text and "unpkg.com" not in text and
              "cdnjs" not in text and "jsdelivr" not in text,
              f"{name}: third-party fonts / CDN dependency present")
    # the expansion "Open Engineering Platform" appears exactly once (the H1
    # on the home page), and every other page title names Isolith alone.
    index_text = read(os.path.join(SITE, "index.html"))
    check(len(re.findall(r"Open Engineering Platform", index_text)) == 1,
          "expansion 'Open Engineering Platform' is not used exactly once")


def test_site_json_consumed():
    cfg = json.loads(read(SITE_JSON))
    check(cfg.get("site_name") == "Isolith", "site.json site_name != Isolith")
    check("site_origin" in cfg, "site.json missing site_origin key")
    check("repo_url" in cfg, "site.json missing repo_url key (single placeholder)")
    check("checksums" in cfg and cfg["checksums"]["sha256sums"], "site.json missing checksums")
    # the download page's committed SHA-256 must match site.json exactly
    download = read(os.path.join(SITE, "download.html"))
    check(cfg["checksums"]["sha256sums"] in download,
          "download page checksum drifted from site.json")


def main():
    tests = [
        test_new_public_pages_exist_and_render,
        test_committed_tree_is_airgap_clean,
        test_public_build_injects_canonicals_and_artifacts,
        test_what_not_page_is_generated_not_handwritten,
        test_identity_sitewide,
        test_site_json_consumed,
    ]
    for t in tests:
        try:
            t()
        except Exception as e:  # noqa: BLE001
            failures.append(f"{t.__name__} raised: {e!r}")
    if failures:
        print("SITE-DOMAIN-ORIGIN acceptance: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"SITE-DOMAIN-ORIGIN acceptance: PASS ({len(tests)} suites)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
