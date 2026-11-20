# Isolith licensing boundary

This file is the human-readable form of the machine-checkable boundary map at
`docs/licensing-boundary.json`. The CI check `scripts/licensing-check.py`
fails when a crate's declared licence disagrees with this map, or when a
dependency introduces a licence outside the allow-list. This document is not
the legal text; the legal texts are the unmodified canonical versions at the
repository root:

- `LICENSE-APACHE`  Apache License, Version 2.0
- `LICENSE-AGPL`    GNU Affero General Public License, Version 3
- `LICENSE`         the index that states which applies where
- `NOTICE`          third-party attribution, generated from `Cargo.lock`

Decision (recorded 2026-10-27): keep the split. Apache-2.0 for applications
and libraries; AGPL-3.0 for the server, the PDM, and the P2P orchestration
components. Enterprise/B2B commercialization of the server half is a possible
later path, not a change to this boundary.

## Rules

**R1, per-crate boundary.** Every crate under `crates/` is licensed as a
whole under exactly one licence, set by its `Cargo.toml` `license` field and
enforced against this map by the CI check. Cross-crate link edges do not
change a crate's licence.

**R2, a library consumed by an AGPL component.** A library or application
stays Apache-2.0 even when it is linked into or consumed by an AGPL-3.0
crate. The licence of a consumer does not change the licence of the consumed
crate. When an AGPL crate links an Apache crate, the combined server work
ships under AGPL (the project authors both sides), while the Apache crate's
own grant to third parties stands. Examples in this tree: `oep-cgr` and
`oep-step` (Apache-2.0) are consumed by `oep-worker` (AGPL-3.0); `oep-geo`
(Apache-2.0) is consumed by the render worker (AGPL-3.0).

**R3, shared contract crates.** A crate whose primary purpose is to define
the contract, schema, or state model of the AGPL server/PDM or P2P
orchestration runtime takes the licence of the component it contracts. The
P2P lane shared contract crates, `oep-job` (job/task spec) and `oep-state`
(distributed job state), are AGPL-3.0. Contract and schema crates of Apache
components stay Apache-2.0.

## The per-crate map

33 workspace crates. Licence column is the SPDX identifier every crate must
declare in its `Cargo.toml`.

| Crate | Licence | Workstream | Why |
|---|---|---|---|
| oep-cgr | Apache-2.0 | P2 | CGR reader (library) |
| oep-3dxml | Apache-2.0 | P3 | 3DXML reader/writer (library) |
| oep-import | Apache-2.0 | P6 | importer hub over the canonical model (library) |
| oep-geo | Apache-2.0 | P6 | geometry core and model representation (library) |
| oep-engine-occ | Apache-2.0 | P6 | OCCT geometry-kernel binding behind the oep-geo seam (library) |
| oep-viewer | Apache-2.0 | P6 | preview viewer, desktop app shell (application) |
| oep-step | Apache-2.0 | P4 | STEP AP242/QIF/JT interop (library) |
| oep-step-pmi | Apache-2.0 | P4 | STEP AP242 semantic PMI import (library) |
| oep-pmi | Apache-2.0 | P6 | PMI model schema (library) |
| oep-qif | Apache-2.0 | P4 | QIF import/export (library) |
| oep-archive | Apache-2.0 | P10 | export-all interop escape hatch (application/library) |
| oep-shell | Apache-2.0 | P7 | modeler shell (application) |
| oep-kinematics | Apache-2.0 | P7 | mates/kinematics semantics over oep-geo (application/library) |
| oep-rules | Apache-2.0 | P7 | rules/checks sandbox engine (library) |
| oep-sketch | Apache-2.0 | P7 | 2D sketcher and parametric variables (application/library) |
| oep-assem | Apache-2.0 | P7 | assembly workbench and solver seam (application/library) |
| oep-analysis | Apache-2.0 | P7 | editor-spine analysis lane (application/library) |
| oep-app | Apache-2.0 | P7 | modeler editor UI: eframe/egui desktop application shell (application) |
| oep-drafting | Apache-2.0 | P7 | 2D drafting workbench (application/library) |
| oep-gate | Apache-2.0 | P9 | P9-M5-T02 GA gate: QA harness that exercises the lane, not part of the served orchestration runtime (application/tooling) |
| oep-update | Apache-2.0 | P10 | advisory non-blocking update checker (application) |
| oep-pdm | AGPL-3.0 | P8 | self-hosted PDM REST server (server/PDM) |
| oep-web | AGPL-3.0 | P8 | PDM web UI merged into the PDM axum router (server/PDM surface) |
| oep-docs | AGPL-3.0 | P13 | docs server on the delivery stack, same compose as the PDM stack (server) |
| oep-lane | AGPL-3.0 | P9 | P2P lane scheduler: queue, DAG, retries, GPU placement (P2P orchestration) |
| oep-job | AGPL-3.0 | P9 | P2P job/task spec schema and validation, shared lane contract (P2P orchestration, R3) |
| oep-state | AGPL-3.0 | P9 | P2P distributed job state, shared lane contract (P2P orchestration, R3) |
| oep-fairshare | AGPL-3.0 | P9 | P2P fair-share scheduling and quota enforcement (P2P orchestration) |
| oep-worker | AGPL-3.0 | P9 | P2P export/convert workers (P2P orchestration runtime) |
| oep-sandbox | AGPL-3.0 | P9 | P2P worker sandbox, RLIMIT caps and seccomp (P2P orchestration runtime) |
| oep-render-worker | AGPL-3.0 | P9 | P2P headless render worker (P2P orchestration runtime) |
| oep-farm | AGPL-3.0 | P9 | P2P checkpoint/resume for long jobs (P2P orchestration runtime) |
| oep-opsdash | AGPL-3.0 | P9 | P2P ops dashboard, served over HTTP (P2P orchestration surface) |

Totals: 21 crates Apache-2.0, 12 crates AGPL-3.0.

## What this means for third-party components

The pure-permissive allow-list (Apache-2.0, MIT, ISC, BSL-1.0, BSD-1/2/3,
CC0-1.0, CC-BY-4.0, MIT-0, Unlicense, Zlib, Unicode-3.0, plus the permissive
font-data licenses OFL-1.1 and Ubuntu-font-1.0 carried by the egui default
fonts) is enforced for
every package in the committed `Cargo.lock`; OR branches that offer a
permissive alternative are acceptable (for example the MIT/ISC branches of
`aws-lc-rs` and the MIT branch of `r-efi`). Identifiers explicitly barred
from the delivered tree are the copyleft ones, GPL-2.0, GPL-3.0, AGPL-1.0,
AGPL-3.0 and LGPL-2.1/3.0 as sole options, consistent with the clean-room
gate (no GPL-3.0 in the delivered tree). The one native C++ component, the
OCCT geometry kernel, is not resolved through Cargo.lock (it is fetched and
built outside Cargo by scripts/kernel) and is carried in NOTICE under its own
text: LGPL-2.1 with the OCCT/Open CASCADE Exception 1.0.

Consistent with the same honesty discipline as the format matrix, the map,
the `Cargo.toml` licence fields, and the dependency allow-list are verified
together by `scripts/licensing-check.py`; the committed proof lives under
`docs/proof/`.
