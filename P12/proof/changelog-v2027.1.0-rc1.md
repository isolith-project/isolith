# Keelworks Changelog

All notable changes to Keelworks are documented here, grouped by category from
conventional commit messages. Releases follow the P0-M5-T03 calendar train +
semver decision. This changelog is generated, never hand-written: edit
scripts/release/changelog.sh, this config, or the commit messages, not this
file's output.
## 2027.1.0-rc1



### Features

- 121ad64 **P0-M3-T01 workspace skeleton with per-workstream crates**
- 98c9e9e **pin Rust, Python, C++ and packaging manifests (P0-M3-T02)**
- 2622329 **P0-M4-T01 gitea actions pipeline lint+unit+integration on every push**
- 26dae2b **precommit quality gate vendored and wired to CI**
- 21dafec **GREEN - oep-pdm REST API v1 stub server + PdmClient (open-from-PDM / save-revisions-to-PDM); content-addressed vault store; docs P7-M2/pdm-integration.md**
- b734648 **oep-pdm service skeleton on Rust axum + web-stack ADR (AGPL-3.0)**
- a405b4d **GREEN - generated python client (python/oep_pdm_client.py) emitted from the OpenAPI v1 contract; generated-client contract proof**
- ce4f077 **P8-M6-T02 GREEN - integration adapter layer (oep_pdm::adapter::PdmAdapter): open/save verbs + item-atom resolution + auto-create + unit instrumentation over the live PDM API**
- 9bacc3e **GREEN - wire real working-set lock / revision state machine / tamper-evident audit into oep-pdm as additive gate surface (lock, transition, audit, audit-verify); generated client + OpenAPI updated**
- daca773 **GREEN - oep-job crate: job/task spec types, validation, P8-M6 REST ref mapping**
- f089ed4 **P9-M2-T01 scheduler - priority queue, DAG deps, backoff retries, dead-letter, at-least-once seam**
- eb7ae61 **GREEN - GPU discovery (gpu) + GPU-aware placement (placement) + scheduler wire-in; required parks, preferred falls back to CPU cleanly, exclusive device reservation**
- 1ef17ae **oep-farm crate - checkpoint/resume for long jobs (beam/farm style)**
- 63e2dec **oep-geo model schema (part/body/feature/assembly/instance) + documented our-own-format serde**
- ff70453 **vendored oep-geo math core (Vec3/Mat3/Mat4/Quat) + property tests**
- 1674b3b **P6-M5-T02 preview viewer shows oep-geo models; camera, selections, clipping (GREEN)**
- b7c53ff **headless oep-geo render worker as a P9 worker function (GREEN)**
- d17ee24 **P9-M3-T02 dogfood base - bring P2 oep-cgr (mesh/gltf/modern), P4 oep-step (read lane, goldens), P6 oep-geo onto the P8/P9 line**
- b2c3c6a **P9-M3-T02 seam - raise upload body cap to 512 MiB so real CAD reaches the vault**
- 687ea49 **P9-M3-T02 - export/convert workers (cgr2gltf, step2mesh) + batch driver**
- 455f2ca **GREEN - oep-sandbox worker sandbox: RLIMIT caps, seccomp no-egress by default, escape fails a test**
- 0dbffcb **GREEN - oep-state: distributed job state (node-loss survival, at-least-once redelivery, idempotent effects)**
- ec38778 **GREEN - oep-fairshare crate: quota enforcement + per-node weighted fair-share admission; oversubscribed teams throttled under load**
- f9d2c95 **ops dashboard - queue depth, node health, job logs, cost/energy**
- 46db430 **GREEN - message bus decision (NATS self-hosted default + sovereignty) and committed topic taxonomy with live probe evidence**
- 0d89287 **P9 GA gate - 100-job render/convert wave across 3+ nodes, all outputs hash-verified in the P8 vault**
- 34bd135 **integrate P8-M4 viewer (oep-web) + P8-M2-T03 relationship graph onto the P9 assembly**
- 225ee00 **one-command docker-compose stack (PDM + P2P + viewer), verified on U1**
- 20a4b0a **signed offline/air-gap install path (USB/ISO), no phone-home**
- 80df39e **GREEN - oep-update crate: advisory non-blocking signed air-gap-safe update checker (decision #11)**
- c846a18 **GREEN - hybrid cloud role ADR (Bastion-shaped relay vs HTTPS, airgap default) + deploy_role module + LAN/air-gap install path proven on NAS**
- 6a8b852 **GREEN - offline working set, sync-back on reconnect, conflict resolution UI (3-way merge)**
- 37161cb **generated client gains the offline-sync surface (gen drift gate green)**
- 0cc5ecd **multi-site replication demo (LAN + remote node), live proof + CI gate**
- 0e595cd **GREEN - native TLS termination in oep-pdm + cert tooling + checksummed vault backup/restore (U1/VM verified, DR handoff to P10-M3-T01)**
- 51ee7da **TWO-MACHINE GA gate for the P10 fresh-install walkthrough**
- fa14054 **commit the recorded two-machine proof + install-walkthrough video**


### Bug Fixes

- c455086 **honor toolchain pin via rust-toolchain.toml, canonical clone URL**
- 8df9595 **CI gate job creates report dir; harness ensures report dir exists**
- d951e40 **CI workflow installs curl so the restore-check health/read legs can run in the rust:1-slim container**




### Documentation

- 2f5518f **P8 lane lists the generated python client + validator**
- 556c53b **P8-M6-T02 design note - integration adapters (clean-room seam, unit-instrumented) + layout map update**
- 67b54fe **gate harness + proof script + measured proof (11 rows, 11 PASS, exit 0, live NAS) + RE note + layout + separate CI gate workflow**
- c07785c **P6-M5-T01 renderer backend decision - WGSL/wgpu offline headless renderer (GREEN)**
- aae17d0 **T02 design note + hash-verified corpus batch proof + python contract test**
- 575c4d3 **drop stray merge marker after sandbox merge**
- b09fa9f **register oep-web (P8-M4-T01 web UI + viewer embed) in the crate map**
- 96b4e46 **record CI-green + PR #151 in the proof manifest**
- e67b8fc **align walkthrough video layout with the committed recordings**


### Tests

- 0b44b22 **assert real Gitea status context names (full form) in acceptance test**
- fdf5375 **RED - PDM REST v1 contract acceptance (Rust PdmClient + independent Python client) + OpenAPI v1 spec + STEP fixture**
- 411d743 **RED - contract tests for the generated python client (openapi spec + generator + validator + docs), no client yet**
- be8be3c **P8-M6-T02 RED - integration adapter acceptance (open/save + revision isolation + instrumentation against the LIVE PdmService over real sockets)**
- 15903d6 **RED - P8 GA gate acceptance (multi-user edit/publish/release + concurrent lock test) + spec gate-surface list**
- 79741a7 **RED - job/task spec acceptance (schema + examples + python schema validator/drift guard + Rust contract tests), no crate yet**
- ddae00b **RED - GPU discovery + placement acceptance (nvidia-smi parse incl DGX [N/A], exclusive reservation, required-park / preferred-CPU-fallback, real run-log) no impl yet**
- 3950710 **P6-M5-T01 renderer backend WGSL/wgpu ADR acceptance test (RED)**
- b11b349 **RED - oep-geo model schema acceptance tests + canonical example doc contract**
- 80df006 **RED - oep-geo math acceptance contract + property tests**
- 0f7e431 **P6-M5-T02 preview viewer acceptance contract (RED)**
- 6a603df **RED - worker sandbox acceptance (caps + no-egress + escape) with no implementation yet**
- d2c198b **RED - distributed state acceptance: node-loss drill (kill worker mid-job, scheduler redelivers, idempotent effects)**
- f009ea5 **RED - fair-share + quota acceptance (oversubscribed teams throttled under load, per-node weighted fair share, conservative defaults), no crate yet**
- 900d145 **RED - message bus decision + topic taxonomy acceptance: NATS self-hosted default, no-cloud sovereignty, alternatives, live probe log**
- 7ea8773 **RED - update checker acceptance (advisory, non-blocking, signed, air-gap-safe), no crate yet**
- 50139a4 **RED - hybrid cloud role acceptance contract (Bastion mesh reuse vs plain HTTPS, ITAR posture, LAN/air-gap install path on NAS)**
- daabb8a **RED - offline working set + sync-back + 3-way merge conflict resolution acceptance contract**
- 960b58f **RED->GREEN replication demo acceptance contract**
- 227d1f8 **RED - TLS terminate + vault backup/restore acceptance (real binary, real sockets)**


### Continuous Integration

- 168e6a6 **install python3 in the GA-gate job (rust image has no python for the acceptance/gate legs)**


### Chores

- 98a899b **bootstrap keelworks meta repo (README, gitignore)**
- 67a44ec **lockfile refresh after oep-job merge**
- 147986b **drop stray commit-msg.txt from the P9-M3-T01 worker**














### Other

- 6d20399 **DR drill - fresh-machine restore from backup under 60 min**



---
Generated by git-cliff from the Keelworks conventional commit history.
