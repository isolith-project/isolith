//! P7 -- Open modeler lane: assembly workbench.
//!
//! `oep-assem` is the OEP assembly workbench: it maps the canonical oep-geo
//! assembly model (parts, instances, 4x4 transforms; P6-M1-T01) plus a mate
//! (constraint) set onto a permissive constraint solver, computes instance
//! placements, writes them back into the oep-geo document, and measures how
//! well the constraints are satisfied.
//!
//! Solver decision: SOLVER-REPLACE (program decision #16) removed the
//! vendored GPL-3.0 SolveSpace tree and put an own bounded solver behind this
//! crate's vendor-neutral seam. The engine is the damped Gauss-Newton
//! least-squares lane from `oep-kinematics` (P7-M3-T02); the decision and
//! consequence analysis are recorded in `workstreams/P7-M3/solver-adr.md` at the repo
//! root (v1.2.0). No GPL-3.0 code or dependency exists in this crate.
//!
//! Modules:
//! - [`adopted`]: the vendor-neutral seam over the solver (own bounded
//!   engine; no vendored third-party tree).
//! - [`anchors`]: named part-local anchor model + deterministic defaults.
//! - [`mates`]: the workbench constraint vocabulary + serialized constraint set.
//! - [`transform_math`]: elementary vector/quaternion/transform helpers.
//! - [`workbench`]: the workbench solve orchestration (this crate's heart).
// Public items carry doc comments where they are real API; the small serde
// model types additionally hold plain data fields, so `missing_docs` is
// allowed here (oep-geo keeps it denied; this crate does not).
#![allow(missing_docs)]

pub mod adopted;
pub mod anchors;
pub mod mates;
pub mod transform_math;
pub mod workbench;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P7-MODELER";

pub use anchors::{Anchor, AnchorRegistry, PartAnchors};
pub use mates::{AnchorRef, ConstraintSet, Mate};
pub use workbench::{
    list_instances, load_document, measure_residuals, save_document, solve, InstanceInfo,
    MateResidual, MateResidualKind, SolveReport, SolveRequest, WorkbenchError,
};
