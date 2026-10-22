//! Document material metadata: a companion format that supplies density.
//!
//! Storage follows the assembly-workbench precedent (`oep-assem-constraints`):
//! metadata about a model that is not part of the five canonical `oep-geo`
//! classes lives in its own deterministic, versioned companion format stored
//! next to the model. Density therefore comes from **document material
//! metadata** without altering the canonical schema (Class 1 additive rule,
//! scope register: "new crates consuming oep-geo; do not alter the canonical
//! model").
//!
//! Format: `format = "oep-analysis-material"`, `version = 1`. A table holds a
//! lookup of solid **body id -> material id**, plus a material registry
//! keyed by id: name and density in grams per cubic centimetre (the common
//! engineering unit; steel ≈ 7.85, aluminium ≈ 2.70). Density is consumed by
//! [`crate::mass`].
//!
//! The canonical length base is the millimetre ([`oep_geo::units`]), so a
//! volume in mm³ converts to cm³ (÷1000) before multiplying by density.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Format tag spoken by [`MaterialTable`].
pub const FORMAT: &str = "oep-analysis-material";
/// Version of the material companion format.
pub const VERSION: u32 = 1;

/// A single material: name + density in grams per cubic centimetre.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Material {
    /// Document-wide material id.
    pub id: String,
    /// Human-readable material name (e.g. "steel-1018").
    pub name: String,
    /// Density in grams per cubic centimetre (g/cm³).
    pub density_g_cm3: f64,
}

/// The material metadata table: a material registry plus a deterministic
/// body-id -> material-id assignment map.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MaterialTable {
    /// Format tag, always [`FORMAT`].
    pub format: String,
    /// Schema version, currently [`VERSION`].
    pub version: u32,
    /// Document unit string (mirrors the oep-geo document).
    pub units: String,
    /// Material registry (id-keyed).
    #[serde(default)]
    pub materials: Vec<Material>,
    /// Body id -> material id assignments.
    #[serde(default)]
    pub body_material: BTreeMap<String, String>,
}

impl MaterialTable {
    /// Format tag value.
    pub const FORMAT_VALUE: &'static str = FORMAT;
    /// Schema version value.
    pub const VERSION_VALUE: u32 = VERSION;

    /// An empty material table for `units` (mirrors the oep-geo document).
    pub fn new(units: impl Into<String>) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            units: units.into(),
            materials: Vec::new(),
            body_material: BTreeMap::new(),
        }
    }

    /// Builder: append a material to the registry.
    pub fn with_material(mut self, material: Material) -> Self {
        self.materials.push(material);
        self
    }

    /// Builder: assign a body to a material.
    pub fn with_assignment(
        mut self,
        body_id: impl Into<String>,
        material_id: impl Into<String>,
    ) -> Self {
        self.body_material
            .insert(body_id.into(), material_id.into());
        self
    }

    /// Density (g/cm³) for a body, resolving its material id from the
    /// assignment map. `None` when the body has no assignment or the assigned
    /// material is not in the registry — never a panic.
    pub fn density_g_cm3_for_body(&self, body_id: &str) -> Option<f64> {
        let material_id = self.body_material.get(body_id)?;
        self.materials
            .iter()
            .find(|m| &m.id == material_id)
            .map(|m| m.density_g_cm3)
    }

    /// Serialize to pretty JSON (deterministic field order, trailing LF).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Deserialize from JSON (pretty or compact).
    pub fn from_json_pretty(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Errors produced when a [`MaterialTable`] is structurally invalid.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialTableError {
    /// The format tag is not [`FORMAT`].
    BadFormat(String),
    /// The version is not [`VERSION`].
    UnsupportedVersion(u32),
    /// A body is assigned a material id that is not in the registry.
    UnresolvedMaterial { body: String, material: String },
}

impl fmt::Display for MaterialTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaterialTableError::BadFormat(fmt) => {
                write!(f, "material table format {fmt:?} is not {FORMAT:?}")
            }
            MaterialTableError::UnsupportedVersion(v) => {
                write!(
                    f,
                    "material table version {v} is not supported (expected {VERSION})"
                )
            }
            MaterialTableError::UnresolvedMaterial { body, material } => {
                write!(f, "body {body:?} is assigned unknown material {material:?}")
            }
        }
    }
}

impl std::error::Error for MaterialTableError {}

impl MaterialTable {
    /// Validate structural invariants: pinned format/version and resolved
    /// material assignments. Returns every violation (never panics).
    pub fn validate(&self) -> Vec<MaterialTableError> {
        let mut errs = Vec::new();
        if self.format != FORMAT {
            errs.push(MaterialTableError::BadFormat(self.format.clone()));
        }
        if self.version != VERSION {
            errs.push(MaterialTableError::UnsupportedVersion(self.version));
        }
        let ids: Vec<&str> = self.materials.iter().map(|m| m.id.as_str()).collect();
        for (body, material) in &self.body_material {
            if !ids.iter().any(|id| id == material) {
                errs.push(MaterialTableError::UnresolvedMaterial {
                    body: body.clone(),
                    material: material.clone(),
                });
            }
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steel_table() -> MaterialTable {
        MaterialTable::new("mm")
            .with_material(Material {
                id: "mat-steel-1018".into(),
                name: "steel-1018".into(),
                density_g_cm3: 7.85,
            })
            .with_assignment("oep-body-bracket", "mat-steel-1018")
    }

    #[test]
    fn format_and_version_pinned() {
        assert_eq!(MaterialTable::FORMAT_VALUE, "oep-analysis-material");
        assert_eq!(MaterialTable::VERSION_VALUE, 1);
    }

    #[test]
    fn density_resolution() {
        let t = steel_table();
        assert_eq!(t.density_g_cm3_for_body("oep-body-bracket"), Some(7.85));
        assert_eq!(t.density_g_cm3_for_body("missing"), None);
    }

    #[test]
    fn serde_roundtrip() {
        let t = steel_table();
        let json = t.to_json_pretty().unwrap();
        let back = MaterialTable::from_json_pretty(&json).unwrap();
        assert_eq!(t, back);
        assert!(json.contains("\"format\": \"oep-analysis-material\""));
    }

    #[test]
    fn validate_catches_unresolved() {
        let bad = MaterialTable::new("mm").with_assignment("b1", "nope");
        assert_eq!(
            bad.validate(),
            vec![MaterialTableError::UnresolvedMaterial {
                body: "b1".into(),
                material: "nope".into(),
            }]
        );
    }
}
