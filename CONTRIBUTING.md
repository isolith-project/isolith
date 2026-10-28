---
title: Isolith Contributing Guide
version: 1.1.0
date: 2026-10-27
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

## Triage: what gets accepted and what gets declined

Every issue and pull request is triaged, not merely answered. Triage means
the maintainer reads it, classifies it with a label, and gives a first
response. This section is the written triage rule so the process does not
depend on one person's private judgment.

### What gets accepted

- Bug reports that name the environment and reproduce the problem (use the
  bug-report template).
- Format fidelity gaps that name the file, the reader, and the observed
  versus expected row from `docs-site/format-matrix.json`. The matrix is the
  honesty surface for fidelity; a gap report compares observed behavior to
  the committed row, not to a wish.
- Capability requests that name the scope-register row they extend
  (`docs/scope-register.json`). A request without a row has nothing to attach
  to; naming the row turns it into a proposal that can be sequenced.
- Documentation defects that point at the claim and the source.
- Pull requests that carry the DCO sign-off, name the card and gate the
  change belongs to, and include the test that proves the change.

### What gets declined, and why

- UI-clone fidelity claims without a green test proving them. "It matches
  CATIA here" is a claim; a green accepted test on real material is the only
  proof the program accepts. Declined with a pointer to the format matrix and
  the scope register.
- Redistribution of vendor CAD files or any material whose license does not
  allow contribution. The clean-room invariant is the hard line: no vendor
  binaries, no copied proprietary source, no Dassault/Autodesk/Siemens
  libraries, public specs and lawful observation only (charter, working
  doctrine). Content that embeds such material is declined, and a pull
  request that adds it is closed without being merged.
- Anything that would break the clean-room invariant in any other form.
- Feature requests not tied to a scope-register row. The register is the
  commitment surface; a request may add or re-sequence a row, but it cannot
  claim a status a green test does not prove.
- Pull requests without a DCO sign-off are not declined outright; they are
  blocked until the sign-off is added (`git commit --amend -s`, then
  force-push your own branch only). CI enforces this via the `dco-sign-off`
  check.

### Labels

New issues start untriaged and get classified in the weekly pass:

- `triage` (default until the pass runs)
- kind: `bug`, `format-fidelity`, `capability`, `documentation`
- outcome: `accepted`, `declined`, `needs-info`
- attention: `good-first-issue`, `help-wanted`
- `dco-blocked` (a pull request whose sign-off or template is incomplete)

### The weekly triage pass

The maintainer runs a triage pass at least weekly over new issues and pull
requests: classify every untriaged item, answer needs-info, move items to
`accepted` or `declined` with the reason on the thread, and keep triage
current. A first response is expected within that pass, which for a small
project means days, not hours.

The weekly pass becomes a scheduled job (a cron) only when the repository is
public; until then it is a manual pass maintained by hand. It is deliberately
not scheduled from this public-surface card.

## Where things live

- `charter.md`       program mission, non-goals, license posture
- `CODE_OF_CONDUCT.md`  community standards and how to report a violation
- `SECURITY.md`      private disclosure path and supported-version policy
- `SUPPORT.md`       what support exists and what does not
- `GOVERNANCE.md`    who decides what, today and if contributors arrive
- `docs/layout.md`   monorepo layout, the workstream -> crate/module map
- `docs/ROADMAP.md`  public roadmap, generated from `docs/scope-register.json`
- `docs/decision-records.md`  index of the decision records with one-line summaries
- `CONTRIBUTING.md`  this guide
- `workstreams/governance/contributor-rules/`, `workstreams/governance/export-and-legal/`, `workstreams/governance/decisions/`  governance decision records and memos
- `crates/oep-*`     Rust workstreams (P2 CGR, P3 3DXML, P6 geometry, ...)
- `python/`          Python tooling and corpus scripts
- `tests/`           acceptance tests; each P0 deliverable ships one

## Reporting problems

Open an issue against this repository using the templates under
`.github/ISSUE_TEMPLATE/` for bug reports, format fidelity gaps, capability
requests, and documentation defects. The triage rule above says what gets
accepted, what gets declined and why, and the labels used. For a suspected
security vulnerability, do not open a public issue; use the private reporting
path in `SECURITY.md` (P11-T08). The
clean-room and patent posture (P0-M2) applies to content too: do not submit
anything that embeds vendor binaries, copied proprietary source, or material
whose license does not allow contribution here.

## Change log

- 1.1.0 (2026-10-27): added the triage rule (what gets accepted, what gets
  declined and why, the labels to use, and the weekly triage pass), indexed
  the new community files (`CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`,
  `GOVERNANCE.md`, `docs/ROADMAP.md`, `docs/decision-records.md`), and pointed
  the reporting path at the issue templates and `SECURITY.md`.
- 1.0.0 (2026-10-15): initial contributing guide. Implements the ratified
  contributor-rules decision (P0-M1-T04): DCO sign-off, no standing CLA,
  CODEOWNERS review semantics, pull request flow, and development discipline.
