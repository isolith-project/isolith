//! Job log: a ring buffer of real lifecycle events recorded by the dashboard.
//!
//! The log is written only when the dashboard observes a real transition on
//! the scheduler (enqueue, dispatch, ack, fail/poison, reclaim, dead-letter),
//! so the "job logs" view is a true audit trail, never fabricated lines.

use std::collections::VecDeque;

use oep_lane::sched::JobId;

/// The event kinds the dashboard can record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// A job was enqueued.
    Enqueue,
    /// A job was dispatched to a node.
    Dispatch,
    /// A job acked successfully.
    Ack,
    /// A job failed transiently (scheduled for retry).
    Fail,
    /// A job was reported poison (terminal).
    Poison,
    /// A job's lease expired and it was reclaimed by the scheduler.
    Reclaim,
    /// A job landed in the dead-letter log.
    DeadLetter,
}

impl LogKind {
    /// Stable machine-readable string for the wire format.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogKind::Enqueue => "enqueue",
            LogKind::Dispatch => "dispatch",
            LogKind::Ack => "ack",
            LogKind::Fail => "fail",
            LogKind::Poison => "poison",
            LogKind::Reclaim => "reclaim",
            LogKind::DeadLetter => "dead-letter",
        }
    }
}

/// One log line.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Absolute time (dashboard clock) the event was observed.
    pub at_ms: u64,
    /// The job the event concerns.
    pub job: Option<JobId>,
    /// The node the event concerns (dispatches, acks, reclaims).
    pub node: Option<String>,
    /// The event kind.
    pub kind: LogKind,
    /// Human detail (e.g. failure reason).
    pub detail: String,
}

impl LogEntry {
    /// A new log line.
    pub fn new(
        at_ms: u64,
        kind: LogKind,
        job: Option<JobId>,
        node: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        LogEntry {
            at_ms,
            job,
            node,
            kind,
            detail: detail.into(),
        }
    }
}

/// A bounded job log (ring buffer). Oldest entries are dropped past `cap`.
#[derive(Debug, Clone)]
pub struct JobLog {
    cap: usize,
    entries: VecDeque<LogEntry>,
}

impl JobLog {
    /// A log that keeps at most `cap` entries.
    pub fn with_capacity(cap: usize) -> Self {
        JobLog {
            cap: cap.max(1),
            entries: VecDeque::new(),
        }
    }

    /// Append an entry, evicting the oldest past capacity.
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() == self.cap {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// All entries, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Number of entries currently held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_evicts_oldest() {
        let mut log = JobLog::with_capacity(3);
        for i in 0..5 {
            log.push(LogEntry::new(
                i,
                LogKind::Enqueue,
                Some(JobId::new(i)),
                None,
                "x",
            ));
        }
        assert_eq!(log.len(), 3);
        let times: Vec<u64> = log.entries().map(|e| e.at_ms).collect();
        assert_eq!(times, vec![2, 3, 4]);
    }
}
