//! P4-M2-T02: AP242 PMI re-export — writer + round-trip reader.
//!
//! This module implements the P4 PMI re-export lane against AP242
//! (ISO 10303-242) semantic PMI:
//!
//! - [`writer`] serializes an [`oep_pmi::PmiDocument`] into an AP242
//!   PART 21 file carrying semantic geometric tolerances, datums, datum
//!   reference frames and notes.
//! - [`reader`] re-imports the emitted subset back into
//!   [`oep_pmi::PmiDocument`] so the round-trip can be verified within
//!   tolerance (the "over and back" the card is about).
//!
//! Entity emission follows the real AP242 structure used by the NIST MBE PMI
//! test models (geometric tolerance complex instances, datum systems with
//! compartments, tolerance zones, composite relationships), which is what an
//! independent STEP reader (e.g. `steputils`) parses.
#![deny(missing_docs)]

pub mod reader;
pub mod writer;
