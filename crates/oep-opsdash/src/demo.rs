//! Demo / driver workloads: real work driven through the dashboard.
//!
//! Two modes:
//!
//! - [`run_wave`] -- a deterministic wave used for the proof and the Python
//!   acceptance: enqueues a fixed mix of jobs (two happy jobs, a job that
//!   fails once then succeeds, a poison job that dead-letters, and a job
//!   whose lease is allowed to expire so a node shows `degraded`), then runs
//!   the full dispatch/ack cycle against a real scheduler on an injected
//!   clock.
//! - [`spawn_live_driver`] -- a background thread that continuously enqueues
//!   and drains jobs on the wall clock, so the served dashboard visibly
//!   moves while someone watches it.
//!
//! Everything is real: the wave only calls the dashboard's real ops API
//! (enqueue/dispatch/ack/fail/poison/reap). What a "worker" does between
//! claim and ack is simulated by name only because there is no render herd
//! wired to a live bus yet; the lifecycle, counters, logs, node health and
//! energy are the real scheduler's.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use oep_lane::sched::{JobId, JobSpec, Priority, RetryPolicy};

use crate::clock::ManualClock;
use crate::dashboard::Dashboard;
use crate::http::SharedDashboard;
use crate::registry::NodeSpec;

/// The node fleet the demo registers (U1 + NAS, honest declared draws).
pub fn fleet() -> Vec<NodeSpec> {
    vec![
        NodeSpec {
            id: "u1".to_string(),
            cpu_millicores: 32_000,
            memory_mib: 128_000,
            heartbeat_stale_ms: 10_000,
            power_w: 350,
            cents_per_kwh: 15, // $0.15/kWh
        },
        NodeSpec {
            id: "nas-ds".to_string(),
            cpu_millicores: 8_000,
            memory_mib: 32_000,
            heartbeat_stale_ms: 10_000,
            power_w: 60,
            cents_per_kwh: 15,
        },
    ]
}

/// Stats a wave reported back, for exact acceptance assertions.
#[derive(Debug, Default, Clone)]
pub struct WaveStats {
    /// The acked (succeeded) job ids.
    pub succeeded: Vec<JobId>,
    /// The poison (dead-lettered) job id.
    pub poison: Option<JobId>,
    /// The lease-expired (reclaimed) job id.
    pub lost: Option<JobId>,
}

/// Deterministic proof wave over `dash` (manual clock, short lease).
///
/// Wave (in order):
/// 1. happy render job on u1         -> succeeded
/// 2. happy convert job on nas       -> succeeded
/// 3. job on u1 fails once, retries  -> succeeded (after clock advance)
/// 4. poison job on u1               -> dead-lettered
/// 5. job on u1 whose lease is never acked -> reclaimed at clock+lease;
///    u1 is marked `degraded` for that job
pub fn run_wave(dash: &mut Dashboard, clock: &ManualClock) -> WaveStats {
    let mut stats = WaveStats::default();

    // 1. Happy render on u1.
    let happy1 = dash
        .enqueue(JobSpec::new("render:example-bracket").with_priority(Priority::new(10)))
        .unwrap();
    let d1 = dash.dispatch_to("u1").unwrap();
    clock.set(dash.clock().now_ms() + 500); // real work: 500ms of elapsed clock
    dash.ack(happy1, d1.generation).unwrap();
    stats.succeeded.push(happy1);

    // 2. Happy convert on nas.
    let happy2 = dash
        .enqueue(JobSpec::new("convert:kf2.catpart").with_priority(Priority::new(8)))
        .unwrap();
    let d2 = dash.dispatch_to("nas-ds").unwrap();
    clock.set(dash.clock().now_ms() + 250); // real work: 250ms
    dash.ack(happy2, d2.generation).unwrap();
    stats.succeeded.push(happy2);

    // 3. Retry job: fail once (transient), then succeed on the retry.
    let retry = RetryPolicy {
        max_attempts: 3,
        base_backoff_ms: 100,
        backoff_factor: 2,
        max_backoff_ms: 5_000,
    };
    let j3 = dash
        .enqueue(
            JobSpec::new("convert:heavy")
                .with_retry(retry)
                .with_priority(Priority::new(6)),
        )
        .unwrap();
    let d3 = dash.dispatch_to("u1").unwrap();
    dash.fail(j3, d3.generation, "transient: bus timeout")
        .unwrap();
    // The job is Retrying until now+100; advance the clock past the window.
    clock.set(dash.clock().now_ms() + 101);
    let d3b = dash.dispatch_to("u1").unwrap();
    dash.ack(j3, d3b.generation).unwrap();
    stats.succeeded.push(j3);

    // 4. Poison job: dead-letters immediately (terminal, no retry consumed).
    let j4 = dash
        .enqueue(JobSpec::new("render:corrupt-model").with_priority(Priority::new(5)))
        .unwrap();
    let d4 = dash.dispatch_to("u1").unwrap();
    dash.poison(j4, d4.generation, "worker: unreadable model")
        .unwrap();
    stats.poison = Some(j4);

    // 5. Lease-expired job: dispatch, never ack, advance past the lease, reap.
    let j5 = dash
        .enqueue(JobSpec::new("convert:lost-worker").with_idempotency("slow-1"))
        .unwrap();
    let d5 = dash.dispatch_to("u1").unwrap();
    debug_assert_eq!(d5.idempotency_key.as_deref(), Some("slow-1"));
    // Lease is 50ms (the dashboard was built with with_lease(50)); jump past it.
    clock.set(dash.clock().now_ms() + 60);
    let reclaimed = dash.reap();
    assert!(
        reclaimed.contains(&j5),
        "lease-expired job must be reclaimed"
    );
    stats.lost = Some(j5);

    stats
}

/// Spawn a background live driver: every `every`, enqueue a burst of jobs,
/// drain them round-robin across the fleet, reap expired leases, and sleep.
/// Runs until `stop` is set true. Returns the stop flag and a live counter of
/// jobs acked.
pub fn spawn_live_driver(
    dash: SharedDashboard,
    every: Duration,
) -> (Arc<AtomicBool>, Arc<AtomicU64>) {
    let stop = Arc::new(AtomicBool::new(false));
    let acked = Arc::new(AtomicU64::new(0));
    {
        let dash = Arc::clone(&dash);
        let stop = Arc::clone(&stop);
        let acked = Arc::clone(&acked);
        thread::spawn(move || {
            let mut seq = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let burst = 2 + (seq % 3) as usize;
                let mut acks = Vec::new();
                {
                    let mut d = dash.lock().unwrap();
                    for i in 0..burst {
                        let id = d
                            .enqueue(JobSpec::new(format!("render:burst-{seq}-{i}")))
                            .unwrap();
                        let node = if i % 2 == 0 { "u1" } else { "nas-ds" };
                        if let Some(disp) = d.dispatch_to(node) {
                            acks.push((disp.job, disp.generation));
                        } else {
                            let _ = id;
                        }
                    }
                    d.reap();
                }
                // Real work: release the lock and wait a real 30ms (well under
                // the serve lease) so the SystemClock-observed runtime and
                // thus the energy are genuinely measured, never fabricated.
                thread::sleep(Duration::from_millis(30));
                {
                    let mut d = dash.lock().unwrap();
                    for (job, gen) in &acks {
                        d.ack(*job, *gen).unwrap_or_default();
                    }
                    d.reap();
                }
                acked.fetch_add(acks.len() as u64, Ordering::Relaxed);
                seq += 1;
                thread::sleep(every);
            }
        });
    }
    (stop, acked)
}
