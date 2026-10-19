---
title: Keelworks Crypto / Integrity Doctrine (Content-Hash Blob Integrity, Artifact and Update-Manifest Signing)
status: Ratified (Wave-3 P11-T04)
version: 1.0.0
date: 2026-10-18
owner: Keelworks program governance, P11 security lane
applies_to: the vault blob store (oep-pdm), every shipped release binary/artifact from the P12 packaging matrix, and the update bundle consumed by the P10-M5-T01 update checker
supersedes: extends the P10-M5-T01 signed-bundle design; does not replace it
---

# Keelworks Crypto and Integrity Doctrine

This record closes the crypto hardening card P11-T04: content-hash integrity
on vault blobs (verified on read/retrieve) and signatures for release
binaries/artifacts and the update bundle that the P10 advisory update checker
consumes per decision #11. Every rule below is machine-enforced by a real test
or a CI gate, not advisory.

## 1. What this closes

| Threat-model stake | Closed by | Concrete mechanism |
|--------------------|-----------|--------------------|
| A6 update/verification keys (secret on build host, pubkey committed) | P11-T04 key posture | Section 5 |
| C7 the signing keys are the most exposed target | P11-T04 | Secrets never enter the repo; key usage is a build-host operation (Section 5) |
| AT3/AT8 tampered artifacts or a maliciously signed bundle | P11-T04 + P11-T06 | Detached gpg signatures per artifact + Ed25519-signed update manifest (Sections 3-4) |
| A7/M-SIGN signature before trust for bundles | P11-T04 + P10-M5-T01 | oep-update verifies the canonical-manifest Ed25519 signature before any payload is exposed (Section 4) |
| SU-2 tampered release binaries during distribution | P11-T04 + P11-T06 | Detached signatures + SHA256SUMS over every released artifact (Section 3) |
| Vault blob corruption/tampering served silently | P11-T04 | Content-hash verification on every blob read (Section 2) |

## 2. Content-hash integrity on vault blobs

The oep-pdm vault store (P8-M1-T02 object model) is content-addressed: each
file's bytes are stored once under a single lowercase hex sha-256 key
(`crates/oep-pdm/src/store.rs`, `VaultStore.blobs`), and every persisted
revision references its blob by that digest (`FileRef.blob_sha256`).

P11-T04 adds the read-side guarantee the content-addressed layout implies:

- Every retrieval (`open_latest`, `open_revision`, sync-conflict side reads,
  conflict resolution reads) funnels through one gate,
  `VaultStore::blob_bytes`, which recomputes the sha-256 of the returned bytes
  and requires it to equal the looked-up key. A mismatch is a
  `StoreError::Integrity`, surfaced as HTTP 500 with error code
  `integrity_error` - never silent bad bytes.
- `VaultStore::load` verifies every readable on-disk blob file the same way:
  the bytes must hash to the file's 64-hex name, or the load fails with
  `StoreError::Integrity`. A tampered or corrupt blob cannot be loaded, so it
  can never be served.
- Writes are self-consistent by construction: blobs are keyed with the digest
  computed over the exact bytes just written.

Design intent: a content-addressed store is only as honest as its read path.
Without the verification, a flipped disk block would be returned as if it were
the addressed content. With it, the failure is loud, named, and testable.

## 3. Release artifact detached signatures

Every hard artifact a release ships (AppImages, MSIs, tarballs, the offline
bundle) carries a detached armored gpg signature produced on the build host:

- `scripts/release/sign-release.sh --assets DIR --version V` signs every
  artifact (`<artifact>.sig`), writes `SHA256SUMS` and a detached
  `SHA256SUMS.sig`, emits `RELEASE-MANIFEST.json` (release, version, date,
  per-artifact sha256/size/url) and `release-signatures.json` inventory.
- `scripts/release/verify-release.sh --assets DIR` is the consumer-side check
  that needs no secret: it verifies every `.sig`, runs `sha256sum -c`, and
  (when the checker binary is present) verifies the update bundle. Exit 0 only
  when everything verifies.

The signer requires the gpg signing key (`oep-airgap-signing`) in the build
host keyring, matching the air-gap bundle posture (Section 5).

## 4. Signed update manifest (decision #11)

The P10-M5-T01 update checker (`crates/oep-update`) is advisory and
non-blocking by rule (decision #11): it never blocks launch, defaults to off
and non-phoning, and it only ever exposes payloads from a signature-verified
manifest. P11-T04 makes the release engine the producer of that signed
manifest:

- `oep-update-sign` (extended by P11-T04) writes an Ed25519-signed
  `update.bundle.json` per release: a `--inline-payload` release manifest
  pinned as `payloads[0]` plus one `--item` entry per artifact (name, sha256,
  size, optional download URL). The signature is over the canonical JSON of
  the manifest; `oep-update-check` verifies it before reporting anything and
  re-checks the inline payload's sha256 (Section 2 doctrine, applied to the
  update payload).
- A tampered or foreign-signed manifest surfaces as `Untrusted` (or an
  advisory parse failure) and its payloads are never exposed; the checker
  keeps running and never blocks the program.
- `verify-release.sh` refuses to report a release until the bundle's signature
  verifies against the committed public key.

## 5. Air-gap signing key posture

Two secret keys exist, both on the build host only; every public key is
committed:

- gpg `oep-airgap-signing` (RSA 2048, uid
  "oep-airgap-signing (Keelworks OEP air-gap release signing)"): pubkey
  committed at `scripts/airgap/keys/oep-airgap-signing.pub.asc`, secret in the
  build-host keyring. Used for artifact detached signatures and the offline
  air-gap bundle.
- Ed25519 `oep-update-signing`: pubkey committed at
  `P11/keys/oep-update-signing.pub` (hex), secret held on the build host at
  `/home/r1a/keelworks-secrets/oep-update-signing/secret.hex` (mode 600),
  consumed via `OEP_UPDATE_SIGN_KEY` / `OEP_UPDATE_SIGN_KEYFILE`. Used to sign
  `update.bundle.json`.

Operational rules:

- Secrets never enter the repository, the CI cache, or any PR diff.
- CI runs signing-tool round trips with scratch keys generated at runtime,
  never the real secret (Section 7).
- `oep-update-sign --gen-key DIR` is the sanctioned way to mint a fresh
  Ed25519 keypair on the build host; the public half is what you commit.
- Key rotation follows the disclosure policy tie-in (P11-T08); compromise of a
  signing key is a full revocation + re-issue event because the key IS trust
  (threat model A6).

## 6. Acceptance and verification (machine-checked)

Two verifications prove the card, both runnable from the repo:

- Tampered-blob test that fails verification: Rust unit tests in
  `crates/oep-pdm/src/store.rs` (in-memory tamper -> `StoreError::Integrity`)
  and `crates/oep-pdm/tests/blob_integrity.rs` (on-disk blob byte flip ->
  load fails with `StoreError::Integrity`; intact control passes).
- Signed-release round trip: `scripts/release/sign-release.sh` +
  `verify-release.sh` on a release artifact set (real packaging output on the
  build host, scratch keys in CI), including a tampered-artifact leg that
  makes verification fail and a tampered-manifest leg that makes
  `oep-update-check` refuse the bundle. Proofs recorded under
  `P11/proof/`.

`tests/test_p11t04_crypto.py` machine-checks the committed mechanism and can
run the full gpg round trip when gpg is available;
`.gitea/workflows/crypto-integrity.yaml` runs it in CI.

## 7. CI enforcement

`.gitea/workflows/crypto-integrity.yaml` holds every branch/PR to the bar:

- the precommit quality gate already runs `cargo test` (which includes the
  Rust blob-integrity and update-checker acceptance tests) and all pytest;
- the crypto job additionally runs the release signing round trip with scratch
  keys and verifies the recorded signed-release proof is present and green.

## 8. Version and ties

- version 1.0.0 (2026-10-18), Ratified.
- Ready to be consumed by the Wave-3 GA gate release drill (t_c701455b): the
  drill signs all packages with `sign-release.sh`, verifies with
  `verify-release.sh`, and hands `update.bundle.json` to the two-machine
  install rehearsal's update-checker step.
