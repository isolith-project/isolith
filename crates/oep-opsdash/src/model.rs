//! The dashboard snapshot wire format (serde JSON). This is THE contract the
//! JSON API and the rendered HTML both consume, so one source of truth.

use std::collections::BTreeMap;

use oep_lane::sched::{DeadLetter, JobId, JobStatus, Priority};
use serde::Serialize;

/// Per-status job counters.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct QueueCounts {
    /// Jobs waiting on dependencies.
    pub waiting: u64,
    /// Jobs ready to dispatch.
    pub ready: u64,
    /// Jobs currently running.
    pub running: u64,
    /// Jobs in the retry backoff window.
    pub retrying: u64,
    /// Jobs acked (succeeded).
    pub succeeded: u64,
    /// Jobs dead-lettered.
    pub dead_lettered: u64,
}

impl QueueCounts {
    /// Total known jobs of any status.
    pub fn total(&self) -> u64 {
        self.waiting
            + self.ready
            + self.running
            + self.retrying
            + self.succeeded
            + self.dead_lettered
    }
}

/// A dead-letter row for the queue view.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeadLetterView {
    /// Raw job id.
    pub job: u64,
    /// Death reason text.
    pub reason: String,
    /// Priority at death.
    pub priority: u8,
    /// Attempts consumed.
    pub attempts_used: u32,
    /// When it died (dashboard clock ms).
    pub dead_at_ms: u64,
}

impl From<&DeadLetter> for DeadLetterView {
    fn from(d: &DeadLetter) -> Self {
        DeadLetterView {
            job: d.job.raw(),
            reason: d.reason.clone(),
            priority: d.priority.raw(),
            attempts_used: d.attempts_used,
            dead_at_ms: d.dead_at_ms,
        }
    }
}

/// A job row for the jobs view.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JobView {
    /// Raw job id.
    pub job: u64,
    /// Lifecycle status.
    pub status: String,
    /// Scheduling priority.
    pub priority: u8,
    /// Retry attempts consumed.
    pub failures: u32,
    /// Dispatch generation (redelivery count).
    pub generation: u64,
}

fn status_str(s: JobStatus) -> &'static str {
    match s {
        JobStatus::Waiting => "waiting",
        JobStatus::Ready => "ready",
        JobStatus::Running => "running",
        JobStatus::Retrying => "retrying",
        JobStatus::Succeeded => "succeeded",
        JobStatus::DeadLettered => "dead-lettered",
    }
}

/// A node row for the nodes view.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NodeView {
    /// Node id.
    pub id: String,
    /// Health projection.
    pub health: String,
    /// CPU capacity millicores.
    pub cpu_millicores: u64,
    /// Memory capacity MiB.
    pub memory_mib: u64,
    /// Declared draw, watts.
    pub power_w: u64,
    /// Jobs currently running on this node.
    pub running_jobs: Vec<u64>,
    /// Jobs acked since registration.
    pub jobs_succeeded: u64,
    /// Jobs reclaimed since registration.
    pub jobs_reclaimed: u64,
    /// Last heartbeat (dashboard clock ms).
    pub last_heartbeat_ms: u64,
}

/// A log row for the jobs/log view.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogView {
    /// When observed (dashboard clock ms).
    pub at_ms: u64,
    /// Job id, if any.
    pub job: Option<u64>,
    /// Node id, if any.
    pub node: Option<String>,
    /// Event kind.
    pub kind: String,
    /// Human detail.
    pub detail: String,
}

/// A per-node cost/energy row.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CostView {
    /// Node id.
    pub node: String,
    /// Energy consumed, mWh.
    pub energy_mwh: u128,
    /// Cost, micro-USD.
    pub cost_usd_micro: u128,
    /// Recorded runs.
    pub runs: u64,
}

/// Cluster-wide cost totals.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct CostTotals {
    /// Total energy, mWh.
    pub energy_mwh: u128,
    /// Total cost, micro-USD.
    pub cost_usd_micro: u128,
    /// Total recorded runs.
    pub runs: u64,
}

/// The complete dashboard snapshot.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DashboardSnapshot {
    /// Format marker.
    pub format: String,
    /// Schema version.
    pub version: u32,
    /// Clock time the snapshot was taken (dashboard clock ms).
    pub at_ms: u64,
    /// Queue depth counters (+ dead-letter log).
    pub queue: QueueCounts,
    /// Dead-letter log.
    pub dead_letters: Vec<DeadLetterView>,
    /// Per-node health + capacity.
    pub nodes: Vec<NodeView>,
    /// Per-job statuses.
    pub jobs: Vec<JobView>,
    /// Recent job log lines.
    pub log: Vec<LogView>,
    /// Cost/energy per node.
    pub cost: BTreeMap<String, CostView>,
    /// Cost/energy totals.
    pub cost_totals: CostTotals,
}

impl DashboardSnapshot {
    /// A snapshot marked with the crate's format header.
    pub fn with_meta(at_ms: u64) -> Self {
        DashboardSnapshot {
            format: crate::FORMAT.to_string(),
            version: crate::VERSION,
            at_ms,
            queue: QueueCounts {
                waiting: 0,
                ready: 0,
                running: 0,
                retrying: 0,
                succeeded: 0,
                dead_lettered: 0,
            },
            dead_letters: Vec::new(),
            nodes: Vec::new(),
            jobs: Vec::new(),
            log: Vec::new(),
            cost: BTreeMap::new(),
            cost_totals: CostTotals {
                energy_mwh: 0,
                cost_usd_micro: 0,
                runs: 0,
            },
        }
    }
}

/// Build a stable machine string for a [`Priority`] (used by debug views).
pub fn priority_raw(p: Priority) -> u8 {
    p.raw()
}

/// Raw id of an optional job id.
pub fn job_raw(j: Option<JobId>) -> Option<u64> {
    j.map(|x| x.raw())
}

/// Map a [`JobStatus`] to a stable string.
pub fn job_status_str(s: JobStatus) -> &'static str {
    status_str(s)
}
