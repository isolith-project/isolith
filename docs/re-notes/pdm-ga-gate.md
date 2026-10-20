# P8-M6-T03 RE note: P8 GA gate (multi-user edit/publish/release + concurrent lock test)

Task: P8-M6-T03 (t_8cee3e28). Delivered as isolith PR (branch
task/p8m6t03-ga-gate-1017a). Version 1 (2026-10-17).

## What landed

The P8 PDM GA gate. This milestone was the integration point where the P8
multi-user lifecycle ceased to be library code and became served, tested, real
behavior over the REST contract:

- Wired three real, already-TDD'd capability modules (from their own milestone
  PRs) into `oep_pdm::VaultStore` and the axum service additively:
  `working_set::LockManager` (P8-M1-T03, object locking), the `state` machine
  (P8-M2-T01, revision lifecycle + maturity), and `audit::AuditLog`
  (P8-M3-T03, append-only tamper-evident chain). The two token accessors
  (`LockToken::wire` / `from_wire`) are the only code added to the copied
  modules.
- Added five additive REST verbs (new paths only -- the existing v1 wire is
  byte-identical): `POST/DELETE /api/v1/items/{id}/lock`, `POST
  /api/v1/items/{id}/revisions/{rev}/transition`, `GET /api/v1/audit`,
  `GET /api/v1/audit/verify`. Declared in `workstreams/pdm/api/openapi-v1.yaml`; the
  generated python client (M6-T01 deliverable) exposes them.
- The store now audits every mutating + read verb; the transition verb is the
  publish gate (lock token required to release) and enforces the state machine
  (released revisions immutable except the terminal obsolete hop).
- New harness `python/oep_tooling/ga_gate_p8.py` + acceptance test
  `tests/ga_gate_p8_test.py` (TDD RED -> GREEN) + proof script
  `scripts/pdm-api-ga-gate-proof.sh` + committed evidence
  `workstreams/pdm/api/proof/ga-gate-proof.json` and
  `workstreams/pdm/api/proof/ga-gate-p8-multiuser.{json,md}`.
- Separate CI gate job `.gitea/workflows/ga-gate-p8.yaml` (no ci.yaml
  collision, the P2/P3/P4 convention).

## Measured result (real material, live on the NAS)

`ga-gate-p8-multiuser.json`: total=11, PASS=11, FAIL=0, SKIP=0, exit 0.

- MULTIUSER: 4 real concurrent OS client processes each edited, published and
  released their own item over the live service.
- LOCKS: exactly one of 4 racers acquired the shared-item lock; a
  forged/foreign token was refused on the publish gate (409) and the real
  holder's token succeeded (200).
- IMMUTABLE: `released -> working` was refused; the released blob re-read
  byte-identical with a stable sha-256.
- AUDIT: 33 entries recorded; the tamper-evident chain re-verified.
- NAS: the built binary was staged on the NAS generic package registry
  (shaped after P8-M5-T03) and re-hashed to its sha-256.

Local gate: cargo test (38 oep-pdm tests incl. the new ga_gate_acceptance),
clippy -D warnings clean, fmt clean, pytest 30 passed.

## What blocks GA closure

- Nothing in this gate is a gate-FAIL (exit 0). Honest SKIP rows are
  generated only when the service is unreachable (`service-unreachable`),
  audit is disabled (`audit-unavailable`), or the NAS is down
  (`nas-unreachable`) -- all tolerant, never crashes.
- The working-set lock is advisory at the data layer (concurrent *editing* of
  working copies is allowed; the lock serializes *publish*). This matches the
  P8-M1-T03 protocol and the P8-M5-T02 conflict-resolution milestone is the
  intended follow-on for leaf-level conflicts.
- Audit identity is a plain principal string (identity/OIDC is P8-M3-T01's
  work); the audit surface records `created_by`/holder as given.
- Token `steal` (crash-recovery force-transfer) exists in the lock manager but
  is not exposed as a REST verb in this milestone; the lease TTL covers the
  crashed-client case.
