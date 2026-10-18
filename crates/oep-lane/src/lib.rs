//! P9 -- P2P compute lane (workstream P9). Reserved crate for the distributed
//! task-flow / compute lane that runs alongside the PDM (P8).
//!
//! P9 render/export workers, the message bus (P9-M1-T02) and the distributed
//! state layer (P9-M2-T03) all route through this crate's types.
//!
//! Wave-2 module landed by this card:
//!
//! - [`sched`] -- P9-M2-T01: the scheduler. Guarantees the compute lane's
//!   execution contract:
//!   * **priority** -- jobs dispatch in priority order (ties FIFO);
//!   * **DAG dependencies** -- a job is runnable only when every dependency
//!     has succeeded; cycles are rejected at enqueue time; a job whose
//!     dependency dead-letters is dead-lettered in cascade;
//!   * **retries with backoff** -- failures reschedule with exponential
//!     backoff up to a per-job attempt budget;
//!   * **dead-letter** -- jobs that exhaust their attempts land in a
//!     dead-letter log (poison jobs) and are never dispatched again;
//!   * **at-least-once seam** -- dispatched work is leased; an unacked job
//!     is reclaimed and redelivered after the lease expires (worker-loss
//!     survival is proven by P9-M2-T03 on top of this contract).
#![deny(missing_docs)]

/// P9-M2-T01 scheduler: priority queue, DAG deps, retries, dead-letter.
pub mod sched;

/// Placeholder identity so the crate builds as a workspace member.
pub const WORKSTREAM: &str = "P9-LANE";
