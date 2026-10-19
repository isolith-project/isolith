//! [`Datum`]/[`DatumFrame`]: the datum-reference model of GD&T.
//!
//! A [`Datum`] designates a real feature (by id references into oep-geo
//! bodies/features) as a datum **feature** with a letter label (e.g. `A`).
//! A [`DatumFrame`] is an ordered set of such datums applied to a tolerance:
//! primary (precedence 1) -> secondary (2) -> tertiary (3), each optionally
//! carrying a material-condition modifier ([`MaterialCondition`]).

use serde::{Deserialize, Serialize};

/// Material-condition modifier applied to a datum or tolerance.
///
/// - [`Self::Mmc`]: maximum material condition (`M`, classic "bonus" datum).
/// - [`Self::Lmc`]: least material condition (`L`).
/// - [`Self::Rfs`]: regardless of feature size (`S`, the default).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MaterialCondition {
    /// Maximum material condition (`M`).
    Mmc,
    /// Least material condition (`L`).
    Lmc,
    /// Regardless of feature size (`S`).
    Rfs,
}

/// A datum **feature**: a real feature designated as a datum by letter label.
///
/// References carry no geometry here; `feature_ids`/`body_ids` are id strings
/// resolved by the importer hub (P6-M4-T02) against an oep-geo document.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Datum {
    /// Unique document-wide id.
    pub id: String,
    /// Single-letter datum label (e.g. `A`, `B`, `C`), unique per document.
    pub label: String,
    /// Ids of the oep-geo features (by id) this datum designates.
    #[serde(default)]
    pub feature_ids: Vec<String>,
    /// Ids of the oep-geo bodies (by id) this datum designates.
    #[serde(default)]
    pub body_ids: Vec<String>,
}

impl Datum {
    /// A datum with the given id and label and no feature references yet.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            feature_ids: Vec::new(),
            body_ids: Vec::new(),
        }
    }

    /// Builder: add a designated feature id.
    pub fn with_feature(mut self, feature_id: impl Into<String>) -> Self {
        self.feature_ids.push(feature_id.into());
        self
    }

    /// Builder: add a designated body id.
    pub fn with_body(mut self, body_id: impl Into<String>) -> Self {
        self.body_ids.push(body_id.into());
        self
    }
}

/// One datum in a [`DatumFrame`], at a precedence with an optional modifier.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DatumFrameRef {
    /// Document-wide id of the referenced [`Datum`].
    pub datum_id: String,
    /// Precedence in the frame: 1 = primary, 2 = secondary, 3 = tertiary.
    pub precedence: u8,
    /// Optional material-condition modifier applied to this datum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<MaterialCondition>,
}

impl DatumFrameRef {
    /// A datum frame ref at the given precedence with no modifier.
    pub fn new(datum_id: impl Into<String>, precedence: u8) -> Self {
        Self {
            datum_id: datum_id.into(),
            precedence,
            modifier: None,
        }
    }

    /// Builder: set the material-condition modifier.
    pub fn with_modifier(mut self, modifier: MaterialCondition) -> Self {
        self.modifier = Some(modifier);
        self
    }
}

/// A datum reference frame: the ordered `A | B | C` block of a tolerance.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DatumFrame {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable name, e.g. `A-B-C`.
    pub name: String,
    /// Ordered datum references (primary..tertiary); precedence validated 1..=3.
    #[serde(default)]
    pub refs: Vec<DatumFrameRef>,
}

impl DatumFrame {
    /// A frame with the given id/name and no datum refs yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            refs: Vec::new(),
        }
    }

    /// Builder: append a datum reference.
    pub fn with_ref(mut self, r: DatumFrameRef) -> Self {
        self.refs.push(r);
        self
    }
}
