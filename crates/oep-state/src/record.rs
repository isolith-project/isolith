//! Durable job record + error types for the P9 distributed-state layer.
//!
//! The [`JobRecord`] is the unit of distributed state that must survive node
//! loss. It is `serde`-round-trippable so durable backends (P8 vault blob,
//! JetStream KV) can persist it verbatim.

use serde::{Deserialize, Serialize};

/// Identifies a job's lifecycle state.
///
/// Transitions (guarded by the scheduler):
///   `Queued -> Running{attempt}` (claim) `-> Succeeded`
///   `Running -> Queued` (lease expiry / redelivery, attempt preserved)
///   `Running -> Failed{reason}` (explicit worker failure)
///   `Queued|Running -> DeadLettered` (attempts exhausted)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    /// Waiting for a worker to claim it.
    Queued,
    /// Claimed by a worker for a lease; redelivered when the lease expires.
    Running {
        /// The worker currently executing the job.
        worker: String,
        /// Absolute lease deadline (ms since a caller-defined epoch); when the
        /// clock passes this, the claim is considered lost and redelivered.
        lease_until_ms: u64,
    },
    /// A worker completed the job and acked.
    Succeeded,
    /// A worker explicitly failed the job.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
    /// Attempts exhausted; the job is parked (dead-letter).
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

    /// `true` when the job is finished (succeeded, failed, or dead-lettered).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::Failed { .. } | JobState::DeadLettered { .. }
        )
    }
}

/// The durable record of one job in the P9 compute lane.
///
/// The opaque `payload` carries whatever the job is about (in the full wave,
/// the `oep-job` [`JobSpec`][spec] JSON); this layer does not interpret it,
/// only preserves it across claims and redeliveries — which is exactly what
/// node-loss survival requires.
///
/// [spec]: https://isolith/isolith (P9-M1-T01 oep-job fmt)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    /// Globally unique, url-safe job id.
    pub id: String,
    /// Named lane the job belongs to (matches the bus taxonomy `oep.jobs.{queue}.*`).
    pub queue: String,
    /// Optional stable idempotency key the effects ledger binds outputs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Opaque job bytes (the job spec); preserved verbatim across redelivery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<u8>,
    /// How many times this job has been claimed (delivery count; >= 1).
    pub attempt: u32,
    /// Current lifecycle state.
    pub state: JobState,
    /// Creation timestamp (ms since a caller-defined epoch).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_record_round_trips() {
        let rec = JobRecord::new("job-1", "render", None, 42);
        let json = serde_json::to_string(&rec).unwrap();
        let back: JobRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn idempotency_key_survives_roundtrip_when_present() {
        let rec = JobRecord::new("job-2", "convert", Some("stable-key".into()), 1);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("stable-key"));
        let back: JobRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.idempotency_key.as_deref(), Some("stable-key"));
    }

    #[test]
    fn state_classification() {
        assert!(JobState::Queued.is_runnable());
        assert!(!JobState::Running {
            worker: "w".into(),
            lease_until_ms: 0
        }
        .is_runnable());
        assert!(JobState::Succeeded.is_terminal());
        assert!(JobState::Failed { reason: "x".into() }.is_terminal());
        assert!(JobState::DeadLettered { reason: "x".into() }.is_terminal());
        assert!(!JobState::Queued.is_terminal());
    }
}
