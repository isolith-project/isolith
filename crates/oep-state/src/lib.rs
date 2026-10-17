//! P9 compute lane (workstream P9), distributed state.
//!
//! P9-M2-T03 delivers the **distributed-state layer** of the P2P compute
//! lane: job records that survive node loss, at-least-once delivery with
//! lease-based redelivery, and a durable effect ledger that makes jobs
//! idempotent (a redelivered run can never double-apply an effect).
//!
//! This crate is deliberately self-contained: it does not depend on the
//! P9-M1-T01 `oep-job` spec crate (which is materialized by a sibling card
//! in the same wave and merged through the same review lane). A job here is
//! an opaque record with an id, queue, opaque payload bytes, an attempt
//! counter, and a state. When the wave lands, the scheduler (P9-M2-T01)
//! binds [`crate::scheduler`] onto `oep-job`'s [`JobSpec`] by encoding the
//! spec as the payload; the idempotency contract (P9-M2-T03) is this crate's
//! [`crate::effects`] ledger.
//!
//! The three guarantees, each proven by test in `tests/node_loss_drill.rs`:
//!
//! - **Node-loss survival.** Job records live in a shared [`JobStore`], not
//!   in the worker. When a worker vanishes mid-job (never acks, never fails),
//!   the job is still in the store; the scheduler redelivers it after the
//!   claim lease expires (`Scheduler::reap`).
//! - **At-least-once delivery.** A claim is for a fixed lease; the attempt
//!   counter bumps on every claim. A job that lost its worker is claimed
//!   again (attempt >= 2). Delivery is at-least-once by construction: a job
//!   is only dropped when attempts exhaust [`SchedConfig::max_attempts`].
//! - **Idempotent jobs.** Side effects go through the [`EffectLedger`], a
//!   durable content-derived key space. An effect key is recorded exactly
//!   once; a redelivered run observes `AlreadyApplied` and skips — no
//!   double-effect, matching the P8 content-address doctrine where writing
//!   the same output twice is a no-op.
//!
//! Serialization uses deterministic `serde` records (a [`record::JobRecord`]
//! round-trips) so state can be persisted to the P8 vault or JetStream KV
//! without loss; the in-memory [`store`] is the test/proof backend, and the
//! [`JobStore`] trait is the seam a NATS JetStream KV store plugs into.
//!
//! [P9-M2-T01]: scheduler
#![deny(missing_docs)]

pub mod effects;
pub mod record;
pub mod scheduler;
pub mod store;
pub mod time;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";
