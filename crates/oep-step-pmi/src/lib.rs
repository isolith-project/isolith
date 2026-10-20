//! P4-M2-T01: STEP AP242 semantic PMI import into the `oep_pmi` model.
//!
//! The P4 STEP/QIF/JT interop lane must import semantic PMI (GD&T geometric
//! tolerances, datums, datum reference frames) into the internal PMI model
//! ([`oep_pmi::PmiDocument`], schema from P6-M1-T02). This crate is the P4-M2
//! wedge: it reads ISO 10303-21 (STEP part 21, AP242) bytes through the
//! vendor-neutral seam of an OSS STEP library and maps the semantic PMI onto
//! the oep-pmi model.
//!
//! Seam doctrine (P4-M1 ADR, `workstreams/step-interop/library-adr/step-library-adr.md`): the wrapped OSS
//! library's types live ONLY inside this crate's private walk
//! ([`import`] module) and never appear on the public surface. The public API
//! exposes only oep-pmi types and coined neutral types ([`PmiImport`],
//! [`PmiImportReport`], [`DroppedItem`], [`Error`]).
//!
//! Honest absence: a STEP file with no PMI is NOT an error by default;
//! [`import`] returns a valid empty [`PmiDocument`] whose [`PmiImportReport`]
//! has `pmi_present == false` and zero counts, so callers can distinguish
//! "parsed, no PMI" from "parsed, PMI mapped". [`import_strict`] turns that
//! honest absence into a typed [`Error::NoPmi`] for callers that require PMI.
#![deny(missing_docs)]

pub mod error;
mod import;

pub use error::Error;
pub use import::{import, import_path, import_strict, DroppedItem, PmiImport, PmiImportReport};
pub use oep_pmi::PmiDocument;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P4-STEP-PMI";
