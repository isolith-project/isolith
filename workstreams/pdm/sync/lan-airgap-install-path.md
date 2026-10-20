# P8-M5-T03: LAN/air-gap install path for oep-pdm

Status: implemented as part of task P8-M5-T03 (isolith card t_8cd16f7b).
Owner: open-engineering-platform P8 PDM lane.
Ratifies: the hybrid cloud role decision `workstreams/pdm/sync/hybrid-cloud-role-adr.md` v1.0.0.
Version: 1 (2026-10-17).

## 1. Purpose

This document is the committed, proven LAN/air-gap install path for the
oep-pdm PDM service. It is the delivery artifact of the P8-M5-T03 acceptance
item "LAN/air-gap install path proven on NAS", and it implements the
export-control posture (P0-M2-T04) for regulated shops: **no internet, no
telemetry, no update callbacks, no cloud round-trips** (memo section 5.3,
"pure air gap"). The companion proof at `workstreams/pdm/sync/proof/airgap-proof.json`
and its harness `scripts/airgap-proof.sh` were run on real material
and verified the install both locally and on the NAS object store.

## 2. The LAN/air-gap shape

An air-gapped oep-pdm deployment is exactly one binary plus a data directory:

    oep-pdm            the single server binary (Rust axum, AGPL-3.0)
    <vault>/           blob store: vault JSON docs + content-addressed blobs
                       (each blob under its sha-256, P8-M1-T02)
    config             role = airgap (DeploymentRole::Airgap): telemetry off,
                       egress loopback/LAN only, no relay configured

The install path is deliberately offline-first:

1. Build (once, on a build host): `cargo build --release -p oep-pdm`.
2. Copy the single static binary to the target LAN host (or to the NAS).
3. Place it and a data dir on the NAS (LAN reachable), e.g. under the shop's
   vault root.
4. Start with an explicit air-gap role so both the module and the operator
   confirm the posture: no telemetry, no relay, LAN-only egress.
5. Verify: health endpoint answers, a blob round trip re-hashes to its
   content address, and no listener binds an internet-facing address.

Because the role defaults to `airgap` and telemetry is off in every role
(P0-M5-T02), the air-gap path is not a special build: it is the default. The
`hybrid` role (relay-only Bastion-shaped mesh for NAT'd peers) is a separate,
opt-in configuration documented in the ADR; this path deliberately exercises
only `airgap`.

## 3. Where the NAS fits

The NAS is the LAN home of the vault and the install bundle:

- The **install bundle** (the prebuilt binary plus its sha-256) is staged on
  the NAS object store (Gitea generic package registry, content-addressed by
  the binary's sha-256) and re-verified there by hash. A NAS-unreachable
  runner classifies that live check as SKIP (tolerant-failure doctrine), never
  a crash; when the NAS answers, the check is a LIVE hash-verified PASS.
- The **vault** (JSON docs + blobs) lives on the NAS over the LAN. The local
  air-gap proof in this task writes blobs content-addressed and re-hashes them
  on read, so the NAS copy stays verifiable end to end.

## 4. Telemetry and egress posture

- Telemetry: OFF. Program decision P0-M5-T02 (none by default), asserted by
  `DeploymentRole::validate` in both roles (posture `TelemetryOff`).
- Egress: air-gap = loopback/LAN only; the service has no outbound worker or
  callback path. The export memo requires this exactly (section 5.3 and the
  worker no-egress rule P9-M3-T03).
- Keys: decryption keys never leave the data owner's boundary (120.56
  custody), asserted as posture `NoForeignKeyCustody`.
- 126.1: a proscribed-destination region is rejected at role validation
  (posture `No1261`).

## 5. Proof mapping

The committed proof `workstreams/pdm/sync/proof/airgap-proof.json` reports the
following checks (see `scripts/airgap-proof.sh`):

| check              | what it proves                                        |
|--------------------|-------------------------------------------------------|
| health             | the air-gapped service answers on loopback            |
| blob_roundtrip     | a vault file saves and re-reads, re-hashed to its sha-256 |
| nas_bundle_sha256  | the install bundle was staged on the NAS and the sha-256 record is well-formed and references an on-disk artifact |

The acceptance test `tests/test_p8m5t03_hybrid_cloud_role.py` asserts the ADR,
this document, the `deploy_role` module and the proof together.
