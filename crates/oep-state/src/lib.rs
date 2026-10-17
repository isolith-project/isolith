//! P9 compute lane (workstream P9), distributed state.
//!
//! P9-M2-T03 delivers the **distributed-state layer** of the P2P compute
//! lane: job records that survive node loss, at-least-once delivery with
//! lease-based redelivery, and a durable effect ledger that makes jobs
//! idempotent.
//!
//! RED (this commit): the crate and its public API surface are declared so
//! the acceptance drill in `tests/node_loss_drill.rs` compiles against the
//! intended contract. The behavior is deliberately unimplemented — the drill
//! fails until the GREEN commit supplies it. Contract: `docs/P9-M2/
//! distributed-state.md`.
#![deny(missing_docs)]

pub mod effects;
pub mod record;
pub mod scheduler;
pub mod store;
pub mod time;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";
