# P9-M2-T03: Distributed state — node-loss survival, at-least-once idempotent jobs

Status: delivered (wave-2 P9 compute lane). Branch: `task/p9m2t03-distributed-state-1017a`.
Crate: `crates/oep-state`.

## The problem this card solves

The P9 compute lane is a mesh of worker nodes executing jobs that mutate the
P8 vault. A worker can die at any moment — hardware fault, OOM-kill, network
partition, `kill -9` during a deploy. When that happens while a job is
mid-flight, two things must be true:

1. **The job is not lost.** Its state must live outside the worker, durable,
   so the scheduler can see "this job was claimed but never finished" and hand
   it to another node.
2. **Re-running it does not double the damage.** Delivery can therefore only
   be *at-least-once* (a vanished ack is indistinguishable from a lost
   worker); correctness comes from making jobs *idempotent* — a redelivered
   run applies each tracked effect exactly once.

This card (P9-M2-T03) owns that distributed-state contract. The full
scheduler (P9-M2-T01, a sibling card in the same wave) builds priorities,
DAG dependencies, backoff and dead-letter policy on top of these primitives;
this card proves the survival + at-least-once + idempotency core.

## Decided design

**`crates/oep-state`** — a self-contained crate (it does NOT depend on the
unmerged `oep-job` spec crate; a job is opaque bytes here, bound to the
`JobSpec` type when the wave merges). Four modules:

| Module | Responsibility |
|---|---|
| `record` | Durable `JobRecord` (id, queue, idempotency key, opaque payload, attempt, `JobState`), serde-round-trippable. |
| `store` | `JobStore` trait (durable, CAS-guarded transitions) + in-memory `MemoryStore` for the drill / embedded single-node use. |
| `time` | Injectable `Clock` (`SystemClock` in prod, `ManualClock` in tests) so lease expiry is deterministic. |
| `scheduler` | `submit` / `claim` (lease + attempt bump) / `reap` (redeliver or dead-letter) / `complete` (CAS-guarded ack) / `fail`. |
| `effects` | `EffectLedger::apply_once` — the idempotency gate: first caller marks the key durably and applies; every later caller (any worker, any attempt) sees `AlreadyApplied`. |

### The three guarantees (all proven by test, `tests/node_loss_drill.rs`)

- **Node-loss survival.** Records live in the shared `JobStore`, never in the
  worker. `submit` persists before any worker claims. A vanished worker leaves
  a `Running` record with an expiring lease; it is never erased.
- **At-least-once delivery.** `claim` bumps `attempt` on every lease.
  `reap(now)` walks leases; an expired lease returns the job to `Queued`
  (redelivered) while `attempt < max_attempts`, else parks it
  (`DeadLettered`). A claim is a compare-and-set, so two workers can never
  own the same job; `complete` is also CAS-guarded so a stale ack after
  redelivery cannot clobber the new owner.
- **Idempotent effects.** All side effects funnel through `EffectLedger`
  under content-derived keys (`{job}:{effect}` or the output content
  address). The dedup mark is durable and written before applying, matching
  the P8 content-address doctrine: re-writing the same output bytes is a
  no-op, so even a redelivery that re-enters the ledger cannot double-apply.

### Why in-memory store is an honest proof

The acceptance is "proven by test, not claimed." `MemoryStore` satisfies the
exact `JobStore` contract a real cluster store implements — records are
shared, keyed, and CAS-transitioned independently of any worker lifetime —
so the node-loss drill's invariants (survive, redeliver, dedup) hold against
the same code path a JetStream KV backend will use. A durable backend swaps
in behind the same trait; nothing in the scheduler or effects changes.

## The node-loss drill (acceptance test)

`tests/node_loss_drill::node_loss_drill_kills_worker_redelivers_no_double_effect`:

1. Scheduler + shared store + `ManualClock` at t=10000; lease 5s, max 3.
2. Submit idempotent job with effects `[e1, e2]`; queue_len == 1.
3. **Worker A thread** claims (attempt 1), applies `e1` via the ledger
   (`Applied`), signals, then *blocks until killed* — it never calls
   `complete`/`fail`. The test advances past the lease and the thread is
   joined (the worker is dead; no ack ever arrives).
4. Effect log is exactly `["e1"]`.
5. `reap(t+5001)` returns `[job_id]` — the expired lease was redelivered.
6. **Worker B** claims (attempt 2, the at-least-once bump) and runs the full
   `[e1, e2]` set: `e1 -> AlreadyApplied` (dedup), `e2 -> Applied`; then
   `complete`.
7. Asserts: state `Succeeded`, `attempt >= 2`, effect log exactly
   `["e1", "e2"]` — **no double-effect despite the job running twice.**

Plus unit coverage: claim exclusivity across workers, stale-ack rejection,
redelivery-then-dead-letter sequencing, ledger exactly-once on full rerun,
serde durability of `JobRecord`, conservative defaults.

## Contract consumed downstream

- P9-M2-T01 scheduler card takes `Scheduler::submit/claim/reap/complete`
  as the durable substrate for queue/priorities/DAG/backoff/dead-letter.
- P9-M3 worker cards take `EffectLedger::apply_once` as the required path for
  every vault write (output blob, new item, new revision) so redelivery is
  free of double-effect.
- P9-M5-T02 GA gate can assert `attempt >= 1` + idempotency-key coverage on
  the same records.

## Conservative defaults

- Lease 30s (short enough that a dead node is noticed, long enough that a
  busy worker is not spuriously resumed).
- `max_attempts = 5` before a job is parked to the dead-letter.
- No retries/backoff policy here — that is P9-M2-T01's fair-use lane.
