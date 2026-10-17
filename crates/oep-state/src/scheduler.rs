//! Scheduler: lease-based at-least-once dispatch (RED API surface).

use crate::record::{JobRecord, StoreError};
use crate::store::JobStore;
use crate::time::Clock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Scheduler configuration. Defaults are deliberately conservative.
#[derive(Debug, Clone)]
pub struct SchedConfig {
    /// Lease length in milliseconds; a claim older than this is redelivered.
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
        Scheduler::with_clock(store, config, Arc::new(crate::time::SystemClock))
    }

    /// A scheduler over `store` driven by an explicit clock (tests).
    pub fn with_clock(store: Arc<dyn JobStore>, config: SchedConfig, clock: Arc<dyn Clock>) -> Self {
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
        _queue: impl Into<String>,
        _payload: Vec<u8>,
        _idempotency_key: Option<String>,
    ) -> Result<String, StoreError> {
        unimplemented!("P9-M2-T03 scheduler submit lands in GREEN")
    }

    /// Number of runnable (queued) jobs in `queue`.
    pub fn queue_len(&self, _queue: &str) -> usize {
        0
    }

    /// Claim a queued job in `queue` for `worker` under a lease.
    pub fn claim(&self, _worker: &str, _queue: &str) -> Result<Option<ClaimedJob>, StoreError> {
        unimplemented!("P9-M2-T03 scheduler claim lands in GREEN")
    }

    /// Reap expired leases; returns the ids redelivered to `Queued`.
    pub fn reap(&self, _now: u64) -> Result<Vec<String>, StoreError> {
        unimplemented!("P9-M2-T03 scheduler reap lands in GREEN")
    }

    /// Acknowledge success for a job from its owning worker.
    pub fn complete(&self, _worker: &str, _id: &str) -> Result<(), StoreError> {
        unimplemented!("P9-M2-T03 scheduler complete lands in GREEN")
    }

    /// Mark a job explicitly failed by its owning worker.
    pub fn fail(&self, _worker: &str, _id: &str, _reason: &str) -> Result<(), StoreError> {
        unimplemented!("P9-M2-T03 scheduler fail lands in GREEN")
    }

    /// Read a job's current record.
    pub fn get(&self, _id: &str) -> Result<Option<JobRecord>, StoreError> {
        unimplemented!("P9-M2-T03 scheduler get lands in GREEN")
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
