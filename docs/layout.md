# Keelworks Monorepo Layout

Keelworks / Open Engineering Platform (OEP) is a monorepo at `don/keelworks`
(NAS Gitea). One crate per code workstream, under `crates/`; Python tooling
lives under `python/`. This file is the machine-verifiable contract checked by
`tests/monorepo_layout_test.py`.

Workstream -> crate/module map (anchor format: `P<lane> <crate>`):

- P2 oep-cgr        -- CGR reader (geometry/tessellation)
- P3 oep-3dxml      -- 3DXML reader/writer
- P6 oep-geo        -- geometry core + model representation
- P6 oep-viewer     -- P6-M5 preview viewer (render seam, camera/select/clip)
- P9 oep-render-worker -- headless oep-geo render worker (P9-M3-T01)
- P9 oep-farm       -- beam/farm-style checkpoint + resume for long jobs (P9-M4-T02)
- P4 oep-step       -- STEP AP242/QIF/JT interop (reserved)
- P8 oep-pdm        -- self-hosted PDM / ENOVIA-lite; REST API v1 service skeleton
  (P8-M1-T01, Rust axum per P8-M1/web-stack-adr.md, AGPL-3.0; retained
  P7-M2-T02 StubServer identity + PdmClient), contract: P8-M6/openapi-v1.yaml,
  generated python client: python/oep_pdm_client.py (P8-M6-T01), validator:
  scripts/validate-openapi.py, integration: P7-M2/pdm-integration.md and
  P8-M6/integration-adapters.md (P8-M6-T02 adapter layer: oep_pdm::adapter,
  unit-instrumented seam the OEP shell and adopted tools use to reach the live
  PDM API). P8-M6-T03 (GA gate) wires the real working-set lock, revision
  state machine and tamper-evident audit log (oep_pdm::{working_set,state,
  audit}) into the served contract as additive gate verbs (lock / lock release
  / transition / audit / audit-verify; see P8-M6/ga-gate.md), with the gate
  harness python/oep_tooling/ga_gate_p8.py and proof under P8-M6/proof/
- P10 oep-pdm sync   -- multi-site replication demo (P10-M4-T01): LAN node +
                       remote node replicating the vault with conflict semantics
                       documented (docs/P10-M4/multi-site-replication.md),
                       driven by the offline-sync surface rooted in
                       P8-M5/hybrid-cloud-role-adr.md (airgap default,
                       deploy_role) + the generated client sync verbs;
                       harness python/oep_tooling/replication_demo.py, live
                       two-host (U1 + garage PC) proof under P10-M4/proof/
- P9 oep-job        -- P2P compute lane: job/task spec (input vault refs,
                       executor, resource caps, output refs)
- P9 oep-lane       -- P2P compute lane: scheduler (queue, priorities, DAG,
                       retries, dead-letter) + the at-least-once seam
                       + GPU discovery & placement (P9-M4-T01: node pool,
                       GPU requests, exclusive device reservation, CPU fallback)
- P9 oep-worker     -- P2P compute lane: export/convert workers (cgr2gltf,
                       step2mesh) + batch convert driver (P9-M3-T02). Dogfoods
                       P2 oep-cgr + P4 oep-step into the P8 vault; outputs
                       hash-verified content addresses (P9-M5-T02 gate)
- P9 oep-sandbox    -- P2P compute lane: worker sandbox (RLIMIT caps + seccomp no-egress)
- P9 oep-state      -- P2P compute lane: distributed job state (node-loss
                       survival, at-least-once delivery, idempotent effects)
- P9 oep-fairshare  -- P2P compute lane: fair share per team/node + quota
                       enforcement
- P9 oep-opsdash    -- ops dashboard for the P9 compute lane: live queue
                       depth, node health, job logs, cost/energy; read from
                       the real oep-lane Scheduler, served over HTTP
- P9 oep-gate       -- P9-M5-T02 GA gate: 100-job render/convert wave across
                       3+ nodes, all outputs hash-verified in the P8 vault
                       (oep_ga_gate driver + oep_ga_node agent + ga_gate_p9)

Not yet materialized as crates (laid out when their workstream starts):
P1 corpus tooling (python), P10 hybrid cloud, P12 QA/CI, P14 integration.

## Conventions

- Rust core: one `crates/oep-<stream>` crate per workstream, listed as a
  member of the root Cargo workspace. No cross-crate `path` deps until the
  P6 importer hub (P6-M4-T02) defines them; the P9 lane is the one
  exception (oep-opsdash path-depends on oep-lane: its whole contract is to
  read the real scheduler, so it must link it).
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
