//! oep-job -- P9: P2P compute lane (workstream P9).
//!
//! P9-M1-T01 delivers the **job/task spec**: the versioned, serializable
//! description of a unit of work in the P9 compute lane. A [`JobSpec`] names
//! four things, exactly the four contract groups of the card:
//!
//! - [`InputRef`] -- **input refs**: the [P8 vault objects] a job consumes.
//!   P8 object refs are the *input currency*: every input is an item,
//!   revision, or content-addressed file of the P8 vault (P8-M1-T02), and the
//!   [`p8`] module maps each ref onto the P8-M6-T01 REST v1 resource the
//!   scheduler must preflight.
//! - [`ExecutorRef`] -- which P9 worker function runs the job (render,
//!   convert, import, export, validate, custom; P9-M3-T01/T02).
//! - [`ResourceCaps`] -- the resource caps the worker sandbox must enforce
//!   (P9-M3-T03) and the GPU request the scheduler must honor (P9-M4-T01).
//! - [`OutputRef`] -- declared outputs, each landing back in the P8 vault as a
//!   content-addressed blob, a new item, or a new revision of an existing item
//!   (verified in PDM by the P9-M5-T02 GA gate).
//!
//! The wire format is our own (`format = "oep-job"`, `version = 1`); a
//! committed JSON Schema in `docs/compute/spec/job-spec-v1.schema.json` is the
//! machine contract, and `docs/compute/spec/examples/*.json` are examples that both
//! this crate (`include_str!`) and the Python acceptance test
//! (`tests/test_p9_job_spec.py`) validate against the same bytes.
//!
//! Validation follows the P8-M1-T02 doctrine: [`JobSpec::validate`] returns
//! every violation as a `{code, message}` pair, never panics, and rejects
//! nothing at load time. Ids and vocabularies are bounded by closed
//! [`spec`] constants that the JSON Schema enum re-states, so schema and code
//! cannot drift (guarded by the Python DRIFT test).
//!
//! [P8 vault objects]: https://192.168.0.101:3200/isolith/isolith
#![deny(missing_docs)]

pub mod p8;
pub mod spec;
pub mod validate;

/// The P8-M1-T02 vault object a job input points at (item | revision | file).
pub use spec::{ExecutorRef, InputRef, JobSpec, OutputRef, OutputTarget, ResourceCaps};
/// The validator's structured `{code, message}` violation pair.
pub use validate::ValidationError;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";
