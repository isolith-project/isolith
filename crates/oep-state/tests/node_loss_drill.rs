//! P9-M2-T03 acceptance: distributed state survives node loss; delivery is
//! at-least-once; jobs are idempotent so redelivery never double-applies.
//!
//! TDD RED: this file documents the acceptance BEFORE the implementation
//! exists. Nothing here is claimed — the node-loss drill *kills* a worker
//! mid-job (its thread halts without acking) and asserts the scheduler
//! redelivers and the effect stays applied exactly once.
//!
//! Drill scenario:
//!   1. Submit an idempotent job whose effect set is [e1, e2].
//!   2. Worker A claims it, applies e1, is KILLED mid-job (no ack).
//!   3. Lease expires -> scheduler reaps -> job redelivered (attempt 2).
//!   4. Worker B claims the redelivery, runs the FULL effect set through the
//!      effect ledger, then acks completion.
//!   5. Assert: job Succeeded, attempt >= 2, and the global effect log holds
//!      exactly [e1, e2] — the ledger deduped e1, and e2 was applied once.

use oep_state::effects::{EffectLedger, EffectOutcome};
use oep_state::record::JobState;
use oep_state::scheduler::{SchedConfig, Scheduler};
use oep_state::store::in_memory;
use oep_state::time::{Clock, ManualClock};
use std::sync::{Arc, Mutex};
use std::thread;

/// Effect keys scoped under a job id: re-writing the same output is a no-op.
fn effect_key(job_id: &str, e: &str) -> String {
    format!("{}:{}", job_id, e)
}

#[test]
fn node_loss_drill_kills_worker_redelivers_no_double_effect() {
    // --- 0. Framework: deterministic clock + shared durable store. ---
    let store = in_memory();
    let clock = Arc::new(ManualClock::new(10_000));
    let sched = Arc::new(Scheduler::with_clock(
        store.clone(),
        SchedConfig {
            lease_ms: 5_000,
            max_attempts: 3,
        },
        clock.clone(),
    ));
    let ledger = EffectLedger::new(store.clone());

    // --- 1. Submit an idempotent job. ---
    let job_id = sched
        .submit("render", b"{}".to_vec(), Some("drill-job".into()))
        .unwrap();
    assert_eq!(sched.queue_len("render"), 1);

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // --- 2. Worker A claims, applies e1, then is killed before e2/ack. ---
    let (e1_applied_tx, e1_applied_rx) = std::sync::mpsc::channel::<()>();
    let (kill_tx, kill_rx) = std::sync::mpsc::channel::<()>();

    let sched_a = sched.clone();
    let ledger_a = ledger.clone();
    let log_a = log.clone();
    let job_id_a = job_id.clone();
    let worker_a = thread::spawn(move || {
        let claimed = sched_a
            .claim("worker-a", "render")
            .expect("claim")
            .expect("job must be claimable");
        assert_eq!(claimed.record.id, job_id_a);
        assert_eq!(claimed.record.attempt, 1);

        let key = effect_key(&job_id_a, "e1");
        let out = ledger_a.apply_once(&key, &mut log_a.lock().unwrap(), |log| {
            log.push("e1".to_string())
        });
        assert_eq!(out, EffectOutcome::Applied);

        // Signal that e1 landed, then block until the test kills this worker
        // mid-job. We never call complete/fail — the worker is gone.
        let _ = e1_applied_tx.send(());
        let _ = kill_rx.recv();
        // ...and we vanish without acking (this closure returns, no ack).
    });

    let _ = e1_applied_rx.recv();
    assert_eq!(log.lock().unwrap().as_slice(), &["e1".to_string()]);
    let _ = kill_tx.send(()); // KILL worker A
    worker_a.join().unwrap();

    // --- 3. Lease expires -> scheduler redelivers. ---
    clock.advance(5_001);
    let redelivered = sched.reap(clock.now()).unwrap();
    assert_eq!(
        redelivered,
        vec![job_id.clone()],
        "expired lease redelivered"
    );

    // --- 4. Worker B claims the redelivery and runs the FULL effect set. ---
    let claimed_b = sched
        .claim("worker-b", "render")
        .expect("claim")
        .expect("redelivered job must be claimable");
    assert_eq!(claimed_b.record.attempt, 2, "at-least-once: attempt bumped");

    for e in ["e1", "e2"] {
        let key = effect_key(&job_id, e);
        let out = ledger.apply_once(&key, &mut log.lock().unwrap(), |log| {
            log.push(e.to_string())
        });
        // e1 was already applied by the dead worker: ledger must dedup it.
        match e {
            "e1" => assert_eq!(out, EffectOutcome::AlreadyApplied, "e1 must not re-apply"),
            _ => assert_eq!(out, EffectOutcome::Applied, "{} applied once", e),
        }
    }
    sched.complete("worker-b", &job_id).unwrap();

    // --- 5. Approval assertions. ---
    let rec = sched.get(&job_id).unwrap().expect("job exists");
    assert_eq!(rec.state, JobState::Succeeded);
    assert!(
        rec.attempt >= 2,
        "retried at least once (attempt {})",
        rec.attempt
    );
    let effects_log: Vec<String> = log.lock().unwrap().clone();
    assert_eq!(
        effects_log,
        vec!["e1".to_string(), "e2".to_string()],
        "no double-effect: each effect applied exactly once despite redelivery"
    );
}

#[test]
fn worker_loss_mid_run_never_loses_the_job() {
    let store = in_memory();
    let clock = Arc::new(ManualClock::new(0));
    let sched = Arc::new(Scheduler::with_clock(
        store,
        SchedConfig {
            lease_ms: 1_000,
            max_attempts: 5,
        },
        clock.clone(),
    ));
    let job_id = sched.submit("convert", b"x".to_vec(), None).unwrap();

    // Claim, then the worker's process is lost: no ack, no fail.
    let _claimed = sched.claim("worker-a", "convert").unwrap().unwrap();
    assert!(sched.get(&job_id).unwrap().is_some(), "job never lost");

    // Advance past lease; scheduler must redeliver to ANY worker.
    clock.advance(2_000);
    assert_eq!(sched.reap(clock.now()).unwrap(), vec![job_id.clone()]);
    let claimed2 = sched.claim("worker-b", "convert").unwrap().unwrap();
    assert_eq!(claimed2.record.attempt, 2);
}

#[test]
fn effect_ledger_applies_each_key_once_even_on_full_rerun() {
    let store = in_memory();
    let ledger = EffectLedger::new(store);
    let mut applied: Vec<String> = Vec::new();

    let a1 = ledger.apply_once("job:x:1", &mut applied, |applied| applied.push("x".into()));
    assert_eq!(a1, EffectOutcome::Applied);
    assert_eq!(applied, vec!["x".to_string()]);

    // Redelivered full re-run: same key MUST NOT apply again.
    let a2 = ledger.apply_once("job:x:1", &mut applied, |applied| applied.push("x".into()));
    assert_eq!(a2, EffectOutcome::AlreadyApplied);
    assert_eq!(applied, vec!["x".to_string()], "no double effect on rerun");
}

#[test]
fn job_record_is_serializable_for_durable_state() {
    let rec = oep_state::record::JobRecord::new("id-1", "render", Some("k".into()), 0);
    let json = serde_json::to_string(&rec).expect("serialize");
    let back: oep_state::record::JobRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, rec);
    assert_eq!(back.state, JobState::Queued);
}

#[test]
fn store_errors_are_displayable() {
    let e = oep_state::record::StoreError::NotFound(String::from("x"));
    assert!(!e.to_string().is_empty());
}

#[test]
fn scheduler_rejects_bad_claim_after_completion() {
    let store = in_memory();
    let clock = Arc::new(ManualClock::new(0));
    let sched = Arc::new(Scheduler::with_clock(
        store,
        SchedConfig {
            lease_ms: 1_000,
            max_attempts: 3,
        },
        clock.clone(),
    ));
    let job_id = sched.submit("render", b"y".to_vec(), None).unwrap();
    let c = sched.claim("worker-a", "render").unwrap().unwrap();
    assert_eq!(c.record.id, job_id);
    sched.complete("worker-a", &job_id).unwrap();
    // Once Succeeded, no one may claim it again.
    assert!(sched.claim("worker-b", "render").unwrap().is_none());
    let rec = sched.get(&job_id).unwrap().unwrap();
    assert_eq!(rec.state, JobState::Succeeded);
}

#[test]
fn defaults_are_conservative() {
    let cfg = SchedConfig::default();
    assert!(cfg.lease_ms >= 5_000, "conservative lease");
    assert!(cfg.max_attempts <= 10, "conservative retry bound");
    assert!(cfg.max_attempts >= 3);
}
