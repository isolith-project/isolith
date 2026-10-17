//! The deterministic `oep-mesh` v1 JSON carrier — the artifact the `step2mesh`
//! convert worker emits (P9-M3-T02).
//!
//! A mesh document is the honest faceted result of parsing a STEP file:
//! distinct vertices (f64 triples, insertion order), triangle indices into
//! that vertex list, the axis-aligned bounding box, and the read-lane source
//! report (product name + source file). No geometry is estimated or
//! reconstructed; the values are the observables the reader mapped.
//!
//! Serialization is byte-deterministic: a fixed struct field order, no maps,
//! and serde_json's shortest-round-trip float formatting mean the same input
//! always produces the same bytes — which is what makes the artifact's
//! sha-256 a stable content address for the P8 vault.

use serde::{Deserialize, Serialize};

use crate::convert::ConvertError;
use oep_step::FacetedModel;

/// The canonical `format` header of the mesh document.
pub const FORMAT: &str = "oep-mesh";
/// The current version of the mesh document schema.
pub const VERSION: u64 = 1;

/// Axis-aligned bounding box over the mesh vertices, when any exist.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox3 {
    /// Minimum corner `[x, y, z]`.
    pub min: [f64; 3],
    /// Maximum corner `[x, y, z]`.
    pub max: [f64; 3],
}

/// The wire document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshDoc {
    /// Must equal [`FORMAT`] (`"oep-mesh"`).
    pub format: String,
    /// Must equal [`VERSION`].
    pub version: u64,
    /// The source file name the mesh was derived from (from the job input).
    pub source_file: String,
    /// Product name from the STEP file (falls back to the file stem).
    pub product_name: String,
    /// Distinct vertex count (always `vertices.len()`; kept explicit).
    pub vertex_count: usize,
    /// Triangle count (always `triangles.len()`; kept explicit for readers
    /// that do not want to parse the arrays).
    pub triangle_count: usize,
    /// Axis-aligned bounding box over the vertices; `None` when empty.
    pub bbox: Option<BBox3>,
    /// Distinct vertices in insertion order.
    pub vertices: Vec<[f64; 3]>,
    /// Triangles; each entry indexes into [`Self::vertices`].
    pub triangles: Vec<[u32; 3]>,
}

impl MeshDoc {
    /// Build a mesh document from the P4 read-lane facets (P9-M3-T02).
    pub fn from_facets(facets: &FacetedModel, source_file: &str) -> Self {
        let bbox = facets.bbox.map(|b| BBox3 {
            min: b.min,
            max: b.max,
        });
        MeshDoc {
            format: FORMAT.to_string(),
            version: VERSION,
            source_file: source_file.to_string(),
            product_name: facets.product_name.clone(),
            vertex_count: facets.vertices.len(),
            triangle_count: facets.triangles.len(),
            bbox,
            vertices: facets.vertices.clone(),
            triangles: facets.triangles.clone(),
        }
    }

    /// Encode the document to bytes (deterministic JSON, no trailing
    /// whitespace).
    pub fn encode(&self) -> Result<Vec<u8>, ConvertError> {
        serde_json::to_vec(self).map_err(|e| ConvertError::Serialize(format!("{e}")))
    }
}
