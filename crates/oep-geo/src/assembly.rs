//! [`Assembly`] and [`Instance`]: the product-structure classes.
//!
//! An assembly is a tree frame; every child is an [`Instance`] that places a
//! part (or a nested sub-assembly) via a 4x4 [`crate::Transform`]. Both target
//! types are *referenced by id*, so the same part can be instanced many times
//! with different placements — the classic CAD pattern this schema models.

use crate::Transform;
use serde::{Deserialize, Serialize};

/// What an [`Instance`] points at: a part or a nested sub-assembly.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InstanceSource {
    /// The instance places the part with this document-wide id.
    Part {
        /// Referenced part id.
        id: String,
    },
    /// The instance places the sub-assembly with this document-wide id.
    #[serde(rename = "sub-assembly")]
    Assembly {
        /// Referenced assembly id.
        id: String,
    },
}

impl InstanceSource {
    /// The referenced part or assembly id.
    pub fn referenced_id(&self) -> &str {
        match self {
            InstanceSource::Part { id } | InstanceSource::Assembly { id } => id,
        }
    }

    /// True when the source is a sub-assembly (used to walk the instance graph).
    pub fn is_assembly(&self) -> bool {
        matches!(self, InstanceSource::Assembly { .. })
    }
}

/// A single placement of a part or sub-assembly inside an assembly.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Instance {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable instance name (e.g. "F1").
    pub name: String,
    /// The referenced part or sub-assembly (exactly one, checked by validate).
    pub source: InstanceSource,
    /// Placement transform in the parent frame (identity by default).
    #[serde(default = "Transform::identity")]
    pub transform: Transform,
    /// Visibility in views (present but complicates nothing: always serialized).
    #[serde(default = "default_visible")]
    pub visible: bool,
}

fn default_visible() -> bool {
    true
}

impl Instance {
    /// An instance named `name` of `source` at the identity transform, visible.
    pub fn new(id: impl Into<String>, name: impl Into<String>, source: InstanceSource) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source,
            transform: Transform::identity(),
            visible: true,
        }
    }

    /// Builder: replace the placement transform.
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Builder: set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// An assembly frame: a named, ordered list of child [`Instance`]s.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Assembly {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable assembly name.
    pub name: String,
    /// Ordered child instances (sub-assemblies are allowed; cycles invalid).
    #[serde(default)]
    pub instances: Vec<Instance>,
}

impl Assembly {
    /// An empty assembly with the given id and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            instances: Vec::new(),
        }
    }

    /// Builder: append a child instance.
    pub fn with_instance(mut self, instance: Instance) -> Self {
        self.instances.push(instance);
        self
    }
}
