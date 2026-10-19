# P8-M2-T01 -- Revision state machine: working / under-review / released / obsolete

Status: implemented. Owner: P8 PDM lane. Version: 1 (2026-10-17).

## 1. Purpose

A PDM *revision* has a lifecycle. This card makes it explicit, typed, safe and
persisted. It replaces the "accepts any string" handling in the P7-M2-T02 stub
(crates/oep-pdm `Revision.state` / `maturity`) with a real state machine that
the live P8 service (P8-M6-T01) plugs in without a rewrite.

The four states and the nullable `maturity` string are adopted verbatim from
the wire contract `workstreams/P8-M6/openapi-v1.yaml` (v1):

    state:    working | under_review | released | obsolete      (default working)
    maturity: string, nullable

## 2. State machine

### 2.1 States

- working       active engineering; the revision is edited freely
- under_review  submitted for peer/formal review
- released      approved and frozen; immutable except for the terminal obsolete hop
- obsolete      superseded or withdrawn; terminal

### 2.2 Legal hops (the complete, exhaustive table)

    working      -> under_review    submit for review
    working      -> released        direct release (fast path; requires final maturity)
    working      -> obsolete        abandoned draft
    under_review -> working         returned for rework
    under_review -> released        approved (requires final maturity)
    under_review -> obsolete        rejected / withdrawn
    released     -> obsolete        superseded later in service

That is 7 legal hops out of 16 possible ordered (from, to) pairs. Every other
hop is illegal and returns a typed error:

- self-hops (4):        TransitionError::IllegalTransition
- released -> working, released -> under_review (2): TransitionError::Immutable (release immutability)
- obsolete -> any (4):  TransitionError::Terminal (obsolete is terminal)

All 16 hops are enumerated and asserted in the unit tests
(crates/oep-pdm/src/state.rs) so the table can never drift from the tests.

### 2.3 The maturity axis and the maturity transient

A revision carries a second, independent axis, Maturity:

    preliminary (rank 0) < detailed (rank 1) < final (rank 2)

Maturity is a *transient* property for the whole time the revision is not
released: while working or under_review it may be set, advanced, or rolled
back freely, because nothing is frozen until release. Release is the end of
the maturity transient:

- entering released requires maturity already at Final, else
  TransitionError::MaturityNotFinal (you cannot release work that has not
  finished maturing) -- this is enforced on both working and under_review
  sources;
- at release maturity is frozen at Final;
- set_maturity / advance_maturity on a released or obsolete revision returns
  TransitionError::Immutable.

Every transition is atomic: it returns a TransitionOutcome (from, to,
maturity before/after) and the intermediate transient value is never
observable at rest.

## 3. Persistence and queryability

RevisionStore roots at a directory:

    <root>/blobs/<sha256>                  content-addressed file payloads
    <root>/revisions/<item_id>/<rev>.json  one snapshot per revision (authoritative)
    <root>/items.json                      item index (revision numbers, latest, created_at)

- Every mutation goes through the state machine first; only a validated result
  is written to disk, so on-disk state is always consistent with the machine.
- Queries (get_revision, list_revisions, latest_revision, all_items) read the
  latest snapshots.
- File payloads are content-addressed by their sha-256 and verified on read
  (HashMismatch on any drift), the same object-store mirror pattern as
  P8-M1-T02 / P1-M4.
- item_id is restricted to [A-Za-z0-9._-] so identifiers can never escape the
  store root (path traversal is impossible by construction).
- Attaching/updating content on a released or obsolete revision is refused
  (immutability is enforced in the store too, not just the machine).

## 4. Acceptance mapping

- state transitions unit-tested, all legal/illegal hops enumerated
  -> crates/oep-pdm/src/state.rs unit tests (16/16 hops + gate + freeze).
- state persisted and queryable
  -> crates/oep-pdm/tests/revision_state_machine_store.rs (temp-dir store,
     reopen-and-query round trips).
- released-state immutability enforced and tested
  -> machine (Immutable hop / set_maturity) + store (content attach refused,
     verified across restart).

## 5. Files

- crates/oep-pdm/src/state.rs     pure state machine (no IO)
- crates/oep-pdm/src/store.rs     persisted, queryable revision store
- crates/oep-pdm/tests/revision_state_machine_store.rs
- tests/test_p8m2t01_revision_state_machine.py (contract lockstep guard)
- this document
