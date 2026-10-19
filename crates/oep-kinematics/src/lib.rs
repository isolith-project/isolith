//! P7-M3-T02 -- own DMU-like mates / kinematics semantics over `oep-geo`.
//!
//! This crate is the clean-room mates/kinematics demo lane of the P7 modeler:
//! it re-implements DMU-like assembly semantics (revolute, prismatic,
//! spherical and rigid joints, driven commands, closed-loop position analysis)
//! on top of the canonical `oep-geo` assembly model (instances + transforms +
//! the vendored math core), using **our own** constraint solver and **no
//! vendor libraries** (see the clean-room invariant test in
//! `tests/cleanroom.rs`).
//!
//! Scope note: this is a *semantics wedge and recorded demo*, not the
//! production assembly constraint solver. P7-M3-T01 owns the production
//! solver ADR (per the ratified P7-M1-T01 record: "adopt an OSS solver lib,
//! do not rebuild"); the solver in [`solver`] is a deliberately dependency-free
//! reference that makes the mate semantics executable, testable and recordable
//! until that adopted solver lands behind the same [`Mechanism`] seam.
//!
//! Modules:
//! - [`pose`]: rigid pose ([`Pose`]) with `oep-geo` transform interop and
//!   Rodrigues rotation-vector helpers.
//! - [`mate`]: the DMU-like mate/joint model ([`Mate`], [`Revolute`],
//!   [`Prismatic`], [`Spherical`], [`Rigid`]) plus residual evaluation.
//! - [`solver`]: the own damped Gauss-Newton least-squares constraint solver
//!   (finite-difference Jacobian, normal equations, Gaussian elimination),
//!   with a mechanism DOF rank report.
//! - [`mechanism`]: a [`Mechanism`] of movable instances + mates over an
//!   `oep-geo` document, with driver overrides and per-frame solve/trace.
//! - [`demo`]: the crank-slider demo fixture (verified against an analytic
//!   ground truth), frame recording and an own software renderer.
#![deny(missing_docs)]

pub mod demo;
pub mod mate;
pub mod mechanism;
pub mod pose;
pub mod raster;
pub mod solver;

pub use mate::{Mate, Prismatic, Revolute, Rigid, Spherical};
pub use mechanism::Mechanism;
pub use pose::Pose;

/// Identity of the fixed inertial ground frame (world coordinates).
pub const GROUND: &str = "~ground~";

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P7-M3-T02";
