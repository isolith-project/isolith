# P8-M6-T03: P8 GA gate -- multi-user edit/publish/release + concurrent lock test

Status: implemented as part of task P8-M6-T03 (keelworks card t_8cee3e28).
Owner: open-engineering-platform P8 PDM lane.
Version: 1 (2026-10-17).

## 1. What this gate is

This is the P8 PDM **GA gate**: a measured, honest acceptance harness that
proves the P8 product can be used by N real concurrent users at once -- not a
log line, not a synthetic aggregate. It follows the exact GA contract of the
P2/P3/P4 gates (measured criteria, honest classification with a closed SKIP
taxonomy, exit 0 iff no FAIL, self-consistent JSON+MD report), with the
difference that its "real material" is a **live PDM service plus real
concurrent client processes**.

One precondition had to be delivered *by this milestone*: the finite-state
layers that the acceptance criteria name (locks respected, revisions immutable
after release, everything verifiable via the audit log) were, until now, real
Rust modules on their own PR branches but **not served over the REST API**. The
GA gate is the integration milestone that wires them in as **additive** REST
surface (never changing the existing v1 wire) and then proves the full cycle:

| verb | path (additive) | operationId | real module behind it |
|------|-----------------|-------------|-----------------------|
| POST | `/api/v1/items/{item_id}/lock`       | `acquireLock`        | `working_set::LockManager` (P8-M1-T03) |
| DELETE | `/api/v1/items/{item_id}/lock`     | `releaseLock`        | `working_set::LockManager` (P8-M1-T03) |
| POST | `/api/v1/items/{item_id}/revisions/{rev}/transition` | `transitionRevision` | `state` machine (P8-M2-T01) |
| GET  | `/api/v1/audit`                     | `listAudit`          | `audit::AuditLog` (P8-M3-T03) |
| GET  | `/api/v1/audit/verify`              | `verifyAudit`        | `audit::AuditLog` (P8-M3-T03) |

These are declared in `P8-M6/openapi-v1.yaml` (new paths) so the generated
python client (the M6-T01 deliverable) exposes `acquire_lock`, `release_lock`,
`transition_revision`, `list_audit` and `verify_audit` -- the gate harness
drives the *same contract* the rest of the product speaks.

## 2. The five legs (what the gate actually measures)

The harness lives in `python/oep_tooling/ga_gate_p8.py` and the acceptance
test in `tests/ga_gate_p8_test.py` (TDD RED -> GREEN). Each leg returns
measured rows; the report rolls them up.

1. **MULTIUSER** -- spawns N real OS processes (multiprocessing spawn, default
   4, parameterized) against the live service. Every process creates its own
   item, saves a working revision, acquires the lock, transitions the revision
   to `released`, releases the lock. A client that does not complete its full
   cycle is a FAIL row -- the acceptance is N clients who actually released,
   not N requests that echo a 200.

2. **LOCKS** -- the concurrent lock test. N racer processes all try to acquire
   the lock on the *same* shared item simultaneously. Exactly one wins (the
   others get the contract `lock_conflict`); then the publish gate is asserted
   over the wire: a forged/foreign token is refused on the transition verb and
   the real winner's token is accepted. Releasing with the right token returns
   the item to the acquirable set.

3. **IMMUTABLE** -- after a revision reaches `released`, the state machine
   refuses the `released -> working` hop (400/409 `invalid_state`) and the
   content-addressed blob re-reads byte-identical with a stable sha-256. This
   is "revisions immutable after release".

4. **AUDIT** -- `GET /api/v1/audit` returns the append-only, tamper-evident
   log (every create / edit / lock / release / read is an entry) and
   `GET /api/v1/audit/verify` re-derives the whole hash chain from disk and
   confirms the head checkpoint -- "everything verifiable via the audit log".

5. **NAS** -- the built oep-pdm binary is staged on the NAS object store
   (Gitea generic package registry, the P8-M5-T03 LAN/air-gap path), fetched
   back and re-hashed to its sha-256. NAS-unreachable is SKIP (tolerant
   doctrine), never FAIL; live NAS answers with a hash-verified PASS.

## 3. Exit code contract

`0` iff no gate-FAIL row anywhere (SKIP allowed). Reports
`ga-gate-p8-multiuser.{json,md}` are self-consistent: the summary totals roll
up exactly to the emitted rows and `schema_version == 1`. A row carries a
closed taxonomy code for every SKIP (`service-unreachable`, `binary-missing`,
`audit-unavailable`, `nas-unreachable`, `no-lock-surface`, `io-error`).

## 4. Honesty boundaries

- A leg that cannot reach the service is SKIP per the taxonomy -- never an
  invented PASS, never a crash.
- The gate never simulates a user with an aggregate counter: N is a real
  process count and every row names a concrete client.
- The audit leg counts real entries and verifies the real chain; a disabled
  audit log is honestly reported (`audit_unavailable`), not fabricated.
- The existing v1 wire is byte-identical: the gate adds paths only, and the
  full stub/contract/client test suites stay green unchanged.

## 5. Components

- `crates/oep-pdm/src/working_set.rs` -- P8-M1-T03 object locking (copied
  verbatim from its milestone branch; token wire accessors added).
- `crates/oep-pdm/src/state.rs` -- P8-M2-T01 revision state machine (copied
  verbatim from its milestone branch).
- `crates/oep-pdm/src/audit.rs` -- P8-M3-T03 tamper-evident audit log (copied
  verbatim from its milestone branch).
- `crates/oep-pdm/src/api.rs` -- additive gate request/response types +
  error codes (`lock_conflict`, `lock_token`, `audit_unavailable`).
- `crates/oep-pdm/src/store.rs` -- `VaultStore` wires the three modules in:
  locks gated by `LockManager`, transitions through `state`, audit append on
  every mutating/read verb, `enable_audit()` under the persist dir.
- `crates/oep-pdm/src/service.rs` -- additive route handlers for the five
  verbs; audit enabled when `--persist-dir` is provided.
- `P8-M6/openapi-v1.yaml` + `scripts/gen_pdm_client.py` + regenerated
  `python/oep_pdm_client.py` -- the gate surface declared in the contract.
- `python/oep_tooling/ga_gate_p8.py` -- the gate harness.
- `tests/ga_gate_p8_test.py` -- the TDD acceptance (RED first, GREEN now).
- `scripts/p8m6t03-ga-gate-proof.sh` -- boots the service, runs the full gate,
  stages the proof on the NAS, writes `P8-M6/proof/p8m6t03-ga-gate-proof.json`.
- `.gitea/workflows/ga-gate-p8.yaml` -- separate CI gate job (no ci.yaml
  collision, the P2/P3/P4 convention).
