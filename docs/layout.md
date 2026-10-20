# Isolith Monorepo Layout

Isolith (public brand for the Open Engineering Platform, OEP) is a monorepo at
`don/isolith` on NAS Gitea; `isolith` is the internal-only codename.
One crate per code workstream, under `crates/`; Python tooling lives under
`python/`. This file is the machine-verifiable contract checked by
`tests/monorepo_layout_test.py`.

Public identity: the Isolith brand kit lives in `assets/brand/` (mark, palette
tokens, usage rules, trademark evidence). Any public-facing surface uses it
(the docs site P13-T03 consumes `assets/brand/` - see its README).

All per-workstream documentation, ADR, and evidence directories (P0-M1, P4-Mx,
P8-Mx, P11, P12, P13-T03, ...) live under `workstreams/`, one level deep, so
nothing P-prefixed sits at the repo root. New workstream records are created
under `workstreams/<Px...>/`.

Workstream -> crate/module map (anchor format: `P<lane> <crate>`):

- P2 oep-cgr        -- CGR reader (geometry/tessellation)
- P3 oep-3dxml      -- 3DXML reader/writer
- P6 oep-geo        -- geometry core + model representation
- P6 oep-viewer     -- P6-M5 preview viewer (render seam, camera/select/clip)
- P9 oep-render-worker -- headless oep-geo render worker (P9-M3-T01)
- P9 oep-farm       -- beam/farm-style checkpoint + resume for long jobs (P9-M4-T02)
- P4 oep-step       -- STEP AP242/QIF/JT interop (reserved)
- P4 oep-step-pmi   -- STEP AP242 semantic-PMI import into oep-pmi (P4-M2-T01)
- P4 oep-import     -- importer hub: P2/P3/P4 adapters feed the canonical oep-geo model (P6-M4-T02)
- P6 oep-pmi        -- PMI model schema: GD&T, datums, notes, tolerances (P6-M1-T02)
- P4 oep-qif        -- QIF (Quality Information Framework) MBD quality data import/export (P4-M4-T01)
- P7 oep-shell      -- modeler shell: OEP interchange workbench command surface (P7-M2-T01)
- P7 oep-kinematics -- mates/kinematics demo, own DMU-ish semantics over oep-geo (P7-M3-T02)
- P7 oep-rules      -- rules/checks sandbox, Knowledgeware-lite documented not-parity (P7-M4-T02)
- P7 oep-sketch     -- 2D sketcher + parametric variable sets, adopted OSS solver via seam (P7-M4-T01)
- P10 oep-archive   -- export-all interop escape hatch: neutral STEP/3DXML/glTF/STL + JSON archive (P10-M3-T02)
- P8 oep-pdm        -- self-hosted PDM / ENOVIA-lite; REST API v1 service skeleton
  (P8-M1-T01, Rust axum per workstreams/pdm/service/web-stack-adr.md, AGPL-3.0; retained
  P7-M2-T02 StubServer identity + PdmClient), contract: workstreams/pdm/api/openapi-v1.yaml,
  generated python client: python/oep_pdm_client.py (P8-M6-T01), validator:
  scripts/validate-openapi.py, integration: workstreams/modeler/interchange/pdm-integration.md and
  workstreams/pdm/api/integration-adapters.md (P8-M6-T02 adapter layer: oep_pdm::adapter,
  unit-instrumented seam the OEP shell and adopted tools use to reach the live
  PDM API). P8-M6-T03 (GA gate) wires the real working-set lock, revision
  state machine and tamper-evident audit log (oep_pdm::{working_set,state,
  audit}) into the served contract as additive gate verbs (lock / lock release
  / transition / audit / audit-verify; see workstreams/pdm/api/ga-gate.md), with the gate
  harness python/oep_tooling/ga_gate_p8.py and proof under workstreams/pdm/api/proof/
- P10 oep-pdm sync   -- multi-site replication demo (P10-M4-T01): LAN node +
                       remote node replicating the vault with conflict semantics
                       documented (docs/delivery/replication/multi-site-replication.md),
                       driven by the offline-sync surface rooted in
                       workstreams/pdm/sync/hybrid-cloud-role-adr.md (airgap default,
                       deploy_role) + the generated client sync verbs;
                       harness python/oep_tooling/replication_demo.py, live
                       two-host (U1 + garage PC) proof under workstreams/delivery/replication/proof/
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
- P8 oep-web        -- P8-M4-T01 web UI merged with the oep-pdm REST v1 router
                       over the same vault store: item explorer, item page with
                       BOM tree, P6 preview embed (oep-viewer PNG renders),
                       compare view (P10-M1-T01 integrates it onto the P9
                       assembly; graph + viewer surfaces via oep_pdm::graph
                       RelationshipGraph, re-exported from oep-pdm)
- P10 oep-update    -- P10-M5-T01 update checker (decision #11): advisory,
                       NON-BLOCKING update check against a configurable source
                       (remote URL / on-site mirror / offline signed bundle);
                       signed manifest verified before any download; runs
                       identically with no source reachable; disable-able and
                       non-phoning by default (off).
- P13 oep-docs      -- P13-T03 docs server on the delivery stack: std-only
                       static file server for docs-site/ (getting-started,
                       admin guide, honest format matrix), same image + compose
                       as the PDM/viewer/P2P stack ("no separate infra").
                       Zero external deps; serves committed docs-site/ only.

Not yet materialized as crates (laid out when their workstream starts):
P1 corpus tooling (python), P10 hybrid cloud, P14 integration.

## P12 QA / CI (release engineering)

- Rationed per-milestone GA gates live under `.gitea/workflows/` (one workflow
  per GA-lane feature + the lane gate; the P12 lane gate is the release
  drill). The E2E scenario suite (`workstreams/release-engineering/e2e-vertical.md`,
  `python/oep_tooling/e2e_vertical.py`, `scripts/e2e-vertical.sh`,
  `.gitea/workflows/ga-gate-p12e2e.yaml`) scripts the reference vertical
  (import CGR/STEP -> model -> release via PDM -> render/convert via P2P ->
  share) headlessly and asserts each stage; its pass is a release-blocking CI
  check (`ga-gate-p12e2e.yaml`) alongside the GA gates, and the
  deliberate-failure repro (`--break-stage release`) proves a broken stage
  blocks. Recorded proofs: `workstreams/release-engineering/proof/e2e-vertical-<ver>.{json,md}`.

P10 hybrid cloud / self-hosted delivery to date ships as tooling + scripts, not
new crates (the PDM server that carries TLS is P8 `oep-pdm`):

- `scripts/gen-stack-certs.sh`        -- local CA + leaf for the stack (P10-M1-T02)
- `scripts/backup/vault-backup.sh`    -- checksummed vault DB+blob backup to a
                                         second disk / archive target (P10-M1-T02)
- `scripts/backup/vault-restore-check.sh` -- producer-side restore round-trip
- `scripts/backup/install-vault-backup-cron.sh` -- daily scheduled backup
- `scripts/p10m1t02-proof.sh`         -- acceptance proof capture (P10-M1-T02)
- `scripts/dr/backup-vault.sh` `scripts/dr/drill-restore.sh` -- DR lane (P10-M3-T01),
  consuming the same `isolith-vault-backup/` bundle contract
- docs: `workstreams/delivery/deploy/` (lane docs + proof), `workstreams/delivery/disaster-recovery/` (DR runbook + proof)

## P12 release engineering

- `workstreams/release-engineering/release-train.md` -- release train, versioning, freeze, canary staging
  and patch policy (P12-T05)
- `python/oep_tooling/release_train.py` -- freeze / verify / release-cut tooling
- `workstreams/release-engineering/freeze-manifests/` -- frozen commit set manifest per candidate
- `workstreams/release-engineering/proof/` -- verified dry-run train cut record

P11 supply-chain / release gating ships as tooling + docs, not new crates:

- `python/oep_tooling/sbom_cve.py` -- P11-T02 engine: emits CycloneDX
  (1.5) + SPDX (2.3) BOMs for the Rust crate tree (from the committed
  Cargo.lock) and the container base images, scans Cargo.lock against the
  rustsec advisory-db, folds in a release-host trivy image scan, enforces
  the critical/high gate with the committed waiver file workstreams/security/waivers.toml.
- `scripts/sbom-cve-gate.sh` -- release-gate driver (exit-code contract:
  0 == gate PASS; 1 == do not release).
- `scripts/sbom-cve-fail-repro.sh` -- deliberate-failure repro (seeded
  critical vuln MUST fail the gate).
- `workstreams/security/sbom-cve.md` (runbook), `workstreams/security/waivers.toml` (waiver register),
  `workstreams/security/artifacts/<ver>/` (committed per-release SBOMs + report),
  `workstreams/security/proof/` (durable proof: clean-scan report, trivy image scan,
  deliberate-failure repro), `workstreams/security/.advisory-db/` (gitignored cache).

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
