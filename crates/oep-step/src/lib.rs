//! P4 -- STEP AP242/QIF/JT interop (workstream P4).
//!
//! oep-step is the P4 read lane. It wraps an existing OSS STEP library behind
//! a vendor-neutral, clean API (P4-M1-T01):
//!
//! - [`parse`] reads an ISO 10303-21 STEP physical file through the wrapped
//!   OSS library and yields [`FacetedModel`]s  -  our own carrier types, with
//!   zero dependency on the underlying library's type system;
//! - [`to_document`] maps the parsed facets onto the ONE canonical internal
//!   format, the [`oep_geo::Document`] native model (part / body / feature),
//!   feeding the P6 importer hub's composition-root story;
//! - [`parse_to_document`] is the one-call convenience for the common path.
//!
//! The underlying library is a private implementation detail (vendor-neutral
//! seam). Library choice and the Fidelity / AP242 semantic-PMI path
//! (Open CASCADE) are recorded in `workstreams/step-interop/library-adr/step-library-adr.md`.
//!
//! Tolerant-failure: any byte input yields a typed [`Error`] or a
//! [`ReadOutcome`], never a panic and never fabricated geometry.
#![deny(missing_docs)]

pub mod error;
pub mod export;
pub mod facets;
pub mod jt;
pub mod pmi;

pub use error::Error;
pub use facets::{FacetBbox, FacetedModel, ReadOutcome, ReadReport, ShellFacts};

use facets::{bbox_over, key_point, NO_FACETS_REASON};
use oep_geo::{Body, BodyKind, BoundingBox, Document, Feature, Part};
use std::collections::HashMap;
use step_io::generated::model::{
    CartesianPointRef, ClosedShellRef, EdgeRef, FaceBoundRef, FaceRef, LoopRef, OrientedEdgeRef,
    PointRef, ShellRef, StepModel, VertexRef,
};

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P4-STEP";

/// Parse STEP bytes through the wrapped OSS STEP library into vendor-neutral
/// facets plus an honesty report.
///
/// Returns [`Error::NotStep`] when the bytes do not carry the
/// `ISO-10303-21;` magic, and a typed parse error when the library rejects
/// the payload. Never panics.
pub fn parse(bytes: &[u8]) -> Result<ReadOutcome, Error> {
    if !bytes.starts_with(b"ISO-10303-21;") {
        return Err(Error::NotStep);
    }
    let (model, report) = step_io::read(bytes).map_err(|e| Error::Parse(format!("{e}")))?;
    let header = model.header().clone();

    let mut warnings = Vec::new();
    for (_key, reason) in &report.dropped {
        warnings.push(format!("dropped by OSS reader: {}", reason.key));
    }

    let (shells, vertices, triangles) = walk_faceted(&model, &mut warnings)?;
    if shells.is_empty() || vertices.is_empty() {
        return Err(Error::NoFacetedGeometry(NO_FACETS_REASON.to_string()));
    }
    let bbox = bbox_over(&vertices);
    let product_name = product_name_of(&model)
        .unwrap_or_else(|| file_stem(&header.file_name).unwrap_or_else(|| "unnamed".to_string()));

    let facets = FacetedModel {
        product_name,
        shells,
        vertices,
        triangles,
        bbox,
    };
    let report = ReadReport {
        file_name: (!header.file_name.is_empty()).then_some(header.file_name),
        schema: Some(format!("{:?}", header.schema)),
        warnings,
        dropped: report.dropped.len(),
    };
    Ok(ReadOutcome { facets, report })
}

/// Map parsed facets onto the canonical `oep_geo::Document` native model.
///
/// Mirrors the P6 importer hub's adapter story (`oep_import::adapters::cgr`):
/// one part owning one body per shell (a closed shell maps to a `Solid`,
/// an open shell to a `Sheet`), body bbox from the parsed vertices, and one
/// `ImportStep` feature recording the real face / shell / vertex / triangle
/// counts. The document is canonical: it validates and serializes
/// deterministically in the oep-geo own-format.
pub fn to_document(facets: &FacetedModel) -> Document {
    let mut doc = Document::new("mm");
    let part_id = "oep-part-step-1".to_string();
    let feature_id = "oep-feature-step-import".to_string();

    let mut feature = Feature::new(&feature_id, "step-import", "ImportStep")
        .with_param("face_count", facets.face_count() as f64)
        .with_param("shell_count", facets.shells.len() as f64)
        .with_param("vertex_count", facets.vertices.len() as f64)
        .with_param("triangle_count", facets.triangles.len() as f64);

    let default_bbox = facets.bbox.map(|b| BoundingBox::new(b.min, b.max));
    let mut part = Part::new(&part_id, &facets.product_name);
    for (i, shell) in facets.shells.iter().enumerate() {
        let body_id = format!("oep-body-step-{}", i + 1);
        let kind = if shell.closed {
            BodyKind::Solid
        } else {
            BodyKind::Sheet
        };
        let body = Body::new(&body_id, format!("step-shell-{}", i + 1), kind)
            .with_bbox(default_bbox.unwrap_or(BoundingBox::new([0.0; 3], [0.0; 3])))
            .with_feature(&feature_id);
        part = part.with_body(&body_id);
        feature = feature.with_body(&body_id);
        doc.push_body(body);
    }
    part = part.with_feature(&feature_id);
    doc.push_feature(feature);
    doc.push_part(part);
    doc
}

/// Convenience one-call path: [`parse`] then [`to_document`].
pub fn parse_to_document(bytes: &[u8]) -> Result<(Document, ReadReport), Error> {
    let outcome = parse(bytes)?;
    let document = to_document(&outcome.facets);
    Ok((document, outcome.report))
}

// ---------------------------------------------------------------------------
// private: the wrapped-library walk (the only place OSS types appear)
// ---------------------------------------------------------------------------

/// Accumulator for the facet walk: deduplicated vertices + triangles.
struct Builder {
    vertices: Vec<[f64; 3]>,
    by_key: HashMap<[u64; 3], u32>,
    triangles: Vec<[u32; 3]>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            vertices: Vec::new(),
            by_key: HashMap::new(),
            triangles: Vec::new(),
        }
    }
    fn vertex(&mut self, p: [f64; 3]) -> u32 {
        let key = key_point(p);
        if let Some(&i) = self.by_key.get(&key) {
            return i;
        }
        let i = self.vertices.len() as u32;
        self.vertices.push(p);
        self.by_key.insert(key, i);
        i
    }
    /// Fan-triangulate a polygon loop of `n >= 3` vertices.
    fn polygon(&mut self, pts: &[[f64; 3]]) -> usize {
        let n = pts.len();
        if n < 3 {
            return 0;
        }
        let indices: Vec<u32> = pts.iter().map(|p| self.vertex(*p)).collect();
        for k in 1..n - 1 {
            self.triangles
                .push([indices[0], indices[k], indices[k + 1]]);
        }
        n - 2
    }
}

/// What the walk produces: shell facts, deduplicated vertices, triangles.
type FacetWalk = (Vec<ShellFacts>, Vec<[f64; 3]>, Vec<[u32; 3]>);

/// Walk the faceted + analytic-solid geometry of the STEP file into shell
/// facts + a deduplicated facet mesh. Two structural families are mapped:
///
/// - `SHELL_BASED_SURFACE_MODEL` (the P6-M4 passthrough): `CLOSED_SHELL` /
///   `OPEN_SHELL` over `FACE_SURFACE`/`PLANE`/`POLY_LOOP`.
/// - `MANIFOLD_SOLID_BREP` (the P4-M3 export lane): `ADVANCED_FACE` /
///   `EDGE_LOOP` / `ORIENTED_EDGE` / `EDGE_CURVE` / `VERTEX_POINT` — the
///   full-topology form OCCT can build, which our own exporter emits.
///
/// Both are reconstructed into the same vendor-neutral facet carrier, so the
/// export lane's output re-imports through this reader (P4-M3-T01 round trip).
/// Anything outside the handled set is a recorded warning, never a panic and
/// never an invented value.
fn walk_faceted(model: &StepModel, warnings: &mut Vec<String>) -> Result<FacetWalk, Error> {
    let mut shells = Vec::new();
    let mut builder = Builder::new();

    // ---- SHELL_BASED_SURFACE_MODEL passthrough (P6-M4 / legacy) ---------
    for sbsm in &model.shell_based_surface_model_arena.items {
        for shell_ref in &sbsm.sbsm_boundary {
            let (closed, face_refs): (bool, Vec<FaceRef>) = match shell_ref {
                ShellRef::ClosedShell(id) => {
                    (true, model.closed_shell_arena.get(id.0).cfs_faces.clone())
                }
                ShellRef::OpenShell(id) => {
                    (false, model.open_shell_arena.get(id.0).cfs_faces.clone())
                }
                other => {
                    warnings.push(format!("SKIP shell ref not mapped: {other:?}"));
                    continue;
                }
            };
            let faces = map_face_refs(model, &face_refs, &mut builder, warnings);
            shells.push(ShellFacts { closed, faces });
        }
    }

    // ---- MANIFOLD_SOLID_BREP (P4-M3-T01 export lane, real topology) -----
    for brep in &model.manifold_solid_brep_arena.items {
        let closed = true;
        let (face_refs, warn) = match &brep.outer {
            ClosedShellRef::ClosedShell(id) => {
                (model.closed_shell_arena.get(id.0).cfs_faces.clone(), None)
            }
            other => (
                Vec::new(),
                Some(format!("SKIP outer shell ref not mapped: {other:?}")),
            ),
        };
        if let Some(w) = warn {
            warnings.push(w);
            continue;
        }
        let faces = map_face_refs(model, &face_refs, &mut builder, warnings);
        shells.push(ShellFacts { closed, faces });
    }

    Ok((shells, builder.vertices, builder.triangles))
}

/// Map a shell's face refs into the builder, returning how many faces yielded
/// at least one triangle. Handles `FACE_SURFACE`/`FACE` (poly loops) and
/// `ADVANCED_FACE` (edge loops -> shared-vertex polygons).
#[allow(clippy::too_many_arguments)]
fn map_face_refs(
    model: &StepModel,
    face_refs: &[FaceRef],
    builder: &mut Builder,
    warnings: &mut Vec<String>,
) -> usize {
    let mut mapped_faces = 0usize;
    for face_ref in face_refs {
        let bounds: Option<(Vec<FaceBoundRef>, bool)> = match face_ref {
            FaceRef::FaceSurface(id) => {
                Some((model.face_surface_arena.get(id.0).bounds.clone(), false))
            }
            FaceRef::Face(id) => Some((model.face_arena.get(id.0).bounds.clone(), false)),
            FaceRef::AdvancedFace(id) => {
                Some((model.advanced_face_arena.get(id.0).bounds.clone(), true))
            }
            other => {
                warnings.push(format!("SKIP face ref not mapped: {other:?}"));
                None
            }
        };
        let Some((bounds, has_edge_loops)) = bounds else {
            continue;
        };
        let mut face_has_triangle = false;
        for bound_ref in &bounds {
            let loop_ref: LoopRef = match bound_ref {
                FaceBoundRef::FaceOuterBound(id) => {
                    model.face_outer_bound_arena.get(id.0).bound.clone()
                }
                FaceBoundRef::FaceBound(id) => model.face_bound_arena.get(id.0).bound.clone(),
                other => {
                    warnings.push(format!("SKIP bound ref not mapped: {other:?}"));
                    continue;
                }
            };
            let pts: Option<Vec<[f64; 3]>> = match &loop_ref {
                LoopRef::PolyLoop(id) => {
                    let polygon = &model.poly_loop_arena.get(id.0).polygon;
                    Some(
                        polygon
                            .iter()
                            .filter_map(|pref| match pref {
                                CartesianPointRef::CartesianPoint(cid) => {
                                    Some(model.cartesian_point_arena.get(cid.0).coordinates.clone())
                                }
                                _ => None,
                            })
                            .filter_map(|c| c.try_into().ok())
                            .collect(),
                    )
                }
                LoopRef::EdgeLoop(id) if has_edge_loops => {
                    let edge_loop = &model.edge_loop_arena.get(id.0);
                    Some(
                        edge_loop
                            .edge_list
                            .iter()
                            .filter_map(|eref| oriented_edge_corner(model, eref, warnings))
                            .collect(),
                    )
                }
                other => {
                    warnings.push(format!("SKIP non-faceted loop: {other:?}"));
                    None
                }
            };
            if let Some(pts) = pts {
                if builder.polygon(&pts) > 0 {
                    face_has_triangle = true;
                }
            }
        }
        if face_has_triangle {
            mapped_faces += 1;
        }
    }
    mapped_faces
}

/// The ordered corner of an oriented edge (the loop passes through each edge
/// in turn, so taking one endpoint per oriented edge reconstructs the polygon).
fn oriented_edge_corner(
    model: &StepModel,
    eref: &OrientedEdgeRef,
    warnings: &mut Vec<String>,
) -> Option<[f64; 3]> {
    let oe = match eref {
        OrientedEdgeRef::OrientedEdge(id) => model.oriented_edge_arena.get(id.0),
        _ => {
            warnings.push(format!("SKIP non-oriented edge in loop: {eref:?}"));
            return None;
        }
    };
    let (start, end) = match &oe.edge_element {
        EdgeRef::EdgeCurve(id) => {
            let e = &model.edge_curve_arena.get(id.0);
            (e.edge_start.clone(), e.edge_end.clone())
        }
        EdgeRef::Edge(id) => {
            let e = &model.edge_arena.get(id.0);
            match (&e.edge_start, &e.edge_end) {
                (Some(s), Some(en)) => (s.clone(), en.clone()),
                _ => {
                    warnings.push("SKIP edge with missing vertex".to_string());
                    return None;
                }
            }
        }
        other => {
            warnings.push(format!("SKIP edge element not mapped: {other:?}"));
            return None;
        }
    };
    let v = if oe.orientation { start } else { end };
    vertex_point(model, &v, warnings)
}

/// Resolve a vertex ref (via `VERTEX_POINT` -> `CARTESIAN_POINT`) to `[x,y,z]`.
fn vertex_point(
    model: &StepModel,
    vref: &VertexRef,
    warnings: &mut Vec<String>,
) -> Option<[f64; 3]> {
    let vp = match vref {
        VertexRef::VertexPoint(id) => model.vertex_point_arena.get(id.0),
        _ => {
            warnings.push(format!("SKIP non-vertex-point: {vref:?}"));
            return None;
        }
    };
    match &vp.vertex_geometry {
        PointRef::CartesianPoint(id) => {
            let c = &model.cartesian_point_arena.get(id.0).coordinates;
            if c.len() == 3 {
                Some([c[0], c[1], c[2]])
            } else {
                warnings.push("SKIP point with dimension != 3".to_string());
                None
            }
        }
        other => {
            warnings.push(format!("SKIP non-cartesian point: {other:?}"));
            None
        }
    }
}

/// Product name from the STEP product cluster, when the underlying library
/// kept it. `None` when absent (see the honest-fallback note in the ADR).
fn product_name_of(model: &StepModel) -> Option<String> {
    model
        .product_arena
        .items
        .iter()
        .map(|p| p.name.clone())
        .find(|n| !n.is_empty())
}

/// `box-reference.step` -> `Some("box-reference")`.
fn file_stem(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?;
    let stem = base.split('.').next()?;
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}
