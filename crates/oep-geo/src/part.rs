//! [`Part`]: an engineering part — the first of the five model classes.
//!
//! A part is a design artifact with an identity (`id`, optional part number),
//! a set of bodies it owns (referenced by id into the document's body table),
//! and the ordered set of features that carried its design history. Geometry
//! is not embedded here; the part is a logical frame over bodies/features.

use serde::{Deserialize, Serialize};

/// An engineering part in an OEP document.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Part {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable part name.
    pub name: String,
    /// Optional part number (PN) for BOM/PDM interop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    /// Ids of the bodies this part owns.
    #[serde(default)]
    pub body_ids: Vec<String>,
    /// Ids of the features that make up this part's history.
    #[serde(default)]
    pub feature_ids: Vec<String>,
}

impl Part {
    /// A part with the given id and name and no bodies/features yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            part_number: None,
            body_ids: Vec::new(),
            feature_ids: Vec::new(),
        }
    }

    /// Builder: set the part number.
    pub fn with_part_number(mut self, part_number: impl Into<String>) -> Self {
        self.part_number = Some(part_number.into());
        self
    }

    /// Builder: add an owned body id.
    pub fn with_body(mut self, body_id: impl Into<String>) -> Self {
        self.body_ids.push(body_id.into());
        self
    }

    /// Builder: add a history feature id.
    pub fn with_feature(mut self, feature_id: impl Into<String>) -> Self {
        self.feature_ids.push(feature_id.into());
        self
    }
}
