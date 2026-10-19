//! [`Note`]: free-text model notes (general notes, leader callouts).
//!
//! Notes carry text plus optional feature/body id references and an optional
//! 3D anchor position. They are the free-text PMI layer alongside the
//! structured GD&T in [`crate::tolerance`].

use serde::{Deserialize, Serialize};

/// A free-text note in the PMI model.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Note {
    /// Unique document-wide id.
    pub id: String,
    /// The note text.
    pub text: String,
    /// Ids of the oep-geo features (by id) this note annotates.
    #[serde(default)]
    pub feature_ids: Vec<String>,
    /// Ids of the oep-geo bodies (by id) this note annotates.
    #[serde(default)]
    pub body_ids: Vec<String>,
    /// Optional 3D anchor position `[x, y, z]` in document units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 3]>,
}

impl Note {
    /// A note with the given id and text and no references yet.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            feature_ids: Vec::new(),
            body_ids: Vec::new(),
            position: None,
        }
    }

    /// Builder: add an annotated feature id.
    pub fn with_feature(mut self, feature_id: impl Into<String>) -> Self {
        self.feature_ids.push(feature_id.into());
        self
    }

    /// Builder: add an annotated body id.
    pub fn with_body(mut self, body_id: impl Into<String>) -> Self {
        self.body_ids.push(body_id.into());
        self
    }

    /// Builder: set the 3D anchor position.
    pub fn with_position(mut self, position: [f64; 3]) -> Self {
        self.position = Some(position);
        self
    }
}
