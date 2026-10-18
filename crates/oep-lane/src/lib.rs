//! P9 -- P2P compute lane (workstream P9). Reserved crate for the distributed
//! task-flow / compute lane that runs alongside the PDM (P8).
//!
//! P9 render/export workers, the message bus (P9-M1-T02) and the distributed
//! state layer (P9-M2-T03) all route through this crate's types.
//!
//! Modules landed to date:
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
//!   * **GPU placement (P9-M4-T01)** -- when a node pool is attached, a job
//!     tagged with a GPU request (see [`placement`]) dispatches only onto a
//!     node holding a matching free device; a *required* GPU job parks (but
//!     never blocks other work) when no GPU is free, and a *preferred* GPU
//!     job falls back to a CPU node cleanly, flagged `cpu_fallback`.
//! - [`gpu`] -- P9-M4-T01: GPU/hardware discovery (device model + the
//!   `nvidia-smi` CSV parser + a static probe for deterministic tests).
//! - [`placement`] -- P9-M4-T01: GPU-aware placement (node pool with
//!   exclusive device reservation, the GPU request typing, the placement
//!   policy and the placement outcome).
#![deny(missing_docs)]

/// P9-M2-T01 scheduler: priority queue, DAG deps, retries, dead-letter.
pub mod sched;

/// GPU / hardware discovery (P9-M4-T01).
pub mod gpu;

/// GPU-aware placement: node pool, requests, policy (P9-M4-T01).
pub mod placement;

/// Placeholder identity so the crate builds as a workspace member.
pub const WORKSTREAM: &str = "P9-LANE";
