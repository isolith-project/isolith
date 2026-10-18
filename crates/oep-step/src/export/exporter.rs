//! Private implementation of the P4 STEP export lane. Type-level docs live in
//! [`super`]; this module is where step-io identifiers are allowed.

use super::{BodyMesh, ExportError, ExportOpts};
use oep_geo::{Document, InstanceSource};
use std::collections::HashMap;

use step_io::build::{CurveInput, FaceBoundInput, Frame, HeaderInput, PartOptions, SurfaceInput};
use step_io::StepBuilder;

/// Honest marking string, shared by FILE_DESCRIPTION + product desc.
pub(crate) const HONEST_MARK: &str = "oep-geo tessellated faceted B-Rep (planar facets). \
    This is a TESSELLATED representation and is NOT an exact analytic B-Rep.";

/// Emit the whole document through the step-io authoring API.
pub(super) fn export<'a>(
    doc: &Document,
    meshes: &[BodyMesh<'a>],
    opts: &ExportOpts,
) -> Result<Vec<u8>, ExportError> {
    let mut b = StepBuilder::new().map_err(author_err)?;
    b.header(&HeaderInput {
        file_name: Some(opts.file_name.clone()),
        description: Some(HONEST_MARK.to_string()),
        authors: vec![opts.org.clone()],
        organizations: vec![],
        originating_system: Some(opts.system.clone()),
        authorisation: None,
        // Deterministic: never stamp wall-clock time into the file.
        timestamp: Some(opts.version.clone()),
    });

    // ---- pass 1: register product chains for every part -------
    let mut parts: HashMap<String, step_io::build::Part> = HashMap::new();
    for p in &doc.parts {
        let part = b
            .part_with(
                &p.name,
                &PartOptions {
                    id: Some(p.id.clone()),
                    description: Some(HONEST_MARK.to_string()),
                    version: Some(opts.version.clone()),
                    version_description: Some(format!("{} (tessellated)", p.name)),
                    definition_description: Some(HONEST_MARK.to_string()),
                },
            )
            .map_err(author_err)?;
        parts.insert(p.id.clone(), part);
    }

    // ---- pass 2: materialize each part's bodies -------
    for p in &doc.parts {
        let part = parts[&p.id];
        for body_id in &p.body_ids {
            let body = doc
                .bodies
                .iter()
                .find(|bd| &bd.id == body_id)
                .ok_or_else(|| ExportError::UnknownBody(body_id.clone()))?;
            let Some(mesh) = meshes.iter().find(|m| m.body_id == body_id) else {
                return Err(ExportError::MissingTessellation(body_id.clone()));
            };
            let positions = mesh.positions;
            let triangles = mesh.triangles;
            if triangles.is_empty() || positions.is_empty() {
                return Err(ExportError::EmptyMesh(body_id.clone()));
            }
            let faces = build_body(&mut b, positions, triangles, body_id)?;
            let name = format!("{} body", body.name);
            b.solid(part, &name, faces).map_err(author_err)?;
        }
    }

    // ---- pass 3: assemblies + instances -------
    let mut asms: HashMap<String, step_io::build::Part> = HashMap::new();
    for a in &doc.assemblies {
        let asm = b
            .part_with(
                &a.name,
                &PartOptions {
                    id: Some(a.id.clone()),
                    description: Some(HONEST_MARK.to_string()),
                    version: Some(opts.version.clone()),
                    version_description: Some(format!("{} assembly", a.name)),
                    definition_description: Some(HONEST_MARK.to_string()),
                },
            )
            .map_err(author_err)?;
        asms.insert(a.id.clone(), asm);
    }
    for a in &doc.assemblies {
        let parent = asms[&a.id];
        for inst in &a.instances {
            let frame = frame_of(&inst.transform);
            let child = match &inst.source {
                InstanceSource::Part { id } => parts
                    .get(id)
                    .copied()
                    .ok_or_else(|| ExportError::UnknownPart(id.clone()))?,
                InstanceSource::Assembly { id } => asms
                    .get(id)
                    .copied()
                    .ok_or_else(|| ExportError::UnknownAssembly(id.clone()))?,
            };
            b.place(parent, child, frame).map_err(author_err)?;
        }
    }

    let text = b.finish().map_err(author_err)?;
    Ok(text.into_bytes())
}

/// Decompose an oep-geo 4x4 row-major transform into the placement frame STEP
/// uses. The oep-geo transform is row-major: elements [0..9] are the 3x3
/// linear part, [12]/[13]/[14] the translation. STEP placements carry
/// rotation + translation only: `axis` = rotated z column (elements 2, 6, 10),
/// `ref_dir` = rotated x column (elements 0, 4, 8), origin = the translation
/// column. Pure rotations decompose exactly; a scale/shear in the 3x3
/// normalizes the axis/ref_dir (the honest passthrough never bakes it into
/// the B-Rep).
fn frame_of(t: &oep_geo::Transform) -> Frame {
    let m = t.components();
    let origin = [m[12], m[13], m[14]];
    let axis = normalize([m[2], m[6], m[10]]);
    let ref_dir = normalize([m[0], m[4], m[8]]);
    Frame {
        origin,
        axis,
        ref_dir,
    }
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n <= 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// Build the planar-facet faces of one body: one `ADVANCED_FACE` per usable
/// triangle, sharing `VERTEX_POINT`s and `EDGE_CURVE`s so the resulting shell
/// is real topology (not `FACE_SURFACE`/`POLY_LOOP`).
fn build_body(
    b: &mut StepBuilder,
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    body_id: &str,
) -> Result<Vec<step_io::generated::model::AdvancedFaceId>, ExportError> {
    // ---- vertices: dedup by coordinate, stable first-use order ----
    let mut vkey: HashMap<[u32; 3], step_io::build::Vertex> = HashMap::new();
    let mut idx_vert: HashMap<u32, step_io::build::Vertex> = HashMap::new();
    for (i, p) in positions.iter().enumerate() {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(ExportError::NonFiniteGeometry(body_id.to_string()));
        }
        let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
        let v = *vkey.entry(key).or_insert_with(|| {
            b.vertex([p[0] as f64, p[1] as f64, p[2] as f64])
                .expect("vertex")
        });
        idx_vert.insert(i as u32, v);
    }

    // ---- shared edges (undirected (min,max) -> EdgeCurveId) ----
    let mut edge_of: HashMap<(u32, u32), step_io::generated::model::EdgeCurveId> = HashMap::new();
    let mut tri_edges: Vec<[(step_io::generated::model::EdgeCurveId, bool); 3]> =
        Vec::with_capacity(triangles.len());
    for tri in triangles {
        let (a, e1, e2) = (tri[0], tri[1], tri[2]);
        let mut out = Vec::with_capacity(3);
        for (u, v) in [(a, e1), (e1, e2), (e2, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            let fwd = u < v;
            let edge = *edge_of.entry(key).or_insert_with(|| {
                b.edge(idx_vert[&u], idx_vert[&v], CurveInput::Line)
                    .expect("line edge")
            });
            out.push((edge, fwd));
        }
        tri_edges.push([out[0], out[1], out[2]]);
    }

    // ---- one planar ADVANCED_FACE per usable triangle ----
    let mut faces = Vec::new();
    for (ti, tri) in triangles.iter().enumerate() {
        let a = positions[tri[0] as usize];
        let e1 = [
            positions[tri[1] as usize][0] - a[0],
            positions[tri[1] as usize][1] - a[1],
            positions[tri[1] as usize][2] - a[2],
        ];
        let e2 = [
            positions[tri[2] as usize][0] - a[0],
            positions[tri[2] as usize][1] - a[1],
            positions[tri[2] as usize][2] - a[2],
        ];
        let n = cross(&e1, &e2);
        let mag = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if mag <= 1e-12 {
            continue; // degenerate: skipped (honest report, not a panic)
        }
        let normal = [n[0] / mag, n[1] / mag, n[2] / mag];
        let plane = Frame {
            origin: [a[0] as f64, a[1] as f64, a[2] as f64],
            axis: [normal[0] as f64, normal[1] as f64, normal[2] as f64],
            ref_dir: normalize([e1[0] as f64, e1[1] as f64, e1[2] as f64]),
        };
        let bounds = tri_edges[ti]
            .iter()
            .map(|&(e, fwd)| (e, fwd))
            .collect::<Vec<_>>();
        let face = b
            .face(
                SurfaceInput::Plane(plane),
                true,
                vec![FaceBoundInput::outer(bounds)],
            )
            .map_err(author_err)?;
        faces.push(face);
    }
    if faces.is_empty() {
        return Err(ExportError::EmptyMesh(body_id.to_string()));
    }
    Ok(faces)
}

fn cross(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn author_err(e: step_io::AuthorError) -> ExportError {
    ExportError::Author(format!("{e}"))
}
