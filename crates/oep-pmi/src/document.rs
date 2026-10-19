//! [`PmiDocument`]: the unit of our own serializable PMI model format.
//!
//! A PMI document is a flat store for the four PMI collections — datums,
//! datum frames, geometric tolerances, notes — and is the object a file in
//! our own format (`format = "oep-pmi"`, `version = 1`) deserializes into.
//! Links (frame -> datum, tolerance -> frame) are id references resolved at
//! [`PmiDocument::validate`] time, never during deserialization (tolerant
//! failure doctrine, P2-M1-T04).

use crate::datum::{Datum, DatumFrame};
use crate::note::Note;
use crate::tolerance::GeometricTolerance;
use crate::validate::ValidationError;
use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying our own PMI format.
pub const FORMAT: &str = "oep-pmi";
/// The current schema version of the format.
pub const VERSION: u32 = 1;

/// A complete OEP PMI document in our own serde format.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PmiDocument {
    /// Format tag — always [`FORMAT`] (`"oep-pmi"`). Required.
    pub format: String,
    /// Schema version — currently [`VERSION`] (`1`). Required.
    pub version: u32,
    /// Document unit of measure (informational string, e.g. `"mm"`).
    pub units: String,
    /// All datums in the document.
    #[serde(default)]
    pub datums: Vec<Datum>,
    /// All datum frames in the document.
    #[serde(default)]
    pub frames: Vec<DatumFrame>,
    /// All geometric tolerances in the document.
    #[serde(default)]
    pub tolerances: Vec<GeometricTolerance>,
    /// All notes in the document.
    #[serde(default)]
    pub notes: Vec<Note>,
}

impl PmiDocument {
    /// A new, empty PMI document in units `units` at the current version.
    pub fn new(units: impl Into<String>) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            units: units.into(),
            datums: Vec::new(),
            frames: Vec::new(),
            tolerances: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// The format tag (`"oep-pmi"`).
    pub fn format(&self) -> &str {
        &self.format
    }

    /// The schema version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The document unit string.
    pub fn units(&self) -> &str {
        &self.units
    }

    /// Append a datum.
    pub fn push_datum(&mut self, datum: Datum) {
        self.datums.push(datum);
    }

    /// Append a datum frame.
    pub fn push_frame(&mut self, frame: DatumFrame) {
        self.frames.push(frame);
    }

    /// Append a geometric tolerance.
    pub fn push_tolerance(&mut self, tolerance: GeometricTolerance) {
        self.tolerances.push(tolerance);
    }

    /// Append a note.
    pub fn push_note(&mut self, note: Note) {
        self.notes.push(note);
    }

    /// Serialize to our canonical pretty JSON (2-space indent, trailing LF).
    ///
    /// Output is deterministic: key order follows struct declaration order
    /// (serde), so the same logical document always serializes to the same
    /// bytes.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Deserialize a PMI document from our JSON format (pretty or compact).
    pub fn from_json_pretty(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Validate structural invariants (see [`ValidationError`] rules).
    pub fn validate(&self) -> Vec<ValidationError> {
        crate::validate::validate_document(self)
    }
}
