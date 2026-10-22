//! P6-M4-T02 — importer hub (`oep-import`): P2/P3/P4 adapters feed the ONE
//! canonical internal format, `oep_geo::Document`.
//!
//! This crate is the repository's designated composition root: it declares the
//! first cross-crate path deps (docs/layout.md: path deps are defined by the
//! importer hub, P6-M4-T02) and exposes a single, typed, non-panicking import
//! surface over the P2 (CGR), P3 (3DXML) and P4 (STEP) reader crates.
//!
//! Canonical internal format: every successful import yields an
//! [`oep_geo::Document`] (`format = "oep-geo"`, current schema version) that
//! `validate()`s clean and serializes deterministically — ONE model shape for
//! every source, no per-source intermediate formats leaking out of this crate.
//! Adapters use the readers' own clean-room outputs (mesh/rep structures);
//! nothing is ever fabricated: an unparseable source is a typed
//! [`ImportOutcome::Skipped`], and the not-yet-started P4 reader is a typed
//! [`ImportOutcome::Reserved`].
//!
//! Modules:
//! - [`SourceKind`]: format classification (CGR / 3DXML / STEP).
//! - [`ImportOutcome`] / [`ImportReport`] / [`ImportError`]: typed results.
//! - [`import_bytes`]: detection-driven dispatch entry point.
//! - [`adapters`]: the P2/P3/P4 adapters feeding `oep-geo`.
#![deny(missing_docs)]

use std::fmt;

pub mod adapters;

/// Re-export the canonical model so hub consumers never import `oep-geo`
/// directly (one-format contract lives behind this crate's facade).
pub use oep_geo::{
    Assembly, Body, BodyKind, BoundingBox, Document, Feature, Instance, Part, Transform,
};

/// The three source formats an adapter can feed into `oep-geo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// P2 - CGR (Dassault geometry/tessellation container, both eras).
    Cgr,
    /// P3 - 3DXML (ZIP archive or standalone XML model document).
    ThreeDXml,
    /// P4 - STEP (ISO 10303-21 faceted read lane; seam closed P7-M2-T01).
    Step,
}

impl SourceKind {
    /// Classify raw bytes into a source kind by observed signatures.
    ///
    /// - CGR: classic u32 LE == 1 at 0x00 (observed magic, hex notebook
    ///   P1-M2-T01) or the modern `V5_CFV2\0` container magic (P2-M7-T01
    ///   real-material latch).
    /// - 3DXML: ZIP local header `PK\x03\x04` or standalone `<?xm`.
    /// - STEP: `ISO-10303-21` (the exchange-structure header).
    ///
    /// `None` (never an error/panic) for anything else, including short input.
    pub fn detect(bytes: &[u8]) -> Option<SourceKind> {
        // Modern CGR/V5 family container (V5_CFV2\0, spec §3.1).
        if bytes.len() >= 8 && &bytes[..8] == b"V5_CFV2\0" {
            return Some(SourceKind::Cgr);
        }
        if bytes.len() >= 4 {
            let head = [bytes[0], bytes[1], bytes[2], bytes[3]];
            // CGR magic u32 LE == 1.
            if head == [0x01, 0x00, 0x00, 0x00] {
                return Some(SourceKind::Cgr);
            }
            // 3DXML ZIP archive (PK\x03\x04) or standalone XML (<?xm).
            if head == *b"PK\x03\x04" || head == [0x3C, 0x3F, 0x78, 0x6D] {
                return Some(SourceKind::ThreeDXml);
            }
        }
        // STEP: ISO-10303-21 header text near the start.
        const STEP_HEADER: &[u8] = b"ISO-10303-21";
        if bytes.starts_with(STEP_HEADER)
            || (bytes.len() > STEP_HEADER.len() && bytes[..STEP_HEADER.len()] == *STEP_HEADER)
        {
            return Some(SourceKind::Step);
        }
        None
    }

    /// Short stable name (`"cgr"`, `"3dxml"`, `"step"`).
    pub fn name(self) -> &'static str {
        match self {
            SourceKind::Cgr => "cgr",
            SourceKind::ThreeDXml => "3dxml",
            SourceKind::Step => "step",
        }
    }
}

/// A structured summary of what an import produced (diagnostics + verification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// Source format that was imported.
    pub source: SourceKind,
    /// Number of parts in the resulting canonical document.
    pub parts: usize,
    /// Number of bodies.
    pub bodies: usize,
    /// Number of assemblies.
    pub assemblies: usize,
    /// Number of instances (across all assemblies).
    pub instances: usize,
    /// Human-readable warnings (e.g. a rep had no member, a link was dangling).
    pub warnings: Vec<String>,
}

impl ImportReport {
    /// An empty report for `source`.
    pub fn new(source: SourceKind) -> Self {
        Self {
            source,
            parts: 0,
            bodies: 0,
            assemblies: 0,
            instances: 0,
            warnings: Vec::new(),
        }
    }
}

/// The typed result of an import.
///
/// `Imported` carries the whole canonical model (large) next to the two small
/// outcome variants; boxing just `document` would churn every caller for no
/// runtime benefit, so the size lint is waived deliberately.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ImportOutcome {
    /// A canonical `oep_geo::Document` plus its import report.
    Imported {
        /// The canonical model (one format for every source).
        document: Document,
        /// Structured statistics about the import.
        report: ImportReport,
    },
    /// The source was recognized but the payload yielded no importable
    /// geometry under the tested clean-room hypotheses (classified skip).
    Skipped {
        /// The source kind.
        source: SourceKind,
        /// Human-readable reason naming the measured evidence.
        reason: String,
    },
    /// The source kind is a registered seam whose reader has not started yet.
    Reserved {
        /// The source kind.
        source: SourceKind,
        /// Human-readable reason.
        reason: String,
    },
}

/// Typed, non-panicking hub errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// No adapter is registered for the requested [`SourceKind`].
    NoAdapterForSource {
        /// The unhandled source kind name.
        source: &'static str,
    },
    /// An adapter's reader reported a failure that is not a clean skip.
    Reader {
        /// Source kind that failed.
        source: &'static str,
        /// Underlying message.
        message: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NoAdapterForSource { source } => {
                write!(f, "no adapter registered for source kind {source:?}")
            }
            ImportError::Reader { source, message } => {
                write!(f, "reader error for {source:?}: {message}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Detection-driven import: classify `bytes` and dispatch to the right
/// adapter in one call.
///
/// Returns `Err(ImportError::NoAdapterForSource { .. })` when a [`SourceKind`]
/// is requested but unhandled — a typed error, never a panic. `None`-detect
/// is a `NoAdapterForSource` on the detector's "unknown" classification.
pub fn import_bytes(kind: SourceKind, bytes: &[u8]) -> Result<ImportOutcome, ImportError> {
    match kind {
        SourceKind::Cgr => adapters::cgr::import_bytes(bytes),
        SourceKind::ThreeDXml => adapters::xml3d::import_bytes(bytes),
        SourceKind::Step => adapters::step::import_bytes(bytes),
    }
}
