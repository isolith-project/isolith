---
title: Isolith Public Repository Hardening Decision
decision: harden_before_visibility_split_on_account
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith program governance, P11 security lane, release lane (P12)
applies_to: the production GitHub repository once it exists, before the visibility flip
supersedes: nothing (first public-repo safety decision; the community-intake surface and the security policy remain binding)
---

# Isolith Public Repository Hardening Decision

This record is the binding decision for how the Isolith repository is secured
before it becomes public. It is versioned like the charter and the other
decision records; any material change bumps the version in the front matter
and adds a line to the change log at the bottom.

The owner opened the public surface on 2026-10-27 (GATE-PUBLIC-SURFACE) with
three wirings: the public surface opens on his explicit signal after the
public website and GitHub are ready; his personal GitHub becomes primary for
Isolith; the Isolith account is separate from every other project. A public
repository with default settings is a supply-chain and integrity problem, not
a community. This record says what hardening is binding, where it is recorded,
and which part waits on the account.

## 1. Decision

The repository is hardened to the baseline in
`docs/hardening/public-repo-hardening.md` before it is made visible:

1. Account security: two-factor authentication required for every member, no
   broad personal-access tokens, a dedicated owner account not shared with
   any other project.
2. Branch protection on `main`: the ported CI set as required status checks,
   no force pushes, no deletions, at least one review for outside pull
   requests with code-owner review, and only an integration card merges to
   main.
3. Tag protection on `v*` and `wave-*`: a release tag cannot be moved or
   deleted by anyone but the release lane.
4. Actions hardening: default `GITHUB_TOKEN` read-only, permissions declared
   per job, every third-party action pinned to a commit SHA, and no
   `pull_request_target` workflow that executes contributor code with
   secrets.
5. Supply-chain guards: Dependabot for Cargo, secret scanning on, and the
   SBOM and licence allow-list checks wired as required jobs.
6. Release signing key story: the fingerprint published in the repository
   and on the site, the private key location and users documented, and a
   rotation and compromise procedure written.
7. CODEOWNERS covering the release lane, the governance records and the brand
   assets.

## 2. What is delivered in the tree

Everything that lives in the repository is delivered now, because it must
exist before visibility: the baseline doc, the enforcement script, the
workflow-hardening validator and gate, the Dependabot config, the release
signing key record and public key, the site security page, and the CODEOWNERS
rework. These are inert until the repository exists; they do not publish
anything.

## 3. What waits on the account

The account or org the owner names does not exist yet. GitHub-side settings
(branch and tag protection, Actions permissions, secret scanning, org-wide
two-factor authentication, Dependabot activation) cannot be applied to a
repository that does not exist. The enforcement script
(`scripts/harden-public-repo.sh`) is written to be run once; it reads the
baseline and this record, applies every API-backed setting, and prints the
readback rows that are the evidence. Org-wide two-factor authentication has
no public REST switch and is a manual owner action, recorded on the card when
flipped.

The GITHUB-PRIMARY-MIGRATION card creates the repository; this record is the
hardening contract that card and the owner execute before the flip. Nothing
is claimed as enforced until the script's readback shows PASS and the manual
two-factor row is recorded.

## 4. Release signing

The release signing key lives on the U1 release lane host
(`docs/release-signing.md`). Its fingerprint is published in the repository,
in `SECURITY.md`, and on the site so downloaders can verify signed artifacts.
Pending before the first visible release: re-add the key's UID off the
retired predecessor brand name onto the Isolith identity and add an expiry;
the fingerprint is unchanged by either, so already-signed artifacts stay
verifiable.

## 5. References

- `docs/hardening/public-repo-hardening.md` (the baseline, section by
  section).
- `scripts/harden-public-repo.sh` (apply + readback).
- `scripts/hardening/check-workflow-hardening.sh` and
  `.github/workflows/hardening.yaml` (validator + gate).
- `docs/release-signing.md`, `SECURITY.md`, `docs-site/security.html`,
  `keys/oep-airgap-signing.pub.asc` (release key story).
- `CODEOWNERS` (coverage).
- `GOVERNANCE.md` and `charter.md` (single-maintainer reality this hardens
  for).

## Change log

- 1.0.0 (2026-10-27): ratify the public repository hardening decision,
  PUBLIC-REPO-HARDENING card.
