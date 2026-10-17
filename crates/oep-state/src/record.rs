//! Durable job record + error types (RED: API surface only).

use serde::{Deserialize, Serialize};

/// Job lifecycle state (RED: the shape; transitions in GREEN).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    /// Waiting for a worker to claim it.
    Queued,
    /// Claimed by a worker for a lease.
    Running {
        /// The worker currently executing the job.
        worker: String,
        /// Absolute lease deadline (ms).
        lease_until_ms: u64,
    },
    /// A worker completed the job and acked.
    Succeeded,
    /// A worker explicitly failed the job.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
    /// Attempts exhausted; the job is parked.
    DeadLettered {
        /// Human-readable final reason.
        reason: String,
    },
}

impl JobState {
    /// `true` when the job is still eligible for dispatch.
    pub fn is_runnable(&self) -> bool {
        matches!(self, JobState::Queued)
    }

    /// `true` when the job is finished.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::Failed { .. } | JobState::DeadLettered { .. }
        )
    }
}

/// The durable record of one job.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    /// Globally unique, url-safe job id.
    pub id: String,
    /// Named lane the job belongs to.
    pub queue: String,
    /// Optional stable idempotency key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Opaque job bytes; preserved verbatim across redelivery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<u8>,
    /// Delivery count (>= 1 after first claim).
    pub attempt: u32,
    /// Current lifecycle state.
    pub state: JobState,
    /// Creation timestamp (ms).
    pub created_ms: u64,
}

impl JobRecord {
    /// A brand-new queued job with no payload yet.
    pub fn new(
        id: impl Into<String>,
        queue: impl Into<String>,
        idempotency_key: Option<String>,
        created_ms: u64,
    ) -> Self {
        JobRecord {
            id: id.into(),
            queue: queue.into(),
            idempotency_key,
            payload: Vec::new(),
            attempt: 0,
            state: JobState::Queued,
            created_ms,
        }
    }
}

/// Errors surfaced by the durable state layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A keyed record was not found.
    NotFound(String),
    /// A concurrent update made a guarded operation inapplicable.
    Conflict(String),
    /// A backend I/O or serialization failure.
    Storage(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(k) => write!(f, "state record not found: {}", k),
            StoreError::Conflict(m) => write!(f, "state conflict: {}", m),
            StoreError::Storage(m) => write!(f, "state storage failure: {}", m),
        }
    }
}

impl std::error::Error for StoreError {}
