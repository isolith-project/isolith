---
title: "Isolith: Your First Pull Request"
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith program (Open Engineering Platform), P0-M1 (program governance and foundations)
applies_to: every repository under the isolith org
supersedes: nothing (first contributor on-ramp page; complements CONTRIBUTING.md)
---

# Your First Pull Request

This page answers three questions in order: what should I work on, what does a
green pull request need, and what happens after I open it. If you have not
built the project yet, do `docs/contributing/build.md` first: the gate is one
command there, and a PR whose change you cannot build and test is not a green
PR.

There is no special "first PR" handler and no ceremony to ask permission for.
The review bar is identical for everyone: DCO sign-off on every commit, a
test that proves the change, and an honest claim. That is the whole list.

## Where to find a scoped task

Scoped work comes from two surfaces, and both are deliberately public:

1. The scope register's planned rows. `docs/scope-register.json` (and its
   human rendering `docs/scope-register.md` / `docs/ROADMAP.md`) is the
   commitment surface: it says what Isolith has promised, what is building
   now, and what is still `planned`. Rows whose status is `planned` with a
   `class-1-additive` cost class and no open workstream are the best first
   targets: they are small, additive, and bounded. Good examples as of
   2026-10-27: `mass-properties`, `exploded-views`, `input-spacebar-accept`,
   `input-snap-inference`, `input-escape-semantics`, `input-cut-region`,
   `input-datum-plane-legibility`, `input-toolbar-customisation`. Kernel-gated
   rows (`sheet-metal`, `weldments`, `mold-tooling`, `routing`) need the OCCT
   kernel tier first, which is a bigger first step; aim for the additive ones.

2. The issue templates and the `good-first-issue` label. The templates under
   `.github/ISSUE_TEMPLATE/` are the four accepted shapes:
   `bug-report`, `capability-request`, `format-fidelity-gap`, and
   `documentation-defect`. A bug report you can reproduce, a fidelity gap you
   can show against `docs-site/format-matrix.json`, or a documentation defect
   you can point to a real green test for are all excellent first PRs, and the
   maintainer triages `good-first-issue` and `help-wanted` labels onto them in
   the weekly pass.

Also open in the sense of findable but not promised: any claim any page makes
that a green test does not back. The honesty discipline means a PR that
closes the gap between a claim and its proof is always in scope. If in doubt,
open a `documentation-defect` issue first and name the gap; the maintainer
will tell you whether it is real and who owns it.

## What a green pull request needs

Opening the PR is not the start of the work; it is the end of it. Before you
open, the change must already be built and tested by the gate in
`docs/contributing/build.md`:

    cargo xtask ci            # whole gate locally: fmt, clippy, tests, pytest

Then the PR itself must satisfy five checks, all enforced by branch
protection except where noted:

1. **DCO sign-off on every commit, and a sign-off line in the PR body.**
   Isolith uses the Developer Certificate of Origin, not a CLA (see
   CONTRIBUTING.md). Every commit must carry `Signed-off-by: Your Name
   <you@example.com>`. The easy way is `git commit -s`. If one is missing,
   CI's `dco-sign-off` check fails and a maintainer asks for
   `git commit --amend -s` + `git push --force-with-lease` on YOUR OWN branch
   only. The PR body carries the same sign-off line (the template shows
   exactly where), and a left-over placeholder is treated as missing.

2. **A test that proves the change, with the command and the proof line.**
   "I tested it" is not evidence; a named test and its green output line is.
   For a Rust crate: a test under the crate or `tests/`, invoked via
   `cargo test -p <crate>` or pushed into the workspace suite. For tooling:
   a `tests/test_*.py` accepted by `pytest tests/`. For a docs or governance
   change: still name the test that keeps it honest (often an existing one).
   If you cannot name a test for your change, the change is probably a claim,
   not a change.

3. **The honesty checklist, actually behaved.** The PR template's checklist
   is not decoration. The two lines that fail most often:
   - "This PR does not claim a status higher than a green accepted test
     proves." No `everything opens`, no parity claim without a test on real
     material, no format claim beyond the matrix.
   - "No generated build output / credentials committed." `.gitignore` and
     `.dockerignore` already block the common cases (target, .venv, build,
     dist); extend them rather than fighting them.

4. **Card and gate, named.** The template asks for the card id or issue
   number and the milestone/gate. If it is a self-motivated change, say
   "maintenance" and be honest that it is not milestone-tracked.

5. **Surface language.** Program prose contains no em dashes (U+2014) and no
   emojis. Several acceptance tests fail a commit that introduces one. Write
   your PR body and commit messages in the same plain register as the tree.

Write the PR until it is a two-sentence answer to "what does this change and
why does the program need it". If you cannot, it is two changes; split it.
One change at a time is program doctrine, and review rejects giant rewrites.

## How review works with a single maintainer

Isolith is, today, a single-maintainer project. GOVERNANCE.md is explicit
about this and it is not a temporary embarrassment: it is the accurate
operating model the contributor rules were written for. What that means in
practice:

- **One person reviews and merges.** Every `CODEOWNERS` row resolves to the
  same owner today. Branch protection blocks a merge until (a) the DCO
  sign-off check passes and (b) that code owner approves. So your PR's
  gatekeepers are automated plus one human, and the human is the tie-break.
- **Do not merge your own PR**, unless a task explicitly says so. Merges into
  main are the maintainer's lane.
- **Triage runs weekly, response time is measured in days, not hours.**
  CONTRIBUTING.md defines a weekly triage pass over new issues and PRs; a
  first response is part of that pass. If you have waited past a week with no
  signal, a polite nudge on the thread is appropriate, not rude.
- **Review comments land on your branch, and you keep pushing to it.** You
  address review on the same task branch, `git commit -s`, and
  `--force-with-lease` only your own branch. A review round is a normal part
  of the loop, not a rejection.
- **The bar is the same for your first PR as for your tenth.** There is no
  priori trust ladder: sign the DCO, prove the change with a test, and say
  only what the test proves. That is the entire social contract, and it is
  written down so it survives any mood.

The maintainer-ship rule changes only when a second maintainer exists, and
GOVERNANCE.md documents exactly how: workstream leads slot into
`CODEOWNERS` rows as trust is earned, and branch protection then requires
their approval in their lane. Nothing in that future changes what a green PR
needs today.

## If in doubt

Open the `capability-request` or `documentation-defect` template and name the
row or the claim first. Naming it turns a vague idea into something the
maintainer can triage in seconds, which is the fastest way to a yes or an
honest no. The worst thing you can do is open a PR that changes runtime
behavior with no test; the second worst is a claim the tree cannot back.
Both bounce, and the bounce is a lost hour of yours, not a lesson the project
wants to teach twice.

## Change log

- 1.0.0 (2026-10-27): first on-ramp page. Scoped-task discovery (scope
  register planned rows + issue templates), the five green-PR checks, and the
  single-maintainer review reality as written in GOVERNANCE.md and
  CONTRIBUTING.md.
