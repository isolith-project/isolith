---
title: "Isolith Governance"
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith program governance
applies_to: every repository under the Isolith org and every contribution
supersedes: nothing (first governance statement; supersedes nothing because the contributor rules decision and the charter remain binding)
---

# Isolith Governance

This page describes how decisions about Isolith are made today, honestly.
It is a governance statement for the public surface, grounded in the charter,
the contributor rules decision, and the decision records that already exist
in this repository. It will change, with a version bump, when the project
stops being a single-maintainer project.

## Current reality: single maintainer

Isolith is, today, a single-maintainer project. That is not a euphemism and
it is not a temporary embarrassment; it is the accurate description, and the
governance below is written to be safe and legible under it.

Concretely, one person today:

- Reviews and merges every pull request (branch protection requires code
  owner approval, and every CODEOWNERS row resolves to that same owner).
- Runs triage and owns the label pass.
- Cuts releases and runs the release gates.
- Is the enforcement contact for the Code of Conduct and the recipient for
  security reports, until the SITE-DOMAIN-ORIGIN card wires the real domain.

Nothing in this document should be read as a promise that more maintainers
are coming. It is a statement of how the project operates with the people it
has.

## Decision path today

- The charter (`charter.md`) is the constitution: mission, non-goals, the
  clean-room invariant, and the license posture. A decision that conflicts
  with the charter is out of scope.
- Product and technical decisions are recorded, not carried in conversation.
  Two kinds of records exist in this repository:
  - Numbered decisions of the master plan (referenced as "decision #N", for
    example decision #15 wave integration and decision #19 editor spine).
    They are the master-plan decision log and are cited by the scope register
    and the ADR pages.
  - Decision records (ADRs and governance decisions) that live in the tree.
    `docs/decision-records.md` is the index with one-line summaries.
- A material decision is made by the maintainer and recorded as a decision
  record with a version and a date (living-document discipline: any material
  change bumps the record's version).
- Changes to code, tooling, standards, docs, or decision records travel the
  pull request path: DCO sign-off, the card and gate the change belongs to,
  the test that proves it, and approval by the owning code owner
  (CONTRIBUTING.md).

## If contributors arrive

The seams for growth already exist, and governance grows along them:

- CODEOWNERS rows are the workstream ownership map
  (`workstreams/governance/contributor-rules/contributor-rules-decision.md`
  ratified the pattern). Today every row resolves to the same owner; as
  contributors earn trust in a workstream, the maintainer slots that person
  in as the owner of that row (a one-line edit), and branch protection then
  requires their approval for files in their lane.
- DCO sign-off, not a CLA, is the contribution gate
  (`workstreams/governance/contributor-rules/contributor-rules-decision.md`):
  contributors keep their copyright; there is no standing contributor
  license agreement and no copyright assignment.
- Governance refinement itself is a tracked lane (P13-T01 in the program
  plan). A governance change is a normal decision record: state it, version
  it, date it, and let an honest record exist for the community to see.
- When a second maintainer exists, the maintainer-ship rule is the new
  reality and this page is updated with a version bump to say so, including
  who can merge, who runs triage, and how tie-breaks on decisions are
  resolved. Until then, the maintainer is the tie-break.

## What governance does not do

- It does not promise shared ownership that does not exist yet.
- It does not publish anything beyond what this repository contains; the
  public-surface gate (SITE-DOMAIN-ORIGIN) controls when files like these
  are shown outside the private instance.
- It does not assign a second maintainer, and it does not create a steering
  committee that no one has joined.

## Version history

- 1.0.0 (2026-10-27): first governance statement: single-maintainer reality,
  decision path today, and the documented path to shared maintainership.
