//! P9-M2-T01 acceptance tests for the scheduler (oep-lane).
//!
//! The card acceptance, mapped to real test evidence:
//!
//! 1. **Enqueues by priority** -- `priority_order_*` drives an end-to-end
//!    dispatch loop and asserts high-priority work runs first, with FIFO
//!    tie-break within a priority.
//! 2. **Respects DAG deps** -- a child job is only ever dispatched after its
//!    parent succeeds (`dag_chains`), a multi-parent fan-in waits for every
//!    parent, and a dead-lettered dependency cascades its dependents to the
//!    dead-letter log (`dependency_cascade`).
//! 3. **Retries with backoff** -- `retry_backoff` walks a real dispatch/fail
//!    loop across the exact injected clock and asserts the job is not
//!    re-dispatched before its backoff window, and recovers on the third
//!    attempt.
//! 4. **Dead-letters poison jobs** -- `poison_dead_letters` runs a job whose
//!    retry budget is exhausted through the scheduler and asserts it lands in
//!    the dead-letter log and is never dispatched again.
//! 5. **At-least-once seam (P9-M2-T03)** -- `lease_reclaim_redelivers`
//!    simulates worker loss (no ack) and asserts the scheduler reclaims the
//!    expired lease and redelivers the job with the **same idempotency key**
//!    and a fresh generation.
//!
//! Time model: every scheduler method takes an explicit `now_ms` (injected
//! clock), so the whole suite is deterministic -- no sleeps, no wall clock.

use oep_lane::sched::{Dispatch, JobId, JobStatus, Priority, RetryPolicy, Scheduler};

fn retry3() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 3,
        base_backoff_ms: 100,
        backoff_factor: 2,
        max_backoff_ms: 10_000,
    }
}

/// Drain the scheduler until nothing is ready at `end_ms`, dispatching each
/// job to a callback that either acks or fails it. Returns the ordered
/// dispatch sequence.
fn run(
    s: &mut Scheduler,
    mut now: u64,
    end_ms: u64,
    handle: impl FnMut(&Dispatch) -> bool,
) -> Vec<Dispatch> {
    let mut out = Vec::new();
    let mut h = handle;
    while now <= end_ms {
        match s.dispatch(now) {
            Some(disp) => {
                out.push(disp.clone());
                let job = disp.job;
                let gen = disp.generation;
                if h(&disp) {
                    s.ack(job, gen).unwrap();
                } else {
                    s.fail(job, gen, now).unwrap();
                }
            }
            None => {
                if s.ready_count() == 0 && s.running_count() == 0 {
                    break;
                }
                now += 1;
            }
        }
    }
    out
}

#[test]
fn priority_order_end_to_end() {
    let mut s = Scheduler::new();
    s.enqueue(oep_lane::sched::JobSpec::new("low").with_priority(Priority::new(1)))
        .unwrap();
    s.enqueue(oep_lane::sched::JobSpec::new("high").with_priority(Priority::new(10)))
        .unwrap();
    s.enqueue(oep_lane::sched::JobSpec::new("mid").with_priority(Priority::new(5)))
        .unwrap();

    let order = run(&mut s, 0, 100, |_| true);
    assert_eq!(order[0].payload, "high");
    assert_eq!(order[1].payload, "mid");
    assert_eq!(order[2].payload, "low");
    assert_eq!(order.len(), 3);
}

#[test]
fn fifo_tiebreak_end_to_end() {
    let mut s = Scheduler::new();
    for p in ["a", "b", "c"] {
        s.enqueue(oep_lane::sched::JobSpec::new(p).with_priority(Priority::new(7)))
            .unwrap();
    }
    let order = run(&mut s, 0, 100, |_| true);
    let payloads: Vec<_> = order.iter().map(|d| d.payload.as_str()).collect();
    assert_eq!(payloads, vec!["a", "b", "c"]);
}

#[test]
fn dag_chain_parent_before_child() {
    let mut s = Scheduler::new();
    let p = s.enqueue(oep_lane::sched::JobSpec::new("parent")).unwrap();
    let c = s
        .enqueue(oep_lane::sched::JobSpec::new("child").with_deps(vec![p]))
        .unwrap();
    let g = s
        .enqueue(oep_lane::sched::JobSpec::new("grandchild").with_deps(vec![c]))
        .unwrap();

    let order = run(&mut s, 0, 100, |_| true);
    let ids: Vec<JobId> = order.iter().map(|d| d.job).collect();
    assert_eq!(ids, vec![p, c, g]);
    // Every job terminal-success at the end.
    assert_eq!(s.status(p), Some(JobStatus::Succeeded));
    assert_eq!(s.status(c), Some(JobStatus::Succeeded));
    assert_eq!(s.status(g), Some(JobStatus::Succeeded));
}

#[test]
fn dag_fan_in_waits_for_all_parents() {
    let mut s = Scheduler::new();
    let a = s.enqueue(oep_lane::sched::JobSpec::new("a")).unwrap();
    let b = s.enqueue(oep_lane::sched::JobSpec::new("b")).unwrap();
    let fan = s
        .enqueue(oep_lane::sched::JobSpec::new("fan").with_deps(vec![a, b]))
        .unwrap();

    // Dispatch a; the fan must NOT dispatch until b also succeeds.
    let da = s.dispatch(0).unwrap();
    assert_eq!(da.job, a);
    s.ack(a, da.generation).unwrap();
    assert_eq!(s.pending_count(), 1);
    // b is also a root job, so it is dispatchable -- but the claim is
    // that the fan is not, i.e. the next dispatch is b, not fan.
    let db = s.dispatch(1).unwrap();
    assert_eq!(db.job, b, "fan must wait until b succeeds too");
    s.ack(b, db.generation).unwrap();

    // Only after BOTH parents succeed does the fan release.
    assert_eq!(s.dispatch(2).unwrap().job, fan);
    assert!(s.dispatch(3).is_none());
}

#[test]
fn retry_backoff_recovery_on_injected_clock() {
    let mut s = Scheduler::new();
    let job = s
        .enqueue(oep_lane::sched::JobSpec::new("flaky").with_retry(retry3()))
        .unwrap();

    // Attempt 1: fail at t=0 -> backoff 100ms.
    let d1 = s.dispatch(0).unwrap();
    assert_eq!(d1.job, job);
    s.fail(job, d1.generation, 0).unwrap();
    assert!(s.dispatch(50).is_none(), "not due until t=100");

    // Attempt 2: fail at t=100 -> backoff 200ms.
    let d2 = s.dispatch(100).unwrap();
    assert_eq!(d2.job, job);
    s.fail(job, d2.generation, 100).unwrap();
    assert!(s.dispatch(299).is_none(), "not due until t=300");

    // Attempt 3: succeeds at t=300.
    let d3 = s.dispatch(300).unwrap();
    assert_eq!(d3.job, job);
    s.ack(job, d3.generation).unwrap();

    assert_eq!(s.status(job), Some(JobStatus::Succeeded));
    assert!(s.dispatch(1000).is_none());
    assert!(s.dead_letters().is_empty());
}

#[test]
fn poison_dead_letters_and_never_dispatches_again() {
    let mut s = Scheduler::new();
    let retry = RetryPolicy {
        max_attempts: 2,
        base_backoff_ms: 0,
        backoff_factor: 2,
        max_backoff_ms: 1_000,
    };
    let job = s
        .enqueue(oep_lane::sched::JobSpec::new("poison").with_retry(retry))
        .unwrap();

    let d1 = s.dispatch(0).unwrap();
    s.fail(job, d1.generation, 0).unwrap();
    let d2 = s.dispatch(0).unwrap();
    s.fail(job, d2.generation, 0).unwrap();

    assert_eq!(s.status(job), Some(JobStatus::DeadLettered));
    assert_eq!(s.dead_letters().len(), 1);
    assert!(s.dead_letters()[0].reason.starts_with("retries exhausted"));
    // Budget exhausted: must never dispatch again, even far in the future.
    assert!(s.dispatch(1_000_000).is_none());
}

#[test]
fn dependency_cascade_to_dead_letter() {
    let mut s = Scheduler::new();
    let a = s.enqueue(oep_lane::sched::JobSpec::new("a")).unwrap();
    let b = s
        .enqueue(oep_lane::sched::JobSpec::new("b").with_deps(vec![a]))
        .unwrap();
    let c = s
        .enqueue(oep_lane::sched::JobSpec::new("c").with_deps(vec![b]))
        .unwrap();

    // a fails permanently (poison) -> b and c must never run.
    let da = s.dispatch(0).unwrap();
    s.poison_report(a, da.generation, "bad input", 0).unwrap();

    assert_eq!(s.status(a), Some(JobStatus::DeadLettered));
    assert_eq!(s.status(b), Some(JobStatus::DeadLettered));
    assert_eq!(s.status(c), Some(JobStatus::DeadLettered));
    assert_eq!(s.dead_letters().len(), 3);
    assert_eq!(s.ready_count(), 0);
    assert!(s.dispatch(100).is_none());
}

#[test]
fn lease_reclaim_redelivers_with_same_idempotency_key() {
    let mut s = Scheduler::with_lease(50);
    let job = s
        .enqueue(oep_lane::sched::JobSpec::new("render").with_idempotency("render-1"))
        .unwrap();

    // Worker takes the job and dies without acking.
    let d1 = s.dispatch(0).unwrap();
    assert_eq!(d1.job, job);
    assert_eq!(d1.idempotency_key.as_deref(), Some("render-1"));

    // Lease expires at t=50; at t=60 the scheduler reclaims and redelivers.
    let reclaimed = s.reclaim_expired(60);
    assert_eq!(reclaimed, vec![job]);
    let d2 = s.dispatch(60).unwrap();
    assert_eq!(d2.job, job);
    assert_eq!(
        d2.idempotency_key.as_deref(),
        Some("render-1"),
        "at-least-once: same idempotency key on redelivery"
    );
    assert!(d2.generation > d1.generation);

    // The replacement worker acks; job is terminal and never redelivered.
    s.ack(job, d2.generation).unwrap();
    assert_eq!(s.status(job), Some(JobStatus::Succeeded));
    assert!(s.reclaim_expired(10_000).is_empty());
}

#[test]
fn mixed_wave_priority_and_dep_respected_together() {
    let mut s = Scheduler::new();
    let base = s.enqueue(oep_lane::sched::JobSpec::new("base")).unwrap();
    let important = s
        .enqueue(
            oep_lane::sched::JobSpec::new("important")
                .with_deps(vec![base])
                .with_priority(Priority::new(10)),
        )
        .unwrap();
    let urgent = s
        .enqueue(oep_lane::sched::JobSpec::new("urgent").with_priority(Priority::new(20)))
        .unwrap();

    // urgent (pri 20) runs before important (pri 10), but important only
    // after base succeeds even though base is pri 0.
    let order = run(&mut s, 0, 100, |_| true);
    let ids: Vec<JobId> = order.iter().map(|d| d.job).collect();
    assert_eq!(ids[0], urgent);
    assert_eq!(ids[1], base);
    assert_eq!(ids[2], important);
}
