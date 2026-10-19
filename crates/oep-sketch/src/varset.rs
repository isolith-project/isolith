//! [`VariableSet`]: named scalar parameters driving sketch dimensions.
//!
//! FreeCAD v1 introduced variable sets (named container of user parameters
//! that dimensions/expressions reference) - the card's "like FreeCAD v1
//! y-arsets" semantics target. We implement the same BEHAVIOR in the chosen
//! shell's own model: a [`VariableSet`] is an ordered (sorted-key) collection
//! of `name -> f64` entries; a sketch dimension references an entry by name
//! ([`crate::sketch::DimExpr::Variable`]) and the solved value follows edits.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An error resolving a variable-set reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableError {
    /// The named entry does not exist.
    NotFound(String),
}

impl std::fmt::Display for VariableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableError::NotFound(name) => write!(f, "variable '{name}' is not defined"),
        }
    }
}

impl std::error::Error for VariableError {}

/// A named collection of scalar parameters, like FreeCAD v1 variable sets.
///
/// Stores entries in a `BTreeMap` so serialization is deterministic (the
/// unconditional determinism rule of our own oep-geo format).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableSet {
    /// The variable entries: name -> value.
    #[serde(default)]
    pub variables: BTreeMap<String, f64>,
}

impl VariableSet {
    /// A new empty variable set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set (or overwrite) a variable value.
    pub fn set(&mut self, name: impl Into<String>, value: f64) {
        self.variables.insert(name.into(), value);
    }

    /// Resolve a variable by name, or return a typed not-found error.
    pub fn get(&self, name: &str) -> Result<f64, VariableError> {
        self.variables
            .get(name)
            .copied()
            .ok_or_else(|| VariableError::NotFound(name.to_string()))
    }

    /// The number of defined variables.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// All defined names, sorted (the deterministic iteration order).
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.variables.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_round_trip() {
        let mut v = VariableSet::new();
        v.set("width", 80.0);
        v.set("height", 40.0);
        assert_eq!(v.get("width").unwrap(), 80.0);
        assert_eq!(v.len(), 2);
        v.set("width", 120.0);
        assert_eq!(v.get("width").unwrap(), 120.0);
        assert_eq!(
            v.get("missing"),
            Err(VariableError::NotFound("missing".into()))
        );
    }

    #[test]
    fn serializes_deterministically_sorted() {
        let mut v = VariableSet::new();
        v.set("b", 2.0);
        v.set("a", 1.0);
        let a = serde_json::to_string(&v).unwrap();
        let b = serde_json::to_string(&v).unwrap();
        assert_eq!(a, b);
        assert!(a.contains("\"a\":1.0"));
    }
}
