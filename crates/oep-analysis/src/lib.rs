//! oep-analysis -- editor-spine analysis lane (decision #19 pull-forward trio).
//!
//! The decision-#19 editor wave pulled mass properties, interference/clash
//! detection, and exploded views forward into the editor spine. Per the scope
//! register these are **Class 1 additive** capabilities: new crates consuming
//! the canonical `oep-geo` model, never altering it. This crate is that
//! consumer.
//!
//! Three capabilities, each with a real-material gate:
//!
//! 1. **Mass properties** ([`mass`]): volume, mass, centroid, and moments of
//!    inertia for a solid body, computed from the geometry the canonical model
//!    actually carries (a solid body's bounding box — the box is an analytical
//!    solid, so the result is verified against its closed form within a
//!    committed tolerance). Density is sourced from the document's **material
//!    metadata** ([`material::MaterialTable`], a companion format stored next
//!    to the model exactly like the assembly workbench stores its constraint
//!    set).
//! 2. **Interference / clash detection** ([`clash`]): walk the placed
//!    instances of an assembly (the resolved mates graph — the workbench
//!    solves mates into instance placements), compute world AABBs, and report
//!    every penetrating pair with its overlap location and volume.
//! 3. **Exploded views** ([`explode`]): deterministically separate the
//!    assembly structure along the mate hierarchy by a pull distance, in
//!    document units, so the editor surface can re-render as the user drags
//!    the interactive pull slider.
//!
//! Seam note (honest scope): at the P6-M1 milestone a solid body's only
//! geometry slot is its axis-aligned bounding box, so the exact kernel path
//! that computes properties from the box IS the analytical solid. When the
//! Open CASCADE binding (oep-engine-occ, KERNEL-BASE/KERNEL-REPLAY) lands, the
//! same `oep-geo` model types feed exact B-Rep through this crate unchanged —
//! the public surface below never changes shape.
//
// Like the sibling workbench crate `oep-assem`, public items carry doc
// comments where they are real API; the serializable data-model types hold
// plain fields, so `missing_docs` is allowed here (oep-geo keeps it denied;
// this analysis lane does not).
#![allow(missing_docs)]

pub mod clash;
pub mod explode;
pub mod mass;
pub mod material;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P7-ANALYSIS";

pub use clash::{Clash, ClashError};
pub use explode::{ExplodeError, ExplodedPlacement};
pub use mass::{MassError, MassProperties};
pub use material::{Material, MaterialTable, MaterialTableError};
