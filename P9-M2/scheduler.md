# P9-M2-T01 -- Scheduler: queue, priorities, DAG deps, retries, dead-letter

Status: implemented (pure, std-only). Owner: P9 compute lane. Version: 1 (2026-10-17).

## 1. Purpose

The P9 compute lane needs a scheduler that owns *when and in what order*
jobs run across the worker fleet. This card delivers the scheduler core as
`crates/oep-lane` (self-contained, std-only, deterministic). It integrates
with the rest of the P9 wave as follows:

- **P9-M1-T01 oep-job** defines the job *spec* (vault refs, executor,
  resource caps, output refs). The scheduler treats the payload as an opaque
  string today and consumes the typed spec once cross-crate deps open at the
  P6 importer hub (P6-M4-T02). The contract note is recorded in
  `JobSpec::payload`.
- **P9-M1-T02** owns the message bus (NATS) + topic taxonomy; this crate is
  bus-agnostic -- dispatch is returned synchronously to the caller, who may
  post it on the bus. No bus dependency is introduced here.
- **P9-M2-T03** builds *distributed state / node-loss survival* on top of
  the **lease/ack contract** this card establishes (section 4).

## 2. What the scheduler guarantees

Pure, unit-testable invariants (all exercised by
`crates/oep-lane/tests/scheduler_acceptance.rs` with an injected clock):

1. **Priority scheduling**: jobs dispatch highest-priority-first; within a
   priority, strictly FIFO by enqueue sequence. `Priority` is a `u8` hint
   (quotas/fair-share are P9-M2-T02).
2. **DAG dependencies**: a job is runnable only when every dependency has
   *succeeded*. Enqueue rejects unknown dependencies; the dependency graph
   (`DepGraph`) rejects cycles structurally. Direct and transitive
   dependents of a dead-lettered job are cascaded into the dead-letter log
   (work must not run on a failed input).
3. **Retries with backoff**: a failed job reschedules on exponential backoff
   (`base * factor^failures`, capped) up to a per-job attempt budget.
4. **Dead-letter**: jobs that exhaust the budget, are reported poison, or
   sit behind a dead-lettered dependency land in the dead-letter log and are
   never dispatched again (visible for the ops dashboard, P9-M5-T01).

## 3. State model

    JobStatus = Waiting | Ready | Running | Retrying | Succeeded | DeadLettered

    enqueue(spec) -> JobId            (deps must exist; ids are monotonic)
    dispatch(now_ms) -> Option<Dispatch>   (marks Running, issues lease+generation)
    ack(job, generation)              (Succeeded; releases dependents)
    fail(job, generation, now_ms)     (Retrying+backoff, or DeadLettered+cascade)
    poison_report(job, generation, now, detail)  (terminal immediately + cascade)
    dead_letter_manually(job, now, detail)
    reclaim_expired(now_ms) -> Vec<JobId>   (Running lease expired -> Ready)

Internal hearings:

- `ready` heap: `(priority desc, seq asc)` via `BinaryHeap`.
- `delayed` heap: retrying jobs ordered by `ready_at_ms`, promoted back into
  `ready` when the injected clock passes their window.
- Every dispatch bumps a per-job `generation`; acks/fails validate it so a
  stale lease (a worker from a previous redelivery) cannot resolve the job
  twice.

## 4. The at-least-once seam (for P9-M2-T03)

- `dispatch` hands out a *lease* (time-bounded ownership) plus the job's
  **idempotency key**.
- If a worker dies without acking, `reclaim_expired` moves the job back to
  Ready and it is **redelivered with the same idempotency key** and a fresh
  generation -- at-least-once delivery of work.
- Because delivery is at-least-once, P9-M2-T03's idempotency contract
  (jobs dedupe by key) is what makes *effect* exactly-once. This card
  unit-tests delivery; T03 runs the kill-a-worker drill on top.

Why lease/ack instead of fire-and-forget emit: the lane's workers can fail
mid-job; at-least-once recovery requires the scheduler to *own* the
Running state and the redelivery decision rather than only push onto a bus.

## 5. Where things live

    crates/oep-lane/src/sched/mod.rs        module root + guarantees doc
    crates/oep-lane/src/sched/priority.rs   Priority (u8 hint)
    crates/oep-lane/src/sched/retry.rs      RetryPolicy + exponential backoff
    crates/oep-lane/src/sched/dag.rs        DepGraph (deps, dependents, cycles)
    crates/oep-lane/src/sched/deadletter.rs DeadLetter record + reasons
    crates/oep-lane/src/sched/scheduler.rs  Scheduler core (queue/dispatch/ack/reclaim)
    crates/oep-lane/tests/scheduler_acceptance.rs  the card acceptance, on an
                                         injected clock, against real code
    docs/layout.md / tests/monorepo_layout_test.py   P9 lane registered

## 6. Verification

- 27 unit tests + 9 integration acceptance tests, all on an injected clock
  (deterministic, no sleeps).
- Acceptance mapped directly:
  - priority order + FIFO tie-break  -> `scheduler_acceptance.rs`
  - DAG parent-before-child + fan-in -> `dag_chain_*`, `dag_fan_in_*`
  - retries with backoff (t=50/100/299/300 on the regression) -> `retry_backoff_*`
  - dead-letter poison + cascade     -> `poison_*`, `dependency_cascade_*`
  - at-least-once redelivery seam    -> `lease_reclaim_*`
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
