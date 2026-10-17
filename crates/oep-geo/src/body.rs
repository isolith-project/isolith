//! [`Body`]: an independent lump of material in an OEP document.
//!
//! A body is the second of the five model classes and owns no geometry yet —
//! the B-Rep/geometry kernel slots arrive at P6-M2/M3. This card reserves the
//! schema record: identity, kind, an optional axis-aligned bounding box, and
//! the set of features that produced it (history linkage kept for P6-M3-T03).

use serde::{Deserialize, Serialize};

/// The coarse classification of a [`Body`]'s geometry.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    /// Volumetric body (enclose a 3D region).
    Solid,
    /// Single/sheet surface body (2D-ish shell, open boundaries allowed).
    Sheet,
    /// Wire/curve body (1D, no surfaces).
    Wire,
}

/// An axis-aligned bounding box (`min`/`max` corners, inclusive).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Minimum corner `[x, y, z]`.
    pub min: [f64; 3],
    /// Maximum corner `[x, y, z]`.
    pub max: [f64; 3],
}

impl BoundingBox {
    /// A box spanning `min`..=`max` (valid when `min[i] <= max[i]` on all axes).
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Self {
        Self { min, max }
    }

    /// True when the box is well-formed and empty-degenerate is allowed:
    /// every `min` component is finite and `min[i] <= max[i]` on all axes.
    pub fn is_valid(&self) -> bool {
        (0..3).all(|i| {
            self.min[i].is_finite() && self.max[i].is_finite() && self.min[i] <= self.max[i]
        })
    }
}

/// An independent body in the document.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Body {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable body name.
    pub name: String,
    /// Solids vs sheets vs wires.
    pub kind: BodyKind,
    /// Optional axis-aligned bounding box (informational; geometry comes P6-M2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<BoundingBox>,
    /// Ids of the features that produced this body (may be empty).
    #[serde(default)]
    pub feature_ids: Vec<String>,
}

impl Body {
    /// A named body of the given kind with no features yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: BodyKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            bbox: None,
            feature_ids: Vec::new(),
        }
    }

    /// Builder: attach a bounding box.
    pub fn with_bbox(mut self, bbox: BoundingBox) -> Self {
        self.bbox = Some(bbox);
        self
    }

    /// Builder: record a producing feature id.
    pub fn with_feature(mut self, feature_id: impl Into<String>) -> Self {
        self.feature_ids.push(feature_id.into());
        self
    }
}
