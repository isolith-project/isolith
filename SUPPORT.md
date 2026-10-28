---
title: "Isolith Support"
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith program governance
supersedes: nothing (first support statement; versioned like the charter)
---

# Isolith Support

Read this before you ask for help. It states, plainly, what support exists
and what does not exist. Isolith is an open-source engineering program in
pre-release, maintained by a small team, and the support story is sized to
that reality rather than to a support contract we do not have.

## What this project is right now

Single maintainer, wave one, pre-GA. 2027.1 is the interop/PDM foundation
release; the editor is the product spine and is not the GA deliverable yet
(see README.md "Status" and the roadmap). Everything below is written for
that state and will be rewritten as the project changes, with a version bump.

## Where to get help

- Documentation: the docs site (getting started, admin guide, format
  capability matrix) is the first stop. In the repository it lives under
  `docs-site/` and the narrative notes under `docs/`.
- Bugs and feature requests: file an issue against this repository using the
  issue templates. Every template labels the honesty rule that applies to
  that kind of report; a report that names the test that reproduces the
  problem is one a maintainer can act on.
- Format capability questions: the format matrix
  (`docs-site/format-matrix.json`) states what each reader opens and does not
  open, per format, with the gate report behind each claim. If the matrix does
  not say it, Isolith does not claim it.
- Security: never file a security issue in public. Use SECURITY.md.
- Conduct: use CODE_OF_CONDUCT.md.

## What you can expect

- Triage: an issue or pull request gets a first response within the weekly
  triage pass (see CONTRIBUTING.md). For a small project this is days, not
  hours.
- Fixes: a fix merges when a green accepted test proves it on real material.
  We do not promise a fix window, a milestone, or a release date on a support
  ticket, because the program does not promise dates it has not earned.
- Releases: on the calendar train. Patch releases carry fixes only.

## What you should not expect

- A support SLA. There is no paid support tier, no support contract, and no
  guaranteed response time measured in hours.
- Vendor-level feature parity on demand. A capability that is not in the
  scope register (`docs/scope-register.json`) is not a commitment; asking for
  it is fine (capability-request template), being owed it is not.
- Guarantees about file compatibility beyond the format matrix. "It opens in
  the reader we ship, in the build we tested, to the fidelity the gate
  measured" is the honest claim. Tailor-made conversions of specific vendor
  files are research, not support.

## Commercial or procurement questions

Isolith participates in defense and government procurement channels. Those
conversations go through the maintainers' business channels, not the issue
tracker, and are out of scope for this public support surface.

## Version history

- 1.0.0 (2026-10-27): first support statement, sized to the single-maintainer
  pre-GA reality.
