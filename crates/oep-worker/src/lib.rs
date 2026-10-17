//! oep-worker -- P9: P2P compute lane, export/convert worker functions
//! (P9-M3-T02).
//!
//! This crate is the P9 `convert` / `export` executor lane. It turns P8 vault
//! inputs into derived artifacts by dogfooding the platform's own readers:
//! CGR -> glTF/GLB through [`oep_cgr`] (P2) and STEP -> faceted mesh through
//! [`oep_step`] (P4), then stores every artifact back in the P8 vault (via
//! [`oep_pdm`]) with its content address hash-verified. P2/P4 output feeds P9
//! input; P9 output feeds the P8 vault.
//!
//! Modules:
//! - [`convert`]   -- the pure byte->artifact conversion functions
//!   (`cgr2gltf`, `step2mesh`), deterministic and typed-outcome.
//! - [`registry`]  -- the P9 worker registry: resolves `executor.kind` +
//!   `executor.function` (the closed `convert` vocabulary from P9-M1) to a
//!   concrete worker.
//! - [`worker`]    -- the job-facing worker: takes a [`oep_job::JobSpec`],
//!   resolves its vault inputs through the P8-M6-T02 [`oep_pdm::adapter`]
//!   seam, runs the worker, and publishes outputs to the vault with sha-256
//!   verification against the server-reported content address.
//! - [`batch`]     -- the batch convert driver (`oep-batch-convert`): runs a
//!   corpus of jobs across the real material and emits a hash-verified
//!   [`batch::BatchReport`].
//! - [`mesh_doc`]  -- the deterministic `oep-mesh` v1 JSON carrier that the
//!   STEP->mesh worker emits.
#![deny(missing_docs)]

pub mod batch;
pub mod convert;
pub mod mesh_doc;
pub mod registry;
pub mod worker;

pub use batch::{BatchManifest, BatchReport, SeedManifest};
pub use convert::{cgr_to_gltf, step_to_mesh, Artifact, ConvertError, ConvertKind};
pub use worker::{run_job, JobOutcome, JobStatus, WorkerError};

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P9-COMPUTE";
