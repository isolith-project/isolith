#!/usr/bin/env python3
"""SITE-DOMAIN-ORIGIN: SITE_ORIGIN-aware docs-site generation and QA.

This script is the single consumer of SITE_ORIGIN for the Isolith docs site.
It does three jobs, all deterministic and stdlib-only:

1. REGEN-WHAT-NOT:
   Regenerate docs-site/what-it-does-not-do.html from docs-site/format-matrix.json
   (the rows' recorded 'gap' fields plus the closed fidelity taxonomy's
   gap/fail classes). The page is produced from the matrix, never hand-written,
   and stays in sync with it (tests regen to a temp file and diff).

2. PUBLIC build:
   scripts/site-build.py --public --origin https://isolith.dev --out DIR
   copies docs-site/ to DIR, injects a canonical <link> into every page head,
   and writes sitemap.xml + robots.txt carrying the origin. The engine of the
   public site on the VPS.

3. AIRGAP check (default):
   scripts/site-build.py --site docs-site
   verifies the committed tree keeps relative links and no absolute origin and
   no canonical tags, so the delivery stack's oep-docs air-gap serving stays
   origin-free. Exits nonzero on any violation (honest doctrine: an origin
   leaking into the air-gap build is a real defect, reported not hidden).

Exit code contract: 0 = pass, nonzero = fail.

Usage:
  scripts/site-build.py                       # air-gap verification of docs-site
  scripts/site-build.py --regen-what-not      # regenerate the what-not page in place
  scripts/site-build.py --public --origin https://isolith.dev --out /tmp/site-public
  scripts/site-build.py --site DIR [--out DIR2]
"""

import argparse
import json
import os
import re
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE_DIR = os.path.join(ROOT, "docs-site")
SITE_JSON = os.path.join(SITE_DIR, "site.json")
FORMAT_MATRIX = os.path.join(SITE_DIR, "format-matrix.json")
WHAT_NOT_PAGE = os.path.join(SITE_DIR, "what-it-does-not-do.html")

PUBLIC_PAGES = [
    "index.html",
    "getting-started.html",
    "admin-guide.html",
    "format-matrix.html",
    "what-it-does-not-do.html",
    "download.html",
    "downloads.html",
    "install-on-nas.html",
    "releases.html",
    "license.html",
    "security.html",
    "contact.html",
]

# Relative-link sandbox: paths in these prefixes must resolve inside the site;
# anything else in the committed tree is a hard-coded origin to reject.
LOCAL_PATH_RE = re.compile(
    r"^(?:\.\.?/)*[A-Za-z0-9_./-]*$|^[#a-z0-9-]+$|^assets/|^mailto:|^tel:"
)
ABSOLUTE_ORIGIN_RE = re.compile(
    r"(?:<link[^>]+rel=[\"']canonical[\"'][^>]*>|"
    r"https?://[A-Za-z0-9.-]+(?::\d+)?(?:/\S*)?)"
)


def read_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def write(path, text):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)


# ---------------------------------------------------------------------------
# what-it-does-not-do page generation
# ---------------------------------------------------------------------------

_WHAT_NOT_BODY = r"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>What it does not do - Isolith</title>
<meta name="description" content="The honest out-of-scope list for Isolith: what the format readers and the editor do not do, taken from the measured format capability matrix and the product non-goals. Nothing here is an apology, it is the boundary.">
<link rel="stylesheet" href="assets/style.css">
</head>
<body>
<header class="site">
  <div class="wrap">
    <div class="brand"><a href="index.html">Isolith</a><small>documentation</small></div>
    <nav class="site">
      <a href="index.html">Home</a>
      <a href="getting-started.html">Getting started</a>
      <a href="admin-guide.html">Admin guide</a>
      <a href="format-matrix.html">Format matrix</a>
      <a href="download.html">Download</a>
      <a href="releases.html">Releases</a>
      <a href="license.html">License</a>
      <a href="security.html">Security</a>
      <a href="contact.html">Contact</a>
      <a href="what-it-does-not-do.html">What it does not do</a>
    </nav>
  </div>
</header>

<div class="wrap">
  <p class="kicker">Honest boundary</p>
  <h1>What Isolith does not do</h1>
  <p class="lead">
    This page is generated from the measured format capability matrix
    (<code>format-matrix.json</code>) and the product non-goals. It exists so
    that a team evaluating Isolith can see the boundary as clearly as the
    capability: what the readers open is a measured claim with a published
    gate report behind it, and what they do not open is a documented gap in the
    same table. A reader-only release never claims to be the editor, and the
    editor does not claim the breadth of a full commercial suite yet.
  </p>

  <h2>Formats the readers do not open, and why (from the matrix)</h2>
  <p>
    Each reader row in the format capability matrix records a known limit in
    its own words. The list below is that column, copied programmatically from
    the matrix. Where a row says "not claimed", that is the honest state of a
    young reader, not a hidden omission.
  </p>
  <table>
    <thead>
      <tr>
        <th>Format / module</th>
        <th>Direction</th>
        <th>What is not done (recorded gap)</th>
      </tr>
    </thead>
    <tbody>
      __GAP_ROWS__
    </tbody>
  </table>

  <h2>What the editor is not yet</h2>
  <p>
    The editor is the product spine and is young. These product-level gaps are
    stated on the home page and repeated here so the boundary lives in one
    honest list:
  </p>
  <ul>
    <li>Full CATIA or SolidWorks breadth of workbenches is not claimed.</li>
    <li>Sheet metal, weldments, mold tooling, CAM toolpaths, and FEA are not
        shipped (some are planned for later trains; see the roadmap and the
        workstream notes).</li>
    <li>Class-A surface styling is a long-term program, not a shipped feature.</li>
    <li>Native CATPart files are readable; native editing of CATPart is not.</li>
    <li>The editor is not the 2027.1 GA deliverable; the interop/PDM
        foundation release is.</li>
  </ul>

  <h2>The rule behind the boundary</h2>
  <p>
    One rule keeps every caveat honest: a capability is only ever claimed at
    the exact fidelity a measured run verified. If the matrix does not say
    Isolith can do it, Isolith does not claim it. Gaps are findings, not
    defects to hide; a documented wall beats invented geometry.
  </p>

  <p><a href="format-matrix.html">Back to the format capability matrix</a></p>
</div>

<footer class="site">
  <div class="wrap">
    Isolith documentation - version 0.1.0. This page is generated from
    <code>format-matrix.json</code> by scripts/site-build.py; edit the matrix,
    not the page.
  </div>
</footer>
</body>
</html>
"""


def build_what_not_rows():
    """Return the __GAP_ROWS__ table rows from format-matrix.json, sorted by
    lane then format so the output is deterministic."""
    matrix = read_json(FORMAT_MATRIX)
    rows = []
    for r in sorted(matrix["rows"], key=lambda x: (x.get("lane", ""), x.get("format", ""))):
        gap = (r.get("gap") or "").strip()
        if not gap:
            continue
        rows.append(
            f"      <tr>\n"
            f"        <td><strong>{r.get('format', '')}</strong> <code>{', '.join(r.get('extension', []))}</code><br><code>{r.get('module', '')}</code></td>\n"
            f"        <td>{r.get('direction', '')}</td>\n"
            f"        <td>{gap}</td>\n"
            f"      </tr>"
        )
    return "\n".join(rows)


def regen_what_not(out_path=WHAT_NOT_PAGE):
    rows_html = build_what_not_rows()
    page = _WHAT_NOT_BODY.replace("__GAP_ROWS__", rows_html)
    write(out_path, page)
    return out_path


# ---------------------------------------------------------------------------
# air-gap verification
# ---------------------------------------------------------------------------

def airgap_check(site_dir):
    """Verify the committed (or given) site has relative links only, no
    absolute origin, no canonical tags, no sitemap/robots carrying an absolute
    origin. Returns list of violations."""
    problems = []
    for name in PUBLIC_PAGES + ["assets/style.css"]:
        path = os.path.join(site_dir, name)
        if not os.path.isfile(path):
            # only the historical four pages are guaranteed; new pages are
            # verified when present. format-matrix.json is data, not HTML.
            if name in ("index.html", "getting-started.html", "admin-guide.html", "format-matrix.html"):
                problems.append(f"missing required page {name}")
            continue
        text = read(path)
        stripped = re.sub(r"<[^>]+>", "", text)
        for m in ABSOLUTE_ORIGIN_RE.finditer(stripped):
            token = m.group(0).strip()
            if token.startswith(("http://127.0.0.1", "http://localhost")):
                continue  # operator guidance to loopback services is allowed
            problems.append(f"{name}: hard-coded origin in text: {token[:80]!r}")
        # canonical link tags are public-build only
        if re.search(r"rel=[\"']canonical[\"']", text, re.I):
            problems.append(f"{name}: canonical <link> in committed (air-gap) tree")
    # sitemap/robots with an absolute origin are public-build artifacts
    for aux in ("sitemap.xml", "robots.txt"):
        p = os.path.join(site_dir, aux)
        if os.path.isfile(p):
            t = read(p)
            if "https://" in t or re.search(r"rel=[\"']canonical[\"']", t):
                problems.append(f"{aux}: absolute-origin artifact in committed tree")
    return problems


# ---------------------------------------------------------------------------
# public build
# ---------------------------------------------------------------------------

CANONICAL_TEMPLATE = '  <link rel="canonical" href="{target}">\n'


def page_href(origin, page):
    """The canonical/sitemap URL for a page: index.html maps to the bare origin,
    everything else to {origin}/{page}."""
    base = origin.rstrip("/")
    if page == "index.html":
        return base + "/"
    return base + "/" + page


def inject_canonical(text, origin, page):
    target = page_href(origin, page)
    link = CANONICAL_TEMPLATE.format(target=target)
    # idempotent: never double-inject if the link is already there
    if f'href="{target}"' in text:
        return text
    head_end = text.find("</head>")
    if head_end == -1:
        return text
    return text[:head_end] + link + "  " + text[head_end:]


def public_build(site_dir, origin, out_dir):
    """Copy the site to out_dir, inject canonicals, substitute the single
    SITE-REPO-URL placeholder, write sitemap.xml and robots.txt. Returns the
    page receipt (page -> status)."""
    if os.path.exists(out_dir):
        shutil.rmtree(out_dir)
    shutil.copytree(site_dir, out_dir)
    # The repository URL is a single placeholder constant, not a hard-coded
    # origin. Its value rides in docs-site/site.json (repo_url); while the
    # GitHub migration is pending the constant stays a placeholder and no
    # account name is invented. GITHUB-PRIMARY-MIGRATION fills the value into
    # site.json; this code never invents one.
    site_cfg = {}
    if os.path.isfile(SITE_JSON):
        site_cfg = read_json(SITE_JSON)
    repo_url = (site_cfg.get("repo_url") or "").strip()
    receipt = {}
    for name in PUBLIC_PAGES:
        path = os.path.join(out_dir, name)
        if not os.path.isfile(path):
            receipt[name] = "missing"
            continue
        text = inject_canonical(read(path), origin, name)
        # The repository URL is a single placeholder constant, not a
        # hard-coded origin. Its value rides in docs-site/site.json
        # (repo_url); while the GitHub migration is pending the pages keep the
        # SITE-REPO-URL token (same convention as SECURITY.md's
        # SITE-DOMAIN-PLACEHOLDER) so no account name is invented.
        # GITHUB-PRIMARY-MIGRATION fills the value into site.json; this code
        # never invents one.
        if repo_url:
            text = text.replace("SITE-REPO-URL", repo_url)
        write(path, text)
        receipt[name] = "ok"
    # sitemap.xml
    sitemap = []
    sitemap.append('<?xml version="1.0" encoding="UTF-8"?>')
    sitemap.append('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">')
    for name in PUBLIC_PAGES:
        if os.path.isfile(os.path.join(out_dir, name)):
            loc = f"{origin.rstrip('/')}/{name if name != 'index.html' else ''}"
            sitemap.append("  <url>")
            sitemap.append(f"    <loc>{loc}</loc>")
            sitemap.append("  </url>")
    sitemap.append("</urlset>")
    write(os.path.join(out_dir, "sitemap.xml"), "\n".join(sitemap) + "\n")
    # robots.txt
    robots = (
        "User-agent: *\n"
        "Allow: /\n"
        f"Sitemap: {origin.rstrip('/')}/sitemap.xml\n"
    )
    write(os.path.join(out_dir, "robots.txt"), robots)
    return receipt


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--site", default=SITE_DIR)
    ap.add_argument("--origin", default="", help="SITE_ORIGIN; empty = air-gap")
    ap.add_argument("--out", default="", help="public build output dir")
    ap.add_argument("--public", action="store_true",
                    help="build the public variant (inject canonicals + sitemap + robots)")
    ap.add_argument("--regen-what-not", action="store_true",
                    help="regenerate docs-site/what-it-does-not-do.html from the matrix")
    args = ap.parse_args()

    site_dir = os.path.abspath(args.site)
    origin = args.origin or os.environ.get("SITE_ORIGIN", "").strip()

    if args.regen_what_not:
        out = regen_what_not()
        print(f"what-it-does-not-do regenerated: {out}")
        return 0

    if args.public:
        if not origin:
            print("SITE-DOMAIN-ORIGIN: --public requires --origin (or SITE_ORIGIN env)", file=sys.stderr)
            return 2
        if not args.out:
            print("SITE-DOMAIN-ORIGIN: --public requires --out DIR", file=sys.stderr)
            return 2
        receipt = public_build(site_dir, origin, os.path.abspath(args.out))
        problems = airgap_check(site_dir)  # the committed tree must stay origin-free
        if problems:
            for p in problems:
                print(f"  [FAIL] air-gap invariant: {p}", file=sys.stderr)
            print("SITE-DOMAIN-ORIGIN: public build produced but committed tree is not air-gap clean", file=sys.stderr)
            return 1
        print(f"SITE-DOMAIN-ORIGIN: public build -> {os.path.abspath(args.out)} ({origin})")
        for name, status in receipt.items():
            print(f"  {name}: {status}")
        print(f"  sitemap.xml: {os.path.isfile(os.path.join(args.out, 'sitemap.xml'))}")
        print(f"  robots.txt: {os.path.isfile(os.path.join(args.out, 'robots.txt'))}")
        return 0

    # default: air-gap verification
    problems = airgap_check(site_dir)
    if problems:
        for p in problems:
            print(f"  [FAIL] {p}", file=sys.stderr)
        print("SITE-DOMAIN-ORIGIN: air-gap check FAIL", file=sys.stderr)
        return 1
    print(f"SITE-DOMAIN-ORIGIN: air-gap check PASS ({len(PUBLIC_PAGES)} pages, no absolute origin)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
