//! P4-M3-T01 -- `oep-step` STEP *export* lane (geometry first, PMI later).
//!
//! This is the P4 write lane for parts and assemblies. It consumes the
//! canonical `oep_geo::Document` (parts / bodies / assemblies / instances)
//! plus a supplier of per-body tessellations, and emits a single ISO 10303-21
//! physical file. Everything this module exposes is vendor-neutral: the
//! wrapped OSS STEP library (`step-io`, the same pinned library the read lane
//! uses) appears only in the private internals, exactly like the read lane.
//!
//! ## Geometry first, PMI later
//!
//! The card's milestone cut is explicit: **geometry first, PMI later**.
//!
//! - *Geometry*: each part's bodies are emitted as real B-Rep topology --
//!   `MANIFOLD_SOLID_BREP` over `ADVANCED_FACE`/`EDGE_LOOP`/`EDGE_CURVE`/
//!   `VERTEX_POINT`, one planar facet per source triangle, shared vertices
//!   and edges so adjacent facets meet topologically. This is the crucial
//!   difference from the P6-M4-T01 passthrough: that exporter emits
//!   `SHELL_BASED_SURFACE_MODEL` over `FACE_SURFACE`/`PLANE`/`POLY_LOOP`,
//!   which Open CASCADE (FreeCAD) reads but **cannot build into a shape**
//!   (valid=false, no solids). The MSBRep form is what the acceptance bar
//!   "re-imports into FreeCAD as a real solid" requires, and it is verified
//!   end-to-end by the Python acceptance harness (`OEP_FREECAD`).
//! - *PMI deferred*: the exporter emits no PMI entities
//!   (`DIMENSIONAL_CHARACTERISTIC` / `GEOMETRIC_TOLERANCE` /
//!   `PROPERTY_DEFINITION` / `SHAPE_ASPECT` are all absent). PMI is owned by
//!   the P4-M2 lane, where the semantic path and units/tolerance model land.
//!
//! ## Honest marking
//!
//! The output is a **tessellated (faceted) representation and is NOT claimed
//! as an exact analytic B-Rep**. `FILE_DESCRIPTION` and the product
//! description state this explicitly (the same honesty contract the P6-M4
//! exporter and the P2-M5-T02 / P4-lane agreement use). No writer in this
//! crate ever claims an analytic B-Rep it did not verify.
//!
//! ## Assembly hierarchy
//!
//! Assemblies are emitted as STEP products; every instance becomes a
//! `NEXT_ASSEMBLY_USAGE_OCCURRENCE` under its parent assembly, with its
//! placement carried by an `ITEM_DEFINED_TRANSFORM` composed of two
//! `AXIS2_PLACEMENT_3D` frames. A part instanced many times shares one
//! product definition; sub-assemblies nest by the same occurrence mechanism.
//!
//! ## Determinism
//!
//! Output is byte-for-byte deterministic for a given document + tessellation
//! supplier + opts: entity ids follow a fixed insertion order, `FILE_NAME`
//! carries the caller's version/org/system strings (never wall-clock time).
//!
//! ## Failures
//!
//! Typed, tolerant, never panics: a body with no tessellation
//! ([`ExportError::MissingTessellation`]), an instance referencing an unknown
//! part / assembly id, a part owning an unknown body id, non-finite
//! geometry, or an all-degenerate mesh return a named variant.
//!
//! ## Public surface
//!
//! [`write_document`] is the one-call path. Its signature uses only neutral
//! types (`oep_geo` types, plain slices, `String`); no `step-io` identifier
//! appears.

use oep_geo::Document;

/// Options that shape the exported file's header identity.
///
/// Sensible defaults are provided; the exporter fills the file with these so
/// output stays deterministic (no wall-clock anything).
#[derive(Debug, Clone)]
pub struct ExportOpts {
    /// `FILE_NAME.name` -- the physical file name (e.g. `"assembly-1.step"`).
    pub file_name: String,
    /// Root product name (used for the FILE_NAME product cluster and shared
    /// across parts; each part keeps its own oep-geo name).
    pub product_name: String,
    /// Version string recorded in `FILE_NAME`.
    pub version: String,
    /// Originating organization, recorded in `FILE_NAME`.
    pub org: String,
    /// Originating system (the exporter identity), recorded in `FILE_NAME`.
    pub system: String,
}

impl Default for ExportOpts {
    fn default() -> Self {
        Self {
            file_name: "isolith-export.step".to_string(),
            product_name: "isolith-model".to_string(),
            version: "0.1.0".to_string(),
            org: "Isolith OEP".to_string(),
            system: "oep-step".to_string(),
        }
    }
}

/// Typed export failure. Every variant is produced deliberately; the exporter
/// never panics (tolerant-failure doctrine, P2-M1-T04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// A body id a part references does not exist in the document's body table.
    UnknownBody(String),
    /// An instance sources a part id that is not in the document.
    UnknownPart(String),
    /// An instance sources an assembly id that is not in the document.
    UnknownAssembly(String),
    /// A body has no tessellation (the supplier returned `None` for it).
    MissingTessellation(String),
    /// A body's tessellation has no usable (non-degenerate) facets.
    EmptyMesh(String),
    /// Geometry contains a NaN or infinite coordinate.
    NonFiniteGeometry(String),
    /// The underlying wrapped library rejected an authored entity (should not
    /// happen for well-formed input; surfaced, never hidden).
    Author(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::UnknownBody(id) => write!(f, "STEP export: unknown body id {id:?}"),
            ExportError::UnknownPart(id) => write!(f, "STEP export: unknown part id {id:?}"),
            ExportError::UnknownAssembly(id) => {
                write!(f, "STEP export: unknown assembly id {id:?}")
            }
            ExportError::MissingTessellation(id) => {
                write!(f, "STEP export: body {id:?} has no tessellation")
            }
            ExportError::EmptyMesh(id) => {
                write!(f, "STEP export: body {id:?} has no usable facets")
            }
            ExportError::NonFiniteGeometry(id) => {
                write!(f, "STEP export: body {id:?} has non-finite coordinates")
            }
            ExportError::Author(msg) => write!(f, "STEP export: writer rejected entity: {msg}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// A per-body tessellation: the body id plus its `positions` (f32 triples) and
/// `triangles` (0-based index triples into `positions`). This is the same
/// vertices/indices/triangles representation the P6 exporters consume
/// (P2-M2-T01), so any mesh carrier can supply it unchanged.
#[derive(Debug, Clone, Copy)]
pub struct BodyMesh<'a> {
    /// The oep-geo body id this tessellation belongs to.
    pub body_id: &'a str,
    /// Vertex positions (f32 triples).
    pub positions: &'a [[f32; 3]],
    /// Triangle indices into `positions`.
    pub triangles: &'a [[u32; 3]],
}

/// Export an `oep_geo::Document` (parts and assemblies) to an ISO 10303-21
/// AP242 physical file.
///
/// `meshes` is an ordered slice of per-body tessellations. A body with no
/// tessellation in the slice yields [`ExportError::MissingTessellation`]; an
/// instance referencing an unknown part / assembly id yields the matching
/// "unknown" variant. The returned bytes are deterministic for fixed inputs.
///
/// The current implementation is a *facets-in / B-Rep-out* passthrough:
/// topology-correct `MANIFOLD_SOLID_BREP` from the supplied tessellation,
/// honestly marked as not an exact analytic B-Rep. PMI is deliberately not
/// emitted (P4-M2 lane).
pub fn write_document<'a>(
    doc: &Document,
    meshes: &[BodyMesh<'a>],
    opts: &ExportOpts,
) -> Result<Vec<u8>, ExportError> {
    exporter::export(doc, meshes, opts)
}

mod exporter;
