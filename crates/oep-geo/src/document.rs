//! [`Document`]: the unit of our own serializable model format.
//!
//! A document is a flat store for the six model classes — part, body,
//! feature, assembly, instance, drawing — and is the object a file in our own
//! format (`format = "oep-geo"`, `version = 2`) deserializes into. All links
//! between classes are id references resolved at [`Document::validate`] time,
//! never during deserialization, so a partially-linked document is still
//! loadable (tolerant-failure doctrine).
//!
//! ## Schema versioning (Class 2 extension, P6-M1-T03 discipline)
//!
//! `version = 1` shipped the original five classes. EDITOR-2D-DRAFTING
//! extends the model with the [`Drawing`] class (drafting workbench sheets /
//! views / dimensions) as a **consumer-additive** change: the `drawings`
//! collection defaults empty, so a v1 payload deserializes unchanged and
//! [`Document::from_json_pretty`] promotes it to the current version in place
//! (v1 loads, v2 writes — the documented migration; see
//! `docs/geometry-drawing-schema.md`).

use crate::assembly::Assembly;
use crate::body::Body;
use crate::drawing::Drawing;
use crate::feature::Feature;
use crate::part::Part;
use crate::validate::ValidationError;
use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying our own model format.
pub const FORMAT: &str = "oep-geo";
/// The current schema version of the format.
///
/// v1: the original five classes. v2 (Class 2 extension): adds the
/// [`Drawing`] class. Bumped with a tolerant migration — see module docs.
pub const VERSION: u32 = 2;

/// A complete OEP geometry document in our own serde format.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Document {
    /// Format tag — always [`FORMAT`] (`"oep-geo"`). Required.
    pub format: String,
    /// Schema version — currently [`VERSION`] (`1`). Required.
    pub version: u32,
    /// Document unit of measure (informational string, e.g. `"mm"`);
    /// the tolerance/units discipline model is P6-M1-T03.
    pub units: String,
    /// All parts in the document.
    #[serde(default)]
    pub parts: Vec<Part>,
    /// All bodies in the document.
    #[serde(default)]
    pub bodies: Vec<Body>,
    /// All features in the document.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// All assemblies in the document.
    #[serde(default)]
    pub assemblies: Vec<Assembly>,
    /// All drawings in the document (Class 2 extension, schema v2; empty for
    /// documents authored before drafting existed).
    #[serde(default)]
    pub drawings: Vec<Drawing>,
}

impl Document {
    /// A new, empty document in units `units` at the current schema version.
    pub fn new(units: impl Into<String>) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            units: units.into(),
            parts: Vec::new(),
            bodies: Vec::new(),
            features: Vec::new(),
            assemblies: Vec::new(),
            drawings: Vec::new(),
        }
    }

    /// The format tag (`"oep-geo"`).
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

    /// Append a part.
    pub fn push_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    /// Append a body.
    pub fn push_body(&mut self, body: Body) {
        self.bodies.push(body);
    }

    /// Append a feature.
    pub fn push_feature(&mut self, feature: Feature) {
        self.features.push(feature);
    }

    /// Append an assembly.
    pub fn push_assembly(&mut self, assembly: Assembly) {
        self.assemblies.push(assembly);
    }

    /// Append a drawing.
    pub fn push_drawing(&mut self, drawing: Drawing) {
        self.drawings.push(drawing);
    }

    /// Promote a parsed document to the current schema version (migration).
    ///
    /// Schema v1 had no `drawings` collection; a v1 payload deserializes with
    /// `drawings` defaulted to empty and a `version` of `1`. `migrate` stamps
    /// the current version and materializes anything an older schema omitted,
    /// so a v1 file loads, validates, and re-serializes as v2 — the P6-M1-T03
    /// migration discipline: tolerant read of older versions, forward write of
    /// the current one.
    pub fn migrate(mut self) -> Self {
        self.version = VERSION;
        self
    }

    /// Serialize to our canonical pretty JSON (2-space indent, trailing LF).
    ///
    /// Output is deterministic: key order follows struct declaration order and
    /// every map is a `BTreeMap` (sorted keys).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Deserialize a document from our JSON format (accepts pretty or compact),
    /// promoting older schema versions to the current one in place.
    pub fn from_json_pretty(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text).map(Self::migrate)
    }

    /// Validate structural invariants (see [`ValidationError`] rules).
    pub fn validate(&self) -> Vec<ValidationError> {
        crate::validate::validate_document(self)
    }
}
