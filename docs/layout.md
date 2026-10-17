# Keelworks Monorepo Layout

Keelworks / Open Engineering Platform (OEP) is a monorepo at `don/keelworks`
(NAS Gitea). One crate per code workstream, under `crates/`; Python tooling
lives under `python/`. This file is the machine-verifiable contract checked by
`tests/monorepo_layout_test.py`.

Workstream -> crate/module map (anchor format: `P<lane> <crate>`):

- P2 oep-cgr        -- CGR reader (geometry/tessellation)
- P3 oep-3dxml      -- 3DXML reader/writer
- P6 oep-geo        -- geometry core + model representation
- P4 oep-step       -- STEP AP242/QIF/JT interop (reserved)
- P8 oep-pdm        -- self-hosted PDM / ENOVIA-lite; REST API v1 service skeleton
  (P8-M1-T01, Rust axum per P8-M1/web-stack-adr.md, AGPL-3.0; retained
  P7-M2-T02 StubServer identity + PdmClient), contract: P8-M6/openapi-v1.yaml,
  integration: P7-M2/pdm-integration.md

Not yet materialized as crates (laid out when their workstream starts):
P1 corpus tooling (python), P9 P2P compute, P10 hybrid cloud, P12 QA/CI,
P14 integration.

## Conventions

- Rust core: one `crates/oep-<stream>` crate per workstream, listed as a
  member of the root Cargo workspace. No cross-crate `path` deps until the
  P6 importer hub (P6-M4-T02) defines them.
- Python tooling/scripts: `python/oep_tooling/` package (corpus pipelines,
  P1 harnesses, MBD validator first cut). Python is for tooling only; core is
  Rust-first (decision 6).
- Docs: per-workstream notes under `docs/`. `layout.md` stays in sync with the
  workspace member list.
- License: posted per module by the P0-M2 legal pass. Coarse posture (decided
  2026-10-15): Apache-2.0 applications/libraries, AGPL-3.0 server/PDM + P2P.

## Reserving a new crate

1. Add the crate under `crates/oep-<stream>/` (Cargo.toml with
   `version.workspace = true`, `src/lib.rs`).
2. Append it to `[workspace] members` in the root `Cargo.toml`.
3. Add a `P<lane> oep-<stream>` line here and re-run
   `cargo build --workspace && cargo test --workspace`.
