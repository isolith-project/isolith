//! Scheduler: lease-based at-least-once dispatch with redelivery on node loss.
//!
//! The scheduler owns the distributed-state guarantees:
//!
//! - [`Scheduler::submit`] persists a job to the shared store (durable
//!   before any worker sees it — node-loss survival starts here).
//! - [`Scheduler::claim`] hands a queued job to exactly one worker under a
//!   lease, bumping `attempt` (at-least-once delivery count). The
//!   transition is a compare-and-set in the store, so two workers can never
//!   claim the same job.
//! - [`Scheduler::reap`] scans for `Running` leases that expired — their
//!   worker is gone. It redelivers (returns to `Queued`) while `attempt <
//!   max_attempts`, otherwise parks the job (`DeadLettered`).
//! - [`Scheduler::complete`] moves a job to `Succeeded` only when the worker
//!   still owns the lease (CAS) — a late ack from a dead-then-redelivered
//!   claim cannot clobber the new owner.
//!
//! This is P9-M2-T03's contract; P9-M2-T01 (the full scheduler card, a
//! sibling in the same wave) adds priorities/DAG/backoff/dead-letter policy
//! on top of these primitives.

use crate::record::{JobRecord, JobState, StoreError};
use crate::store::JobStore;
use crate::time::{Clock, SystemClock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Scheduler configuration. Defaults are deliberately conservative.
#[derive(Debug, Clone)]
pub struct SchedConfig {
    /// Lease length in milliseconds; a claim older than this is considered
    /// lost and is redelivered.
    pub lease_ms: u64,
    /// Maximum delivery attempts before a job is parked (dead-letter).
    pub max_attempts: u32,
}

impl Default for SchedConfig {
    fn default() -> Self {
        SchedConfig {
            lease_ms: 30_000,
            max_attempts: 5,
        }
    }
}

/// The result of a successful claim.
#[derive(Debug, Clone)]
pub struct ClaimedJob {
    /// The job as now recorded (state `Running`, attempt bumped).
    pub record: JobRecord,
}

/// The scheduler: durable dispatch state + lease/redelivery logic.
pub struct Scheduler {
    store: Arc<dyn JobStore>,
    clock: Arc<dyn Clock>,
    config: SchedConfig,
    next_id: AtomicU64,
}

impl Scheduler {
    /// A scheduler over `store` with `config`, using the wall clock.
    pub fn new(store: Arc<dyn JobStore>, config: SchedConfig) -> Self {
        Scheduler::with_clock(store, config, Arc::new(SystemClock))
    }

    /// A scheduler over `store` driven by an explicit clock (tests).
    pub fn with_clock(
        store: Arc<dyn JobStore>,
        config: SchedConfig,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Scheduler {
            store,
            clock,
            config,
            next_id: AtomicU64::new(1),
        }
    }

    /// Persist a new queued job and return its assigned id.
    pub fn submit(
        &self,
        queue: impl Into<String>,
        payload: Vec<u8>,
        idempotency_key: Option<String>,
    ) -> Result<String, StoreError> {
        let id = format!("job-{:05}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let mut record = JobRecord::new(&id, queue, idempotency_key, self.clock.now());
        record.payload = payload;
        self.store.put(&record)?;
        Ok(id)
    }

    /// Number of runnable (queued) jobs in `queue`.
    pub fn queue_len(&self, queue: &str) -> usize {
        self.store
            .all()
            .map(|recs| {
                recs.iter()
                    .filter(|r| r.queue == queue && r.state.is_runnable())
                    .count()
            })
            .unwrap_or(0)
    }

    /// Claim a queued job in `queue` for `worker` under a lease.
    ///
    /// Returns `None` when no job is runnable. Exactly one concurrent caller
    /// wins any given job because the state transition is a CAS in the store.
    pub fn claim(&self, worker: &str, queue: &str) -> Result<Option<ClaimedJob>, StoreError> {
        let lease_until_ms = self.clock.now().saturating_add(self.config.lease_ms);
        let running = JobState::Running {
            worker: worker.to_string(),
            lease_until_ms,
        };
        for rec in self.store.all()? {
            if rec.queue != queue || !rec.state.is_runnable() {
                continue;
            }
            if self
                .store
                .transition(&rec.id, &rec.state, running.clone(), true)?
            {
                let updated = self
                    .store
                    .get(&rec.id)?
                    .expect("job that was just transitioned exists");
                // Guard: the CAS says we own it; reject a state we can't honor.
                if !matches!(updated.state, JobState::Running { .. }) {
                    return Err(StoreError::Conflict(format!(
                        "claim {} landed in {:?}",
                        updated.id, updated.state
                    )));
                }
                return Ok(Some(ClaimedJob { record: updated }));
            }
        }
        Ok(None)
    }

    /// Reap expired leases. For every `Running` job whose lease has passed
    /// `now`, redeliver it (back to `Queued`) if attempts remain, otherwise
    /// park it (`DeadLettered`). Returns the ids that were redelivered.
    pub fn reap(&self, now: u64) -> Result<Vec<String>, StoreError> {
        let mut redelivered = Vec::new();
        let recs = self.store.all()?;
        for rec in recs {
            let lease_until_ms = match rec.state {
                JobState::Running { lease_until_ms, .. } => lease_until_ms,
                _ => continue,
            };
            if lease_until_ms >= now {
                continue; // lease still live
            }
            let expired = JobState::Running {
                worker: match &rec.state {
                    JobState::Running { worker, .. } => worker.clone(),
                    _ => unreachable!(),
                },
                lease_until_ms,
            };
            if rec.attempt < self.config.max_attempts {
                if self
                    .store
                    .transition(&rec.id, &expired, JobState::Queued, false)?
                {
                    redelivered.push(rec.id.clone());
                }
            } else if self.store.transition(
                &rec.id,
                &expired,
                JobState::DeadLettered {
                    reason: format!("attempts exhausted ({})", rec.attempt),
                },
                false,
            )? {
                // parked; not returned as redelivered.
            }
        }
        Ok(redelivered)
    }

    /// Acknowledge success for a job, but only from the worker that owns the
    /// current claim (CAS on `Running{worker}`). A stale ack after
    /// redelivery to another worker is rejected.
    pub fn complete(&self, worker: &str, id: &str) -> Result<(), StoreError> {
        let rec = self
            .store
            .get(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let expected = match &rec.state {
            JobState::Running {
                worker: w,
                lease_until_ms,
            } if w == worker => JobState::Running {
                worker: worker.to_string(),
                lease_until_ms: *lease_until_ms,
            },
            other => {
                return Err(StoreError::Conflict(format!(
                    "cannot complete {} from {} while {:?}",
                    id, worker, other
                )))
            }
        };
        self.store
            .transition(id, &expected, JobState::Succeeded, false)?;
        Ok(())
    }

    /// Mark a job explicitly failed by its owning worker (lease preserved).
    pub fn fail(&self, worker: &str, id: &str, reason: &str) -> Result<(), StoreError> {
        let rec = self
            .store
            .get(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let expected = match &rec.state {
            JobState::Running {
                worker: w,
                lease_until_ms,
            } if w == worker => JobState::Running {
                worker: worker.to_string(),
                lease_until_ms: *lease_until_ms,
            },
            other => {
                return Err(StoreError::Conflict(format!(
                    "cannot fail {} from {} while {:?}",
                    id, worker, other
                )))
            }
        };
        self.store.transition(
            id,
            &expected,
            JobState::Failed {
                reason: reason.to_string(),
            },
            false,
        )?;
        Ok(())
    }

    /// Read a job's current record.
    pub fn get(&self, id: &str) -> Result<Option<JobRecord>, StoreError> {
        self.store.get(id)
    }

    /// The scheduler configuration.
    pub fn config(&self) -> &SchedConfig {
        &self.config
    }

    /// The shared store (read access for inspection/tests).
    pub fn store(&self) -> &Arc<dyn JobStore> {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::in_memory;
    use crate::time::ManualClock;

    fn clocked(lease_ms: u64, max_attempts: u32, now: u64) -> (Scheduler, Arc<ManualClock>) {
        let store = in_memory();
        let clock = Arc::new(ManualClock::new(now));
        let sched = Scheduler::with_clock(
            store,
            SchedConfig {
                lease_ms,
                max_attempts,
            },
            clock.clone(),
        );
        (sched, clock)
    }

    #[test]
    fn claim_is_exclusive_across_workers() {
        let (sched, _) = clocked(1_000, 3, 0);
        let id = sched.submit("q", vec![], None).unwrap();
        let a = sched.claim("w1", "q").unwrap().expect("first claim");
        assert_eq!(
            a.record.state,
            JobState::Running {
                worker: "w1".into(),
                lease_until_ms: 1_000
            }
        );
        assert_eq!(a.record.attempt, 1);
        // w2 cannot claim while w1 holds the lease.
        assert!(sched.claim("w2", "q").unwrap().is_none());
        assert_eq!(sched.queue_len("q"), 0);
        let _ = id;
    }

    #[test]
    fn complete_requires_owning_worker() {
        let (sched, _) = clocked(1_000, 3, 0);
        let id = sched.submit("q", vec![], None).unwrap();
        let _ = sched.claim("w1", "q").unwrap();
        assert!(
            sched.complete("w2", &id).is_err(),
            "stranger cannot complete"
        );
        assert!(sched.complete("w1", &id).is_ok());
        assert_eq!(sched.get(&id).unwrap().unwrap().state, JobState::Succeeded);
    }

    #[test]
    fn reap_redelivers_once_then_dead_letters() {
        let (sched, clock) = clocked(100, 2, 0);
        let id1 = sched.submit("q", vec![], None).unwrap();
        let id2 = sched.submit("q", vec![], None).unwrap();
        // w1 claims all runnable jobs, then vanishes.
        let c1 = sched.claim("w1", "q").unwrap().expect("claim 1");
        let c2 = sched.claim("w1", "q").unwrap().expect("claim 2");
        // Claim order is not guaranteed (no FIFO here; priorities belong to
        // P9-M2-T01). Both were claimed, each exactly once.
        let mut claimed = vec![c1.record.id.clone(), c2.record.id.clone()];
        claimed.sort();
        assert_eq!(claimed, vec![id1.clone(), id2.clone()]);
        assert_eq!(sched.queue_len("q"), 0);

        // Lease expiry -> both redelivered.
        clock.advance(101);
        let mut redelivered = sched.reap(clock.now()).unwrap();
        redelivered.sort();
        assert_eq!(redelivered, vec![id1.clone(), id2.clone()]);
        assert_eq!(sched.queue_len("q"), 2);

        // w2 picks jobs; the second redelivery is attempt 2 (the cap is 2).
        let d1 = sched.claim("w2", "q").unwrap().expect("w2 claims");
        let d2 = sched.claim("w2", "q").unwrap().expect("w2 claims 2");
        assert_eq!(d1.record.attempt, 2, "redelivered => second attempt");
        assert_eq!(d2.record.attempt, 2);

        // Both leases lapse again; attempt is now at the cap -> both parked.
        clock.advance(101);
        let more = sched.reap(clock.now()).unwrap();
        assert_eq!(more.len(), 0, "at the cap nothing is redelivered");
        let recs = sched.store().all().unwrap();
        let parked = recs
            .iter()
            .filter(|r| matches!(r.state, JobState::DeadLettered { .. }))
            .count();
        assert_eq!(parked, 2, "exhausted attempts park the job (dead-letter)");
        assert_eq!(sched.config().max_attempts, 2);
    }
}
