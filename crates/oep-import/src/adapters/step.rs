//! P4 adapter — STEP → canonical [`Document`] (reserved seam).
//!
//! `oep-step` is an empty skeleton in wave 1: the STEP AP242/QIF/JT reader is
//! P4-M1-T01 and has NOT started (docs/layout.md: "wave 1 does not start P4").
//! The hub still registers the adapter so [`crate::SourceKind::Step`] is a
//! total, non-panicking part of the dispatch table — the honest, typed outcome
//! for a source whose reader does not exist yet is [`ImportOutcome::Reserved`],
//! never fabricated output.

use crate::{ImportOutcome, SourceKind};

/// Reason emitted by the reserved STEP seam.
pub const RESERVED_REASON: &str =
    "oep-step is a reserved skeleton in wave 1; the STEP reader (P4-M1-T01) has not started";

/// Import STEP bytes. Always returns a typed [`ImportOutcome::Reserved`] —
/// P4's reader does not exist yet and the hub never fabricates geometry.
pub fn import_bytes(_bytes: &[u8]) -> Result<ImportOutcome, crate::ImportError> {
    Ok(ImportOutcome::Reserved {
        source: SourceKind::Step,
        reason: RESERVED_REASON.to_string(),
    })
}
