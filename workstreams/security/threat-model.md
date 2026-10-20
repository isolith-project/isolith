---
title: Keelworks / Open Engineering Platform (OEP) Stack Threat Model
status: Ratified (Wave-3 P11-T01)
version: 1.1.0
date: 2026-10-18
owner: Keelworks program governance, P11 security lane
applies_to: the integrated delivery stack (oep-pdm REST API, oep-web UI, oep-update checker, the P9 P2P/NATS compute lane, oep-sandbox worker boundary, and the P10 air-gap / TLS / backup tooling)
supersedes: nothing (first P11 threat model)
---

# Keelworks Stack Threat Model

This is the threat model for the Keelworks / Open Engineering Platform (OEP)
stack as it exists at the start of Wave-3 (P11 security lane). It documents
the assets, trust boundaries, threat actors, and mitigations mapped to the
stack, per card P11-T01. Every mitigation below ties to a real component
(committed file or crate on `main`) or to a P11-T* card that closes it; the
mapping tables in section 8 are machine-checked by
`tests/test_p11_threat_model.py`, which fails on any orphan risk (a risk with
no mitigation, or a mitigation that points at nothing real).

Program convention: the P9-M3 sandbox card already carried a worker-boundary
statement (docs/compute/workers/sandbox.md, section 5). This document folds that in as
the authoritative worker-boundary section and adds the network, web, bus,
cluster-trust, air-gap, update-checker, and supply-chain surfaces on top.

## 1. System of interest

The stack is the integrated, self-hosted Keelworks deployment delivered by
the P10 wave (P10-M1 one-command compose stack). Everything runs inside a
customer's network (decision #10); air-gap and on-prem are the default and
the ITAR sovereignty selling point; remote access is not a product problem.

| # | Component | Real artifact | Port / surface | Security posture today |
|---|-----------|---------------|----------------|------------------------|
| C1 | PDM service `oep-pdm` | crates/oep-pdm (Rust axum, AGPL-3.0) | REST API v1 on /api/v1, workstreams/pdm/api/openapi-v1.yaml; host bind 127.0.0.1:8787 by default | SECURE BY DEFAULT (SEC-AUTHZ-DEFAULT): the P11-T03 RBAC boundary (P8-M3 role matrix + no-cross-tenant deny-wins rule) is wired into the shipped default start path, so every request is decided before its handler runs (403 cross-role / unauthenticated, 404 cross-tenant masked). The open contract is reachable only via the explicit `--no-auth` opt-out, which prints a loud startup warning that authorization is disabled (controlled test / air-gap use only). Principal roles/tenant are client-declared headers, authorization-grade only, until the P8-M3-T01 identity layer lands and stamps a verified passport; the decision path stays byte-for-byte. Integrity controls: opaque lock tokens gate publish (working_set.rs), released revisions are immutable (state.rs), and an append-only tamper-evident audit chain records reads and changes with /audit/verify (audit.rs). TLS optional via --tls-cert/--tls-key (rustls), off by default (P10-M1-T02). |
| C2 | Web UI viewer `oep-web` | crates/oep-web (oep-web binary) | HTTP 127.0.0.1:8790 by default | Serves the item explorer, BOM tree, compare view, and a server-side PNG preview embed (preview.rs renders oep-geo documents to PNG through the CPU rasterizer; the browser never executes user model content). Own persisted store (viewer_data volume). SECURE BY DEFAULT (SEC-AUTHZ-RESIDUAL): the merged REST v1 surface is gated exactly like oep-pdm (P11-T03 boundary; `--no-auth` is the explicit, loudly-warned opt-out) and the P11-T03 attack matrix is proven denied against the shipped default boot (crates/oep-web/tests/web_auth_boundary.rs). The read-only HTML/PNG workbench routes stay open until the P8-M3-T01 identity layer lands; their loopback bind default (decision #10) keeps them LAN-off, recorded as accepted risk with that compensating control. |
| C3 | P2P compute lane: NATS bus + scheduler + workers | nats-server (P9-M1 ADR, self-hosted Apache-2.0) + crates/oep-lane (scheduler), oep-job (job spec), oep-worker, oep-state, oep-fairshare, oep-opsdash, oep-gate | NATS on the internal mesh; ops dashboard 127.0.0.1:8080 | The bus is one self-hosted binary, loopback-provable, no cloud dependency (sovereignty decision). No bus authentication/tls to nodes beyond the mesh (P8-M3 identity layer is a plan); the bus is protected by network segmentation today. JetStream workqueue streams, dead-letter, KV buckets; worst case capped and DLQ'd. |
| C4 | Worker sandbox | crates/oep-sandbox (RLIMIT caps + seccomp no-egress, fail-closed) | n/a | Sandboxed compute workers (cgr2gltf, step2mesh, renderers) run under kernel-enforced CPU/RAM/disk/file ceilings and a seccomp filter that denies every non-AF_UNIX socket by default; if the filter cannot be installed the worker is never spawned (fail-closed). Workers have no network path at all; the bus client lives in the job agent outside the sandbox. |
| C5 | Update checker `oep-update` | crates/oep-update | outbound http(s) only when enabled and configured | Advisory and non-blocking (decision #11): every status maps to Proceed, there is no Block variant, default is Off with no I/O, payloads are exposed only after an Ed25519 signature and payload-sha256 check. |
| C6 | Air-gap + TLS + backup tooling | scripts/airgap, scripts/backup, scripts/dr, scripts/gen-stack-certs.sh | n/a | Signed offline bundle install (gpg manifest signature + per-file sha256), local CA + leaf for oep-pdm TLS, checksummed vault backup to a second disk with restore round-trip and DR drill. |
| C7 | Update/verification keys | scripts/airgap/keys/oep-airgap-signing.pub.asc (public); secret on the build host only | n/a | The most exposed attack target of the whole system: whoever holds a signing key can make the checker trust a bundle and can make an air-gap install accept an image set. |

Deployment facts that bound the model:

- Host binds default to 127.0.0.1 for all three services (workstreams/delivery/deploy/deploy/.env.example); the stack is LAN-off until an operator publishes it (decision #10 on-prem posture).
- The NAS is passive storage / repo host only and never hosts services (decision #9); approved run targets are U1/VM-class hosts and the garage PC.
- No component phones home by default (decision #11, P0-M5-T02 stands); telemetry is out, update checking is advisory.
- P9-M3 sandbox boundary: worker to host. Host-kernel CVEs, malicious supervisor root, and NATS/P2P TLS are explicitly out of the sandbox card's boundary and are handled by the escalation layers in section 7 and by the network cards here.

## 2. Assets

| Asset | What it is | CIA priority | Where it lives |
|-------|------------|--------------|----------------|
| A1 Vault product data | Item catalogue, revisions, content-addressed file blobs (CAD: CGR/STEP/3DXML/OEP-GEO and outputs) | C high, I high, A medium | pdm_data volume (store.json + blobs/), replicated by backup to second disk |
| A2 Audit chain | Append-only tamper-evident record of reads/changes/releases | I high, C medium (sovereignty: reads are access events), A medium | audit.log + audit.head in the vault persist dir; audit head recorded in backup meta.json |
| A3 Working-set / revision state | Live lock tokens, revision lifecycle (working/under_review/released/obsolete) | I high, A high (a corrupt lock table stalls editing) | oep-pdm store state (in-memory manager + persist dir) |
| A4 Compute outputs | Render PNGs, converted meshes, checkpoint blobs, job result hashes | I high, C medium, A medium | worker outputs written to the PDM vault (hash-verified content addresses, P9-M5) |
| A5 Queues and job state | JetStream streams, KV buckets (queue state, node registry, worker registry, scheduler meta) | A high, I high | nats-server JetStream storage |
| A6 Update/verification keys | Ed25519 update-sign key, air-gap gpg signing key | I critical (key = trust), C high | secret on the offline build host only; public verification keys committed |
| A7 Signing/public manifest data | Signed release bundles, update.bundle.json, air-gap bundle + sha256 + sig | I high | build host output, shipped on media, consumed by oep-update / airgap scripts |
| A8 Operator configuration | .env (binds, update-check posture, local secret), TLS certs/keys, backup targets | C high, I high | host filesystem /opt/keelworks, cert dir, second disk |
| A9 Job/content metadata in the ops dashboard | Queue depth, node health, job logs, cost | C low-medium | oep-opsdash served on the mesh |

## 3. Trust boundaries

```
                     [Internet / outside fence: nothing is supposed to reach this]
                                       |
        (operator publishes a bind -> widens TB1)         <-- NW-8 misconfig risk
                     +----------------+
   browser/client ---> TB2 web/API <---+   oep-web / oep-pdm  (C1,C2)
                     |                 |
                     |            TB3 vault store (A1,A2,A3) on pdm_data
                     |                 |
                     |      oep-update ---> TB7 update source (mirror/remote/bundle) [C5]
                     |                 |
   LAN device ---> TB4 bus/mesh (NATS, internal network only) [C3]
                     |      job agents (bus clients, trusted)    -> run
                     |         +------ TB5 worker sandbox ------+  [C4]
                     |         |  sandboxed compute worker      |
                     |         +-------------------------------+
                     |
              TB6 air-gap media (build host <-> target) [C6]
              TB8 backup/DR (vault <-> second disk)     [C6]
```

| TB | Boundary | Trusted side | Untrusted side | What crosses it |
|----|----------|--------------|----------------|-----------------|
| TB1 | Host loopback bind | host processes | LAN / other hosts | service traffic; closed by default, operator may publish (escalation) |
| TB2 | Web/API HTTP(S) surface | oep-web / oep-pdm | browsers, modeler clients, REST clients | JSON contract payloads, blob downloads/uploads, lock/transition/sync verbs |
| TB3 | Vault store boundary | oep-pdm process | on-disk persist dir | store.json, blobs, audit chain; tamper-evident on the audit side only |
| TB4 | Bus boundary | mesh identity + job agents | LAN devices, rogue agents that can connect to NATS | jobs, heartbeats, RPC, state, logs (oep.* topics) |
| TB5 | Worker sandbox | host + job agent | worker process (untrusted compute code) | fd 0/1/2, scratch file, job argv; NO network |
| TB6 | Air-gap media | signed bundle content | whatever crosses the gap on USB/ISO | container images, compose, scripts, manifest + signatures |
| TB7 | Update source | signed manifest (after verify) | remote URL / mirror dir / bundle file | update.bundle.json bytes (advisory only) |
| TB8 | Backup/DR | vault | second disk / archive target | vault bundle + manifest.sha256 + meta.json (audit head) |

Model boundary (out of scope): host OS/kernel integrity outside the worker
cap chain, physical security of the server room, NAS admin, and the customer
network's own perimeter devices. Those are operationally owned; the model
records their residual risk where a threat reaches them (CL-4, W-5).

## 4. Threat actors

| Actor | Description and capability | Assumed intent |
|-------|----------------------------|----------------|
| AT1 External internet attacker | Reaches the stack only if the operator publishes a bind or a bastion; otherwise network-isolated by posture | Opportunistic scanning, exploit of any exposed service |
| AT2 LAN network attacker | A rogue device, compromised laptop, or on-LAN adversary who can reach the mesh and any published service; can sniff or inject on the LAN | Read or modify product data, submit forged jobs, DoS the bus |
| AT3 Insider / operator | A legitimate user or admin of the customer shop with host and LAN access; actions are logged by the audit chain | Careless or malicious read/modify of vault, tampering with bundles, config, backups |
| AT4 Compromised worker | Malicious, buggy, or hijacked compute code running inside the sandbox | Escape the sandbox, exhaust resources, exfiltrate vault data to the network |
| AT5 Compromised host / node root | Full root on a worker node, the PDM host, or a mesh node | Steal artifacts or keys, rewrite config/audit, pivot |
| AT6 Supply-chain adversary | Compromises a dependency, the build host, the toolchain, or release signs a malicious artifact | Ship tampered binaries/manifests that the trust chain will accept |
| AT7 Malicious update-source operator | Controls the reachable update source (remote URL, on-site mirror, or bundle placement) | Feed a fake update flag; at worst cause Untrusted (no payload exposure without the key) |
| AT8 Rogue admin on the build side | Holds or steals the signing key, or tampers on the build host | Sign and slip in a malicious bundle that the checker and air-gap install will trust |

## 5. What the update checker is specifically allowed to do (decision #11)

The update checker (C5) is the stack's only designed reach-out surface. Its
security contract is non-negotiable and encoded in crates/oep-update:

- Advisory and non-blocking: the program runs identically whether the source
  is found, empty, unreachable, or hostile. `check()` NEVER returns an error
  that could gate launch; `Probe::preflight` maps every status to Proceed and
  there is deliberately no Block variant.
- Signed-before-trusted: an Ed25519 signature over the canonical manifest is
  verified, then the inline payload sha256 is re-checked, before any payload
  is exposed (signing.rs, verify_and_report).
- Non-phoning by default: `UpdateConfig::default` is Off with no I/O; when
  enabled the checker talks only to the configured source. There is no
  telemetry surface in the crate (P0-M5-T02 stands).
- Air-gap safe: sources are a remote URL, an on-site mirror, or an offline
  signed bundle placed by an admin; the generated .env carries
  OEP_UPDATE_CHECK=off by default (P10-M2).

The consequence: the worst realistic harm a hostile update source can cause
safely (AT7) is a spurious "update available" flag that an operator might
waste time on. It cannot push bytes into any host, because the applied update
is a separate, operator-gated step. The one route to real harm is holding the
signing key (AT8, UC-3/W-5), which is exactly where P11-T04 hardening lands.

## 6. Risk register

Columns: ID, surface, threat, actor, impact, mitigation IDs (full text in the
inventory, section 7). Severity is rated for the default, on-prem, LAN-off
deployment; an operator who publishes binds or enables the update source moves
the NW and UC rows up a step.

| ID   | Surface | Threat | Actor | Impact | Mitigations |
|------|---------|--------|-------|--------|-------------|
| NW-1 | PDM/web API (C1, C2) | No authentication or RBAC on the REST/vault surface; any LAN client can read/modify items, revisions, files, audit, locks | AT2, AT3 | Critical: full vault read/write, audit pollution | M-LOOP, M-RBAC |
| NW-2 | PDM/web API (C1, C2) | Plain-HTTP transport (TLS off by default) lets a LAN sniffer read vault bytes and lock tokens in transit | AT2 | High: confidentiality of product data, token theft | M-TLS |
| NW-3 | Web UI (C2) | CSRF against browser-driven mutating APIs (lock, transition, sync resolve) once sessions exist | AT2, AT3 | Medium: state changes fired from a third-party page | M-PTN |
| NW-4 | Web UI (C2) | XSS / script injection through rendered vault content or the explorer/UI | AT2 | Medium: session hijack / UI defacement in the browser | M-PTN |
| NW-5 | PDM/web API (C1) | IDOR / object-level authz bypass on items, revisions, files, audit log (post-RBAC) | AT2, AT4 | High: cross-tenant/cross-role reads and writes | M-RBAC |
| NW-6 | P2P bus (C3) | A LAN device connects to NATS and submits forged jobs, reads job data, or fakes heartbeats | AT2 | High: compute theft, job-data disclosure, scheduler confusion | M-ISM, M-PTN |
| NW-7 | P2P bus (C3) | Protocol abuse: malformed subjects outside the taxonomy, malformed job payloads, bus floods | AT2 | Medium: queue/jetstream exhaustion, DoS, DLQ churn | M-ISM, M-JOBQ, M-PTN |
| NW-8 | Deployment (C1..C3) | Operator publishes a bind beyond loopback (0.0.0.0, LAN publish) without compensating controls; containers already listen on 0.0.0.0 inside | AT2 | High: widens every NW surface to the LAN | M-LOOP, M-OUTB, M-TLS |
| W-1 | Worker sandbox (C4) | Worker code exhausts CPU/RAM/disk beyond its grant | AT4 | Medium: node starvation, degraded availability | M-SBOX |
| W-2 | Worker sandbox (C4) | Compromised worker exfiltrates vault data or reaches the network | AT4 | High: if bypassed, data loss at machine speed | M-SBOX |
| W-3 | Worker sandbox (C4) | Worker reads another job's inputs/outputs on a shared node (shared scratch) | AT4 | Medium: cross-job data disclosure | M-SBOX, M-SBX2 |
| W-4 | Worker sandbox (C4) | Parser or worker bug (CGR/STEP/3DXML reads) is an attack surface; privileged escalation out of the sandbox via host-kernel CVE | AT4 | High: escape defeats the boundary | M-SBOX, M-SBX2, M-PTN |
| W-5 | Node/host (C4, C6) | Compromised node root steals artifacts or the signing key | AT5, AT8 | Critical: key theft undermines the whole trust chain | M-SBOX, M-KEYP, M-SIGN, M-AUDIT |
| CL-1 | Vault (C1) | Insider reads or modifies vault objects without a trace | AT3 | High: undetected data theft or change | M-AUDIT, M-RBAC |
| CL-2 | Vault (C1) | Insider tampers with a released revision (integrity of record) | AT3 | High: silent corruption of released data | M-STATE, M-AUDIT |
| CL-3 | Vault/backup (C1, C6) | Insider tampers with backups or rewrites the audit chain on disk | AT3 | High: destroy the evidence of CL-1/CL-2 | M-AUDIT, M-BAK |
| CL-4 | Host (C1, C6) | Root on the PDM host rewrites config, vault, audit, and backups (bounded residual) | AT3, AT5 | High: full local compromise; out-of-band artifacts limit blast radius | M-AUDIT, M-BAK |
| AG-1 | Air-gap install (C6) | Tampered bundle or image crosses the air gap on media | AT3, AT8 | Critical: malicious image set accepted if signatures are not verified | M-AIRGAP, M-SIGN |
| AG-2 | Air-gap install (C6) | Build host is compromised or coerced and signs a malicious bundle | AT8 | Critical: the signed bundle is then trusted by every install | M-AIRGAP, M-SUPPLY |
| AG-3 | Air-gap deploy (C5) | A rogue admin poisons the on-site update mirror with a hostile bundle | AT7 | Low: worst case an incorrect advisory flag (no payload exposure without the key) | M-SIGN, M-KEYP |
| AG-4 | Air-gap deploy (C3, C6) | Air-gapped hosts drift on known CVEs because patching only arrives on media | AT6 | Medium: stale-vulnerability exposure on the fence | M-SBOM, M-AIRGAP, M-SIGN |
| UC-1 | Update checker (C5) | The checker phones home or beacons when the operator believes it is off | AT2 | Medium: metadata leak / reach-back signal | M-UPDOFF, M-OUTB |
| UC-2 | Update checker (C5) | Hostile remote/mirror feeds a malicious bundle | AT7 | Low: no exposure without a valid signature (never fails closed) | M-SIGN, M-UPDNV |
| UC-3 | Update checker (C5) | Public-key or config replacement (TOFU) defeats bundle verification | AT3, AT8 | High: attacker substitutes key and bundle to push their own manifest | M-KEYP, M-SIGN |
| UC-4 | Update checker (C5) | Stale-but-signed downgrade confusion, or an update check itself is suppressed | AT7, AT3 | Low: operator sees no/newer/wrong flag; no automated apply exists | M-UPDNV, M-KEYP, M-SIGN |
| SU-1 | Supply chain | Unpinned or vulnerable dependencies reach the build and the artifact | AT6 | Medium-high: CVE in a crate or base image | M-SBOM, M-SUPPLY |
| SU-2 | Supply chain | Tampered release binaries / update manifests during distribution | AT6 | Critical: trusted code with attacker edits | M-KEYP, M-SUPPLY |
| SU-3 | Supply chain | Non-reproducible builds mask source-to-artifact drift | AT6 | Medium: cannot prove artifact == source | M-SUPPLY |
| SU-4 | Compliance | ITAR/EAR misclassification of a release | AT3, AT6 | High: export-control violation for a defense product | M-EXPORT |
| OP-1 | Operations | Vault data loss or accidental deletion with no recovery path | AT3, AT5 | Critical: unrecoverable product data | M-BAK, M-DR |
| OP-2 | Operations | A workload starves the bus or scheduler (fair-share bypass, runaways) | AT2, AT4 | Medium: degraded compute availability | M-JOBQ, M-SBOX |
| DI-1 | Process | A real vuln is found but there is no responsible-disclosure route or SLA | AT1..AT8 | Medium: vulnerability lingers undisclosed | M-DISC |

## 7. Mitigation inventory

Each mitigation is either COMPONENT (one of the real committed artifacts below)
or CARD (a P11-T* wave-3 card that closes it). The test requires every
COMPONENT path to exist in the repository and every CARD id to be in the
committed P11 set (T01..T08).

| ID | Mitigation | Kind | Closes in (component path or card) | Why it holds |
|----|------------|------|-------------------------------------|--------------|
| M-LOOP | Default loopback bind, LAN-off until published | COMPONENT | workstreams/delivery/deploy/deploy/docker-compose.yml, workstreams/delivery/deploy/deploy/.env.example | All three services bind 127.0.0.1 by default (decision #10 on-prem posture); the LAN surface stays closed unless an operator changes OEP_*_BIND. |
| M-TLS | TLS on the PDM/API surface with a trustable chain | COMPONENT | workstreams/delivery/deploy/tls-and-backup.md, scripts/gen-stack-certs.sh, crates/oep-pdm/src/service.rs | oep-pdm terminates rustls via axum-server when --tls-cert/--tls-key are given; local CA + leaf provisioning script; verified handshake in P10-M1-T02. |
| M-RBAC | Authentication + RBAC with bypass/IDOR test coverage | CARD | P11-T03 (auth boundary tests) | SECURE BY DEFAULT: the P11-T03 suite asserts cross-role, cross-tenant and unauthenticated denial (403/404) against the SHIPPED DEFAULT BINARY of the PDM, not an opt-in path, and passes on the P8-M3 RBAC implementation. The platform enforces this boundary on every start; `--no-auth` is the only opt-out and it is loudly warned. Closes NW-1 and NW-5 now; the P8-M3-T01 identity layer later replaces the header principal while keeping the same decision path. |
| M-PTN | Self-run penetration-test day on the web UI, P2P protocol, and sandbox | CARD | P11-T07 | Last pre-release gate: authz, XSS, CSRF, injection, P2P abuse, and sandbox-escape attempts, with no critical findings open on a public path. |
| M-OUTB | No-phone-home, no-external-dependency posture | COMPONENT | workstreams/delivery/deploy/stack.md, workstreams/delivery/airgap/air-gap-install.md, workstreams/compute/spec/message-bus-adr.md | Self-hosted single-binary bus, no cloud control plane, no telemetry (P0-M5-T02); the stack runs offline or air-gapped with zero reach-back. |
| M-ISM | Mesh-only NATS with taxonomy as the contract | COMPONENT | workstreams/compute/spec/message-bus-adr.md, workstreams/compute/spec/topic-taxonomy.json, workstreams/delivery/deploy/deploy/docker-compose.yml | The bus binds to the internal mesh only, accepts only taxonomy subjects (validated against the committed JSON), and its acceptance rules out non-loopback/cloud contact. |
| M-JOBQ | Conservative job caps, scheduler fairness, dead-letter | COMPONENT | crates/oep-job/src/spec.rs, crates/oep-lane, crates/oep-fairshare | ResourceCaps default conservative (P9-M2-T02); JetStream caps delivery (max_deliver -> dead-letter) and per-team quotas cap a hostile or runaway workload. |
| M-SBOX | Kernel-enforced worker ceilings + no network egress, fail-closed | COMPONENT | crates/oep-sandbox/src/lib.rs, crates/oep-sandbox/src/limits.rs, crates/oep-sandbox/src/seccomp.rs | RLIMIT_CPU/AS/DATA/FSIZE/NOFILE hard ceilings; seccomp denies every non-AF_UNIX socket; a worker requested egress-off never spawns if the filter cannot install. |
| M-SBX2 | Documented escalation layers for hostile bodies | COMPONENT | crates/oep-sandbox/src/lib.rs, docs/compute/workers/sandbox.md | Dedicated unprivileged uid, per-worker netns, cgroup-v2 hardener, transitive container isolation: optional deploy options that raise the boundary for rooted/hostile deploys. |
| M-STATE | Immutable-after-release lifecycle + lock-gated publish | COMPONENT | crates/oep-pdm/src/state.rs, crates/oep-pdm/src/working_set.rs | Released revisions only allow the terminal obsolete hop; every publish requires a live, opaque one-time lock token (steal/expiry semantics in working_set.rs). |
| M-AUDIT | Append-only tamper-evident audit with verification | COMPONENT | crates/oep-pdm/src/audit.rs, crates/oep-pdm/src/service.rs | Every read/change/release is an access event in a hash-chained log; GET /api/v1/audit/verify re-derives the chain; the audit head is carried in backup meta.json. |
| M-BAK | Checksummed, content-addressed backup to a second disk | COMPONENT | scripts/backup/vault-backup.sh, scripts/backup/vault-restore-check.sh, scripts/backup/install-vault-backup-cron.sh, workstreams/delivery/deploy/tls-and-backup.md | Daily producer writes manifest.sha256, self-verifies with sha256sum -c, verifies blob names against content hashes; the NAS is never a target (decision #9). |
| M-DR | Timed disaster-recovery restore drill | COMPONENT | scripts/dr/backup-vault.sh, scripts/dr/drill-restore.sh, workstreams/delivery/disaster-recovery/dr-runbook.md | Quarterly timed restore to a fresh machine consuming the same backup bundle contract (P10-M3-T01). |
| M-SIGN | Signature-before-trust for bundles and update manifests | COMPONENT | crates/oep-update/src/signing.rs, crates/oep-update/src/lib.rs, scripts/airgap/build-offline-bundle.sh, scripts/airgap/keys/oep-airgap-signing.pub.asc | Ed25519 canonical-manifest signature verified before payload exposure, payload sha256 re-checked (oep-update); gpg detached signature over air-gap MANIFEST + per-file sha256 (scripts/airgap). |
| M-AIRGAP | Signed offline install with verify, rollback, and no phone-home | COMPONENT | scripts/airgap/airgap-verify.sh, scripts/airgap/airgap-install.sh, scripts/airgap/airgap-rollback.sh, scripts/airgap/airgap-lib.sh, workstreams/delivery/airgap/air-gap-install.md | Bundle + manifest signature verified before install, every file sha256-checked against the signed MANIFEST, update check off by default, image load without any registry pull. |
| M-SUPPLY | Pinned toolchain/deps, reproducible builds, hardened artifacts | CARD | P11-T06 | P0-M3-T02 toolchain pin extended to a full locked+verified dependency set; clean-room rebuild must reproduce the artifact; CI fails on unpinned/unverified dependency changes. |
| M-UPDOFF | Update checker off by default, no I/O when disabled | COMPONENT | crates/oep-update/src/lib.rs, workstreams/delivery/update-checker/update-checker.md, workstreams/delivery/airgap/air-gap-install.md | UpdateConfig::default is Off with an empty source; enabled only by explicit config; generated .env sets OEP_UPDATE_CHECK=off; there is no telemetry surface. |
| M-UPDNV | Advisory non-blocking checker that never fails closed | COMPONENT | crates/oep-update/src/lib.rs, crates/oep-update/tests/update_checker_acceptance.rs | Every failure maps to a status, never to a blocking error; Probe::preflight always returns Proceed; four acceptance tests pin launch-without-source, launch-with-old-bundle, signed-before-trusted, and no-analytics behavior. |
| M-KEYP | Cryptographic content integrity + release signing with key custody | CARD | P11-T04 | Blob content-hash verified on read, release binaries and the update bundle carry signatures, using the air-gap signing-key posture (secret on the build host only, public key committed); a tampered-blob test must fail verification. |
| M-SBOM | SBOM + CVE scanning gating every release | CARD | P11-T02 | CI emits an SBOM and scan report per release and gates on known critical/high CVEs (or documented waivers); a seeded critical vuln fails the gate. |
| M-EXPORT | Export-control classification file required per release | CARD | P11-T05 | Every release needs an explicit ITAR/EAR classification file (data-locality, no restricted content) or the release fails; repro removes it and fails. |
| M-DISC | Responsible-disclosure policy and contact path | CARD | P11-T08 | workstreams/security/disclosure-policy.md + SECURITY.md document how to report, the SLA/triage, and coordinated-disclosure terms; triage dry-run on a real low-severity finding. |

Accepted / residual items (not orphans; each is deliberately accepted or
covered by a CARD):

- CL-4 and W-5 can be fully defeated only by an out-of-band root adversary.
  The controls cap the blast radius (M-SBOX, M-KEYP, M-SIGN, M-AUDIT, M-BAK)
  and the disclosure route exists (M-DISC); true closure for a host-root
  attacker is out of scope for the product boundary and is operationally owned.
- UC-3 (key or config replacement) cannot be fully closed by on-host code: a
  root attacker who can rewrite both the public key and the bundle wins by
  construction. The practical closure is key custody (M-KEYP), out-of-band
  key distribution, and signed release artifacts; the checker itself never
  fails open (a source with no configured key is Untrusted).

## 8. Mapping table (risks to mitigations) and orphan check

The register (section 6) and inventory (section 7) are the single mapping
table; each risk row names its mitigation IDs and every mitigation names a
real component or a P11-T* card. `tests/test_p11_threat_model.py` machine-
checks, on committed real material:

1. Every risk in the register names at least one mitigation, and every named
   mitigation id exists in the inventory (no orphan risk, no dangling ref).
2. Every inventory entry is named by at least one risk (no orphan mitigation).
3. Every COMPONENT entry points at a file that exists in this repository.
4. Every CARD entry is one of the eight committed P11 cards (T01..T08).
5. The whole document passes the program surface-language rule (no em dashes,
   no curly quotes).

Cross-references into the wave-3 lanes: the gap positions (NW-1/NW-5 toward
P11-T03, SU/AG/W-5 toward P11-T04/T06, AG-4/SU-1 toward P11-T02, SU-4 toward
P11-T05, NW-3/NW-4/NW-6/NW-7/W-4 toward P11-T07, DI-1 toward P11-T08, plus
the Wave-3 GA gate card that consumes them all in the release drill) are
exactly the P11-T* closures in the table above.

## 9. Verification

- `python3 tests/test_p11_threat_model.py` (and under pytest in CI) runs the
  section, surface-language, and no-orphan checks against the committed workstreams/security/
  threat-model.md and the repository tree. Exit 0 = pass.
- Human review: the mapping table is the review surface; a reviewer checks
  that each risk row's mitigations are the right ones for the asset/actor,
  and that every COMPONENT and CARD closure is genuinely landed or claimed.

## 10. Change log

- 1.1.0 (2026-10-18): SEC-AUTHZ-DEFAULT. C1 posture updated: the shipped PDM
  binary now enforces the P11-T03 RBAC boundary by default (the suite asserts
  the attack matrix against the default binary, not an opt-in path); the open
  contract is the explicit, loudly-warned `--no-auth` opt-out for controlled
  test / air-gap use. M-RBAC is therefore a live closure of NW-1 and NW-5 now,
  not gated on the identity layer. Everything else (register, inventory,
  decision #11) unchanged. Keeps the two P11 documents (threat model and
  auth-boundary) in agreement.
- 1.0.0 (2026-10-18): first ratification. Assets, trust boundaries, actors,
  risk register (32 risks), mitigation inventory (22 entries), orphan-checked
  mapping table, and the decision #11 update-checker contract. Written against
  main @ e67b8fc. Bump the version and append a line here on any material
  change (mirrors the vault-conventions bump rule).
