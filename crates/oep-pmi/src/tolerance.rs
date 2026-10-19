//! [`GeometricTolerance`] and [`Characteristic`]: the GD&T model.
//!
//! A geometric tolerance (feature control frame) names one of the closed
//! 14-value ASME Y14.5 / ISO 1101 [`Characteristic`]s, a tolerance value (with
//! an optional second value for composite/profile zones), an optional diameter
//! zone flag, an optional material-condition modifier, an optional reference
//! to a [`crate::DatumFrame`], and the feature/body id references the
//! tolerance applies to.

use crate::datum::MaterialCondition;
use serde::{Deserialize, Serialize};

/// The closed set of ASME Y14.5 / ISO 1101 geometric characteristics.
///
/// Serialized kebab-case: `profile-of-line`, `circular-runout`, etc.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Characteristic {
    /// Straightness (form).
    Straightness,
    /// Flatness (form).
    Flatness,
    /// Circularity / roundness (form).
    Circularity,
    /// Cylindricity (form).
    Cylindricity,
    /// Profile of a line (profile).
    ProfileOfLine,
    /// Profile of a surface (profile).
    ProfileOfSurface,
    /// Angularity (orientation).
    Angularity,
    /// Perpendicularity (orientation).
    Perpendicularity,
    /// Parallelism (orientation).
    Parallelism,
    /// Position (location).
    Position,
    /// Concentricity (location).
    Concentricity,
    /// Symmetry (location).
    Symmetry,
    /// Circular runout (runout).
    CircularRunout,
    /// Total runout (runout).
    TotalRunout,
}

/// A GD&T feature control frame applied to referenced features.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GeometricTolerance {
    /// Unique document-wide id.
    pub id: String,
    /// The geometric characteristic (closed 14-value set).
    pub characteristic: Characteristic,
    /// Primary tolerance value, in document units. Finite and non-negative.
    pub value: f64,
    /// True when the zone is a diameter (the value is preceded by `Ø`).
    pub diameter_zone: bool,
    /// Optional material-condition modifier on the toleranced feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_condition: Option<MaterialCondition>,
    /// Optional datum frame applied to this tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<String>,
    /// Optional second tolerance value (composite/profile zone), finite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_value: Option<f64>,
    /// Ids of the oep-geo features (by id) this tolerance applies to.
    #[serde(default)]
    pub feature_ids: Vec<String>,
    /// Ids of the oep-geo bodies (by id) this tolerance applies to.
    #[serde(default)]
    pub body_ids: Vec<String>,
    /// Optional callout/explanation note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl GeometricTolerance {
    /// A tolerance with the given id/characteristic/value and no extras.
    pub fn new(id: impl Into<String>, characteristic: Characteristic, value: f64) -> Self {
        Self {
            id: id.into(),
            characteristic,
            value,
            diameter_zone: false,
            material_condition: None,
            frame_id: None,
            second_value: None,
            feature_ids: Vec::new(),
            body_ids: Vec::new(),
            note: None,
        }
    }

    /// Builder: set the diameter-zone flag.
    pub fn with_diameter_zone(mut self, yes: bool) -> Self {
        self.diameter_zone = yes;
        self
    }

    /// Builder: set the material-condition modifier.
    pub fn with_material_condition(mut self, mc: MaterialCondition) -> Self {
        self.material_condition = Some(mc);
        self
    }

    /// Builder: attach a datum frame by id.
    pub fn with_frame(mut self, frame_id: impl Into<String>) -> Self {
        self.frame_id = Some(frame_id.into());
        self
    }

    /// Builder: set the optional second tolerance value.
    pub fn with_second_value(mut self, second: f64) -> Self {
        self.second_value = Some(second);
        self
    }

    /// Builder: add an applied feature id.
    pub fn with_feature(mut self, feature_id: impl Into<String>) -> Self {
        self.feature_ids.push(feature_id.into());
        self
    }

    /// Builder: add an applied body id.
    pub fn with_body(mut self, body_id: impl Into<String>) -> Self {
        self.body_ids.push(body_id.into());
        self
    }

    /// Builder: set the callout note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}
