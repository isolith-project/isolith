//! P2 adapter - CGR → canonical [`Document`].
//!
//! The CGR reader's `oep_cgr::mesh::Mesh` is the clean-room internal mesh
//! carrier (vertices / indices / triangles, P2-M2-T01). This adapter maps it
//! into the canonical `oep_geo::Document`:
//!
//! - a deterministic structural map [`from_mesh`]: one `Part` owning one
//!   `Solid` `Body` whose `BoundingBox` is computed from the mesh vertices
//!   (bbox = exact axis-aligned min/max over every vertex - a real observable,
//!   not an estimate);
//! - a real-bytes entry [`import_bytes`] that runs the old-era extractor
//!   `oep_cgr::mesh::extract` AND the modern-era latch
//!   `oep_cgr::modern::extract_modern` (P2-M7-T01) on `V5_CFV2` containers,
//!   under the tolerant-failure doctrine: classified skip, never a panic,
//!   never fabricated geometry. A structurally self-consistent mesh maps to a
//!   canonical document; otherwise the outcome is a typed
//!   [`ImportOutcome::Skipped`].
//!
//! P7-M2-T01 wiring: the workbench's CGR load button calls this adapter for
//! both eras, so the real latched modern mesh (NIST STC-06, golden counts
//! records=18 / triangle=1389 / index=2003 / vertex=1037 / max_index=1990)
//! travels through the ONE canonical hub surface.

use crate::{ImportError, ImportOutcome, ImportReport, SourceKind};
use oep_cgr::mesh::MeshError;
use oep_cgr::modern::{extract_modern, ModernCounts, ModernOutcome};
use oep_geo::{Body, BodyKind, BoundingBox, Document, Part};

/// Re-export the CGR internal mesh type as the adapter's mesh carrier.
pub use oep_cgr::mesh::Mesh as CgrMesh;

/// The observed modern CATIA container magic (spec §3.1).
const OUTER_MAGIC: [u8; 8] = *b"V5_CFV2\0";

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

/// Build the canonical one-part-one-solid Document for a mesh, recording
/// `triangle_count` / `vertex_count` on the import feature.
fn doc_from_mesh(mesh: &CgrMesh) -> Document {
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
            .with_param("triangle_count", mesh.triangles().len() as u64)
            .with_param("vertex_count", mesh.vertices().len() as u64)
            .with_body(&body_id),
    );
    doc.push_body(body);
    doc.push_part(part);
    doc
}

/// Deterministic canonical mapping: one readable mesh → a `Document` with one
/// part owning one solid body (bbox from the mesh vertices).
///
/// This is format-independent and fully deterministic (the mesh type alone,
/// per P2-M2-T01); it is the P2 adapter's canonical core.
pub fn from_mesh(mesh: &CgrMesh) -> Document {
    doc_from_mesh(mesh)
}

/// Canonical mapping for a modern-era latched mesh that additionally records
/// the measured modern decode counts (records / index_count / max_index) on
/// the import feature, so a workbench load surfaces the full real-material
/// numbers that the committed numeric-epsilon golden locks.
pub fn from_modern_mesh(mesh: &CgrMesh, counts: &ModernCounts) -> Document {
    let mut doc = doc_from_mesh(mesh);
    let feat = &mut doc.features[0];
    feat.parameters
        .insert("records".to_string(), (counts.records as u64).into());
    feat.parameters.insert(
        "index_count".to_string(),
        (counts.index_count as u64).into(),
    );
    feat.parameters
        .insert("max_index".to_string(), (counts.max_index as u64).into());
    doc
}

fn report_for_mesh(doc: &Document) -> ImportReport {
    ImportReport {
        source: SourceKind::Cgr,
        parts: doc.parts.len(),
        bodies: doc.bodies.len(),
        assemblies: 0,
        instances: 0,
        warnings: vec![],
    }
}

/// Import raw bytes through the CGR reader (both eras).
///
/// Tolerant-failure contract: any CGR payload yields either a canonical
/// document (when a structurally self-consistent triangle stream is found)
/// or a typed [`ImportOutcome::Skipped`] with the classified reason. Never
/// panics, never invents geometry.
pub fn import_bytes(bytes: &[u8]) -> Result<ImportOutcome, ImportError> {
    // Modern-era path: V5_CFV2 containers (modern CATIA family) use the
    // latched modern decoder (P2-M7-T01), which returns a real validated
    // spatial mesh plus the measured counts.
    if bytes.len() >= OUTER_MAGIC.len() && bytes[..OUTER_MAGIC.len()] == OUTER_MAGIC {
        return match extract_modern(bytes) {
            Ok((counts, mesh)) => {
                let document = from_modern_mesh(&mesh, &counts);
                let report = report_for_mesh(&document);
                Ok(ImportOutcome::Imported { document, report })
            }
            Err(ModernOutcome::NotCgrContainer) => Ok(ImportOutcome::Skipped {
                source: SourceKind::Cgr,
                reason: "modern path: no V5_CFV2 container found".to_string(),
            }),
            Err(ModernOutcome::NoBrepStreams) => Ok(ImportOutcome::Skipped {
                source: SourceKind::Cgr,
                reason: "modern path: no unique MainDataStream/SurfacicReps pair".to_string(),
            }),
            Err(ModernOutcome::NoCoordinateTable) => Ok(ImportOutcome::Skipped {
                source: SourceKind::Cgr,
                reason: "modern path: visualization point table absent or undecodable".to_string(),
            }),
            Err(ModernOutcome::Invalid(detail)) => Ok(ImportOutcome::Skipped {
                source: SourceKind::Cgr,
                reason: format!("modern path: structural invariant failed ({detail})"),
            }),
            Err(ModernOutcome::Mesh(_)) => Ok(ImportOutcome::Skipped {
                source: SourceKind::Cgr,
                reason: "modern path: unexpected error-side mesh variant".to_string(),
            }),
        };
    }

    // Old-era path: the polygon-soup triangle-stream extractor on classic
    // .cgr files (P2-M2-T01).
    match oep_cgr::mesh::extract(bytes) {
        Ok(mesh) => {
            let document = from_mesh(&mesh);
            let report = report_for_mesh(&document);
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
