---
title: Isolith Commercial Model and CLA Memo
decision: services_and_addons_no_cla_dual_licensing_off_table
version: 1.0.0
date: 2026-10-29
status: Ratified
owner: Isolith program governance, P0-M1 (program governance and foundations)
applies_to: every repository under the isolith org; the commercial posture of the AGPL server/P2P stack (PDM, compute lane, enterprise auth) and the contribution rules in workstreams/governance/contributor-rules/contributor-rules-decision.md
supersedes: nothing (first commercial-model record); amends contributor-rules-decision.md 1.0.0 in place via version bump
---

# Isolith Commercial Model and CLA Memo

This memo answers one question, decided at a gate: does B2B or enterprise
commercialization of the Isolith server stack require a Contributor License
Agreement (CLA) before the repository accepts outside pull requests at scale?

The answer is recorded in Don's decision, D-ISOLITH-1 (2026-10-29): no CLA.
The program commercializes via services and add-ons under AGPL, the current
DCO posture stays, and dual licensing (selling an AGPL exception) is off the
table permanently. The rest of this memo documents why, grounds the commercial
surface in the actual crates, and records what the contributor-rules record
now says so no future reader can treat this as an open question.

## 1. The tension this memo resolves

On 2026-10-27 Don kept the license split: Apache-2.0 for applications and
libraries, AGPL-3.0 for the server (PDM), and the P2P orchestration, and said
the program "maybe commercialize[s] that at the enterprise or b2b level".

The existing contribution rule is DCO with no CLA, recorded in
`workstreams/governance/contributor-rules/contributor-rules-decision.md`
(version 1.0.0, 2026-10-15). That record rejects a CLA on the grounds that "a
CLA exists to give a project relicensing leverage or copyright assignment, and
this program wants neither". That sentence was written when nothing was
commercial. It is still true, but it is no longer merely a default: it is now
a deliberate, permanent policy that the commercial model depends on. The
reason it matters before the first outside PR is cost asymmetry: resolving the
question is free today and grows with every outside commit that would need to
consent to a retrofitted CLA. Don decided it now, while the contributor base
is effectively one maintainer.

## 2. The two viable models

### Model 1: Services and add-ons under AGPL, no CLA (chosen)

Sell support, integration, training, managed hosting, or a separately-owned
proprietary enterprise add-on that talks to the AGPL server over its API.
This is compatible with the DCO posture for two reasons:

- The AGPL (section 13) compels source disclosure only when a *modified*
  version of the licensed work is offered to third parties over a network. An
  enterprise running the server internally, unmodified or internally modified,
  owes nothing. The software service provisions of AGPL do not reach internal
  deployment.
- A separately-owned add-on that communicates with the AGPL server over its
  public API at arm's length is a separate work, not a derivative of the
  server. Proprietary code does not become AGPL merely by calling the server
  over HTTP. The add-on can be sold closed-source without breaching the AGPL
  terms of the server itself.

No CLA is needed because nothing here licenses any outside contributor's code
to anyone new. Contributions stay under the same open license as before, under
the DCO. There is no contributor friction, no retrofitting, no consent wall.

### Model 2: Dual licensing under AGPL, requires a CLA (rejected, permanent)

The dual-license model (the classic MySQL/Qt path) sells an AGPL exception:
proprietary customers pay for the same code under a closed license or under
permissive terms. Selling that exception requires the project to own or hold
sufficient rights in every contribution, because granting a proprietary
license to third parties reaches beyond the rights the contributors already
granted under AGPL. That requires a CLA or copyright assignment from every
contributor.

The decisive practical fact: retrofitting is impossible. Every commit that
ever landed under DCO carries a grant under the project license only, not a
grant to the project itself. You cannot spin up a dual-license program later
without unanimous consent from everyone who ever contributed. The cost of
closing the door today is zero; the cost of leaving it open grows with every
outside commit. This memo does not merely prefer model 1; it records that
dual licensing is off the table permanently, so no later reader reopens it as
an "unresolved" option that a different commercial answer could unlock.

## 3. The commercial surface, grounded in the code

The crates that are the actual commercial surface are, without exception,
already AGPL-3.0 (verified against `crates/*/Cargo.toml`):

- PDM revisions and audit: `oep-pdm` (AGPL) carries the revision lifecycle
  state machine (`crates/oep-pdm/src/state.rs`, four-state lifecycle plus a
  maturity axis), the append-only tamper-evident audit log
  (`crates/oep-pdm/src/audit.rs`, chained SHA-256, JSON-lines store with a
  committed head checkpoint), and the content-addressed blobstore. This is the
  sellable core: revision integrity and a verifiable audit trail over vault
  content.
- Compute lane: `oep-lane` (AGPL) is the P2P scheduler (priority queue,
  dependency DAG, lease/ack retry and dead-letter seam), `oep-fairshare`
  (AGPL) is weighted fair-share with quota enforcement, `oep-farm` (AGPL) is
  checkpoint/resume, and `oep-worker`/`oep-render-worker` (AGPL) are the
  CGR/STEP/render job implementations, with `oep-sandbox` (AGPL) isolating
  worker execution. This is the sellable capability: verifiable distributed
  render and conversion at scale inside the enterprise.
- Enterprise auth: `oep-pdm` identity (accounts, passport, password and PHC
  hashing) plus the P11-T03 RBAC boundary enforced by `oep-web` (AGPL) as the
  default start path, with the no-auth path explicitly a residual
  configuration. This is the sellable trust surface: role boundary and
  authorization for multi-user deployments.

Every commercial wedge is therefore inside the chosen model's rules already.
There is nothing here that dual licensing would unlock and model 1 does not
already permit.

## 4. Is a proprietary add-on architecturally separable? (findings)

The PDM server exposes a public, versioned contract: `openapi-v1.yaml`
(OpenAPI 3.0.3, base path `/api/v1`) at
`workstreams/pdm/api/contract.md`, single source of truth with a generated
python client and a drift gate. That is a clean arm's-length seam: a
separately-owned add-on can talk to the AGPL server over this documented HTTP
API and remain a separate work. The PDM surface is separable today.

Two findings for Don, stated plainly because model 1 only holds while the
add-on boundary genuinely exists:

1. The compute lane has no public REST contract. Only the message-bus ADR and
   the lease/ack scheduler protocol exist under `workstreams/compute/`. A
   proprietary managed-compute, priority, or quota add-on built now would have
   to speak internal P2P protocol or hook into the scheduler, which blurs the
   "separate work at arm's length" claim. If managed compute or enterprise
   scheduling is a commercial product, the compute lane needs its own public,
   documented API contract before that add-on's separability claim is honest.
   That is a separate follow-up card, not a change to this decision.
2. Enterprise auth is enforced inside `oep-web` (the P11-T03 RBAC boundary),
   not behind a public extension point. An IdP/SSO or governance add-on that
   must plug into the RBAC flow has no documented seam today. Same consequence
   as finding 1: a follow-up contract task if enterprise SSO is a product.

Both findings are open items for the product roadmap, not defects in this
decision. Model 1 does not require them to be built; it requires the boundary
to exist in each place a proprietary add-on is actually sold, and it does not
exist yet for compute and auth. If a future product decides to build a
proprietary add-on over a seam that does not exist because of this memo, that
is a violation of the model, not an exercise of it.

## 5. What the DCO record now says (amendment)

Per the card, the contributor-rules record is amended so its CLA rejection is
a deliberate, permanent policy rather than a default that predates
commercialization:

- The sentence "this program wants neither" is now tied to D-ISOLITH-1 and the
  services/add-ons model. It is a standing policy, not a convenience.
- The only posture that would ever require owning every contribution, selling
  an AGPL exception, is ruled out permanently. The case-scoped corporate
  contributor agreement escape hatch in the record's options section remains,
  but it is for a specific contributor, never a standing rule and never the
  dual-license program.

If model 2 had been chosen, the amendment would instead have been: require
CLA plus DCO, Harmony-style contribution agreements (Apache ICLA/CCLA
compatible), bot-enforced so no PR merges without a recorded agreement, and a
new card to add that machinery before the first outside PR. That is recorded
here only so the alternative path is understood; it is not being executed.

## 6. Recommendation and decision

Recommendation: Model 1, no CLA, DCO unchanged, dual licensing permanently off
the table. This is the smallest set of changes consistent with the enterprise
/B2B plan, keeps zero contributor friction, and closes the retrofitting risk
at zero cost while the contributor base is one person.

Decision, recorded 2026-10-29 as D-ISOLITH-1 = A in the decisions ledger
(`~/.hermes/cron/state/decisions-ledger.md`):

- Commercialize via support, integration, training, managed hosting, and a
  separately-owned proprietary enterprise add-on talking to the AGPL server
  over its API.
- DCO stays. No CLA, no copyright assignment, no standing contributor
  agreement.
- No AGPL exception is ever sold. Dual licensing is permanently unavailable.
- The contributor-rules decision is amended (this memo is the amendment's
  companion record).

## 7. Follow-ups

- Amend only: the contributor-rules record is version-bumped in place
  (1.0.0 to 1.1.0) by this memo's companion edit; no new card required.
- If managed compute or enterprise SSO becomes a product, open a contract card
  to give the compute lane and the auth seam public API surfaces (findings 1
  and 2), so the separability claim stays honest.
- Before the first outside PR is accepted, a human verifies the contributor
  path still says DCO only and that no language anywhere promises dual
  licensing. If either check fails, the PR is held and governance is
  re-contacted; this is a gate, not a formality.

## 8. References

- workstreams/governance/contributor-rules/contributor-rules-decision.md
  (1.0.0, being amended to 1.1.0)
- workstreams/pdm/api/contract.md and workstreams/pdm/api/openapi-v1.yaml
  (the public PDM REST v1 seam)
- workstreams/compute/ (scheduler, message-bus ADR, GA gate; no public
  REST contract)
- crates/oep-pdm, oep-web, oep-lane, oep-fairshare, oep-farm, oep-worker,
  oep-render-worker, oep-sandbox (AGPL commercial surface) and their
  Cargo.toml license fields
- charter.md (license posture, clean-room, sovereignty)
- decisions ledger D-ISOLITH-1 (2026-10-29)

## Change log

- 1.0.0 (2026-10-29): initial record. Decides services and add-ons under AGPL,
  no CLA, dual licensing permanently off the table, per Don's decision
  D-ISOLITH-1. Ratified.
