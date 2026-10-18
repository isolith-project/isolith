//! Dead-letter records for the scheduler (P9-M2-T01).

use super::{JobId, Priority, RetryPolicy};

/// A job moved to the dead-letter log. Dead-lettered jobs are **never**
/// dispatched again: they represent either a poison job (exhausted the retry
/// budget or explicitly reported bad by a worker) or a job whose dependency
/// dead-lettered (cascade; work must not proceed on a failed input).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadLetter {
    /// The dead job.
    pub job: JobId,
    /// Human-readable reason: "retries exhausted", "poison reported by
    /// worker", or "dependency dead-lettered: <id>".
    pub reason: String,
    /// The job's declared retry policy at death time.
    pub retry: RetryPolicy,
    /// The job's priority at death time (for the ops dashboard, P9-M5-T01).
    pub priority: Priority,
    /// The wall-clock ms at which the job was dead-lettered.
    pub dead_at_ms: u64,
    /// How many attempts were consumed before giving up.
    pub attempts_used: u32,
}

/// Why a job is dead-lettered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeadLetterReason<'a> {
    /// The retry budget was exhausted after `attempts` failures.
    RetriesExhausted {
        /// Number of failures consumed before giving up.
        attempts: u32,
    },
    /// A worker explicitly reported the job as poison (will never succeed).
    PoisonReported(&'a str),
    /// A dependency dead-lettered, so this job can never be satisfied.
    DependencyDeadLettered {
        /// The dependency whose death blocked this job.
        dependency: JobId,
    },
    /// The job was dead-lettered by direct external request.
    Manual(&'a str),
}

impl<'a> DeadLetterReason<'a> {
    /// Render the reason as the stable, human-readable text stored in
    /// [`DeadLetter::reason`].
    pub fn message(&self) -> String {
        match self {
            DeadLetterReason::RetriesExhausted { attempts } => {
                format!("retries exhausted after {attempts} attempts")
            }
            DeadLetterReason::PoisonReported(detail) => {
                format!("poison reported by worker: {detail}")
            }
            DeadLetterReason::DependencyDeadLettered { dependency } => {
                format!("dependency dead-lettered: {dependency}")
            }
            DeadLetterReason::Manual(detail) => {
                format!("dead-lettered manually: {detail}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_messages_are_stable() {
        assert_eq!(
            DeadLetterReason::RetriesExhausted { attempts: 3 }.message(),
            "retries exhausted after 3 attempts"
        );
        assert_eq!(
            DeadLetterReason::PoisonReported("parse: corrupt header").message(),
            "poison reported by worker: parse: corrupt header"
        );
        assert_eq!(
            DeadLetterReason::DependencyDeadLettered {
                dependency: JobId::new(7)
            }
            .message(),
            "dependency dead-lettered: job 7"
        );
        assert_eq!(
            DeadLetterReason::Manual("canceled in UI").message(),
            "dead-lettered manually: canceled in UI"
        );
    }
}
