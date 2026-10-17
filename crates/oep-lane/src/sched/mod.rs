//! P9-M2-T01 -- scheduler core for the P2P compute lane.
//!
//! Ownership of the execution contract:
//!
//! - **priority** -- jobs dispatch in priority order; ties are strictly FIFO
//!   by enqueue sequence.
//! - **DAG dependencies** -- a job is runnable only once every dependency has
//!   reached the terminal success state; enqueue rejects unknown
//!   dependencies and defensive cycle detection is applied; when a dependency
//!   dead-letters, its dependents dead-letter in cascade (no work proceeds on
//!   a failed input).
//! - **retries with backoff** -- a failed job reschedules on an exponential
//!   backoff schedule up to its per-job attempt budget.
//! - **dead-letter** -- jobs that exhaust the attempt budget, or that a
//!   worker reports as poison, are parked in the dead-letter log and never
//!   dispatched again.
//! - **at-least-once seam (P9-M2-T03)** -- every dispatch is a *lease*: the
//!   job is marked running until ack'd. If the lease expires without an ack
//!   (worker loss), the scheduler reclaims the job and redelivers it **with
//!   the same idempotency key**, so at-least-once delivery + idempotent
//!   execution gives exactly-once effect. The durability *drill* (kill a
//!   worker mid-job) is P9-M2-T03's acceptance; the contract below is the
//!   part this card owns and unit-tests.
//!
//! Layering:
//!
//! - [`priority`] -- [`Priority`]: ordering hint (higher dispatches first).
//! - [`retry`] -- [`RetryPolicy`]: exponential backoff with cap.
//! - [`dag`] -- pure dependency graph: edges, transitive dependent queries,
//!   cycle detection.
//! - [`deadletter`] -- [`DeadLetter`] record + reason.
//! - [`scheduler`] -- [`Scheduler`]: owns the job registry, the ready heap,
//!   the delayed (backoff) heap and the dead-letter log.
//!
//! Time is never read from the wall clock here: every method takes an
//! explicit `now_ms`, so tests drive the schedule deterministically (the
//! injected-clock pattern used across oep-pdm).

pub mod dag;
pub mod deadletter;
pub mod priority;
pub mod retry;
pub mod scheduler;

pub use dag::{CycleError, DepGraph, UnknownDependency};
pub use deadletter::{DeadLetter, DeadLetterReason};
pub use priority::Priority;
pub use retry::RetryPolicy;
pub use scheduler::{AckError, Dispatch, JobId, JobSpec, JobStatus, Scheduler};
