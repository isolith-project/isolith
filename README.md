# Isolith

The public product brand for the Open Engineering Platform (OEP): an open,
self-hosted, interoperable engineering suite. "Keelworks" is the internal
program codename only and is never used on a public surface (it collides with
a registered mark; see `assets/brand/NAME.md` and decision record 0001).

OEP is an open-source alternative to CATIA-class CAD/CAE/PDM: readers for the
proprietary geometry files primes actually exchange (CGR, 3DXML, CATPart), a
geometry core, a self-hosted PDM layer, and P2P compute, with hybrid-cloud
delivery and no per-seat paywall.

## Start here

- Mission, non-goals, license posture: [charter.md](charter.md)
- Getting started, admin guide, honest format matrix (docs site):
  [docs-site/index.html](docs-site/index.html)
- Repo layout map: [docs/layout.md](docs/layout.md)
- Brand / identity kit: [assets/brand/](assets/brand/)

## Layout

- `crates/`    Rust product code (oep-cgr, oep-3dxml, oep-step, oep-geo,
               oep-pdm, oep-web, oep-docs, and the rest of the workspace)
- `docs-site/` static documentation site served by the oep-docs service
- `docs/`      milestone / workstream notes and the repo layout map
- `workstreams/`  workstream documentation, ADRs, and per-milestone evidence
               (the P0-Mx / P11 / P12 style directories live here, one level
               deep so the repo root stays clean)
- `assets/brand/` Isolith identity kit
- `corpus/`    dev-only reference geometry corpus (never shipped)
- `standards/` adopted public standards reference material
- `tests/` `scripts/` `python/` acceptance tests, runbooks, release tooling
- `.gitea/`    CI/CD workflows

## Build and test

```sh
cargo build --workspace && cargo test
uv run pytest tests/
```

The pre-commit quality gate (scripts, CI wiring) is in `ci/`.

## Repo policy

Two repositories only: the canonical development repo and `keelworks-canary`.
Feature work merges into `main`; `release/N` branches are frozen trains cut
from `main`; every wave closes with exactly one checkpoint commit onto `main`.
See `charter.md` and the P12 policy docs on main for the details.
