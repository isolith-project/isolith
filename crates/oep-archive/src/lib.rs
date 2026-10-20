//! oep-archive -- P10-M3-T02: retention/archive policy + export-all interop
//! escape hatch.
//!
//! This crate is the "take your data out" lane of Isolith / Open
//! Engineering Platform. It composes the platform's own readers and exporters
//! (P2 CGR + glTF, P3 3DXML, P4 STEP, P6 oep-geo, P8 vault) into one
//! documented, neutral archive a customer can always walk out with:
//!
//! - [`export::export_all`] walks a P8 [`oep_pdm::store::VaultStore`] and
//!   writes an archive of neutral geometry (STEP / glTF / STL / 3DXML) plus a
//!   JSON vault metadata dump, a retention-policy snapshot, and a sha-256
//!   integrity manifest ([`schema::ArchiveManifest`]).
//! - [`import::verify_import`] proves the archive re-imports: it validates the
//!   metadata schema, rebuilds a vault from it, and re-imports every neutral
//!   STEP geometry file back into an oep-geo [`Document`], writing an
//!   [`schema::ImportReport`].
//! - [`retention::RetentionPolicy`] documents the retention/archive defaults
//!   (released gold retained indefinitely; working/under_review/obsolete are
//!   review horizons, never auto-delete timers).
//!
//! No data in the archive depends on a proprietary lock-in format: the
//! `geometry/` tree is neutral interchange only, `model/` carries the open
//! documented oep-geo JSON for fidelity, and `raw/` preserves the original
//! vault bytes separately (clearly marked, not the portability guarantee).
#![deny(missing_docs)]

pub mod error;
pub mod export;
pub mod import;
pub mod retention;
pub mod schema;
pub mod stl;

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P10-ARCHIVE";

pub use error::ArchiveError;
pub use export::{export_all, FileKind, FlatMesh};
pub use import::verify_import;
pub use retention::RetentionPolicy;
pub use schema::{ArchiveManifest, ImportReport, VaultExport};
