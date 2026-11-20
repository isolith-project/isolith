---
title: "Isolith Public Repository Hardening Baseline"
version: 1.1.0
date: 2026-10-27
status: Ratified
owner: Isolith program governance, P11 security lane
applies_to: the production GitHub repository (account or org named by the owner; coordinates with GITHUB-PRIMARY-MIGRATION) before that repository is made visible
supersedes: nothing (first hardening baseline; material change bumps the version per living-document discipline)
---

# Isolith Public Repository Hardening Baseline

This document is the binding pre-visibility checklist for taking the Isolith
repository public. The owner opened the public surface on 2026-10-27
(GATE-PUBLIC-SURFACE) with the explicit wiring: his personal GitHub becomes
primary for Isolith, the Isolith account is separate, and the public surface
opens only after the public website and GitHub are ready.

A public repository with default settings is a supply-chain and integrity
problem, not a community. This page states every protection that must be
active, the exact settings, and who applies them. Nothing on this page
publishes anything; it hardens before visibility.

The enforcement tooling is `scripts/harden-public-repo.sh` (idempotent apply
and readback for everything that has an API) and the in-tree configuration in
this branch. The release signing key fingerprint is in
`docs/release-signing.md`, `SECURITY.md`, `docs-site/security.html`, and
`keys/oep-airgap-signing.pub.asc`.

## Status split: account not yet created

The owner has not yet created and named the Isolith GitHub account or org.
Until it exists, the GitHub-side settings below cannot be applied. They are
specified here with exact payloads so that the GITHUB-PRIMARY-MIGRATION card
applies them the moment the repository exists, before the visibility flip.
What can be delivered in-tree today is delivered: CODEOWNERS, the hardening
validator, Dependabot config, the release key publication, and this baseline.

## 1. Account security

Apply at the Isolith account or org the owner names:

- **2FA for every member.** In an org, enable "Require two-factor
  authentication for everyone in your organization"
  (`https://github.com/organizations/<org>/settings/security`, org owner
  clicks Enable). GitHub has no public REST switch for this, so it is a
  manual owner action, but it is mandatory and it is recorded as verified
  when `scripts/harden-public-repo.sh` runs its check step over a two-FA
  enforced claim in the readback comment.
- **No broad personal access tokens.** Members authenticate with SSH keys or
  fine-grained PATs scoped to the Isolith repos they touch, never a classic
  PAT with `repo` scope. The automation token that drives CI and the release
  lane is a fine-grained token (where GitHub permission model allows:
  `contents: read` on the repo, `actions: read`, `secrets: read` only where a
  job needs them). It lives in the operator keyring on U1, never in the
  repository.
- **A dedicated owner account, not shared.** Isolith is separate from the
  Red-One-Alpha account (`redonealpha-don`) and from every other project.
  GITHUB-PRIMARY-MIGRATION holds this as a hard rule; this baseline repeats
  it.

## 2. Branch protection on `main`

Exact payload (`scripts/harden-public-repo.sh` applies it):

```json
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "lint", "unit", "integration", "gateway",
      "quality-gate", "dco-sign-off", "lockpin",
      "licensing-boundary", "crypto-integrity",
      "p11t02-sbom-cve", "export-control-selfcheck"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "require_code_owner_reviews": true,
    "dismiss_stale_reviews": false
  },
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_linear_history": false,
  "required_conversation_resolution": true
}
```

Context names are the ported CI job names (decision: the Gitea job names are
the contract; on GitHub check names default to job names, so GITHUB-PRIMARY-
MIGRATION confirms each from `gh pr checks` on its test PR and updates the
list here only if GitHub names them differently, with the confirmation
recorded).

`required_linear_history` stays off because the policy (decision #15 wave
integration) closes each wave with a documented checkpoint commit onto main;
merge commits are the integration lane's mechanism. "Only an integration card
merges to main" is enforced on GitHub as: main is pull-request-only, code
owner review is required, and merges are executed by the release or
integration lane (today that is the single owner, per the governance record).

## 3. Tag protection

Protect every release and wave tag from being moved or deleted by anyone but
the release lane:

- `v*` product release tags (v2027.1.0-rc1, v2027.1.0-rc2 and beyond).
- `wave-*` wave checkpoint tags (wave-1-3, wave-4).

`scripts/harden-public-repo.sh` applies GitHub tag protection rules
(`POST /repos/{owner}/{repo}/tags/protection`) for both patterns, and the
stronger repository ruleset variant is documented in the script when the
classic API falls short. The release lane alone moves or deletes a tag, and
that lane is the owner plus the release automation using its own scoped
token. Ordinary members get no tag write.

## 4. Actions hardening

- **Default `GITHUB_TOKEN` read-only.** Repo setting
  `actions.permissions.default_workflow_permissions` = `read`
  (script applies it). No job inherits write by default.
- **Permission declared per job.** Every workflow job declares an explicit
  `permissions:` block; the validator
  (`scripts/hardening/check-workflow-hardening.sh`, wired as
  `.github/workflows/hardening.yaml`) fails a workflow that omits it.
- **Third-party actions pinned to a commit SHA.** Every `uses:` line for an
  action outside this repository pins the full 40-hex commit SHA with a
  comment naming the release the SHA came from. The validator fails on tag or
  branch refs.
- **No `pull_request_target` that executes contributor code with secrets.**
  The validator rejects a `pull_request_target` trigger whose job checks out
  or executes the PR head or uses a secret. If a trusted-PR workflow is ever
  genuinely needed, the approved pattern is: trigger on `pull_request_target`,
  check out the base commit only, never execute contributor code, no
  `secrets:` in the same job.

## 5. Supply-chain guards

- **Dependency updates.** `.github/dependabot.yml` enables Dependabot for the
  Cargo ecosystem (weekly) and for GitHub Actions. This is the GitHub-native
  equivalent of the Gitea pinning work and runs once the repository exists.
- **Secret scanning, push protection, dependency graph, Dependabot alerts,
  and vulnerability alerts** are enabled at the repo level by
  `scripts/harden-public-repo.sh` (`security_and_analysis` patch plus the
  vulnerability-alerts endpoint). Nothing in the tree triggers them; they are
  GitHub-side settings.
- **SBOM and licence allow-list are required jobs.** The SBOM/CVE gate
  (`p11t02-sbom-cve`, from the ported `ga-gate-p11` workflow) and the licence
  allow-list check (`licensing-boundary`, from the LICENSE-FILES-AND-BOUNDARY
  lane) are in the required status check list in section 2. A main merge
  cannot happen without them green.

## 6. Release signing key

The release signing GPG key lives on the U1 build host. Full custody,
rotation and compromise procedure: `docs/release-signing.md`. The published
fingerprint, public key and verification instructions are on the site
(`docs-site/security.html`) and in `SECURITY.md`. This baseline requires,
before the first visible signed release:

- the fingerprint published (done in this branch);
- the public key in-tree (`keys/oep-airgap-signing.pub.asc`, done);
- the identity on the key brought current (the UID still carries the retired
  predecessor brand name and a `localhost` email; re-add a UID with the
  Isolith identity once the site domain is known, keeping the same
  fingerprint, so existing signed artifacts stay verifiable). This is a
  release-lane action, logged PENDING in `docs/release-signing.md`.

## 7. CODEOWNERS

`CODEOWNERS` at the repository root covers: the release lane (release,
CHANGELOG, cliff.toml, air-gap scripts, release engineering), the governance
records (charter, governance, code of conduct, security policy, decision
records), and the brand assets (`assets/brand/`, docs-site brand files), on
top of the per-workstream crate rows. The default row resolves to the
`@<org>/Owners` team, the same owner across every row today (single
maintainer), so no file falls between rows.

## 8. How to apply

1. The owner creates and names the Isolith GitHub account or org and gives
   the automation access (GITHUB-PRIMARY-MIGRATION step 1).
2. GITHUB-PRIMARY-MIGRATION creates the private `isolith` repo, pushes
   history, and ports CI. The ported workflows must pass the hardening
   validator before the visibility flip.
3. Run `scripts/harden-public-repo.sh` (see `--help`):
   - `HARDEN_OWNER=<org> HARDEN_REPO=isolith ./scripts/harden-public-repo.sh --check` (read-only),
   - then without `--check` to apply, then again to verify PASS rows.
4. The org owner toggles org-wide 2FA (manual, recorded in the run log).
5. Record the API readback from the final verify run as the evidence block on
   the card; a signed release with an unpublished key is unverifiable, so the
   fingerprint block from `docs/release-signing.md` ships with the release
   notes of the first visible release.

## Applied state

Verified by `scripts/harden-public-repo.sh --check` against the live repository
after the visibility flip. Every row below is an API readback, not an
intention.

- **Visibility:** public at `github.com/isolith-project/isolith`, org
  `isolith-project` owned by Don's personal account `dmltallen`.
- **Branch protection on main:** admins bound (`enforce_admins` true), no force
  push, no deletions, conversation resolution required, strict status checks
  over the eleven required contexts (lint, unit, integration, gateway,
  quality-gate, dco-sign-off, lockpin, licensing-boundary, crypto-integrity,
  p11t02-sbom-cve, export-control-selfcheck).
- **Required reviews: 0, deliberately.** A solo-maintainer repository cannot
  require an approval: GitHub does not let a maintainer approve their own pull
  request, so `enforce_admins` plus one required review would deadlock every
  merge, including the change that would relax it. The value is
  `HARDEN_REQUIRED_APPROVALS` (default 1) and it goes to 1 in the same pass
  that adds a second maintainer.
- **Tag protection:** rulesets on `refs/tags/v*` and `refs/tags/wave-*`
  blocking deletion and updates. The legacy `/tags/protection` endpoint GitHub
  used in 1.0.0 is retired; a tag ruleset is the live mechanism and the script
  now creates and reads that.
- **Supply chain:** secret scanning enabled, secret scanning push protection
  enabled, Dependabot alerts and automated security fixes enabled.
- **Disclosure:** private vulnerability reporting enabled on the repository,
  which is the channel SECURITY.md 1.3.0 names as primary.
- **Actions:** default workflow token read-only.
- **Outstanding manual step:** org-wide 2FA. GitHub does not expose the
  requirement through the REST API, so the org owner toggles it at
  `github.com/organizations/isolith-project/settings/security`. Until it is
  on, the row stays open.

## Version history

- 1.1.0 (2026-11-20): applied state recorded after the public flip. Adds the
  solo-maintainer approvals decision, the ruleset-based tag protection, the
  supply-chain and disclosure state, and the one manual step that remains.
- 1.0.0 (2026-10-27): first hardening baseline, PUBLIC-REPO-HARDENING card.
  Locks account security, branch and tag protection, Actions hardening,
  supply-chain guards, release signing key publication and CODEOWNERS as the
  pre-visibility checklist, with the enforcement script and in-tree config.
