---
title: "Isolith Release Signing Key"
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith release lane (P12 release train), P11 security lane
applies_to: every signed Isolith release artifact (air-gap offline bundles, release tags and the release manifests they carry)
supersedes: nothing (first release signing key custody and rotation record)
---

# Isolith Release Signing Key

Every published Isolith release artifact is signed so a person downloading it
can verify that it really came from the Isolith release lane. A signed
release with an unpublished key is unverifiable by the people downloading it,
so this page publishes the fingerprint, states where the private key lives
and who may use it, and records the rotation and compromise procedure.

## Fingerprint

```
9898 A66D 42D6 FFD3 EC5E A782 7811 0F3A DAC1 EE92
```

- Fingerprint: `9898A66D42D6FFD3EC5EA78278110F3ADAC1EE92`
- Key id (long): `0x78110F3ADAC1EE92`
- Algorithm: RSA 2048, signing and certification only (`[SC]`; no encryption
  subkey, deliberate: this key exists to sign releases, not to encrypt).
- Created: 2026-10-18.
- Expiry: none set (rotation must add one; see below).
- Signing identity in the key: `oep-airgap-signing`.

## Where the fingerprint is published

- This record (`docs/release-signing.md`).
- `SECURITY.md` (release verification section).
- The public docs site: `docs-site/security.html`.
- The public key itself ships in the repository at
  `keys/oep-airgap-signing.pub.asc` (the same file the air-gap install path
  tells operators to import, `scripts/airgap/airgap-lib.sh`).

## Where the private key lives and who can use it

- The private key lives on the **U1 build host**, in the GPG home of the
  release lane operator (`~/.gnupg`, keyring `pubring.kbx`). It never leaves
  that host. It is never committed to a repository, never uploaded to CI, and
  never copied onto an artifact that ships.
- The air-gap bundle builder signs on U1:
  `scripts/airgap/build-offline-bundle.sh` with
  `SIGN_KEY_ID=oep-airgap-signing` (the default), which writes detached
  signatures over `MANIFEST.json`, `SHA256SUMS`, and the final tarball.
- **Who may use it:** the release lane, which is the P12 release train, and
  today that is the single maintainer. The governing principle: only the
  person who can already cut a release and push a release tag may sign.
  Anyone else who needs a signed artifact asks the release lane over the
  private channel; the key is not shared.
- The crypto-integrity gate in CI (`crypto-integrity`) re-derives the release
  blob hashes and checks the signing posture on the release refs, so a signed
  artifact cannot be claimed without the lane's run on U1.

## How a user verifies a release

```sh
gpg --import keys/oep-airgap-signing.pub.asc
# confirm the imported fingerprint:
# 9898A66D42D6FFD3EC5EA78278110F3ADAC1EE92
gpg --verify-options show-fingerprint --verify isolith-offline-<VERSION>.tar.gz.sig isolith-offline-<VERSION>.tar.gz
```

or, for the air-gap bundle, `scripts/airgap/airgap-verify.sh --bundle
...` performs the import and detached-verify for the operator. A download
that does not carry a signature, or a signature by any other key, is not
verifiable and must be treated as untrusted.

## Pending before the first visible release

1. **Refresh the signing identity.** The key's UID still carries the retired
   predecessor brand name and a `localhost` email placeholder. Before the
   first public release, add a UID with the Isolith identity and the real
   site domain once SITE-DOMAIN-ORIGIN has it, then revoke the stale UID.
   The primary fingerprint is unchanged by a UID edit, so every artifact
   already signed stays verifiable.
2. **Set expiry.** Rotate to a key with an expiry (the current key has none),
   or add an expiration to this one, so a lost key self-destructs eventually.

Both are release-lane actions owned by the same person who owns the key.

## Rotation procedure

1. Generate the replacement key on U1 with an expiry set, using the Isolith
   identity decided in the pending step, and change the builder default
   (`SIGN_KEY_ID` / `PUBLIC_KEY_FILE`) to point at it.
2. Write a short transition statement signed by the old key and the new key,
   published with the new fingerprint in this page, `SECURITY.md`, and
   `docs-site/security.html`, and referenced from the release notes of the
   first release signed with the new key.
3. Keep the old key in the keyring, imported to `keys/` entry only if still
   needed to verify old downloads; mark it "retired" in this page. Do not
   delete a key that old releases still need for verification.
4. Bump the version of this page and every page that carries a fingerprint.

## Compromise procedure

If the private key is suspected lost or compromised:

1. Treat every artifact signed with that key as untrusted. Downgrade all its
   signatures to "not trusted" in the documentation.
2. Pause release signing immediately; do not cut or sign another release
   with the suspect key.
3. Rotate per the procedure above (new key, transition statement, republish
   fingerprints). For the old key, issue a revocation certificate
   (`gpg --gen-revoke` on the key) and distribute it so verifiers stop
   trusting it; record the revoke date here.
4. Record a P11 incident note under `workstreams/security/` naming: the key
   id, when compromise was suspected, the artifact window affected, and the
   disposition. Publish the honest summary in the release notes.
5. Audit where the key material was stored to fix the exposure (keys are
   never meant to leave the U1 keyring).

## Version history

- 1.0.0 (2026-10-27): first release signing key record. Publishes the
  fingerprint, key custody, usage, verification, rotation and compromise
  procedure for the PUBLIC-REPO-HARDENING card.
