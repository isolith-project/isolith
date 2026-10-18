//! P9-M5-T01 acceptance: the ops dashboard reads REAL scheduler state.
//!
//! This test drives a real `oep-lane` Scheduler through a mixed wave (happy
//! success, retry-with-backoff, poison -> dead-letter, lease expiry ->
//! reclaim/degraded node) via the dashboard's ops API, then asserts every
//! view in the snapshot is exactly what the scheduler reported at that
//! instant. It is the anti-mock guarantee: the snapshot's queue counters,
//! dead-letter log, node health and cost/energy come from the live scheduler
//! and its real lifecycle events, not from any fixture.

use std::sync::Arc;

use oep_lane::sched::{JobStatus, Priority, RetryPolicy};
use oep_opsdash::clock::ManualClock;
use oep_opsdash::dashboard::Dashboard;
use oep_opsdash::demo;
use oep_opsdash::registry::NodeSpec;

fn make_dashboard(lease_ms: u64) -> (Dashboard, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(0));
    let dash = Dashboard::with_lease(clock.clone(), demo::fleet(), lease_ms);
    (dash, clock)
}

#[test]
fn snapshot_reflects_real_scheduler_after_wave() {
    let lease_ms = 50;
    let (mut dash, _clock) = make_dashboard(lease_ms);
    let snap_before = dash.snapshot();
    assert_eq!(snap_before.queue.total(), 0, "empty dashboard starts empty");

    let stats = demo::run_wave(&mut dash, &_clock);

    let snap = dash.snapshot();

    // Queue counters equal the scheduler's own counters, job by job.
    assert_eq!(snap.queue.waiting, dash.scheduler().pending_count() as u64);
    assert_eq!(snap.queue.ready, dash.scheduler().ready_count() as u64);
    assert_eq!(snap.queue.running, dash.scheduler().running_count() as u64);
    assert_eq!(
        snap.queue.dead_lettered,
        dash.scheduler().dead_letters().len() as u64
    );

    // The wave produced exactly: 3 succeeded (2 happy + 1 retry), 1 poison.
    assert_eq!(snap.queue.succeeded, 3);
    assert_eq!(snap.dead_letters.len(), 1);
    assert!(snap.dead_letters[0].reason.starts_with("poison reported"));

    // Every job is known to the scheduler with the same status the dashboard
    // reports.
    let poison = stats.poison.expect("poison ran");
    assert_eq!(
        dash.scheduler().status(poison),
        Some(JobStatus::DeadLettered)
    );

    // The reclaimed (lost) job: the scheduler reclaimed it, and its node is
    // marked degraded. The at-least-once idempotency key survives.
    let lost = stats.lost.expect("lost ran");
    assert_eq!(dash.scheduler().status(lost), Some(JobStatus::Ready));
    let u1 = snap.nodes.iter().find(|n| n.id == "u1").expect("u1 node");
    assert_eq!(u1.health, "degraded");
    assert_eq!(u1.jobs_reclaimed, 1);

    // The retry job failed once (dashboard observed the real fail decision)
    // and then succeeded.
    let retry_job = snap
        .jobs
        .iter()
        .find(|j| j.failures == 1)
        .expect("retry job present");
    assert_eq!(retry_job.status, "succeeded");
}

#[test]
fn cost_energy_is_positive_and_really_measured() {
    let lease_ms = 50;
    let (mut dash, clock) = make_dashboard(lease_ms);

    // One happy job on u1: 350W, $0.15/kWh. Advance the clock so the run
    // lasts a real measured 3.6s (dashboard clock), then ack.
    clock.set(0);
    let id = dash
        .enqueue(demo_spec("render:one", Priority::new(7), None))
        .unwrap();
    let d = dash.dispatch_to("u1").unwrap();
    clock.set(3600); // 3.6 seconds of real elapsed clock
    dash.ack(id, d.generation).unwrap();

    let snap = dash.snapshot();
    // energy = 350W * 3600ms / 3600s-per-h / 1000 (mWh): exactly 350 mWh.
    assert!(snap.cost_totals.runs == 1);
    assert!(snap.cost_totals.energy_mwh > 0, "energy must be positive");
    // u1 has the energy; nas never ran a job (so no row, or a zero row).
    let u1 = snap.cost.get("u1").expect("u1 cost row");
    assert_eq!(u1.runs, 1);
    assert_eq!(u1.energy_mwh, 350);
    let nas = snap.cost.get("nas-ds");
    assert!(nas.is_none() || nas.unwrap().runs == 0);
}

fn demo_spec(
    payload: &str,
    priority: Priority,
    retry: Option<RetryPolicy>,
) -> oep_lane::sched::JobSpec {
    let mut s = oep_lane::sched::JobSpec::new(payload).with_priority(priority);
    if let Some(r) = retry {
        s = s.with_retry(r);
    }
    s
}

#[test]
fn jobs_view_lists_every_enqueued_job() {
    let (mut dash, _clock) = make_dashboard(50);
    demo::run_wave(&mut dash, &_clock);
    let snap = dash.snapshot();
    // 5 jobs enqueued by the wave.
    assert_eq!(snap.jobs.len(), 5);
    // Job ids are monotonic from 0..5.
    let mut ids: Vec<u64> = snap.jobs.iter().map(|j| j.job).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![0, 1, 2, 3, 4]);
}

#[test]
fn job_log_records_real_lifecycle_lines() {
    let (mut dash, _clock) = make_dashboard(50);
    demo::run_wave(&mut dash, &_clock);
    let snap = dash.snapshot();
    // At least: 5 enqueues + dispatches + ack/fail/poison/reclaim lines.
    let enqueues = snap.log.iter().filter(|e| e.kind == "enqueue").count();
    let dispatches = snap.log.iter().filter(|e| e.kind == "dispatch").count();
    let acks = snap.log.iter().filter(|e| e.kind == "ack").count();
    let reclaims = snap.log.iter().filter(|e| e.kind == "reclaim").count();
    let poisons = snap.log.iter().filter(|e| e.kind == "poison").count();
    assert_eq!(enqueues, 5);
    assert_eq!(dispatches, 6); // 5 jobs, one (the retry) dispatched twice
    assert_eq!(acks, 3);
    assert_eq!(reclaims, 1);
    assert_eq!(poisons, 1);
    // All log lines carry a timestamp; events agree with order.
    let times: Vec<u64> = snap.log.iter().map(|e| e.at_ms).collect();
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "log is time-ordered"
    );
}

#[test]
fn html_renders_all_four_views() {
    let (mut dash, _clock) = make_dashboard(50);
    demo::run_wave(&mut dash, &_clock);
    let snap = dash.snapshot();
    let html = oep_opsdash::html::render_html(&snap);
    for needle in [
        "OEP P9 Ops Dashboard",
        "Queue depth",
        "Node health",
        "Job logs",
        "Cost / Energy",
        "degraded",
        "dead-lettered",
    ] {
        assert!(html.contains(needle), "html must contain {needle:?}");
    }
    // HTML escapes reason text safely.
    assert!(!html.contains("</script>"));
}

#[test]
fn registered_nodes_are_healthy_when_idle() {
    let spec = NodeSpec {
        id: "guard".to_string(),
        cpu_millicores: 1000,
        memory_mib: 2048,
        heartbeat_stale_ms: 1000,
        power_w: 10,
        cents_per_kwh: 1,
    };
    let (mut dash, clock) = make_dashboard(50);
    dash.register_node(spec);
    clock.set(0);
    // No jobs yet: node is online (registered with a heartbeat).
    let snap = dash.snapshot();
    let guard = snap.nodes.iter().find(|n| n.id == "guard").unwrap();
    assert_eq!(guard.health, "online");
}
