---
title: Isolith Contributing Guide
version: 1.0.0
date: 2026-10-15
status: Ratified
owner: Isolith program (Open Engineering Platform), P0-M1 (program governance and foundations)
applies_to: every repository under the isolith org; every contribution type (Rust crates, Python tooling, standards material, docs, decision records)
supersedes: nothing (first contributing guide)
---

# Isolith Contributing Guide

Isolith (working title: Open Engineering Platform, OEP) is an open,
self-hosted, interoperable engineering suite. This guide tells you how to
contribute to it. It implements the contributor rules decided in
`workstreams/governance/contributor-rules/contributor-rules-decision.md` (P0-M1-T04). If you have not read the
charter yet, start there: `charter.md` covers mission, non-goals, the clean-room
invariant, and the license posture. Read `docs/layout.md` for where workstreams
live in this repository.

This document is versioned like the charter and the decision records. Any
material change bumps the version in the front matter and adds a line to the
change log at the bottom.

## What a contribution is

A contribution is any change you submit to a isolith repository: a Rust
crate, Python tooling, standards material, documentation, or a decision record.
Every contribution, including your very first one, travels the same path:
certify it, open a pull request, get owner review, merge.

## Certifying your work: the DCO, not a CLA

Isolith uses the Developer Certificate of Origin (DCO). There is no standing
Contributor License Agreement, no click-through agreement, and no copyright
assignment. The full reasoning is in the contributor-rules decision; here is
what you actually do:

Every commit you contribute must carry a `Signed-off-by` line. The easy way is
git's sign-off flag:

    git commit -s

That appends a line like this to the commit message:

    Signed-off-by: Your Name <you@example.com>

By adding it you certify the DCO facts: you authored the contribution or have
the right to submit it; it is original work or correctly attributed prior work;
and it is submitted under the project license for the directory it lands in
(the hybrid Apache-2.0 / AGPL-3.0 posture from the charter). The sign-off is a
statement of origin and right, not a copyright assignment. You keep your
copyright; your code stays under the same open license as everyone else's.

If a commit is missing its sign-off, CI fails the status on the pull request
and a maintainer will ask you to fix it before review. Fix it with:

    git commit --amend -s

then force-push your own task branch with `--force-with-lease` (never
force-push any other branch).

## What CODEOWNERS means

The repo-root `CODEOWNERS` file is the review policy. It mirrors
`docs/layout.md`: one owner row per workstream, plus governance directories,
and a catch-all default so nothing falls between rows. Think of it as friendly
routing: a pull request that touches an owned path automatically asks its code
owner to review. If your PR spans two owned paths, both owners are asked, and
main branch protection blocks the merge until each applicable rule has an
approving code owner. Wave 1 is single-maintainer, so every row resolves to the
same owner today; the rows exist so workstream leads slot in as one-line owner
edits later.

## How to open a pull request

1. Isolate your change in a fresh git worktree off the latest main. Never
   build directly on main and never reuse another contributor's branch.

       git fetch gitea
       git worktree add .worktrees/<your-task> -b task/<your-task> gitea/main
       cd .worktrees/<your-task>

2. Name the branch `task/<slug-lowercase-hyphens>` with a short unique suffix,
   for example `task/cgr-header-1015a`. One worktree, one branch, one task.
3. Write your change with discipline (see below), run the checks, and commit
   with `git commit -s` so the DCO line is present.
4. Push the branch and open a pull request against `main`:

       git push -u gitea task/<your-task>

5. If the local `gitea` remote has a post-commit hook it pushes automatically;
   verify the remote branch exists afterward.
6. In the PR body, say what you changed, how you verified it, and what test or
   acceptance check proves it. Before-and-after proof is welcome. Address owner
   review comments on the same branch.

Do not merge your own pull request unless the task says so. Merges into main
require the branch protection checks to pass: the sign-off status check and
approving code owner approval for the files you touched.

## Development discipline

The program runs on verified milestones, and the plan skill discipline applies
to most tasks: write the acceptance test first (TDD), isolate in a worktree,
verify against real material, then hand off for review. Concretely:

- Write the acceptance test before the implementation it verifies. Watch it
  fail, then make it pass. A task is not done because a log line says so; it
  is done because a test passed on real material.
- Run the repo checks locally before pushing:

      pytest tests/ -q          # Python acceptance tests (stdlib, no network)
      cargo test --workspace    # Rust crates under crates/

- Keep changes limited to the assigned task. One change at a time is program
  doctrine; gigantic rewrites are rejected in review.
- Respect the surface-language rule: program prose contains no em dashes
  (U+2014). Several acceptance tests fail commits that introduce them.
- Never commit credentials, environment files, or generated build output.
  `.gitignore` already excludes build and venv artifacts; extend it rather
  than fighting it.

## Where things live

- `charter.md`       program mission, non-goals, license posture
- `docs/layout.md`   monorepo layout, the workstream -> crate/module map
- `CONTRIBUTING.md`  this guide
- `workstreams/governance/contributor-rules/`, `workstreams/governance/export-and-legal/`, `workstreams/governance/decisions/`  governance decision records and memos
- `crates/oep-*`     Rust workstreams (P2 CGR, P3 3DXML, P6 geometry, ...)
- `python/`          Python tooling and corpus scripts
- `tests/`           acceptance tests; each P0 deliverable ships one

## Reporting problems

Open an issue against this repository for bug reports and feature requests.
For a suspected security vulnerability, do not open a public issue; contact the
maintainers directly per the security disclosure policy (P11-T08). The
clean-room and patent posture (P0-M2) applies to content too: do not submit
anything that embeds vendor binaries, copied proprietary source, or material
whose license does not allow contribution here.

## Change log

- 1.0.0 (2026-10-15): initial contributing guide. Implements the ratified
  contributor-rules decision (P0-M1-T04): DCO sign-off, no standing CLA,
  CODEOWNERS review semantics, pull request flow, and development discipline.
