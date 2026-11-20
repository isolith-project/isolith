---
title: Isolith Release Train, Versioning, and Patch Policy
decision: release_train_calver_semver_canary_staged_patch_model
version: 1.2.0
date: 2026-10-18
amended: 2026-11-20
status: Ratified
owner: Isolith program governance, P12-T05 (release engineering)
applies_to: every released artifact of the Isolith / Open Engineering Platform (OEP) monorepo -- oep-pdm, oep-geo, oep-viewer, oep-cgr, oep-3dxml, oep-step, the P2P compute lane, and the install/package bundles (P12-T04) once they exist
supersedes: nothing (first release-engineering record; implements the P0-M5-T03 decision "release train and versioning (semver + calendar, e.g. 2027.1)")
---

# Isolith Release Train, Versioning, and Patch Policy

This record is the binding operating policy for how Isolith ships: the train
cadence, the version scheme, the branch/freeze mechanics, the staged canary
rollout, and the patch/hotfix rules. It implements the P0-M5-T03 decision and
is the canonical answer to card P12-T05. It is versioned like the renderer
backend decision (P6-M5-T01), the web-stack decision (P8-M1-T01), and the
message-bus decision (P9-M1-T02); any material change bumps the version in the
front matter and adds a line to the change log at the bottom.

The acceptance for this record is exercised by a real dry-run train cut on the
U1 run target (host `promaxgb10-83f1`, 192.168.0.67) against the current
program integration head; the run is recorded in `workstreams/release-engineering/proof/` and the freeze
manifest in `workstreams/release-engineering/freeze-manifests/`.

## 1. Cyclic train model

Isolith ships on a quarterly calendar train. Each train is one release
planning and stabilization cycle that ends in a candidate (`-rc1`) and, after
canary soak, a general-availability (GA) cut.

- **Train identity = calendar version** per P0-M5-T03: `YYYY.N`, where `YYYY`
  is the calendar year of GA and `N` is the train number within that year
  (`1` = Q1 ... `4` = Q4). Example: `2027.1` is the first train of 2027.
- Each train gets **one release branch** `release/<YYYY.N>` cut from the trunk
  at the freeze point.
- The trunk is `main` (protected, P0-M4 branch protection). While the wave-3
  backlog has not yet been consolidated into `main` (see section 7), the
  program integration head (the tip of the stacked task branches) is the
  recorded cut point; the trunk pointer is kept at `workstreams/release-engineering/release-trunk`.

### Train lifecycle

1. **Open** -- a train is open when its branch exists but its candidate has
   not shipped. The track to the next train (`release/<next>` absent) is the
   normal development period; feature work merges to `main` as usual.
2. **Feature freeze** -- at the cut date the release engineer:
   a. resolves and records the cut point (section 4),
   b. cuts `release/<YYYY.N>` from that point,
   c. writes the freeze manifest (section 3) to `workstreams/release-engineering/freeze-manifests/`,
   d. tags `v<version>-rc1` on the cut commit,
   e. pushes branch + tag to the canary repo (section 5).
   From the freeze tag onward, `release/<YYYY.N>` accepts **no feature
   commits**. A freeze does not stop work on `main`: it stops merges onto the
   release branch.
3. **Canary soak** -- the candidate sits in the pre-release channel for the
   soak window (3 hours, Don 2026-11-20) while internal consumers and the
   acceptance suite validate it (section 5).
4. **GA promotion** -- at the end of a clean soak, the same frozen commit is
   published on `main` as the GA tag `v<YYYY.N.0>` with release notes
   (P12-T07) and, once packaging exists (P12-T04), binary artifacts attached
   per the R1A tag-with-release policy. Public standing stays governed by the
   P0 public-standing decision (private until the working CGR reader and legal
   memo exist).
5. **Maintenance** -- after the next train opens, the previous line lives in
   patch maintenance until it reaches end of life (section 6).

## 2. Version scheme (semver + calendar)

One version string, semver-compatible, whose leading components are the
calendar identity:

```
YYYY.N.P[-PRERELEASE]
 ^   ^ ^  ^---------- optional semver pre-release (rcN, alpha.N, beta.N)
 |   | --------------- patch counter; 0 on a new train, 1,2,... on patch trains
 ^--------------------- train number within the year (1..99)
 year (calendar)
```

- GA of train `2027.1` is `2027.1.0`.
- Candidates are `2027.1.0-rc1`, `-rc2`, ... (pre-release sorts before GA per
  semver; a promoted GA is strictly `2027.1.0`, never an `-rc`).
- Patch train for the `2027.1` line is `2027.1.1`, then `2027.1.2`, ...
- Git tags always carry a leading `v`: `v2027.1.0`, `v2027.1.0-rc1`.
- Leading zeros are forbidden (`2027.01.0` is invalid); the scheme is enforced
  by `python/oep_tooling/release_train.py validate` and asserted in
  `tests/test_p12release_train.py`.

## 3. Feature freeze and the frozen set of commits

A frozen set of commits is any commit range recorded in a **freeze manifest**,
one per candidate, at `workstreams/release-engineering/freeze-manifests/<version>.json`. The manifest is
the machine-readable contract that the release actually reflects a specific,
unchanged git state:

```json
{
  "schema": "isolith.release-freeze.v1",
  "version": "2027.1.0-rc1",
  "train": "2027.1",
  "cut_from": {"ref": "<trunk ref>", "sha": "<40-hex>"},
  "cut_branch": "release/2027.1",
  "cut_tree": "<git tree sha at cut>",
  "frozen_commits_full": <int>,
  "frozen_commits_first_parent": <int>,
  "frozen_newest": "<40-hex>",
  "frozen_oldest": "<40-hex>",
  "canary": {"repo": "isolith/isolith-canary", "branch": "canary/2027.1.0-rc1", "tag": "v2027.1.0-rc1"},
  "created_utc": "<ISO-8601>",
  "host": "<host>"
}
```

Rules that make a freeze a freeze:

- **A release branch is immutable once tagged.** No direct pushes to
  `release/*`, no force pushes, no commits except approved cherry-picked
  patches (section 6). Enforced by branch protection on `release/*`.
- **A tag is immutable.** The candidate's content is the tagged commit; a
  defect is fixed with a *newer* version, never by rewriting the tag.
- **The freeze manifest travels with the candidate.** Manifests are committed
  to the monorepo under `workstreams/release-engineering/freeze-manifests/` (so they are versioned with the
  policy and the tool that produced them) and are regeneratable/verifiable via
  `release_train.py verify`.
- **Freeze components are real git facts.** `frozen_commits_full` /
  `frozen_commits_first_parent` / `cut_tree` are computed from the actual
  object database at cut time; `verify` recomputes them from `cut_from.sha`
  and fails if the recorded state no longer resolves.

## 4. The release cut (procedure)

Executed by the release engineer with `python/oep_tooling/release_train.py`
after the feature-freeze checks pass (trunk green on CI, no open P0/P1,
working tree clean in the cutting worktree):

```
release_train.py release-cut \
  --version <YYYY.N.P-rcN> --trunk <ref> \
  --canary isolith/isolith-canary [--dry-run]
```

The tool, in order: resolves the trunk; verifies the version scheme; writes
the freeze manifest; verifies manifest fidelity; creates `release/<YYYY.N>`
at the cut sha; tags `v<version>` on it; pushes branch + tag to the canary
remote (`isolith/isolith-canary`). `--dry-run` prints every git command
without running the mutating ones. The manifest is written first and, on a
successful cut, committed with the branch creation.

## 5. Staged rollout: canary channel first, GA second

### Repository topology (1.1.0)

One writable repository, one read-only mirror, and the canary as a channel
rather than a repository:

- **GitHub `isolith-project/isolith` is the single writable source of truth.**
  Branch, tag, release and CI on that repository are authoritative. The org is
  owned by Don's personal account, separate from the Red One Alpha account.
- **NAS Gitea `isolith/isolith` is a read-only mirror.** It is fast-forwarded
  from GitHub after each landing and is never a merge target. A commit that
  exists only on Gitea is a finding, not a source.
- **`isolith/isolith-canary` is retired as a publish target** and retained as
  the archive of `v2027.1.0-rc1` and `v2027.1.0-rc2`. Its role is replaced by
  GitHub pre-releases on the primary repository.

This section supersedes the exactly-two-repositories-on-NAS-Gitea clause of
decision #15. Records that reference the mirror URL (proof JSON, `docs/status`
snapshots, air-gap probe scripts) keep the URL of the run that produced them;
they are evidence of what happened, not configuration.

Every candidate passes through a GitHub **pre-release** on the primary
repository before it is general availability. Each candidate receives:

- the release branch at the freeze point, and
- the `v<version>` tag (e.g. `v2027.1.0-rc1`), and
- a GitHub pre-release entry naming the candidate and linking
  `workstreams/release-engineering/...` notes, with the signed artifacts and
  `SHA256SUMS` attached, and
- the same tag forwarded to the Gitea mirror on its next fast-forward.

Promotion to GA requires **all** of:

1. canary soak of the full window (3 hours) with **zero
   P0/P1** findings attributed to the candidate;
2. CI green on the frozen commit (lint + unit + integration + precommit
   gate, the same jobs that run on every push);
3. the acceptance suite green against the frozen tree on a run target
   (fresh-install walkthrough, per the P10-M5 GA-gate pattern once it
   applies to this lane);
4. release-manager sign-off (program governance; for the private phase, Don).

On GA, the candidate's tag is re-tagged on `main` as `v<YYYY.N.0>` (same
commit sha -- promotion changes the tag name, never the tree) and a GitHub
release is created on the primary repository; the mirror picks the tag up on
its next fast-forward. The pre-release entry stays as the historical record of
the staged path.

A candidate with a P0/P1 canary finding is **not promoted**; it is fixed on
`main` and re-cut as `-rc2`. A candidate that fails soak is never renamed
upwards -- it stays `-rcN` and is superseded.

## 6. Patch / hotfix policy: when a patch is cut vs the next train

**A patch release (`<YYYY.N.P+1>`) is cut when** a defect in a *shipped* GA
train meets at least one of:

- **P0/P1 functional regression** -- a shipped line no longer performs its
  core function for an existing adopter;
- **data-integrity bug** -- geometry loss, vault/PDM corruption, audit-chain
  break, silent content-address mismatch;
- **security advisory** -- a vulnerability with a disclosed or exploitable
  path, or an export-control / legal-compliance fix;
- **release-blocking packaging defect** -- an installed bundle/train fails to
  start or upgrade without manual intervention.

Patch trains are cut **from the `release/<YYYY.N>` branch of the shipped
line**, never from `main`, so the frozen feature set is preserved. The fix is
developed on `main` first (one change at a time per program rule), then
cherry-picked onto the release branch, re-frozen as `<YYYY.N.P+1>-rc1`,
fast-tracked through canary (soak 24-48h with zero P0/P1 on the patch), and
promoted as `v<YYYY.N.P+1>`. A patch never carries features.

**Everything else goes to the next train.** Features, enhancements, UX
changes, non-blocking bug fixes, performance work without integrity impact,
and documentation land on `main` and ship in the next quarterly train. In
particular:

- two or more unrelated non-blocking fixes **must** wait, even batched, if
  none is P0/P1 and none touches data integrity;
- a feature is **never** patched onto a shipped line -- it changes the train
  identity and demands a new train;
- a mid-cycle fix that is P0/P1 but trivially safe is still a patch (it may
  batch with other approved patches cut together).

Patch cadence: as-needed, batched when more than one approved patch exists,
never scheduled on a fixed rhythm. A train line reaches **end of life** two
trains after its successor GA (e.g. `2027.1` EOLs when `2027.3` GAs), after
which no patches are cut for it and adopters are directed to the current
train.

## 7. Current program state and the first candidate (dry run)

As of this record (2026-10-18) the program is mid wave-3: `main` carries only
the P0 bootstrap (5 merged PRs; 148 feature PRs open, none merged), and the
real integrated state lives in stacked task branches whose tip is
`task/p10m5t02-ga-gate-1018a` (e67b8fc). The release-train machinery defined
above therefore cuts its **first candidate** from the program integration
head rather than from `main`, and the trunk pointer at `workstreams/release-engineering/release-trunk`
records that head. When the wave-3 consolidation lane merges the backlog into
`main`, the cut point resolves to `main` and the policy above applies
unchanged -- the procedure is identical, only the ref differs.

The dry-run cut is not a product release: it exercises the full mechanical
path (freeze, branch, tag, canary push, verify) end to end so the process is
proven on real material before the first real train. It is recorded in
`workstreams/release-engineering/proof/`.

## 8. Roles and gates

- **Release manager** (program governance / Don while private): sets cut
  dates, signs off GA, owns patch triage.
- **Release engineer**: executes `release-cut` and promotion per this record;
  any deviation from the procedure is a review item.
- **Review lane / CI**: independent; promotion requires CI green on the frozen
  commit (never "green enough" on a neighbor).
- **No automated publish off the canary.** Nothing pushes to a GA channel
  unless the staged path above has completed.

## Change log

- 1.0.0 (2026-10-18) -- Ratified. Implements P0-M5-T03; fixes train cadence
  (quarterly), version scheme (`YYYY.N.P` + semver pre-release), freeze
  manifest contract, canary-first staged rollout, and the patch-vs-next-train
  trigger list.
- 1.2.0 (2026-11-20) -- Soak window reduced from 5 business days to 3 hours
  (Don). The 5-day default was written before a pre-release channel existed and
  assumed external canary consumers that this program does not have; a single
  maintainer with a green gate and a two-machine rehearsal behind the cut is
  the real signal, and a 5-day hold adds latency without adding evidence. The
  zero-P0/P1 condition, the frozen-commit rule and the no-feature-commits rule
  are unchanged.
- 1.1.0 (2026-11-20) -- Repository topology amended per the Gate 0 answer of
  2026-10-27 (GitHub becomes primary, Gitea demoted to internal mirror) and the
  identity decision of 2026-11-19 (`isolith-project`). Supersedes the exactly-
  two-repositories clause of decision #15: one writable repository on GitHub,
  one read-only mirror on Gitea, and the canary is a GitHub pre-release
  channel rather than a second repository.
