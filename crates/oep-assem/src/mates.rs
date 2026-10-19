//! The assembly workbench constraint (mate) vocabulary.
//!
//! Mates are the user-facing constraint language of the OEP assembly
//! workbench, mapped onto the adopted solver's constraint engine in
//! [`crate::workbench`]. The vocabulary is deliberately small and honest:
//! every mate maps to a solver constraint the engine solves robustly, and the
//! solver ADR (`P7-M3/solver-adr.md`) records what is out of scope today.
//!
//! Mates are serializable in their own documented format
//! (`format = "oep-assem-constraints", version = 1`) so a fixture can carry
//! its constraint set next to the oep-geo model it constrains.

use serde::{Deserialize, Serialize};

/// A reference to one named anchor of one instance.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnchorRef {
    /// Instance id in the assembly.
    pub instance: String,
    /// Anchor name on the instance's part.
    pub anchor: String,
}

/// A single workbench constraint between instances.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Mate {
    /// The instance is fixed in place; its world pose is authoritative.
    Fixed {
        /// Instance id.
        instance: String,
    },
    /// Two anchor points must coincide (world positions equal).
    Coincident {
        /// First anchor.
        a: AnchorRef,
        /// Second anchor.
        b: AnchorRef,
    },
    /// The point anchor lies on the infinite plane of the plane anchor.
    OnPlane {
        /// Point anchor whose origin goes onto the plane.
        point: AnchorRef,
        /// Plane anchor (origin + normal define the plane).
        plane: AnchorRef,
    },
    /// Signed distance from the point anchor to the plane anchor.
    PlaneDistance {
        /// Point anchor.
        point: AnchorRef,
        /// Plane anchor.
        plane: AnchorRef,
        /// Signed distance in document units.
        distance: f64,
    },
    /// The world directions of two anchors are parallel (opposite allowed).
    /// Both anchors must be Z-axis anchors (see [`crate::anchors::Anchor`]).
    Parallel {
        /// First anchor direction.
        a: AnchorRef,
        /// Second anchor direction.
        b: AnchorRef,
    },
}

/// A full constraint set for one assembly, in our own documented format.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConstraintSet {
    /// Format tag, always `"oep-assem-constraints"`.
    pub format: String,
    /// Schema version, currently `1`.
    pub version: u32,
    /// Document unit string (mirrors the oep-geo document).
    pub units: String,
    /// Assembly id this set constrains.
    pub assembly: String,
    /// Ordered mates.
    #[serde(default)]
    pub mates: Vec<Mate>,
}

impl ConstraintSet {
    /// Format tag value.
    pub const FORMAT: &'static str = "oep-assem-constraints";
    /// Schema version value.
    pub const VERSION: u32 = 1;

    /// An empty constraint set for `assembly` in units `units`.
    pub fn new(units: impl Into<String>, assembly: impl Into<String>) -> Self {
        Self {
            format: Self::FORMAT.to_string(),
            version: Self::VERSION,
            units: units.into(),
            assembly: assembly.into(),
            mates: Vec::new(),
        }
    }

    /// Builder: append a mate.
    pub fn with_mate(mut self, mate: Mate) -> Self {
        self.mates.push(mate);
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    fn aref(instance: &str, anchor: &str) -> AnchorRef {
        AnchorRef {
            instance: instance.into(),
            anchor: anchor.into(),
        }
    }

    #[test]
    fn serde_roundtrip() {
        let cs = ConstraintSet::new("mm", "asm-1")
            .with_mate(Mate::Coincident {
                a: aref("i1", "origin"),
                b: aref("i2", "top"),
            })
            .with_mate(Mate::Parallel {
                a: aref("i1", "top"),
                b: aref("i2", "top"),
            });
        let json = cs.to_json_pretty().unwrap();
        let back = ConstraintSet::from_json_pretty(&json).unwrap();
        assert_eq!(cs, back);
        assert!(json.contains("\"format\": \"oep-assem-constraints\""));
        assert!(json.contains("\"kind\": \"coincident\""));
    }

    #[test]
    fn format_and_version_pinned() {
        assert_eq!(ConstraintSet::FORMAT, "oep-assem-constraints");
        assert_eq!(ConstraintSet::VERSION, 1);
    }
}
