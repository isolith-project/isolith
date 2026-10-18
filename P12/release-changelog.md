---
title: Keelworks Release Notes and Changelog Automation
decision: git_cliff_conventional_changelog_on_release_cut
version: 1.0.0
date: 2026-10-18
status: Ratified
owner: Keelworks program governance, P12 release engineering lane
applies_to: every release cut (calendar train tags vYYYY.N.P[-PRE] per P0-M5-T03),
  the release/YYYY.N release branches, and the release tooling (P12-T05)
supersedes: nothing (first changelog-automation decision)
---

# Keelworks Release Notes and Changelog Automation

This record is the binding decision for how Keelworks generates release notes
and the changelog, so a user can read one categorized document and know what a
release contains and why. It is versioned like the charter and the release
train decision (P0-M5-T03); any material change bumps the version in the front
matter and adds a line to the change log at the bottom.

Every shipped tag, release branch, and the release tooling itself (P12-T05)
implement against this record.

## 1. Decision

Keelworks generates release notes and the changelog wholly from the
conventional commit history (the repo's commit discipline) using git-cliff,
driven by `cliff.toml` at the repository root and the wrapper
`scripts/release/changelog.sh`. The changelog is never hand-written:

- The generator runs in CI on every release cut
  (`.gitea/workflows/release-changelog.yaml`), which triggers when a release
  tag is pushed and on `workflow_dispatch` (manual or dry-run cut).
- The generator walks the frozen commit range the tag sees on the release
  branch and renders a categorized changelog (Features, Bug Fixes,
  Performance, Refactoring, Documentation, Tests, Continuous Integration,
  Chores, Build, Other) with a 7-character commit id and the subject for each
  entry.
- The rendered markdown becomes the Gitea release notes body and is uploaded
  as a versioned `.md` release asset, so GA (and every prerelease candidate)
  ships with human-readable release notes bound to the tag.
- A dry run with no tag yet renders the whole history under the requested
  version, so the release-train dry run (P12-T05) emits the changelog
  artifact before the tag exists.

Why git-cliff: it is the conventional-commits changelog generator used as the
reference implementation, ships as a single static binary (air-gap friendly,
sha512-pinned in CI), the repository is a Rust workspace so the toolchain is
aligned, and it has no runtime registry or node dependency. coag was the
alternative considered and rejected for this repository: git-cliff's
template + commit-parser model fits the calendar-train tag shape with less
configuration.

## 2. Why this decision

1. The repository already uses conventional commit messages on every branch;
   the change history is therefore machine-readable without duplicating it by
   hand, and a release manifest produced by reading `main`'s commits is
   reproducible at any time from the same git objects.
2. The P0-M5-T03 decision names the exact tag+release shape the generator
   must honor (calendar trains and semver). Centralizing that mapping in one
   config (`cliff.toml`) and one wrapper makes the release pipeline honest
   about what went into a cut.
3. Defense and engineering customers need to trust what a binary contains.
   A generated, checksummed release-notes artifact (sha256 next to the file,
   verified in the acceptance test) is part of the reproducibility doctrine
   (P0-M5-T03 section 7).

## 3. Ops contract

- VERSION is YYYY.N.P[-PRE] per P0-M5-T03 (for example 2027.1.0, 2027.1.0-rc1),
  tag is v+VERSION. The generator only READS tags; P12-T05 owns creating them.
- `scripts/release/changelog.sh [VERSION] [OUTPUT]`:
  - if `vVERSION` exists in the repo, renders that tagged release;
  - if it does not (dry run), renders the whole history as that version;
  - pins git-cliff 2.14.2 semantics (`GIT_CLIFF_VERSION`), honors
    `GIT_CLIFF_BIN` or PATH, writes OUTPUT (default CHANGELOG.md), writes a
    `.sha256` next to it, and prints a per-category count table.
- `.gitea/workflows/release-changelog.yaml`:
  - triggers on `push` of tags matching `v20xx.*` (the release cut) and on
    `workflow_dispatch` (dry run with `version` and optional `ref` inputs);
  - fetches FULL history and tags (the depth-1 pipeline checkout is not
    enough for a changelog), checks out the tag or requested ref;
  - installs a sha512-pinned git-cliff for the runner architecture
    (x86_64-unknown-linux-gnu or aarch64-unknown-linux-gnu) and verifies the
    hash before use;
  - runs `scripts/release/changelog.sh`, then attaches the markdown to the
    Gitea release as release notes plus a versioned asset via
    `scripts/release/attach-release-notes.py`, and always prints the
    changelog to the job log.
- `.gitea/workflows/p12-t07-changelog.yaml` runs `tests/test_release_changelog_acceptance.py`
  on every push/PR, so the contract is enforced in CI like the other lanes.

## 4. Dry-run train handoff (P12-T05)

P12-T05's dry-run train cut creates the release branch and candidate tag. That
tag push (or the equivalent `workflow_dispatch` when the dry run never pushed
the tag to Gitea, e.g. a host-local cut) makes this lane emit the changelog
artifact for the cut, giving the train its release notes before GA. The first
dry-run candidate tag was v2027.1.0-rc1; this card's proof was generated from
that tag's actual commit history on the release branch (see P12/proof/).

## 5. Verification

- The committed proof `P12/proof/changelog-v2027.1.0-rc1.md` was generated
  with `scripts/release/changelog.sh 2027.1.0-rc1` from the actual release
  history; its sha256 sits next to it.
- The acceptance test regenerates that changelog with git-cliff over the real
  history and requires an identical sha256 whenever a git-cliff binary is
  present (graceful skip otherwise, so CI with no binary still enforces the
  structure + checksum legs).

## 6. Change log

- 1.0.0 (2026-10-18): initial ratification. git-cliff + cliff.toml +
  changelog.sh; release-cut CI workflow triggers on calendar tags and
  workflow_dispatch; release notes attached to Gitea releases; acceptance
  test + dry-run proof for v2027.1.0-rc1.
