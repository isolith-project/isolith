---
title: Keelworks Supply-Chain Hardening (Pinned Dependencies, Reproducible Builds, Signed Artifacts)
status: Ratified (Wave-3 P11-T06)
version: 1.0.0
date: 2026-10-18
owner: Keelworks program governance, P11 security lane
applies_to: every artifact built from the Keelworks / Open Engineering Platform (OEP) monorepo -- all workspace crates, every shipped binary, the P12 packaging and release bundles
supersedes: extends the P0-M3-T02 toolchain pin (does not replace it)
---

# Keelworks Supply-Chain Doctrine

This record closes the M-SUPPLY mitigation from the P11 threat model
(`workstreams/security/threat-model.md` section 7): the P0-M3-T02 toolchain pin extended to a
full locked and verified dependency set, a reproducible build, and a
release-artifact manifest that the P11-T04 signing card consumes. Every rule
below is machine-enforced by CI, not advisory.

## 1. Mapping and concrete closure

| Threat-model stake | Closed by | Concrete mechanism |
|--------------------|-----------|--------------------|
| SU-1 Unpinned or vulnerable deps reach the build | P11-T06 + P11-T02 | Lock-pin gate (this record) rejects any unpinned/unverified dependency change; P11-T02 adds SBOM + CVE scanning on top. |
| SU-3 Non-reproducible builds mask source-to-artifact drift | P11-T06 | Clean-room double build must be byte-identical or list a tolerated nondeterminism; enforced in CI on the release branch. |
| SU-2 / AT6 Artifact tampering or signing a malicious artifact | P11-T04 (+ P11-T06 seam) | This record emits the sha256 artifact manifest; P11-T04 signs release binaries and the update bundle with it. |
| M-SUPPLY inventory row | P11-T06 | Everything in this document. |

## 2. Pinned dependency set

The dependency set is locked and verified. There is no unpinned dependency in
any build path:

- **Rust**: the workspace root `Cargo.lock` (v4) is committed and is the only
  resolution authority. Every external package in it is from
  `registry+https://github.com/rust-lang/crates.io-index` and carries a
  checksum, or is a git dependency pinned to an exact 40-hex rev. Workspace
  `path` members are part of this source tree, not external deps.
- **Toolchain**: `rust-toolchain.toml` pins channel 1.98.1 (P0-M3-T02), held
  in lockstep with `.tool-versions` and `.python-version`.
- **Python**: `uv.lock` at the repo root commits the (currently empty) tooling
  dependency set; `pyproject.toml` is the manifest.
- **C++**: no external third-party sources are compiled from source in CI;
  the OCCT domain review is a plan, not a dependency. `CMakePresets.json` pins
  the generator/toolsets.

### Verification (the gate)

`scripts/supply-chain/verify-lockpin.sh` runs on every push and pull request:

1. `Cargo.lock` must be tracked by git.
2. A fresh `cargo generate-lockfile` in a clean checkout of the same manifests
   must produce a byte-identical `Cargo.lock`; if the manifests changed in a
   way that would move the lockfile, the commit is rejected.
3. Every external package must be verifiable: registry entries carry a
   checksum; git entries pin an exact rev. Anything else fails.
4. The Rust toolchain must pin a concrete channel (never `stable`).
5. Python deps must be lockfile-pinned (`uv.lock`).

Any of these failing is a hard CI failure. This is the literal "CI fails on
any unpinned/unverified dependency change" acceptance.

## 3. Reproducible build

The release artifact must be a pure function of the source, the lockfile, and
the pinned toolchain: build it twice in clean rooms and you get the same
bytes.

`scripts/supply-chain/reproducible-build.sh` performs a clean-room rebuild:

1. Two independent `CARGO_TARGET_DIR`s, each built from scratch (no
   incremental reuse, no shared cache), from the same locked source.
2. A deterministic environment: `SOURCE_DATE_EPOCH` fixed, `CARGO_INCREMENTAL=0`,
   and `RUSTFLAGS=--remap-path-prefix=<src>=.` so the checked-out absolute
   path cannot leak into the artifact.
3. Every produced executable (`target/release/*` excluding the cargo metadata
   artifacts) is sha256'd in both rooms.
4. Any byte difference fails the build; otherwise a proof record
   `workstreams/security/proof/reproducible-build-<ver>.{json,md}` and an artifact manifest
   `workstreams/security/proof/reproducible-build-<ver>.SHA256SUMS` are written.

### Tolerated nondeterminism list

Documented and explicit. As of version 1.0.0 the list is EMPTY: the measured
clean-room double build of the full workspace produced byte-identical
artifacts for all shipped binaries (proof in `workstreams/security/proof/`). Anything that
later cannot be made byte-identical must be added here with a reason and a
migration plan, in the same review that bumps this document, and the script
fails until the diff is understood. Known non-artifact nondeterminism
(intermediates such as the cargo fingerprints/`.d` files or the registry
cache) is by design not part of the artifact contract and is not tracked.

## 4. Signed artifacts (P11-T04 seam)

Signing crypto itself is the P11-T04 card (content-hash integrity on blobs,
detached signatures for release binaries, signed update bundle). This card
provides the reproducible seam T04 signs:

- The reproducible build emits `...SHA256SUMS`, a manifest of every release
  artifact with its sha256. Nothing is signed here; the manifest is the
  canonical input.
- P11-T04 signs those exact bytes (the air-gap signing-key posture: secret on
  the build host only, public key committed per P10-M2 / P10-M5-T01).
- Because the artifact set is reproducible and hash-verified, a signature over
  the manifest binds the release to a specific, rebuildable set of binaries.

## 5. CI enforcement and the release branch

`.gitea/workflows/supply-chain.yaml` implements both gates:

- **lockpin** runs on every push and pull request (all branches). It is the
  failure gate for unpinned dependency state.
- **reproducible-build** is gated to the release branch line: pushes to
  `release/*` and pull requests whose base is `release/*` run the full
  clean-room double build, plus `workflow_dispatch` so it can be run on any
  ref on demand. Per P12-T05 the release branch is immutable and changes enter
  by merge or approved cherry-pick, so the check runs at the branch level
  before anything ships. The byte-identity proof is recorded for every green
  release-branch run.

The GA-gate convention applies (P12 release drill): a release may not be cut
with a red reproducible-build check or with a red lockpin check on its branch
head.

## 6. Versioning and change log

Bump the version in the front matter and append a line here on any material
change (program convention, mirrors vault-conventions).

- 1.0.0 (2026-10-18): first ratification. Full lock-pin verified set, clean-
  room reproducible build with empty tolerated-nondeterminism list, release-
  branch CI gates, and the P11-T04 signing seam. Verified against main.
