//! P6 -- Geometry core and model representation (workstream P6).
//!
//! Skeleton crate: milestone cards add the part/body/feature/assembly model
//! schema, PMI schema, math primitives, B-Rep ops and exporters.
#![deny(missing_docs)]

/// Math primitives: [`Vec3`], [`Mat3`], [`Mat4`], [`Quat`] (P6-M2-T01).
///
/// Vendored std-only f64 math core. Layout convention: `Mat4`/`Mat3` are stored
/// **column-major** to match the P6-M1-T01 instance `transform: [f64;16]`
/// column-major contract. All scalars are `f64` (P6-M1-T03: double everywhere).
pub mod math;

/// Placeholder identity so the crate builds as a workspace member.
pub const WORKSTREAM: &str = "P6-GEO";
