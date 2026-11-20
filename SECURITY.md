---
title: "Isolith Security Policy"
version: 1.3.0
date: 2026-11-20
status: Ratified
owner: Isolith program governance, P11 security lane (P11-T08 disclosure lane)
applies_to: every repository under the Isolith org and every released artifact
supersedes: nothing (first security policy; material changes bump the version, per living-document discipline)
---

# Isolith Security Policy

Version 1.3.0 (2026-11-20): the disclosure path now names GitHub private
vulnerability reporting as the primary channel, so a report has a working
route before the `isolith.dev` mail alias exists. The alias remains the
long-term address and `don@redonealpha.com` is the interim mailbox.
Version 1.2.0 (2026-10-27) had replaced the placeholder token with the
domain address (SITE-DOMAIN-ORIGIN).

This document is the disclosure policy for Isolith ("Isolith", the Open
Engineering Platform / OEP monorepo) and the P11-T08 responsible-disclosure
lane. It tells a security researcher what to do, what we commit to, and what
we will not promise. Read it before you report.

The program doctrine applies here too: no claim above what a green test or a
recorded disposition proves. This page states commitments it can keep and
points at the records that back them.

## Private disclosure path

A vulnerability in Isolith is not a public issue. Do not open a public issue,
pull request, or discussion that describes an exploitable weakness or includes
a working exploit.

Report it privately through the repository's **private vulnerability
reporting**: on `github.com/isolith-project/isolith`, Security tab, then
`Report a vulnerability`. That channel is private to the maintainer and is the
one that works today.

The address of record is `security@isolith.dev`, a forwarding alias landing in
the maintainer's existing mailbox; no data is processed on any Isolith-operated
mail server. Until that alias is live, `don@redonealpha.com` reaches the same
mailbox directly. The safe harbor below applies to any report made through
whichever of these channels was used.

What to include in a report:

- The Isolith release or commit you tested (for a release, the YYYY.N train
  and patch level, e.g. 2027.1, 2027.1.1; for a build, the commit SHA).
- The component and version: `oep-pdm`, `oep-web`, `oep-opsdash`,
  `oep-sandbox`, `oep-update`, a reader crate (`oep-cgr`, `oep-3dxml`,
  `oep-step`, ...), or the delivery stack.
- The steps to reproduce, as short as possible, and whether you ran against a
  release artifact or a development build.
- The impact you observed. State what you observed; a severity guess is
  itself a claim and should be labeled as one.

## Supported versions

Isolith releases on a calendar train: the user-facing version is YYYY.N,
patches append a third segment (2027.1.1). The release-train decision
(`workstreams/governance/decisions/release-train-versioning.md`) is the
treaty text; in plain terms:

- Security fixes ship in a patch release of the newest published train.
  A patch contains fixes only, never new features (decision, patch policy).
- The immediately preceding train receives security backports on a
  case-by-case basis, only when the maintainer can verify the fix applies
  and the release pipeline can produce the patch without breaking the
  no-new-features rule.
- Older trains are not supported and receive no fixes.

That is the honest support surface for a single-maintainer project: the
current train unconditionally, the previous train when it is practical, and
nothing older.

## Response expectation

What we commit to (kept with a single maintainer):

- Acknowledgement of a private report within 3 business days.
- An initial assessment (valid, invalid, or needs more information) within
  7 business days of acknowledgement.
- A disposition: fixed, being fixed with an estimate, or declined with the
  reason, recorded on the internal pentest/proof trail.

What we do not commit to (because the program cannot keep it):

- A fix deadline. A fix ships when the acceptance test passes on real
  material and the release pipeline can carry it. If a fix cannot be produced
  for the current train, the report is answered with the honest state, not a
  date.
- An embargo guarantee beyond the reporter's own discretion. If we publish a
  fix, the commit message and release notes describe the change without
  gratuitous exploit detail.

## Bug bounty

There is no bug bounty. No reward, payment, or recognition program exists for
security findings, and reporting never implies one. We are grateful for
responsible reports, and we will attribute findings in the pentest and
release records when the reporter wants credit.

## Verify releases: signing key

Every Isolith release artifact is signed, and the key is published so a
download can be verified by anyone who receives it.

- Fingerprint: `9898A66D42D6FFD3EC5EA78278110F3ADAC1EE92`
- Public key: `keys/oep-airgap-signing.pub.asc` in the repository.
- Full custody, rotation and compromise procedure: `docs/release-signing.md`.

Verify a release with:

```sh
gpg --import keys/oep-airgap-signing.pub.asc
gpg --verify-options show-fingerprint --verify <file>.sig <file>
```

A signed artifact claims the release lane vouches for it. A download without
a valid signature by the key above is not verifiable and must be treated as
untrusted. A harness that wants to skip manual verification imports the key
and checks the fingerprint before trusting a bundle signature.

## What backs these claims: dispositions, SBOM, export control

The security posture is documented and largely machine-gated. The honest
pointers:

- Pen-test dispositions: the self-conducted pen-test day
  (`workstreams/security/pentest-2026-10-18.md`) records findings with a
  severity and a disposition (FIXED, TRACKED, WAIVED) against the real stack
  (oep-pdm, oep-web, oep-opsdash, oep-sandbox). Note plainly: this is a
  self-conducted assessment of a specific build, not a third-party audit and
  not a standing certification. "No known critical findings on that build"
  is the claim; "audited product" is not made anywhere.
- Threat model: `workstreams/security/threat-model.md` is the standing model
  (P11-T01) that the dispositions reference.
- SBOM and CVE scanning: `workstreams/security/sbom-cve.md` describes the
  P11-T02 gate that runs on every release: an SBOM derived from the committed
  `Cargo.lock`, a rustsec-advisory CVE scan, and a release-blocking policy
  with a committed waiver file (`workstreams/security/waivers.toml`). Scan
  reports and BOMs are recorded under `workstreams/security/proof/`.
- Supply-chain doctrine: `workstreams/security/supply-chain.md` (pinned
  dependencies, reproducible builds, signed release artifacts).
- Export-control posture: each release ships an ITAR/EAR classification file
  under `workstreams/security/export-control/` (checked by CI, P11-T05). The
  binding analysis is `workstreams/governance/export-and-legal/export-control-itar-eccn.md`.
  Isolith code and general-purpose engineering data are not ITAR technical
  data by default; the classification files are per release and are the
  authoritative statement for that release.

Open questions about any of the above are welcome as issues, but a suspected
vulnerability always goes to the private path first.

## Version history

- 1.2.0 (2026-10-27): public disclosure alias is now the real domain address
  `security@isolith.dev` (SITE-DOMAIN-ORIGIN). Standing content unchanged.
- 1.1.0 (2026-10-27): add release verification section that publishes the
  release signing key fingerprint and the verify path (PUBLIC-REPO-HARDENING).
- 1.0.0 (2026-10-27): first security policy. Implements the P11-T08
  disclosure lane: private path, supported versions, response expectation,
  no bug bounty, pointers to dispositions, SBOM and export-control records.
