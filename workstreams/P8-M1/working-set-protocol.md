# P8-M1-T03 -- Working-set protocol: lock / edit / publish (object locking)

Status: implemented (P8-M1-T03). Owner: P8 PDM lane. Version: 1 (2026-10-17).
Delivered as keelworks/keelworks PR #<TBD>, branch `task/p8m1t03-working-set-1016a`
(TDD: RED then GREEN).

## 1. Purpose

Wave-2 P8 PDM lane, milestone P8-M1-T03. A PDM user does not normally edit
the vault's own copy of a file in place. The classic vendor answer is
*check-out*: reserve the file, copy it into a private workspace, edit there,
check it back in, and (in many systems) make the source object read-only for
everyone else while it is out.

This card deliberately rejects that model. Keelworks uses **object locking**:
a working set is the set of vault objects a principal currently holds
short-lived **leases** on, for one edit session. The protocol is exactly three
steps --

    lock        acquire a lease on each object you intend to edit
    edit        work on your local working copy (never blocked, concurrent)
    publish     write a new revision back, gated on still holding the lease

and its guarantee is **single-writer at publish time, not single-editor at all
times**. Reading and constructing working copies are never exclusive. The lock
is the serialization point only for *publishing* a new revision of a given
object.

Design rules for this card (hard):

* clean-room only -- no vendor binaries, no copied source, no ENOVIA/Dassault
  or any other PDM's API shapes, constants, or terminology;
* our own lease semantics (token + TTL), documented here and in the crate docs;
* support concurrent edit now, and leave the publish-time conflict surface
  explicit so P8-M5-T02 ("offline mode + conflict resolution UI") can attach
  to it without redesign.

## 2. The three-step flow

```
 step 1  LOCK
   token = lockmanager.acquire(&object_id, &holder)?
     - free or already-expired object -> token granted
     - live lease held by someone else  -> ConflictingLock (retry later / re-read)
     - same holder re-acquiring        -> lease refreshed, new token

 step 2  EDIT
   read the latest revision (or any revision), work on a local copy.
   Nothing here touches the lock manager. Others may do the same on their
   own copies concurrently.

 step 3  PUBLISH
   lockmanager.authorize_publish(&object_id, &token)?
     - no record, stale token (stolen/refreshed), or expired lease -> denied
     - live token -> Ok, safe to write the new revision atomically
   (...revision-state machine and where-used are later milestones.)
```

A client that crashes mid-session leaves a lease that quietly expires; the
object is acquirable again after the TTL without admin action.

## 3. Lock-state machine

One lock record exists per object at any moment. State is *derived* from the
record plus the current clock, never stored redundantly (no drift between a
flag and a timestamp).

    state (derived)          meaning
    ------------------------ ---------------------------------------------
    Free                     no record, or record.expires <= now
    Held by H                record.holder == H  and  record.expires > now

Transitions (all token-checked except acquire of a free object):

    Free        --acquire(H)-->            Held by H         (mint token)
    Held by H   --re-acquire(H)-->         Held by H         (refresh lease, new token)
    Held by A   --acquire(B)-->            error ConflictingLock
    Held by any --steal(B)-->              Held by B         (force, old token dies)
    Held by H   --release(token)-->        Free              (token must match)
    Held by H   --clock>expires-->         Free (expired)    (no action needed; lazy)

Every transition consumes/validates the opaque `LockToken`. A stolen or
refreshed lease invalidates the old token, so a superseded client can neither
release someone else's lock nor publish against it -- that is the mechanism
that makes conflict surfaces turn up at *publish*, exactly where they belong.

## 4. Concurrency semantics (documented contract)

1. **Single-writer per object.** At most one live lease exists per object, and
   only its holder may publish. `acquire` is atomic (internal mutex); N racing
   acquirers serialize and exactly one wins.
2. **Concurrent edit is allowed.** The lease does not block reads or the
   construction of working copies. Two users may both hold a revision open and
   edit it; the moment they both try to publish, the second one's token is no
   longer live (lease expired or refreshed) and publish is denied -- this is
   the conflicts surface P8-M5-T02 builds on.
3. **Leases time out.** Every lock has a TTL (default 1h, constructor-configurable).
   A crashed or wedged client cannot lock an object forever; after the TTL the
   object is free again. `reap_expired()` removes stale records for hygiene,
   but correctness never depends on it: every operation already treats expired
   leases as free.
4. **Tokens are opaque and server-minted.** Clients never construct a token;
   they are compared by value only. Ownership checks (release / publish) fail
   closed with `TokenMismatch` / `LockingExpired` when the caller is stale.
5. **Steal is the escape hatch.** Administrator / recovery path forcibly
   transfers a lease; the invalidation rule above means the displaced client's
   subsequent writes fail loudly instead of silently clobbering.
6. **Thread safety.** `LockManager` is `Send + Sync`; all state is behind a
   mutex and every operation is a short atomic read-modify-write. A `Clock` is
   injected so unit tests run on a manual clock (deterministic timeouts, no
   sleeps) while production uses the system clock.

## 5. Where things live

    crates/oep-pdm/src/lib.rs                  crate root, module wired
    crates/oep-pdm/src/working_set.rs          LockManager, LockToken, states, tests
    crates/oep-pdm/tests/...                   (unit tests live in the module
                                                per P8-M1-T03; integration later)

The crate is deliberately dependency-free for this milestone (no serde/web), so
the lock core can be reasoned about, fuzzed, and reused by whichever web stack
P8-M1-T01's ADR lands on.

## 6. Acceptance, as verified

* Protocol spec: this document.
* Unit tests for lock states: acquire (free / conflicting / same-holder
  refresh), steal (takeover + token invalidation), release (correct / wrong
  token / not-held), timeout (expiry -> re-acquirable, stale publish denied,
  reaper). See `working_set.rs` `#[cfg(test)] mod tests`.
* Concurrency semantics documented: section 4.
* Concurrent-acquire race test: N threads at one object, exactly one winner.
* `cargo test --workspace`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo fmt --all --check`, and the precommit quality gate: all pass.

## 7. Follow-on seams (not this card)

* Web/REST surface for lock verbs -> P8-M1-T01 / P8-M6-T01.
* Conflict resolution UI + offline working set -> P8-M5-T02 (publish gate is
  the explicit attachment point).
* Release authority / admin steal authorization -> P8-M3-T02 (RBAC), P8-M3-T03
  (audit log records every acquire/steal/release).
