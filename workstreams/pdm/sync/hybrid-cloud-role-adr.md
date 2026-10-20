---
title: Isolith P8 Hybrid Cloud Role Decision (Bastion-shaped P2P relay vs plain HTTPS; default LAN/air-gap)
decision: adopt_bastion_shaped_p2p_relay_for_natted_remote_with_lan_airgap_default
version: 1.0.0
date: 2026-10-17
status: Ratified
owner: Isolith program governance, P8-M5 (sync / hybrid)
applies_to: oep-pdm (server/PDM), P8-M5-T01 (sync client), P8-M5-T02 (offline mode), P9 P2P orchestration and P10-M2-T02 (offline install), the reference vertical P14
supersedes: no prior P8 network-role record; the pure-HTTP stub transport deferral (P7-M2-T02) is resolved to LAN-direct plus optional relay, not public HTTPS
---

# Isolith P8 Hybrid Cloud Role Decision

This record is the binding product decision for how a Isolith PDM deployment
reaches remote (NAT'd) peers and how it deploys in regulated shops: it answers
the P8-M5-T03 question "hybrid cloud role: P2P relay when NAT'd (Bastion-
shaped), or pure LAN/air-gap deployment" and names **Bastion-shaped P2P mesh
relay** for the NAT'd hybrid role with **pure LAN/air-gap as the default
posture**, and **plain HTTPS rejected** as the remote-access transport. It is
versioned like every other binding decision in this program; any material
change bumps the version in the front matter and adds a line to the change log
at the bottom, per program doctrine.

The decision is ratified against the committed `deploy_role` module in
`crates/oep-pdm` (which encodes the posture matrix as executable assertions),
the LAN/air-gap install path in `workstreams/pdm/sync/lan-airgap-install-path.md`,
and the recorded real-material proof in `workstreams/pdm/sync/proof/airgap-proof.json`
captured from a live air-gap boot with a NAS-staged, hash-verified install
bundle. The acceptance test in `tests/test_p8m5t03_hybrid_cloud_role.py`
asserts this document and the artifacts together, on real material, not on
mocks.

## 1. Decision

Isolith ADOPTS a **two-role deployment model** for the oep-pdm lane, encoded
in `crates/oep-pdm/src/deploy_role.rs` as a first-class, serializable
`DeploymentRole`:

- **`airgap` (default for regulated shops).** The deployment stays entirely
  inside the shop boundary: no internet, no telemetry, no update callbacks, no
  cloud round-trips. This is the posture the export-control memo P0-M2-T04
  (section 5.3) names "pure air gap", and it is the default the reference
  vertical (P14, small defense subcontractor shop) starts from. Mobile view is
  reachable only over the LAN or not at all.

- **`hybrid` (LAN plus relay; opt-in, credential-gated).** A self-hosted,
  **relay-only** P2P mesh **shaped after the Bastion mesh pattern** (R1A
  product, Don sole developer) carries remote/mobile traffic when a peer is
  NAT'd. The relay forwards encapsulated traffic and **holds no decryption
  keys, sees no plaintext, and stores nothing**; all endpoints terminate
  end-to-end encryption inside their own security boundary. This is
  P0-M2-T04 section 5.3's "hybrid / remote (LAN + Bastion-shaped relay,
  mobile view)" and it is only permitted when the memo's 120.54(a)(5) /
  120.56 conditions hold and no 126.1-country touch exists.

**Plain HTTPS is REJECTED as the remote-access transport** (section 4). For a
NAT'd remote peer, a publicly reachable HTTPS listener or a cloud reverse proxy
that terminates TLS creates a key-holding, decrypting third party, which is
exactly the custody violation the memo forbids (120.54(a)(5), 120.56(b)) and a
standing attack surface. HTTPS remains allowed only inside the LAN boundary
(and for the admin console where the operator is in the boundary), never as
the NAT-traversal path for controlled data.

The Bastion element is reuse of a **pattern**, not a runtime dependency on the
R1A Bastion product: Isolith does not link, embed, or require Bastion
containers. The mesh shape (self-hosted control plane + relay-only forwarder +
E2E WireGuard-class transport, just-in-time node approval, tamper-evident
audit trail) is adopted as architecture; the implementation stays clean-room
and the P2P orchestration stays AGPL-3.0 per the license-posture decision #3,
so the OEP hybrid role never introduces a foreign or proprietary control
dependency.

## 2. Why this decision

Five facts drive it, all grounded in P0-M2-T04 (the program export-control
posture, task P0-M2-T04, internal memo 2026-10-15) and the Bastion mesh
pattern (R1A product record, Topics/Bastion Mesh VPN):

1. **The custody rule makes a decrypting relay an export control event.** Under
   22 CFR 120.56, technical data is "released" (an export) to a foreign person
   when access information enables access to unencrypted controlled data; who
   holds decryption keys is a defense-trade-controls decision, and keys must
   stay with US persons / the data owner, never with the service or a third
   party. Plain HTTPS through a public reverse proxy puts termination (and
   therefore entry to the clear) in the hands of the transport, which the memo
   forbids. A relay-only P2P mesh never sees the clear, so the custody rule is
   structurally satisfied: the relay cannot be a release because it cannot
   enable unencrypted access.

2. **The E2E carve-out (120.54(a)(5)) is exactly what a relay-only P2P mesh
   provides.** Unclassified technical data is not an export when it is E2E
   encrypted with FIPS-compliant crypto, decryption happens only inside the
   originator's and recipient's security boundaries, and no third party
   (including our own relay) is provided a means of decryption. A
   Bastion-shaped mesh is built for exactly this: endpoints encrypt, the
   forwarder relays ciphertext. Plain HTTPS from browser to a proxy breaks the
   "never unencrypted between boundaries" requirement at the proxy.

3. **`airgap` is the only posture the memo treats as default-clean, and it is
   free to ship.** P0-M2-T04 section 5.3 makes the pure air gap the ITAR
   shop's baseline (no internet, no telemetry, no update callbacks, everything
   inside the boundary). Making it the default role costs no machinery:
   telemetry is already off by program decision P0-M5-T02, the worker sandbox
   has no egress by default (P9-M3-T03), and the offline install path
   (P10-M2-T02) is the same artifact the `airgap` role needs. `hybrid` is
   strictly an opt-in for shops that meet the memo's conditions and want
   mobile/NAT'd reach.

4. **The 126.1 proscribed-destination rule is encoded, not assumed.** The
   memo's hybrid carve-out excludes any 126.1-country touch and the relay can
   never be placed there. The posture matrix in `deploy_role.rs` asserts a
   non-empty, 126.1-free region/operator check at role-activation time, so a
   misconfigured relay placement fails closed instead of silently exporting.

5. **Bastion-shaped reuse is the program's own proven pattern, and it keeps
   the license story clean.** The Bastion product record documents a working
   self-hosted, FIPS-friendly, relay-only mesh (Tailscale wire protocol,
   headscale core under BSD-3, DERP relay for NAT traversal, JIT approval,
   tamper-evident audit) validated end to end in its S10 install rehearsal.
   Isolith reuses the shape without embedding the proprietary product, which
   keeps the AGPL-3.0 P2P orchestration boundary (decision #3) intact while
   giving the P9 compute / P10 delivery lanes a relay design already proven in
   production-shaped conditions. For a NAT'd remote peer the mesh is strictly
   better than plain HTTPS on every axis this memo scores: transport is E2E,
   the relay is not a key holder, and no public TLS ingress is invented.

## 3. Posture matrix (executable in `deploy_role.rs`)

The module encodes this matrix; the acceptance test asserts each row:

| role   | telemetry            | network egress           | relay keys        | 126.1 gate                    |
|--------|----------------------|--------------------------|-------------------|-------------------------------|
| airgap | forced off           | none (loopback/LAN only) | n/a               | not applicable (no relay)     |
| hybrid | forced off           | to the relay only        | never held        | region must be non-126.1      |

Both roles reject a telemetry-enabled config, reject egress beyond the LAN (or
the relay) and reject any configured decryption key custody outside the data
owner's boundary. The module implements `DeploymentRole::validate` returning a
typed `PostureViolation` list, so a bad role config fails closed at activation.

## 4. Options considered

1. **Bastion-shaped P2P mesh relay (ADOPTED for `hybrid`).** Self-hosted,
   relay-only forwarder + E2E transport, NAT traversal via a relay (Tailscale
   wire protocol / DERP-class), JIT node approval, tamper-evident audit.
   Meets 120.54(a)(5) + 120.56 because the relay holds no keys and never sees
   plaintext; no public HTTPS ingress; matches the Bastion pattern the R1A
   product already proved. P2P orchestration stays AGPL-3.0, no foreign or
   proprietary runtime dependency. Cost: operating and securing a relay node,
   which the memo already requires to be non-key-holding and non-126.1.

2. **Plain HTTPS for remote access (REJECTED).** A public HTTPS listener or
   cloud reverse proxy that terminates TLS creates a decrypting third party,
   violating the custody rule (120.56(b)) and breaking the E2E requirement
   (120.54(a)(5)) the moment the proxy holds a session key; it is also a
   standing internet attack surface and gives the transport sight of what is
   transferred even if blobs are opaque. HTTPS stays permitted inside the LAN
   boundary (admin console, LAN client to the PDM server), never as the NAT
   path for controlled data.

3. **Pure LAN/air-gap only, no hybrid ever (CONSIDERED, rejected as sole
   policy).** This is the safest posture and remains the default `airgap`
   role, but shipping it as the ONLY option would drop the mobile/NAT'd
   requirement the P8-M5 lane exists to serve and the memo's hybrid carve-out
   explicitly contemplates. The program keeps `airgap` as default and `hybrid`
   as a gated opt-in, rather than either-only.

4. **WireGuard point-to-point tunnels hand-configured per peer (DEFERRED).**
   Fine for a handful of static peers on a trusted LAN, but does not give NAT
   traversal, JIT approval, or an audit trail out of the box, and duplicates
   what the relay mesh already provides. Revisit only if a future deployment
   has a fixed-peer LAN-only need that the relay mesh over-services.

## 5. Role activation and the GUI

`DeploymentRole` is chosen by operator configuration (env or config file), not
inferred: an `airgap` deployment never attempts to reach a relay, and a
`hybrid` deployment requires an explicit `relay` endpoint, an operator-declared
region/operator label, and an explicit telemetry-off assertion before the
PDM service will start in hybrid mode. A config that would place a hybrid
relay in a 126.1 region, or enable telemetry in either role, is rejected at
activation; the failure is a typed posture violation, never a silent continue.

## 6. Real-material proof

Committed with this record:

- `crates/oep-pdm/src/deploy_role.rs` -- the std-only deployment-role module
  encoding the two roles and the posture matrix as executable assertions.
- `workstreams/pdm/sync/lan-airgap-install-path.md` -- the LAN/air-gap install path:
  single `oep-pdm` binary, no internet, no telemetry; staged on the NAS and
  hash-verified there.
- `workstreams/pdm/sync/proof/airgap-proof.json` -- the exact bytes of a live
  air-gap boot on the reference host U1 (2026-10-17): health check, a
  content-addressed blob round trip re-verified by sha-256, and the NAS-staged
  install bundle re-verified by hash on the NAS object store.
- `scripts/airgap-proof.sh` -- the proof harness that produced it
  (starts the `oep-pdm` binary with no internet-facing listener, exercises the
  contract, stages the install bundle content-addressed on the NAS and verifies
  it back).

The acceptance test `tests/test_p8m5t03_hybrid_cloud_role.py` reads the ADR
and these artifacts on real material, mirroring the renderer-backend and
web-stack ADR test conventions.

## 7. Guardrails

- **Default posture is `airgap`.** Any production deployment that does not
  explicitly opt into `hybrid` runs air-gapped; the module's default is
  `airgap` and telemetry is off in both roles.
- **The relay never holds keys, never stores, never terminates.** Enforced by
  the module's assertion surface and by the deployment doc; the relay is a
  forwarder of encapsulated traffic only.
- **126.1 is a hard gate.** A hybrid config that cannot assert a non-126.1
  region/operator fails closed at activation, encoded in `deploy_role.rs`.
- **No public HTTPS ingress for controlled data.** The LAN boundary keeps
  direct HTTPS; remote/NAT'd access is relay-only and E2E.
- **License boundary.** Bastion is reused as architecture/pattern, never as a
  linked runtime; P2P orchestration stays AGPL-3.0 (decision #3). No vendor
  binary or copied source (clean-room invariant).
- **Acceptance gate.** `tests/test_p8m5t03_hybrid_cloud_role.py` runs in CI
  and asserts the ADR, the module, the install path doc and the live proof
  together; it will fail if the posture matrix regresses.

## 8. Change log

- 1.0.0 (2026-10-17): Ratified. Two-role deployment model adopted: `airgap`
  default for regulated shops, `hybrid` as gated opt-in using a
  relay-only, Bastion-shaped P2P mesh for NAT'd remote access; plain HTTPS
  rejected as the NAT-traversal transport (120.54(a)(5) / 120.56 custody);
  126.1 gate encoded. Posture matrix lives in `deploy_role.rs`; install path
  proven on NAS.
