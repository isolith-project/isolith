# P13-T03 - Docs site: getting-started, admin guide, honest format matrix

Status: delivered 2026-10-18. Owner: P13 community/docs/sovereignty-narrative
lane (release-surface slice pulled forward). Card: t_a3444dca on the Keelworks
board. Version: 1.

## 1. What this delivers

The documentation site that a new team reads before (and after) they install
the Open Engineering Platform stack, served by the P10 delivery stack itself
no "separate docs infra":

- `docs-site/` - a self-contained static site (plain HTML/CSS, no framework,
  no external assets, air-gap safe):
  - `index.html`          - what OEP is, the four-service stack, posture.
  - `getting-started.html`- the eight-step verified install checklist and
    first-use actions (the same checklist the P10 GA gate verified on two
    machines).
  - `admin-guide.html`    - install/ops guide: online one-command path AND the
    signed air-gap path (P10-M2), TLS (P10-M1-T02), checksummed daily backup,
    quarterly DR (P10-M3), and the advisory/off-by-default update posture
    (decision 11).
  - `format-matrix.html`  - the honest format capability matrix: what each
    reader opens and at what fidelity, every row bound to a committed P2/P3/P4
    GA report with its known limits stated as rows of their own.
  - `format-matrix.json`  - machine-checkable copy of the table + the truth
    anchors the QA spot-check asserts.
- `crates/oep-docs` - a zero-dependency, std-only static file server
  (healthz contract `GET /healthz`). It is a fourth service in the SAME
  `keelworks/oep-stack` image and compose file as PDM/viewer/P2P
  ("docs served from the sum"): `workstreams/delivery/deploy/deploy/{Dockerfile,
  docker-compose.yml, install.sh, .env.example}` were extended additively.
  The P10-M5 GA-gate surface (the three core services) is unchanged; docs is
  an addition with its own health endpoint and its own port (OEP_DOCS_PORT,
  default 8792).
- `scripts/format-matrix-check.py` - the QA gate: closed-set fidelity
  taxonomy, page/JSON sync, overclaim-token guard, and spot-checks of the
  truth anchors against the committed P2/P3/P4 GA matrices (honest exit-code
  contract: 0 iff no FAIL; absent ground truth classifies SKIP, never a
  fabricated PASS).
- `tests/test_p13t03_docs_site.py` - TDD acceptance suite (written before the
  content, RED then GREEN).

## 2. Honest-format-matrix doctrine

The capability table uses a closed set of fidelity classes (mesh-approximate,
surfaces-to-mesh, brep-topology, pmi-semantic, accuracy-within-eps, ... plus
pmi-reader-gap and jt-visualization as honest limit rows), and every numeric
claim is spot-checked against the committed GA matrices:

- P2-M6-T03 GA gate report  (total=24 pass=1 open=11 fail=0 skip=12; the
  old-era MeshNotFound wall is stated as a limit, not hidden).
- P3-M5-T02 GA gate report  (total=18 pass=9 open=7 fail=0 skip=2; topology
  goldens Star/Dolphin/Baby/Hand/Octopus/AttributeAll).
- P4-M5-T03 GA gate matrix  (fidelity cells=64 PASS 14 / FAIL 2 / SKIP 48;
  keelworks-box hausdorff=0 vol_rel_err=0 eps 1e-9) plus the P4-M2-T03 PMI
  fidelity matrix that names the two FAIL topics: circular runout and
  tolerance modifiers under step-io 0.2.5 - documented gaps, never relabeled.

Sovereignty-narrative language: factual ("data stays on your network / under
your control", "air-gap is a first-class install option", "clean-room readers,
lawful observation"), no vendor attacks anywhere.

## 3. Verification

Real, measured, committed under `workstreams/docs-site/proof/`:

- Site rendered from the repo by the `oep-docs` binary: `/`,
  `/getting-started.html`, `/admin-guide.html`, `/format-matrix.html`,
  `/format-matrix.json`, `/assets/style.css` all HTTP 200 with their markers;
  `/healthz` 200 `{"status":"ok"}`; missing path 404; traversal rejected 404.
- `scripts/format-matrix-check.py` run with the committed P2/P3/P4 GA
  ground truth present: 8/8 PASS, exit 0 (proof json committed).
- pytest acceptance: 4/4 passed; `cargo test` on the workspace: exit 0;
  `oep-docs` unit suite 6/6; rustfmt clean; clippy -D warnings clean.
- `docker compose config`: four services (docs/pdm/viewer/p2p) on the one
  stack image.
- Dockerfile builds the stack image including the docs binary + site.

## 4. Live surface

An operator who wants `docs.openengineeringplatform.example` points that vhost
(CNAME/nginx/caddy or /etc/hosts + TLS) at the docs port on the P10 host, per
their own network policy. The stack serves the site from `GET :8792/` with a
`/healthz` health surface - no separate docs server, repo, or host.

## 5. Files changed

see PR. New: docs-site/, crates/oep-docs, scripts/format-matrix-check.py,
tests/test_p13t03_docs_site.py, docs/layout.md line, workstreams/docs-site/. Modified
additively: workstreams/delivery/deploy/deploy/{Dockerfile, docker-compose.yml, install.sh,
.env.example}. The P10-M5 GA gate surface (three core services + their
install.sh contract) is preserved unchanged.
