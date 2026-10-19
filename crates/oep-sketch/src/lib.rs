//! P7 -- 2D sketcher + parametric variable sets in the chosen shell.
//!
//! P7-M4-T01 (GREEN): the chosen shell (P7-M1-T01) is our own thin Rust shell
//! around Open CASCADE; FreeCAD is NOT the shell, it is the behavioral
//! reference. So this crate implements sketcher and variable-set semantics in
//! the chosen shell's semantics, matching FreeCAD v1 variable-set behavior
//! without importing FreeCAD (clean-room invariant).
//!
//! What lives here:
//!
//! - [`sketch`]: a declarative 2D [`Sketch`] model - points, line segments,
//!   and geometric/dimensional constraints. Dimension constraints reference a
//!   variable-set entry BY NAME ([`DimExpr::Variable`]) or a literal
//!   ([`DimExpr::Literal`]), the FreeCAD-v1-variable-set semantics.
//! - [`varset`]: [`VariableSet`] - named scalar parameters editable at the
//!   model level (like FreeCAD v1 variable sets / y-arsets), driving sketch
//!   dimensions.
//! - [`solve`]: the ADOPTED OSS constraint solver (choice + rationale:
//!   `P7-M4/sketch-solver-adr.md`) reached ONLY through a vendor-neutral
//!   seam. The public surface is our own types, never the solver's. Editing
//!   a variable re-solves and the solved geometry follows.
//! - [`features`]: [`Sketch`] and [`VariableSet`] stored as `oep-geo`
//!   [`oep_geo::Feature`]s (kinds `Sketch` / `VariableSet`) in the canonical
//!   document - the shell-contract requirement that sketches and var sets live
//!   as oep-geo features.
//! - [`drive`]: the acceptance demonstration - a solved sketch dimension
//!   drives a MODEL dimension (an `Extrude`-style feature parameter and the
//!   solid body bbox), and follows a variable edit.
//!
//! Tolerant-failure doctrine: a missing variable, an unknown entity reference,
//! or a diverging solve returns a typed [`SketchError`], never a panic and
//! never fabricated geometry.
#![deny(missing_docs)]

pub mod drive;
pub mod features;
pub mod sketch;
pub mod solve;
pub mod varset;

pub use drive::{body_width, drive_sketch_to_document};
pub use features::{
    sketch_from_feature, sketch_to_feature, varset_from_feature, varset_to_feature,
};
pub use sketch::{Constraint, DimExpr, LineSeg, Sketch};
pub use solve::{solve_sketch, SketchError, SolveOutcome};
pub use varset::VariableSet;

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P7-SKETCH";
