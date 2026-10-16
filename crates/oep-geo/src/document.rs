//! [`Document`]: the unit of our own serializable model format.
//!
//! A document is a flat store for the five model classes — part, body,
//! feature, assembly, instance — and is the object a file in our own format
//! (`format = "oep-geo"`, `version = 1`) deserializes into. All links between
//! classes are id references resolved at [`Document::validate`] time, never
//! during deserialization, so a partially-linked document is still loadable
//! (tolerant-failure doctrine).

use crate::assembly::Assembly;
use crate::body::Body;
use crate::feature::Feature;
use crate::part::Part;
use crate::validate::ValidationError;
use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying our own model format.
pub const FORMAT: &str = "oep-geo";
/// The current schema version of the format.
pub const VERSION: u32 = 1;

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

    /// Serialize to our canonical pretty JSON (2-space indent, trailing LF).
    ///
    /// Output is deterministic: key order follows struct declaration order and
    /// every map is a `BTreeMap` (sorted keys).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Deserialize a document from our JSON format (accepts pretty or compact).
    pub fn from_json_pretty(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Validate structural invariants (see [`ValidationError`] rules).
    pub fn validate(&self) -> Vec<ValidationError> {
        crate::validate::validate_document(self)
    }
}
