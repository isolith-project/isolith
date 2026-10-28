# Isolith docs site (P13-T03, SITE-DOMAIN-ORIGIN)

The static documentation site for Isolith: getting-started, the admin/install
guide, the honest format capability matrix, the generated "what it does not
do" boundary, download, releases, license, security, and contact pages. It is
served by the P10 delivery stack itself via the `docs` compose service
(`oep-docs`, a zero-dependency std-only static server) on the same image as
the PDM, viewer and P2P services, so the site renders in an air-gapped
browser with no external assets.

## Files

    site.json             SITE_ORIGIN and site identity settings (see below)
    index.html            landing + what the stack is
    getting-started.html  the eight-step verified install checklist + first use
    admin-guide.html      online + air-gap install, TLS, backup, DR, update posture
    format-matrix.html    the honest format capability table (rendered)
    format-matrix.json    machine-checkable copy of the table + truth anchors
    downloads.html        desktop downloads page (rows rendered from downloads.json)
    downloads.json        machine-readable downloads manifest (schema isolith.
                          downloads-manifest.v1: platform, artifact, version,
                          sha256, signature_id) - the site never hard-codes
                          asset names
    what-it-does-not-do.html  GENERATED from format-matrix.json gaps by
                          scripts/site-build.py --regen-what-not (never handwritten)
    download.html         installers via GitHub Releases + SHA-256 checksums +
                          the one-paragraph export-control statement
    releases.html         calendar train, current release, changelog, support window
    license.html          the precise Apache-2.0 / AGPL-3.0 boundary
    security.html         points at SECURITY.md and the private disclosure path
    contact.html          hello@ / security@ / conduct@ (email aliases only)
    assets/style.css      self-contained styling (no external assets, air-gap safe)
    assets/images/        real captures used on the landing page
    assets/media-manifest.md  provenance for every capture
    proof/                committed QA output (format-matrix-check.json)

## SITE_ORIGIN and site generation

The public origin is not hard-coded in the pages. `docs-site/site.json`
carries the settings that site generation consumes:

- `site.json`.`site_origin` - the SITE_ORIGIN value (empty in the committed
  tree = air-gap build; `https://isolith.dev` is what the public build uses).
- `repo_url` - the single placeholder constant for the repository URL. While
  the GitHub migration is pending it stays a placeholder so no account name
  is invented; pages carry the `SITE-REPO-URL` token exactly like
  SECURITY.md carries a placeholder until the real value lands.
- `checksums` - the committed SHA-256 values shown on the download page.

`scripts/site-build.py` is the single consumer of these settings:

    python3 scripts/site-build.py                       # air-gap check: fail if
                                                        # any committed page has an
                                                        # absolute origin/canonical
    python3 scripts/site-build.py --regen-what-not      # regenerate what-it-does-not-do.html
                                                        # from format-matrix.json
    python3 scripts/site-build.py --public --origin https://isolith.dev \
        --out /tmp/site-public                          # inject canonicals, write
                                                        # sitemap.xml + robots.txt

Air-gap build: with SITE_ORIGIN empty the committed tree is served as-is with
relative links and no absolute origin. That is what `oep-docs` inside the
delivery image serves, and the air-gap check enforces it in CI.

## How it is served

Public: a Caddy vhost on the Hetzner VPS serves a static export of the public
build (canonical tags, sitemap.xml, robots.txt) with automatic TLS and HSTS.
No dynamic PDM, viewer or compute surface is exposed publicly. The exact DNS
records and the production Caddy block are in
`docs-site/DNS-AND-PROD-APPLY.md`.

Delivery stack: `workstreams/delivery/deploy/deploy/docker-compose.yml`
defines the `docs` service (`oep-docs --root /srv/docs --addr 0.0.0.0:8792`);
the site is copied into the stack image under `/srv/docs` and served with a
health surface `GET /healthz`. Host binds default to 127.0.0.1.

## How it is verified

- `test_p13t03_docs_site.py` validates the four historical pages + the format
  matrix schema/taxonomy and runs the format-matrix QA script.
- `scripts/site-build.py` (no args) is the air-gap/SITE_ORIGIN check: it fails
  if a committed page hard-codes an absolute origin or carries a canonical
  tag. It runs on every push (see `.gitea/workflows/ci.yaml`).
- `scripts/format-matrix-check.py` is the format table QA gate (see its own
  docstring); exit 0 iff no FAIL row. The "what it does not do" page is
  regenerated from the same matrix by site-build and asserted in sync by the
  acceptance test.

Serve locally without Docker for a quick status:

    python3 scripts/site-build.py                       # air-gap check must pass
    cargo run -p oep-docs -- --root docs-site --addr 127.0.0.1:8792

Then `curl 127.0.0.1:8792/`, `/getting-started.html`, `/admin-guide.html`,
`/format-matrix.html`, `/download.html`, `/releases.html`, `/license.html`,
`/security.html`, `/contact.html`, `/what-it-does-not-do.html`, and
`/healthz`.
