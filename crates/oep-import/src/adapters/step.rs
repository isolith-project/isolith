//! P4 adapter - STEP → canonical [`Document`].
//!
//! P7-M2-T01 closes the reserved seam: `oep-step` now has a real read lane
//! (P4-M1-T01, `parse_to_document` over the wrapped OSS STEP library), so the
//! workbench's STEP load button travels through the ONE canonical hub surface
//! like every other source. The adapter still respects the tolerant-failure /
//! never-fabricated doctrine: a payload that is not STEP, or that the reader
//! rejects or classifies as geometry-less, is a typed
//! [`ImportOutcome::Skipped`] with the measured reason, never a panic.

use crate::{ImportError, ImportOutcome, ImportReport, SourceKind};

/// The ISO 10303-21 exchange-structure header marker.
const STEP_HEADER: &[u8] = b"ISO-10303-21;";

/// Import STEP bytes through the P4 read lane into a canonical document.
///
/// Any input yields a typed outcome (Imported / Skipped), never a panic.
pub fn import_bytes(bytes: &[u8]) -> Result<ImportOutcome, ImportError> {
    if !bytes.starts_with(STEP_HEADER) {
        return Ok(ImportOutcome::Skipped {
            source: SourceKind::Step,
            reason: "not an ISO 10303-21 STEP physical file (missing header marker)".to_string(),
        });
    }
    match oep_step::parse_to_document(bytes) {
        Ok((document, _report)) => {
            let instances = document.assemblies.iter().map(|a| a.instances.len()).sum();
            let report = ImportReport {
                source: SourceKind::Step,
                parts: document.parts.len(),
                bodies: document.bodies.len(),
                assemblies: document.assemblies.len(),
                instances,
                warnings: vec![],
            };
            Ok(ImportOutcome::Imported { document, report })
        }
        Err(oep_step::Error::NotStep) => Ok(ImportOutcome::Skipped {
            source: SourceKind::Step,
            reason: "STEP read lane: bytes lack the ISO 10303-21 header".to_string(),
        }),
        Err(oep_step::Error::NoFacetedGeometry(reason)) => Ok(ImportOutcome::Skipped {
            source: SourceKind::Step,
            reason: format!("STEP read lane: no faceted geometry mapped ({reason})"),
        }),
        Err(e) => Ok(ImportOutcome::Skipped {
            source: SourceKind::Step,
            reason: format!("STEP read lane classified: {e}"),
        }),
    }
}
