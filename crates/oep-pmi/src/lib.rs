//! P6 -- Geometry core and model representation (workstream P6): PMI model.
//!
//! P6-M1-T02 delivers `oep-pmi`, the internal PMI (Product & Manufacturing
//! Information) model schema — GD&T tolerances, datums + datum reference
//! frames, and model notes — serialized in our own documented format
//! (`format = "oep-pmi"`, `version = 1`; spec in
//! `docs/pmi-model-schema.md`).
//!
//! This crate is the **import destination** for CAD PMI: the P4 interop
//! workstream (`oep-step`, STEP AP242/QIF) maps semantic PMI entities into
//! this model (P4-M2-T01). It deliberately carries no cross-crate dependency
//! (layout.md: no path deps until the P6 importer hub, P6-M4-T02): references
//! to oep-geo entities (parts, bodies, features) are plain id strings that the
//! importer hub resolves across documents.
//!
//! Modules:
//! - [`document`]: the [`PmiDocument`] container + our-own-format (de)serializer.
//! - [`datum`]: [`Datum`], [`DatumFrame`], [`DatumFrameRef`], [`MaterialCondition`].
//! - [`tolerance`]: [`GeometricTolerance`], the closed 14-value GD&T set [`Characteristic`].
//! - [`note`]: [`Note`], free-text/leader model notes.
//! - [`validate`]: structural invariant rules as [`ValidationError`]s.
#![deny(missing_docs)]

pub mod datum;
pub mod document;
pub mod note;
pub mod tolerance;
pub mod validate;

/// [`MaterialCondition`]: MM LMC RFS modifiers on datums and tolerances.
pub use datum::{Datum, DatumFrame, DatumFrameRef, MaterialCondition};
/// [`PmiDocument`]: the unit of our own serializable PMI model format.
pub use document::PmiDocument;
/// [`Note`]: a text note attached to model features.
pub use note::Note;
/// [`Characteristic`]/[`GeometricTolerance`]: the GD&T model.
pub use tolerance::{Characteristic, GeometricTolerance};
/// [`ValidationError`]: a single documented structural violation.
pub use validate::ValidationError;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P6-PMI";
