//! P2 adapter — CGR → canonical [`Document`].
//!
//! The CGR reader's `oep_cgr::mesh::Mesh` is the clean-room internal mesh
//! carrier (vertices / indices / triangles, P2-M2-T01). This adapter maps it
//! into the canonical `oep_geo::Document`:
//!
//! - a deterministic structural map [`from_mesh`]: one `Part` owning one
//!   `Solid` `Body` whose `BoundingBox` is computed from the mesh vertices
//!   (bbox = exact axis-aligned min/max over every vertex — a real observable,
//!   not an estimate);
//! - a real-bytes entry [`import_bytes`] that runs `oep_cgr::mesh::extract`
//!   (tolerant-failure doctrine: classified skip, never a panic, never
//!   fabricated geometry). When the extractor returns a structurally
//!   self-consistent mesh, `from_mesh` produces the canonical document;
//!   otherwise the outcome is a typed [`ImportOutcome::Skipped`].

use crate::{ImportError, ImportOutcome, ImportReport, SourceKind};
use oep_cgr::mesh::MeshError;
use oep_geo::{Body, BodyKind, BoundingBox, Document, Part};

/// Re-export the CGR internal mesh type as the adapter's mesh carrier.
pub use oep_cgr::mesh::Mesh as CgrMesh;

/// Compute the exact axis-aligned bounding box over every mesh vertex.
/// Returns `None` when the mesh has no vertices (nothing to bound).
fn mesh_bbox(mesh: &CgrMesh) -> Option<BoundingBox> {
    let mut iter = mesh.vertices().iter();
    let first = *iter.next()?;
    let mut min = first;
    let mut max = first;
    for &v in iter {
        for i in 0..3 {
            if v[i] < min[i] {
                min[i] = v[i];
            }
            if v[i] > max[i] {
                max[i] = v[i];
            }
        }
    }
    Some(BoundingBox::new(
        [min[0] as f64, min[1] as f64, min[2] as f64],
        [max[0] as f64, max[1] as f64, max[2] as f64],
    ))
}

/// Deterministic canonical mapping: one readable mesh → a `Document` with one
/// part owning one solid body (bbox from the mesh vertices).
///
/// This is format-independent and fully deterministic (the mesh type alone,
/// per P2-M2-T01); it is the P2 adapter's canonical core.
pub fn from_mesh(mesh: &CgrMesh) -> Document {
    let mut doc = Document::new("mm");
    let body_id = "oep-body-cgr-1".to_string();
    let body = Body::new(&body_id, "cgr-mesh-body", BodyKind::Solid)
        .with_bbox(mesh_bbox(mesh).unwrap_or_else(|| BoundingBox::new([0.0; 3], [0.0; 3])))
        .with_feature("oep-feature-cgr-import");
    let part_id = "oep-part-cgr-1".to_string();
    let part = Part::new(&part_id, "cgr-mesh-part")
        .with_body(&body_id)
        .with_feature("oep-feature-cgr-import");
    doc.push_feature(
        oep_geo::Feature::new("oep-feature-cgr-import", "import-mesh", "ImportedMesh")
            .with_param("triangle_count", mesh.triangles().len() as f64)
            .with_param("vertex_count", mesh.vertices().len() as f64)
            .with_body(&body_id),
    );
    doc.push_body(body);
    doc.push_part(part);
    doc
}

/// Import raw bytes through the CGR reader (`oep_cgr::mesh::extract`).
///
/// Tolerant-failure contract: any CGR payload yields either a canonical
/// document (when a structurally self-consistent triangle stream is found) or
/// a typed [`ImportOutcome::Skipped`] with the classified reason. Never
/// panics, never invents geometry.
pub fn import_bytes(bytes: &[u8]) -> Result<ImportOutcome, ImportError> {
    match oep_cgr::mesh::extract(bytes) {
        Ok(mesh) => {
            let document = from_mesh(&mesh);
            let report = ImportReport {
                source: SourceKind::Cgr,
                parts: document.parts.len(),
                bodies: document.bodies.len(),
                assemblies: 0,
                instances: 0,
                warnings: vec![],
            };
            Ok(ImportOutcome::Imported { document, report })
        }
        Err(MeshError::NotCgrPreamble) => Ok(ImportOutcome::Skipped {
            source: SourceKind::Cgr,
            reason:
                "missing observed CGR header spine (magic 01 00 00 00 / zero preamble / template)"
                    .to_string(),
        }),
        Err(MeshError::MeshNotFound(detail)) => Ok(ImportOutcome::Skipped {
            source: SourceKind::Cgr,
            reason: format!("no self-consistent triangle stream found ({detail})"),
        }),
        Err(e) => Err(ImportError::Reader {
            source: SourceKind::Cgr.name(),
            message: e.to_string(),
        }),
    }
}
