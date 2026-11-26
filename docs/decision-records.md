---
title: "Isolith Decision Record Index"
version: 1.0.3
date: 2026-10-27
status: Ratified
owner: Isolith program governance
applies_to: every decision record, ADR, and numbered decision referenced in the tree
supersedes: nothing (first index; new decision records update this page, and a material change to a listed record's summary bumps its own version, not this page's)
---

# Isolith Decision Record Index

This page is the index of the decision records that live in this repository.
Each record is versioned and dated (living-document discipline). The
one-line summary below is a pointer, not the record: read the record before
relying on it.

The numbered decisions of the master plan (referenced in prose as "decision
#N", for example decision #15 wave integration and decision #19 editor spine)
are the master-plan decision log and are cited throughout the scope register,
the roadmap, and the ADR pages below. Where a numbered decision is enshrined
in the tree, the in-tree record is listed here (for example the wave
integration policy that carries decision #15). The master plan itself lives
in the program vault, not in this repository.

## Governance and program decisions

| Record | Date | Version / status | One-line summary |
|---|---|---|---|
| [Contributor rules](workstreams/governance/contributor-rules/contributor-rules-decision.md) | 2026-10-15 | 1.0.0, Ratified | Contributions are certified with the DCO (`Signed-off-by`), there is no standing CLA and no copyright assignment, and CODEOWNERS per workstream is the review policy. |
| [Release train and versioning](workstreams/governance/decisions/release-train-versioning.md) | 2026-10-15 | 1.0.0, Ratified | Hybrid versioning: calendar train YYYY.N for products, semver for component crates, patches to a shipped train carry fixes only. |
| [Telemetry](workstreams/governance/decisions/telemetry-decision.md) | 2026-10-15 | 1.0.0, Ratified | Telemetry off by default, opt-in crash reports only, no silent data collection. |
| [Editor spine](workstreams/governance/decisions/editor-spine.md) | 2026-10-22 | 1.0.0, Ratified | Full send on the editor spine (decision #19): the editor is the product bar, with Class-A surfacing moved into scope from day one. |
| [Wave integration and checkpoint policy](workstreams/release-engineering/wave-integration-and-checkpoint-policy.md) | 2026-10-18 | 1.0.0, Ratified | Decision #15: every wave integrates onto one branch and closes with one documented checkpoint commit onto main, and the program is bounded to exactly two repositories. |
| [Export-control classification](workstreams/security/export-control.md) | 2026-10-18 | 1.0.0, Ratified | Every release carries a CI-checked ITAR/EAR classification file asserting data locality and no restricted content; absence fails the release. |
| [Export-control memo (P0-M2-T04)](workstreams/governance/export-and-legal/export-control-itar-eccn.md) | 2026-10-15 | 1.0, memo | The internal posture: how Isolith engineering data flows classify under ITAR/EAR and the air-gap posture; code and general-purpose data are not ITAR technical data by default. |
| [Name and brand (0001)](assets/brand/NAME.md) | 2026-10-20 | 1.1 | The program name is Isolith; the former codename retired after a trademark collision (decision record 0001). |
| [Public repository hardening](workstreams/governance/decisions/public-repo-hardening.md) | 2026-10-27 | 1.0.0, Ratified | Before visibility: harden the public repo to the baseline (account security, branch and tag protection, Actions hardening, supply chain, release key publication, CODEOWNERS); GitHub-side rows apply when the account exists. |

## Technical decision records (ADRs)

| Record | Date | Version / status | One-line summary |
|---|---|---|---|
| [B-Rep kernel operations](workstreams/geometry-core/kernel/boolean-ops-occ-adr.md) | 2026-10-18 | 1.3.0 | Real B-Rep operations use Open CASCADE, reached through the vendor-neutral `oep-geo` Evaluator seam. |
| [Class-A surfacing strategy](workstreams/geometry-core/kernel/srf-class-a-adr.md) | 2026-10-22 | 1.0.0 | Class-A / ICEM-class surfacing is a long-horizon program: continuity, fairing, and surface reconstruction, not vanilla OCC. |
| [Renderer backend](workstreams/geometry-core/viewer/renderer-backend-wgpu-adr.md) | 2026-10-16 | 1.0.0 | Renderer is WGSL/wgpu, offline and headless. |
| [Assembly solver](workstreams/modeler/assembly/solver-adr.md) | 2026-10-18 | 1.2.0, implemented | Replace SolveSpace through the seam with a permissive-engine constraint solver on the assembly workbench. |
| [Drafting workbench](workstreams/modeler/drafting/adr.md) | 2026-10-22 | ACCEPTED | The drawing record is a Class 2 `oep-geo` extension; views and dimensions are computed from the model, never fabricated. |
| [Sketch constraint solver](workstreams/modeler/sketch-and-rules/sketch-solver-adr.md) | 2026-10-16 | 1.0.0 | Adopt an OSS 2D GCS behind a seam; do not rebuild one and do not import FreeCAD. |
| [Native CATPart alpha target](workstreams/native-reader/feasibility/alpha-target-brep-first-adr.md) | 2026-10-16 | 1.0.0 | The native CATPart reader alpha targets B-Rep and geometry first, feature-tree reconstruction deferred. |
| [PDM web stack](workstreams/pdm/service/web-stack-adr.md) | 2026-10-17 | 1.0.0 | The P8 PDM web stack is Rust axum. |
| [Hybrid cloud role](workstreams/pdm/sync/hybrid-cloud-role-adr.md) | 2026-10-17 | 1.0.0 | Bastion-shaped P2P relay instead of plain HTTPS; default deployment is LAN or air-gapped. |
| [Message bus](workstreams/compute/spec/message-bus-adr.md) | 2026-10-17 | 1.0.0 | NATS self-hosted is the default message bus, with a committed topic taxonomy. |
| [STEP library](workstreams/step-interop/library-adr/step-library-adr.md) | 2026-10-16 | 1.0.0 | The STEP reader/writer library choice for the P4 interop lane. |
| [PMI import](workstreams/step-interop/pmi/pmi-import-adr.md) | 2026-10-16 | 1.0.0 | Semantic PMI import approach on the P4-M2 lane. |
| [JT import](workstreams/step-interop/qif/jt-import-decision-adr.md) | 2026-10-16 | 1.0.0 | JT is imported visualization-first (mesh / modern-era geometry for viewing); B-Rep and PMI are not claimed. |
| [Render-seam performance](workstreams/modeler/ui/decisions/render-seam-performance.md) | 2026-11-25 | 1.0.1, Proposed | Wave-D interactive seam measured per stage (assemble 75.6 percent of the frame; the 120 MB per-frame re-upload about 191 ms); recommends GPU-resident geometry with CPU-side retention, defers the depth-buffer route to its own card, and records that no CPU-side option reaches 16.6 ms p95 at 1M visible triangles. |

## How to use this index

- A change to a decision's substance must go through the pull request path
  with the DCO sign-off and a test that proves the change (CONTRIBUTING.md),
  then bump the record's own version and add a line to its change log.
- A new decision record (or ADR) is added to the tree where its workstream
  lives, following the same front-matter conventions, and this page gains a
  row.
- The scope register and the roadmap are the living surfaces that reference
  these records; a decision here has no effect until a row or a page that
  carries it is updated with a green test behind the claim.

## Version history

- 1.0.2 (2026-10-27): add the public repository hardening decision row.
- 1.0.0 (2026-10-27): first index of the in-tree decision records with
  one-line summaries and the numbered-decisions note.
