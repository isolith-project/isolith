//! P9 -- P2P compute lane (workstream P9). **Beam/farm-style checkpoint and
//! resume for long jobs** (P9-M4-T02).
//!
//! Long compute jobs (render waves, format-conversion batches, GA gates) can
//! run for a long time across many small *work items*. Mainstream batch
//! engines (Apache Beam, MapReduce farms) solve the "a worker was evicted
//! mid-job" problem with **durable checkpoints**: progress is persisted while
//! the job runs, and a replacement worker resumes from the *last checkpoint*
//! instead of restarting the whole job. That is exactly this crate.
//!
//! The contract this card owns:
//!
//! 1. **A long job is an ordered sequence of deterministic work items.** An
//!    item at a given index always produces the same output bytes (given the
//!    job's fixed inputs / seed). Determinism is what makes resume
//!    byte-equivalent: re-running the items after the checkpoint produces
//!    bits identical to what a never-evicted run would have written there.
//!    See [`items`].
//! 2. **Progress is durable, not memory.** A [`farm::FarmRunner`] streams
//!    item outputs into an [`artifact::Artifact`] and every `K` items writes
//!    a [`record::Checkpoint`] to a [`store::CheckpointStore`] (in-memory
//!    for tests, a file directory for the real drill). The checkpoint
//!    records *how many items completed* and the *byte length + FNV-1a hash*
//!    of the output prefix so far.
//! 3. **Resume verifies the prefix before continuing.** A fresh runner loads
//!    the latest checkpoint, reads the artifact prefix that should already
//!    exist, recomputes its hash and refuses to continue on mismatch
//!    ([`farm::FarmError::Corruption`]) - it never silently produces output
//!    that diverges from a clean run. It then truncates the artifact back to
//!    the checkpoint boundary (dropping a partial item a dying worker may
//!    have started) and resumes from the checkpoint's `next_item`.
//! 4. **Byte-equivalence is the acceptance.** The drill
//!    (`tests/checkpoint_resume_acceptance.rs`, also emitted as
//!    `P9-M4/proof/checkpoint-resume-proof.json` by the `oep-farm-drill`
//!    binary) runs the same long job twice: once clean to completion, once
//!    with a worker evicted mid-job and resumed from the last checkpoint.
//!    The final artifact bytes of both runs must be **identical, byte for
//!    byte** (asserted in Rust and via SHA-256 in the Python acceptance
//!    test).
//!
//! Placement in the P9 wave:
//!
//! - The **idempotency key** of the P9-M1 job spec and the scheduler's
//!   **at-least-once redelivery** (P9-M2-T01 lease/ack seam) decide *when* a
//!   job is given to a worker again. This crate answers *how* that worker
//!   makes the redelivery cheap and correct: it resumes from the last
//!   checkpoint instead of re-running everything, and the prefix hash proves
//!   the already-done output is intact.
//! - Output bytes are streamed into the artifact the job's output refs
//!   (P9-M1) will point at; the GA gate (P9-M5) can content-verify them.
//! - Like `oep-state` (P9-M2-T03), this crate is deliberately self-contained
//!   (std-only, no cross-crate path deps until the P6 importer hub): a job is
//!   an opaque id + a deterministic item source, and the binding onto
//!   `oep-job`'s `JobSpec` happens in a later milestone.
#![deny(missing_docs)]

/// The streaming artifact that holds a job's output bytes.
pub mod artifact;
/// The farm runner: run with checkpoints, resume after eviction.
pub mod farm;
/// Deterministic hash digest helpers (FNV-1a 64, SplitMix64).
pub mod hash;
/// A long job's work items (deterministic item source).
pub mod items;
/// The durable progress record (wire format + integrity).
pub mod record;
/// Where checkpoints live (in-memory and file-backed stores).
pub mod store;

pub use artifact::{Artifact, FileArtifact, VecArtifact};
pub use farm::{FarmConfig, FarmError, FarmRunner, RunReport};
pub use hash::{fnv1a64, splitmix64};
pub use items::{ItemSource, SeededItemSource};
pub use record::{Checkpoint, CheckpointError, FORMAT, VERSION};
pub use store::{CheckpointStore, DirStore, MemoryStore, StoreError};

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-FARM";
