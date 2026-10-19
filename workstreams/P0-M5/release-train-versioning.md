---
title: Keelworks Release Train and Versioning Decision
decision: calendar_train_plus_semver_components
version: 1.0.0
date: 2026-10-15
status: Ratified
owner: Keelworks program governance, P0-M5 (MBD/corpus standards and product policy)
applies_to: every shipped artifact (release tags, containers, installers), every component/library crate (oep-cgr, oep-3dxml, oep-geo, oep-step, oep-pdm), the PDM server, P2P workers, CLI and GUI entry points, and all downstream release tooling
supersedes: nothing (first release engineering decision)
---

# Keelworks Release Train and Versioning Decision

This record is the binding product decision for how Keelworks versions its
releases and its components. It is versioned like the charter and the
telemetry decision (P0-M5-T02); any material change bumps the version in the
front matter and adds a line to the change log at the bottom.

Every crate, container, installer, release tag, and the release tooling
itself (P12) implements against this record. It exists so that a user can
answer, from one glance at a version string, both: which release train this
build belongs to, and what API compatibility it promises.

## 1. Decision

Keelworks uses a HYBRID versioning scheme with two deliberately different
dimensions:

1. CALENDAR TRAIN for every product release: the user-facing and operational
   version of the platform is YYYY.N where YYYY is the year of the train and
   N is the train sequence within that year. The first product train of 2027
   is 2027.1, the second 2027.2. A patch to an already-shipped train appends
   a third segment: 2027.1.1, 2027.1.2. This is the version a user installs,
   runs `keelworks --version` and sees, and reports when filing a bug.

2. SEMANTIC VERSIONING (semver) for every component and library crate: each
   crate in the workspace (oep-cgr, oep-3dxml, oep-geo, oep-step, oep-pdm and
   any future crate) carries an independent semver X.Y.Z that promises API
   compatibility to its consumers. A calendar train does not replace semver;
   it pins a set of component versions. A concrete example: "Keelworks 2027.1
   ships oep-cgr 0.3.2, oep-3dxml 0.2.5, oep-geo 0.1.9".

The two dimensions answer different questions and must not be collapsed into
one. The calendar version answers "which platform do I have installed". The
semver version answers "is my code that calls this library still valid".

## 2. Why this decision

Three facts drive it:

1. Engineering suites need a user-facing version that communicates cadence
   and support. A plain X.Y.Z on the whole product tells a defense shop
   nothing about when their install was cut or which train they are on.
   Calendar trains (YYYY.N) make support windows, patch eligibility, and
   "which release am I on" self-evident. The convention is established
   practice and is honest about age in the version string itself.

2. Components need strict compatibility promises. The reader and geometry
   crates are libraries other code will link against (P4, P7, P9, P12
   tooling). Removing or renaming a public API must be loudly expressed as a
   major bump, never hidden in a feature release. Semver is the least
   surprising contract for Rust crates in a Cargo workspace.

3. Reproducibility and supply-chain integrity (program doctrine, P11-T02,
   P11-T04, P11-T06) only work if a release names exactly what went into it.
   A train is therefore not just a version number: it is a manifest of pinned
   component versions and the pinned toolchain (P0-M3-T02), so a release can
   be reproduced and verified.

## 3. Release train

A train is a time-boxed product release cut from main. Cadence: TWO trains
per year by default, 2027.1 in the first half and 2027.2 in the second half,
aligned to the program's milestone gates so each train carries at least one
consumer-visible capability. A year with nothing shippable still ships the
numbered train with maintenance content rather than skipping, so the number
communicates support position.

Mechanics:

- A train is CUT from main at a freeze point onto a release branch named
  release/YYYY.N (for example release/2027.1).
- After the freeze, only regression fixes and security fixes land on the
  release branch. Main continues forward toward the next train; a feature
  that misses the cutoff goes in the next train and never holds the current
  one (train model, not release-on-readiness model).
- The release branch is validated against the corpus and golden-file harness
  (P0-M4-T02, P2-M5-T03) and the full monorepo test suite before GA.
- The train ships as a Gitea release with a tag vYYYY.N (for example
  v2027.1), the offline distribution bundle, and a recorded pin of every
  component version and the toolchain.

## 4. Semantic versioning for components

Every crate in the workspace follows semver 2.0.0 semantics independently:

- MAJOR bump: breaking change to the public API. Signatures removed or
  renamed, behavior contracts changed, minimum supported dependency raised in
  a breaking way.
- MINOR bump: backward-compatible feature addition or behavior improvement,
  no breaking signature change.
- PATCH bump: backward-compatible bug fix only.

Pre-1.0 handling: crates start at 0.x as the monorepo skeleton establishes
(oep-* crates at 0.1.0). Under semver, in the 0.x range the minor digit is
the effective breaking-change digit, so an API break in a 0.x crate bumps its
minor version (0.2.0, not 0.1.1). This is stated here so workers and
reviewers apply the same rule.

A crate release that would also land in a shipped train follows the patch
policy below; a crate-only release that is not part of any train is tagged
independently and its semver is authoritative on its own.

## 5. Patch policy

A patch release to a shipped train (2027.1.1, 2027.1.2) contains ONLY:

- security fixes, and
- regression fixes to behavior that was correct in the previous train.

A patch release NEVER contains new features, new dependencies with new
capabilities, or intentional API changes. The calendar patch digit bumps on
each such release, and each affected component bumps its own semver patch (or
minor for a 0.x API-affecting fix) in the same release manifest.

Patch releases are the only way content reaches a shipped train after its
freeze; everything else waits for the next train.

## 6. Prerelease and candidate versions

Before a train GA, the release branch produces candidates with prerelease
suffixes appended to the calendar version, in ascending order of maturity:

- 2027.1-alpha.1, 2027.1-alpha.N  (early, unstable)
- 2027.1-beta.1, 2027.1-beta.N   (feature-complete, testing)
- 2027.1-rc.1, 2027.1-rc.N       (release candidate, gate for GA)

A candidate passes the corpus harness before it is tagged rc. The GA tag is
the bare calendar version without suffix (v2027.1). Component crates may use
their own semver prerelease tags (for example 0.3.2-rc.1) on crates.io-style
publishing; the platform tag above is what a user-facing build carries.

## 7. Reproducibility and artifact integrity

A release is reproducible and verifiable at three levels:

- TOOLCHAIN: the exact toolchain is pinned by the P0-M3-T02 pin files
  (rust-toolchain.toml, .tool-versions, .python-version, CMakePresets.json,
  uv.lock) and those files are recorded in the release manifest, so the build
  inputs are reproducible.
- SBOM: every release records a software bill of materials listing all
  dependencies and their versions, produced by the CI pipeline (P11-T02), and
  (per P0-M5-T02) containing no telemetry or analytics SDK unless a user has
  explicitly opted in for crash reporting.
- SIGNATURE: released artifacts are signed or at minimum SHA-256 hashed and
  the hashes published with the release (P11-T04), so a deployed build's
  provenance can be verified against the manifest.

An artifact whose manifest cannot be reconstructed is treated as untrusted
for anything requiring supply-chain assurance (P11).

## 8. Enforcement

The scheme is enforced automatically so a future change cannot silently
break it:

- A version-consistency test (this card's acceptance test,
  tests/test_release_train_versioning_acceptance.py, plus a CI step) checks
  that crate versions in Cargo.toml are strict semver, that any train/calendar
  tag matching the repo is YYYY.N or YYYY.N.P (no fourth segment), and that a
  pre-1.0 crate used a minor bump for any breaking change recorded in its
  changelog.
- Release tags are created only by the release tooling (P12-T05), never by
  hand, so tag shape stays enforceable.
- The checklist in P12-T05 (cut branch, freeze, staged rollout, patch policy)
  is bound to this record: a release that violates the freeze rules is
  rejected in review.

## 9. Cross-references

- Charter: versioned like the charter; implements the program's
  reproducibility and sovereignty doctrine.
- P0-M5-T02 telemetry decision: crash reports carry component version/build
  identity per this scheme, and releases contain no telemetry SDK by default.
- P0-M3-T02 toolchain pinning: the pin files a train records.
- P12 release engineering lane: P12-T05 (release train mechanics this record
  governs), P12-T06 (install rehearsal per train), P12-T07 (release notes and
  changelog automation), P12-T08 (status page).
- P11-T02 / P11-T04 / P11-T06: SBOM, signing, and supply-chain pinning this
  record depends on.

## 10. Change log

- 1.0.0 (2026-10-15): initial ratification. Hybrid calendar-train + semver
  scheme, cadence of two trains per year, patch and prerelease policy,
  reproducibility and enforcement sections.
