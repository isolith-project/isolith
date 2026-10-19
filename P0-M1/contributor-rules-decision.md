---
title: Keelworks Contributor Rules Decision
decision: dco_sign_off_no_standing_cla_with_codeowners
version: 1.0.0
date: 2026-10-15
status: Ratified
owner: Keelworks program governance, P0-M1 (program governance and foundations)
applies_to: every repository under the keelworks org (this monorepo first); all contribution types: code, standards, docs, and decision records
supersedes: nothing (first contributor-rules record)
---

# Keelworks Contributor Rules Decision

This record is the binding product decision for how contributions to Keelworks
are certified and who owns review of what. It implements the charter goals
(open, self-hosted, clean-room) and the hybrid license posture (Apache-2.0
applications and libraries, AGPL-3.0 server/PDM and P2P orchestration). Every
later governance document, starting with P0-M1-T05 CONTRIBUTING, implements
against this record.

This document is versioned like the charter. Any material change bumps the
version in the front matter and adds a line to the change log at the bottom.

## 1. Decision

Keelworks uses the Developer Certificate of Origin (DCO) as its contribution
certification mechanism, not a Contributor License Agreement (CLA). Every
contribution is certified by a per-commit `Signed-off-by` line, the standard
git sign-off. There is no standing CLA, no click-through contributor agreement,
and no copyright assignment.

Keelworks adopts a CODEOWNERS policy scoped per workstream. The repo-root
`CODEOWNERS` file is the policy artifact: every code workstream, every
governance directory, and a catch-all default carry named owners, and main
branch protection requires an approving code owner for every applicable rule
before a pull request merges.

## 2. Why DCO, not a CLA

1. The license posture already grants every right a CLA would secure. Under
   Apache-2.0 every recipient already holds a perpetual, worldwide,
   non-exclusive license; under AGPL-3.0 the same is true and networked
   modifications must be offered back. A CLA exists to give a project
   relicensing leverage or copyright assignment, and this program wants
   neither. Adding a CLA on top of these licenses buys no protection that is
   not already in the file header.
2. Friction is a real cost here. The target contributors are small engineering
   shops, defense subcontractors, researchers, and students. A mandatory
   legal signature is the single best-documented deterrent to small
   contributors. DCO is one line in a commit and can be signed by anyone who
   legally can contribute.
3. DCO gives the exact assurance the program actually needs: per-commit
   provenance. Each Signed-off-by line certifies that the signer has the right
   to submit the work under the project license, that the work is original or
   correctly attributed prior work, and that nothing copied or infringing is
   being submitted. This is the licensing twin of the clean-room invariant in
   the charter: provenance is attested all the way down, and the attestation
   travels with each commit instead of living in a database.
4. It is the standard of the kernel-class, permissive-plus-copyleft projects
   this program wants to behave like. Adopting it costs nothing and removes a
   reason for a would-be contributor to bounce off the doorway.

## 3. Options considered

1. A standing CLA for all contributors (Apache-style ICLA, or a Harmony
   agreement), with or without a CCLA for corporate entities. Rejected. It is
   redundant with the license grant, adds paperwork friction, and answers a
   question this program never asked (should the project own or have assigned
   more rights than the license already grants? no).
2. No certification mechanism at all, trusting the license alone. Rejected.
   Provenance matters to a clean-room program and to honest attribution. The
   license says what the code may do; it says nothing about where a given
   diff came from. DCO is the cheap layer that answers that.
3. DCO only, enforced per commit and per pull request. Adopted. This record.
4. Both a CLA and a DCO (the Eclipse-style belt and suspenders). Deferred and
   explicitly not adopted today. If a foundation move or a specific corporate
   contributor ever genuinely requires a signed agreement, that is a new
   decision record, scoped to that case, never a standing program rule.

## 4. What a contribution certifies

When an author adds a `Signed-off-by` line (git's `-s` flag) they certify the
material facts of the DCO: they authored the contribution or have the right to
submit it; it is original work or correctly attributed prior work that they are
allowed to submit; and it is submitted under the project license for the
directory it lands in (per the hybrid posture). They are NOT assigning
copyright and no other rights transfer. The sign-off is a statement of origin
and right, not an assignment.

One sentence a contributor should be able to rely on: contributing to Keelworks
requires signing off your commits under the DCO, nothing more, and your code
stays under the same open license as everyone else's.

## 5. CODEOWNERS policy

The root `CODEOWNERS` file (first file Gitea searches for) is the policy
artifact. It mirrors the monorepo layout in `docs/layout.md`:

- Catch-all default: the keelworks owners team governs everything not matched
  below, so nothing can fall between ownership rows.
- Per workstream: one row per Cargo workspace crate (`crates/oep-*`), plus
  rows for `python/`, `docs/`, `tests/`, `standards/`, and the P0-* governance
  directories. Each row names the owner of review for that path.
- Wave 1 single-maintainer reality: the only current member is Don (`@don`),
  and every row resolves to him. The rows are forward-declared deliberately so
  that when a workstream lead materializes, the change is a one-line owner
  edit per row, not a policy debate. Until then one maintainer owns review of
  everything, which is the honest state of a pre-first-release program.

Review semantics follow Gitea: code owners on a rule are auto-requested to
review pull requests touching matching files, rules are composable (a PR that
touches two owned paths needs approval under both rules), and main branch
protection blocks the merge until every applicable rule has an approving code
owner. Once more than one owner exists for a path, branch protection must be
configured so the pull request author does not count as the required approving
owner for their own change.

## 6. Enforcement

The decision is enforced by automation so it cannot silently drift:

- Sign-off check in CI: every commit in a mergeable pull request carries a
  `Signed-off-by` line. The check is part of the P0-M4-T01 pipeline and fails
  the status on any unsigned commit.
- Main branch protection: P0-M4-T01 enables Gitea branch protection on `main`
  requiring the sign-off status check to pass and code owner approval for the
  modified files per the `CODEOWNERS` file. Required approvals live with the
  owners team, not a stale named list, so adding a maintainer updates
  enforcement automatically.
- The acceptance test in this repo, `tests/test_contributor_rules_decision.py`,
  stays green. It exists to catch edits that weaken the record itself, change
  the CLA answer, or let `CODEOWNERS` drift out of sync with the real Cargo
  workspace members.

## 7. Handoff to downstream tasks

The substance is decided here; the prose and the enforcement land elsewhere, so
this record hands off rather than absorbs:

- P0-M1-T05 writes CONTRIBUTING and README skeleton, which document these rules
  for contributors (how to sign off, what CODEOWNERS means, how to open a PR).
- P0-M4-T01 implements branch protection and the CI sign-off check described in
  section 6.
- P13-T01 refines OSS governance (permissions, issue labels, triage) and keeps
  `CODEOWNERS` in sync with `docs/layout.md` as workstream leads materialize.

## 8. Coverage across the program

- Applies to every repository in the keelworks org, starting with this
  monorepo, and to every contribution type: Rust crates, Python tooling,
  standards material, documentation, and decision records all travel the same
  DCO sign-off and CODEOWNERS review path.
- Applies equally to maintainers and first-time contributors. Nobody is exempt
  from sign-off; that is what keeps the provenance chain complete.
- The DCO is a provenance layer over the code review. It does not replace the
  clean-room and patent posture in P0-M2, the corpus license sweep in P1-M1-T06,
  or the security disclosure path in P11-T08. Those checks still run on the
  content; DCO answers who may submit it.

## 9. References

- charter.md (mission, license posture, clean-room invariant, sovereignty)
- P0-M2-T01 and P0-M2-T02 (legal and patent posture under the clean-room regime)
- P0-M1-T05 (CONTRIBUTING and README, where these rules are documented)
- P0-M4-T01 (branch protection + CI sign-off check)
- P13-T01 (OSS governance refinement, CODEOWNERS sync)
- docs/layout.md (the workspace layout the CODEOWNERS file mirrors)
- Gitea 1.27 documentation, Code Owners (file lookup order, owner syntax,
  branch-protection enforcement), verified 2026-10-15

## Change log

- 1.0.0 (2026-10-15): initial record. Decides DCO with per-commit
  Signed-off-by, no standing CLA, plus a per-workstream repo-root CODEOWNERS
  policy enforced by branch protection, per P0-M1-T04. Ratified.
