---
title: P11-T03 Auth Boundary (RBAC bypass attempts, IDOR on vault objects)
status: Ratified (Wave-3 P11-T03)
version: 1.1.0
date: 2026-10-18
owner: Keelworks program governance, P11 security lane
applies_to: the oep-pdm REST API v1 surface (items, revisions, files, audit log, lock/transition, sync), served behind the P8-M3 RBAC authorizer
supersedes: nothing (first auth-boundary record; closes threats NW-1 and NW-5 in P11/threat-model.md via mitigation M-RBAC)
---

# P11-T03 Auth Boundary (RBAC bypass attempts, IDOR on vault objects)

## 1. Purpose

This card is the enforcement half of the P11 security wave's authentication
story. P11-T01 classified two vault-surface threats as Critical/High:

| Risk | Threat | Mitigation |
|------|--------|------------|
| NW-1 | No authentication or RBAC on the REST/vault surface | M-RBAC (this card) |
| NW-5 | IDOR / object-level authz bypass on items, revisions, files, audit log | M-RBAC (this card) |

The deliverable is a security test suite that attempts cross-role and
cross-tenant access on items, revisions, files and audit-log endpoints via
the PDM REST API and asserts denial (403/404). It runs in CI, passes on the
current RBAC implementation (P8-M3), and contains a test that fails on a
default-permissive RBAC stub (proving the suite actually asserts). Clean-room
invariant: the enforcement is our own P8-M3 RBAC engine wired to our own REST
contract; no vendor PDM authorization model is referenced.

## 2. What the boundary enforces

The enforcement layer lives in `crates/oep-pdm/src/authz/` and answers one
question per request before any contract handler runs: may this principal
perform the action this endpoint requires, on this vault object, in this
tenant?

Denial contract (what a caller observes, and why it is never an oracle):

| Case | HTTP status | Contract error code | Why |
|------|-------------|---------------------|-----|
| Unauthenticated (no principal) | 403 Forbidden | forbidden | No identity, no decision; 403 because the card's acceptance names 403/404 (D2) |
| Cross-role (role lacks the action) | 403 Forbidden | forbidden | The request is visible but not permitted |
| Cross-tenant (object belongs to another tenant) | 404 Not Found | not_found | IDOR masking: a foreign object is indistinguishable from a missing one |
| Endpoint outside the mapped contract surface | 404 Not Found | not_found | Same masking for the route table itself |
| Genuinely absent object | 404 Not Found | not_found | The handler's own 404 (unchanged contract behavior) |

The legitimate same-tenant caller is unaffected: every Read/Edit/Create walk
the role matrix as documented (section 4) and pass through to the real
handlers unchanged.

## 3. The enforcement stack

```
Request
  -> authz::gate::authz_middleware (axum layer, opt-in)
       -> principal::Principal::from_headers  (X-Oep-User / X-Oep-Roles /
                                               X-Oep-Tenant)
       -> endpoints::classify(method, path [, body])  (endpoint -> Action)
       -> resource::item_tenant(item)                (object -> owning tenant)
       -> policy::strict_boundary_authorizer().decide(subject, resource,
             action, env)                            (P8-M3 RBAC core)
       -> 403 / 404 / pass-through
  -> the unchanged contract handler
```

The P8-M3-T02 RBAC core (`crates/oep-pdm/src/rbac/`) is the current RBAC
implementation the suite must pass on: a static role matrix plus an
attribute-based (ABAC) rule layer with default-deny semantics. This card
contributes the REST wiring the RBAC module always anticipated: the gate is
served by `PdmService::start_with_authorizer` / `build_authorized_router`
and, SECURE BY DEFAULT (SEC-AUTHZ-DEFAULT), the shipped `oep-pdm` /
`oep-pdm-stub` binaries boot through it on every start
(`service::serve_from_args` builds `strict_boundary_authorizer` unless the
operator passes the explicit `--no-auth` opt-out). The open contract is
therefore the opt-OUT, reserved for controlled test / air-gap use, and
opting out prints a loud startup warning. The library `PdmService::start*`
paths (no authorizer) remain open for embedders and in-process tests, which
is why the acceptance's default-path leg boots the real binary.

Wiring principle: the gate is layered on the SAME handlers and route table
(`service::shared_router`). Nothing is rewritten per-route; classification
is the only difference.

Scope note (SEC-AUTHZ-DEFAULT): this card's secure-by-default wiring covers
the shipped PDM control plane (`oep-pdm`, `oep-pdm-stub`). The `oep-web`
browser workbench (P8-M4-T01) merges the same REST v1 router onto its UI
surface and remains open until the identity layer lands; it is a separately
tracked residual (its loopback-bind default keeps it LAN-off per decision
#10) and must adopt the same gate before any public exposure.

## 4. Endpoint to action map

The single source of truth is `authz::endpoints::classify`. The map is
exhaustive over the contract routes served by `service::build_router` (the
layout/acceptance machinery checks this stays true):

| Verb | Path | RBAC action (required role) |
|------|------|-----------------------------|
| GET  | /api/v1/health | (open, no auth) |
| GET  | /api/v1/items | Read |
| POST | /api/v1/items | CreateItem (designer/admin) |
| GET  | /api/v1/items/{item_id} | Read |
| GET  | /api/v1/items/{item_id}/file/latest | Read |
| GET  | /api/v1/items/{item_id}/revisions | Read |
| POST | /api/v1/items/{item_id}/revisions | EditRevision (designer/admin) |
| GET  | /api/v1/items/{item_id}/revisions/{rev} | Read |
| GET  | /api/v1/items/{item_id}/revisions/{rev}/file | Read |
| POST | /api/v1/items/{item_id}/lock | EditRevision |
| DELETE | /api/v1/items/{item_id}/lock | EditRevision |
| POST | /api/v1/items/{item_id}/revisions/{rev}/transition | by target state: under_review -> SubmitForReview; released -> Release; obsolete -> MakeObsolete; working -> Review |
| GET  | /api/v1/audit | ManageSystem (admin only, see D3) |
| GET  | /api/v1/audit/verify | ManageSystem (see D3) |
| POST | /api/v1/sync/push | EditRevision (target item from body) |
| GET  | /api/v1/sync/conflicts/{sync_id} | Read |
| GET  | /api/v1/sync/conflicts/{sync_id}/file/{side} | Read |
| POST | /api/v1/sync/conflicts/{sync_id}/resolve | EditRevision |
| GET  | /ui/sync/conflicts/{sync_id} | Read |
| POST | /ui/sync/conflicts/{sync_id}/resolve | EditRevision |
| any | anything else | 404 (uniform, no oracle) |

The role matrix itself is P8-M3-T02 `RoleMatrix::standard` and unchanged:
Designer (read, create/edit, submit), Reviewer (read, review), Release
Authority (read, release, obsolete), Admin (all). A malformed transition body
maps conservatively to EditRevision; a real release only ever follows a
parseable `"to":"released"`, which maps to Release, so a malformed body can
never grant a release.

## 5. Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| D1 | Tenant isolation is absolute: even Admin cannot cross a tenant through this gate (cross-tenant deny-wins applies to every action). | The cleanest, strongest boundary to test; a scoped cross-tenant grant for a future support role becomes an explicit Allow rule when the identity layer lands. The P8-M3 engine cannot express "deny unless role is admin" (conditions compare one attribute to a value), so strict is also what the engine can honestly deliver today. |
| D2 | Unauthenticated requests get 403 Forbidden (not 401). | The card's acceptance names 403/404; 403 keeps the suite's assertions within that contract. A principal is required for any protected endpoint. |
| D3 | Reading/verifying the audit log is ManageSystem (admin only), not Read. | The audit trail records every principal's reads and changes (sovereignty reads, P8-M3-T03). Granting all readers the audit log would itself leak cross-role access patterns. Admins, and only admins, may page through or verify it. |
| D4 | Cross-tenant responses are 404 (existence masked); cross-role denials are 403. | Separates IDOR-masking (hide the object) from authorization failure (the object is visible but the action is not permitted). Both are in the acceptance's "403/404" set. |
| D5 | Identity headers are client-declared in this card. | Authentication (who you are) is P8-M3-T01's job; authorization (what you may do) is this card's job. The suite pins the decision path; the identity layer will replace the header principal with a verified passport and keep the decision path byte-for-byte. Until then the boundary must be understood as authorization-grade only. |
| D6 | Authorization is ON by default (SEC-AUTHZ-DEFAULT): the shipped binaries enforce the RBAC boundary on every start; the open contract is only reachable by explicitly passing `--no-auth`, which prints a loud startup warning that authorization is disabled. | Defense customers review "secure when configured" as not secure; an unauthenticated control-plane API is the classic lateral-movement foothold even inside the customer network (decision #10 reduces but does not remove exposure: insider, misconfiguration, co-tenant LAN). Making the open path the explicit, warned opt-out keeps controlled test/air-gap use possible while the default denies. The acceptance measures the suite against the DEFAULT binary, not an opt-in path. |

## 6. How the suite proves it asserts (the permissive-stub test)

`crates/oep-pdm/tests/auth_boundary.rs` runs the SAME attack probes in three
modes against real sockets:

1. Strict mode (`strict_boundary_authorizer`): every cross-role,
   cross-tenant and unauthenticated attempt is asserted denied (403/404),
   and every legitimate same-tenant call asserted allowed (2xx).
2. Permissive mode (`permissive_authorizer`, the default-permissive stub):
   the exact same attack list must NOT be denied, and the strict/permissive
   denial counts must differ.
3. Default-binary mode (SEC-AUTHZ-DEFAULT, the acceptance's measured leg):
   the suite is re-run against the SHIPPED `oep-pdm` binary started with no
   flags at all. Every cross-role / cross-tenant / unauthenticated probe must
   be denied (403/404), legitimate same-tenant use must stay green, and the
   `--no-auth` opt-out must both print a loud startup warning and serve the
   open contract (zero denials), so the default is measurable as strictly
   more protective.

`strict_mode_denies_what_permissive_allows` therefore fails if the boundary
stops asserting (a permissive authorizer grants, the strict leg's denials
vanish, and the two counts become equal). This was verified mechanically on
this card: booting the strict denial test against the permissive authorizer
turns it RED (`reviewer creates item granted under strict boundary (status
201)`). A default-permissive RBAC stub is exactly what this test catches.

## 7. Clean-room note

The authorization semantics under test are our own: the P8-M3-T02 role
matrix/ABAC engine and the vault model's ownership tags mapped by
`authz::resource`. No vendor PDM client (ENOVIA/3DEXPERIENCE, Teamcenter, or
any other) authorization surface, object model or semantics are copied,
re-exported or referenced anywhere in this card's code or tests.

## 8. Verification

Run the whole gate the same way CI does:

```
cargo test -p oep-pdm --test auth_boundary        # strict + permissive + DEFAULT-BINARY legs
cargo test -p oep-pdm --lib authz                 # unit: map, principal, resource, policy, gate
python3 tests/test_p11_auth_boundary.py           # acceptance: committed artifacts + invariants
python3 tests/test_p11_threat_model.py            # threat model (P11-T01): no-orphan + posture in sync
```

CI runs `ga-gate-p11t03` (`.gitea/workflows/ga-gate-p11.yaml`) on every push
and every pull request to the repo.

## 9. Change log

- 1.1.0 (2026-10-18): SEC-AUTHZ-DEFAULT. The shipped `oep-pdm` /
  `oep-pdm-stub` binaries now enforce the RBAC boundary on every start
  (decision D6); the open contract is the explicit, loudly-warned `--no-auth`
  opt-out for controlled test / air-gap use. The suite gained the
  default-binary leg: the attack matrix is re-run against the real default
  binary (denied 403/404, legit use green) and `--no-auth` is proven to warn
  and open. Open-contract harnesses (P8/P10 pytest legs, GA-gate tooling,
  one-shot proof scripts) now pass `--no-auth` explicitly. P11/threat-model.md
  C1 + M-RBAC updated to the wired, default-on posture so both documents
  agree.
- 1.0.0 (2026-10-18): first ratification. Enforcement layer, denial contract
  (403 cross-role / 404 cross-tenant), endpoint map, decisions D1-D5, and the
  two-mode mutation-proof suite, on top of the P8-M3-T02 RBAC core. Written
  against main @ 094bea1 plus the P8-M3-T02 RBAC bundle (cherry-picked).
  Bump the version and append a line here on any material change.
