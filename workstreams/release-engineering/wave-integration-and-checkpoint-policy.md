---
title: Keelworks Wave Integration and Checkpoint Policy
decision: integrate-every-wave-onto-one-branch-and-emit-a-documented-checkpoint
version: 1.0.0
date: 2026-10-18
status: Ratified
owner: Keelworks program governance, P12 (release engineering)
applies_to: every wave and milestone; the wave/N integration branches; main; the release train (P12-T05); the single canary repo; every dispatched worker's definition of done
supersedes: the implicit stacked-branch integration practice used through waves 1 and 2, under which per-card PRs were opened and never merged
---

# Keelworks Wave Integration and Checkpoint Policy

This record is the binding operating policy for how a wave's work becomes part
of the product. It exists because the first two waves were built on stacked
branches, each card opened a PR, no merge step existed, and integration was
implicit and never recorded. The measured result is in section 1. Section 6
also bounds repository creation: the program has exactly two repositories and
this policy forbids generating more.

## 1. The problem this fixes (measured 2026-10-18)

- 157 PRs were open in the canonical repo at one point; 113 held work that was
  not in `main`.
- 44 of those 157 were redundant (their content was already in `main` via
  another lineage), which is itself the symptom: nobody could tell what had
  landed.
- 74 of the 113 branches forked from a pre-integration base (`9dd24b0`), so a
  naive merge would REVERT roughly 71k lines that `main` had since grown.
- Each workstream GA gate validated its own branch lineage, not the union of
  the program. `main`'s `oep-pdm`, for example, carried `audit.rs` and
  `sync.rs` but no `rbac/`, `identity/`, or `admin`, so a GA'd P8 shipped a
  PDM with no RBAC.

Root cause: "done" meant "the worker's run finished and its PR was opened",
not "the work is on `main`". The next wave would have reproduced the same pile.

## 2. Decision

Every wave integrates onto a single per-wave integration branch, every
completed card merges into that branch as it finishes, and the wave closes
with exactly one documented checkpoint commit onto `main`. Integration is a
step in the workflow, not an afterthought performed later.

## 3. The model

1. At wave start, cut `wave/N` from `main` (for example `wave/3`). That branch
   is the wave's integration target.
2. Cards branch from `wave/N`, never from another card's branch, and never
   stacked on a sibling. Their PRs target `wave/N`.
3. When a card's PR is green and mergeable, it is merged into `wave/N`
   (squash, one commit per card). Commit message form:
   `card(<card-id>): <title> (#<pr>)`.
4. CI runs on `wave/N` after each merge, so an integration break surfaces
   within minutes instead of at the end of the wave.
5. At wave end, exactly one PR `wave/N` to `main` is merged with the
   checkpoint commit (section 5). The next wave cuts from the new `main`.

Because cards target `wave/N`, the open-PR pile cannot form: a card's PR lives
only between the worker finishing and the integrator merging it.

## 4. Definition of done (changed)

A card is DONE when its PR is merged into `wave/N` and the `wave/N` CI is
green. A green worker run with an open PR is NOT done. This single change is
what prevents the pile.

## 5. The checkpoint (the documented artifact)

At wave end the integrator emits, in the same commit that merges `wave/N` to
`main`:

- `workstreams/release-engineering/wave-checkpoints/wave-N.json`, schema `keelworks.wave-checkpoint.v1`:
  wave id; the source `wave/N` tip sha and the resulting `main` sha; every
  card (id, title, PR number, squashed commit sha); every gate proof path with
  its SHA-256; test counts; and the list of deferrals.
- `workstreams/release-engineering/wave-checkpoints/wave-N.md`: the human wave report. What shipped, what
  was deferred and why, gate results, known risks, decisions taken.
- An annotated tag `wave-N` on the checkpoint commit, so the whole wave is one
  `git diff wave-(N-1)..wave-N` away and revertible in a single step.
- A vault update (the Keelworks topic page and the project history log) in the
  same motion, so the durable record moves with the code.

The manifest follows the freeze-manifest discipline already used by the
release train (`workstreams/release-engineering/release-train.md`, schema `keelworks.release-freeze.v1`),
so the two compose (section 7).

## 6. Repository and artifact hygiene (anti-proliferation)

The program has exactly TWO Gitea repositories and this policy forbids
generating more without an explicit human decision:

- `keelworks/keelworks` is the single canonical development repository. Every
  wave, card, integration branch, tag, manifest, report, proof, and gate lives
  here as a branch, a tag, or a directory. `main` and `wave/*` are branches in
  this one repo.
- `keelworks/keelworks-canary` is exactly ONE canary repository for staged
  rollout, REUSED by every train and every wave. It is never per-wave, never
  per-train, never per-release.

Forbidden without a recorded human decision in
`board.program.json.human_decisions`:

- per-workstream repositories, per-wave repositories, per-card repositories,
  per-release repositories, mirror repositories, fork repositories, or any
  other repository whose content could be a branch, tag, or directory inside
  the canonical repository;
- a second canary repository;
- a new repository whose stated reason is satisfied by a branch or a directory.

This supersedes the earlier plan note about one repo per workstream
(`oep-meta` plus per-workstream repos). Workstreams are directories inside the
one canonical repo; P0 through P14 already live that way.

Legacy allowance: `oep-meta` exists from the wave-1 kickoff and is the one
grandfathered repository. It receives NO new work, holds no artifact that is
not also in the canonical repo, and is archived on Don's decision. It is the
only exception and the tooling reports it separately as a warning rather than
tolerating it silently. Anything beyond `keelworks`, `keelworks-canary`, and
the legacy `oep-meta` is a policy violation.

Also bounded here because they are the same failure shape:

- Worktrees are ephemeral. A worker's worktree is removed when its card is
  merged or abandoned. The repository is not a worktree and a worktree is not a
  repository. Stale worktrees are pruned in the wave checkpoint step.
- Branch and tag counts are capped by the wave, not by the change. A wave adds
  its `wave/N` branch, its card branches (deleted on merge), and one `wave-N`
  tag at the checkpoint.
- A new repository request must name why a branch or a directory cannot serve
  the need. If that sentence cannot be written, the answer is no.

## 7. Composition with the release train

Wave integration and the release train are different cadences on the same
artifacts. A wave is an integration and verification boundary; a train is a
frozen, signed, distributed release. When a wave is also a release:

1. the wave checkpoint lands on `main` and is tagged `wave-N`;
2. the release train cuts `release/YYYY.N` from the checkpoint commit, not from
   a chain tip;
3. the train freezes, verifies, and stages the candidate through the single
   canary repo exactly as `workstreams/release-engineering/release-train.md` describes.

The release drill (the wave GA gate) therefore always cuts from a documented
checkpoint, which is what makes the drill meaningful.

## 8. Roles and gates

- Workers author commits on card branches and open PRs against `wave/N`. They
  do not merge to `main`.
- The integrator is a single serialized role (a job, not several concurrent
  agents): merge the next green card PR into `wave/N`, re-run CI, repeat. One
  merger at a time is what keeps `wave/N` from racing.
- `main` and `wave/*` are protected. Direct pushes to `main` are not part of
  the workflow; the only path to `main` is the wave checkpoint PR.
- Human gates are unchanged: decision cards resolved through kanban comments
  and recorded in `board.program.json.human_decisions`.

## 9. Retrofit for waves 1 and 2 and the current wave 3

Waves 1 and 2 never got a checkpoint, and wave 3 is running under the old
practice. The retrofit applies this policy backward as a one-time operation:

1. Create the integration target `wave/1-3` from `main`.
2. Triage every open PR (card INTEGRATION-T01): in-main, genuinely missing,
   conflicting, or wave-3. Close the in-main ones as already satisfied.
3. Merge or port the genuinely missing work onto `wave/1-3` (INTEGRATION-T02),
   each item verified by its own acceptance test. Never merge a stale-fork
   branch wholesale: 74 of them fork from `9dd24b0` and would revert later
   work.
4. Verify the union (INTEGRATION-T03): build, full CI, every workstream GA
   gate, and the two-machine install rehearsal against `wave/1-3`.
5. Emit the first checkpoint: `wave/1-3` to `main` with
   `workstreams/release-engineering/wave-checkpoints/wave-1-3.json` and `.md`, tagged `wave-1-3`.
6. Adopt the policy for wave 4 onward.

After the retrofit the repository reflects the product, and the release drill
cuts from a checkpoint rather than from a chain tip.

## 10. Change log

- 1.0.0 (2026-10-18): Initial ratification. Establishes the per-wave
  integration branch, the changed definition of done, the checkpoint manifest
  and report, the two-repository hygiene bound, the release-train composition,
  and the waves 1-3 retrofit.
