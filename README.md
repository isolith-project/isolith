# Isolith

Isolith is an open-source, self-hosted engineering platform for teams that
need to own their own CAD data. It reads the proprietary geometry files
sub-tier suppliers actually exchange (CGR, 3DXML, CATPart), gives you a
geometry core, a self-hosted PDM layer, and P2P compute, and replaces the
per-seat paywall with software you run yourself.

The name Isolith is the public product brand. The internal program codename is
"Keelworks", which is never used on a public surface because it collides with
a registered mark (see `assets/brand/NAME.md` and decision record 0001).

## Why it exists

Commercial CAD/PLM gives you the formats and the platform the vendor controls.
Interop readers are expensive and subscription-gated, native files are
proprietary, and for defense and aerospace supply chains the exported models
can carry data the customer does not want in a foreign cloud. Isolith is built
for that gap: sovereign, self-hosted, ITAR-clean interchange, with the
documented formats (STEP AP242, QIF, JT) covered and the proprietary exchange
formats read first, because that is where engineering teams actually bleed.

## Status

Wave 3 (release-readiness) is complete. The 2027.1 train candidate
`2027.1.0-rc2` is cut from `main`, signed, and published to the canary
repository; `main` is the integrated development head. 2027.1.0-rc1 did not
carry the docs site or brand kit, rc2 does. The Program is private on the NAS
Gitea until a public release is approved.

## Quick start

The whole stack (PDM, viewer, P2P dashboard, docs site) runs from one
command, either as containers or from source.

```sh
# Containers: from the deploy directory on a machine with docker
cd workstreams/P10-M1/deploy
./install.sh            # builds/loads the stack image, starts all services

# From source
cargo build --release
./target/release/oep-pdm       # REST API on http://127.0.0.1:8787
./target/release/oep-docs      # docs site on http://127.0.0.1:8792
```

Stop with `./install.sh stop`. The getting-started guide, admin guide, and the
honest format matrix are served by the docs site itself (see the docs section
below).

## What's inside

- Readers for the interchange formats primes rely on: CGR (triangle
  tessellation), 3DXML, and STEP AP242 with semantic PMI; QIF and JT interop
  are in the program scope.
- Geometry core (`oep-geo`): one canonical model, golden-tested against real
  material, not a toy.
- Self-hosted PDM (`oep-pdm`): items, revisions, working sets, RBAC and
  tamper-evident audit, REST API, secure by default.
- P2P compute lane: job spec, scheduler, sandboxed render/convert workers with
  content-hash-verified outputs.
- Preview viewer and a static docs site, both part of the stack.

Honest limits are stated, not hidden: the native CATPart reader is a gated
two-year scope, and the format matrix in the docs site shows exactly what each
reader does and does not support.

## Build and test

```sh
cargo build --workspace && cargo test
uv run pytest tests/
```

The repo layout map lives in `docs/layout.md`; workstream records (ADRs,
evidence, milestones) live under `workstreams/`. The pre-commit quality gate is
in `ci/`.

## Contributing and license

- Charter (mission, non-goals, license posture): `charter.md`
- Contribution guidelines: `CONTRIBUTING.md`
- Identity kit (mark, palette, usage rules, trademark evidence):
  `assets/brand/`

License: Apache-2.0 for applications and libraries; AGPL-3.0 for the
server/PDM and P2P orchestration components. No GPL-3.0 ships in delivered
software. Exports are classified per release (ITAR/ECCN posture).

## Documentation

- Docs site (getting started, admin guide, honest format matrix):
  `docs-site/index.html`, served by `oep-docs` in the stack
- Repo layout: `docs/layout.md`
