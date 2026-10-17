//! [`Feature`]: a modelling operation in a part's design history.
//!
//! The third of the five model classes. This card defines the *record* (id,
//! name, an open-vocabulary operation `kind`, a parameter map, and the bodies
//! the operation read/wrote); deterministic replay of a feature graph is the
//! separate P6-M3-T03 card. `kind` is an open PascalCase vocabulary (example:
//! `Extrude`, `Fillet`, `Hole`, `Revolve`, `Pattern`, `Mirror`) so readers and
//! kernels can negotiate without a closed enum; unknown kinds never crash this
//! crate (tolerant-failure doctrine, P2-M1-T04).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Feature parameters: an ordered (sorted-key) map of arbitrary JSON values.
///
/// `BTreeMap` (not `HashMap`) so serialization is deterministic — a hard
/// requirement of our own documented format.
pub type Parameters = BTreeMap<String, serde_json::Value>;

/// A modelling operation in a part's design history.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Feature {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable feature name (e.g. "mount-hole").
    pub name: String,
    /// Open PascalCase operation vocabulary (e.g. `Extrude`, `Fillet`).
    pub kind: String,
    /// Numeric/other parameters of the operation.
    #[serde(default)]
    pub parameters: Parameters,
    /// Ids of the bodies the operation produced or consumed.
    #[serde(default)]
    pub body_ids: Vec<String>,
}

impl Feature {
    /// A feature with no parameters or body effects yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            parameters: Parameters::new(),
            body_ids: Vec::new(),
        }
    }

    /// Builder: add a parameter (any serde-serializable value is accepted).
    pub fn with_param(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }

    /// Builder: record an affected body id.
    pub fn with_body(mut self, body_id: impl Into<String>) -> Self {
        self.body_ids.push(body_id.into());
        self
    }
}
