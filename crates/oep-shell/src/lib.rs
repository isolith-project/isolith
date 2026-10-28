//! P7 -- OEP modeler shell (the adopted thin own shell, P7-M1-T01).
//!
//! The P7-M1 open-modeler-shell decision ratifies a thin own Rust shell
//! around Open CASCADE: one canonical model (oep-geo), one renderer (the
//! wgpu preview from P6-M5), and the "workbench" as the shell's own
//! panel/command surface. This crate is that shell. P7-M2-T01 delivers the
//! first workbench, the OEP interchange workbench:
//!
//! - buttons/commands for CGR / 3DXML / STEP / PDM load and save
//!   ([`workbench::WorkbenchAction`] / [`workbench::Workbench`]),
//! - every load goes through the importer hub (`oep-import`) so P2/P3/P4
//!   readers feed the ONE canonical `oep_geo::Document`,
//! - PDM load/save run against the P8 API contract (`oep-pdm::PdmClient`),
//!   currently satisfied by the in-memory local stub (the "save path stubbed
//!   against the PDM API contract" named by the acceptance),
//! - interchange save lanes that have no writer yet answer with a typed,
//!   honest `InterchangeSavePending` instead of fabricated output.
//!
//! Command-line parity is native (P7-M5-T01): the shell surface is a
//! machine-callable command set, demonstrated by the `workbench` example
//! binary that drives the same [`workbench::Workbench`] the panel will later
//! host.
//!
//! The P7-M5-T02 help surface: every command is described once in
//! [`commands`] (the metadata single source of truth); the `help` / `help`
//! per-command docs, the machine-readable JSON index and the committed
//! markdown help index are all generated from it and drift-guarded by test.
#![deny(missing_docs)]

/// The help-surface command metadata and generated help index (P7-M5-T02).
pub mod commands;

/// The OEP interchange workbench command surface.
pub mod workbench;

/// The interactive editing surface (INTERACTIVE-SHELL): the editor session
/// that turns oep-shell + oep-viewer into a mouse-grade editor (sketch,
/// constraints, variable set, extrude/revolve, live recompute, undo/redo,
/// viewport and STEP export). See [`editor`] for the surface contract.
pub mod editor;

/// The interactive assembly editing surface (EDITOR-ASSEM-EDIT): create,
/// edit, delete mates in the viewport against the live solver.
pub mod assem;

/// Self-contained SHA-256 for content-hashing rendered frames and traces
/// (std-only, so the surface needs no third-party crate on this lane).
pub mod sha256;

/// The startup update-check surface (decision #20): the workbench calls
/// [`update::run_startup_check`] once at launch and renders a non-modal
/// notice when an update is available. Advisory only.
pub mod update;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P7-SHELL";

/// The interchange workbench and its types.
pub use workbench::{DocumentStats, Interchange, Workbench, WorkbenchAction, WorkbenchOutcome};
