//! P4 lane JT reader (visualization-first wedge, P4-M4-T03).
//!
//! Vendor-neutral public seam. The JT reader is built clean-room from the
//! published format description (Siemens JT v10 File Format Reference Rev-B
//! and JTIAP PSI 14-1 V3), behind the same vendor-neutral discipline as the
//! STEP lane in this crate: the public surface below deals only in Keelworks
//! concepts (a model made of parts, each with a name, optional bounding box,
//! optional material and a facet mesh), never in JT container or codec
//! vocabulary. Anything the spec documents but this wedge does not decode yet
//! yields a typed [`Error`], never a panic and never fabricated geometry.
//!
//! Layouts implemented follow the plans in `workstreams/P4-M4/jt-reader-plan.md` and the
//! decode notes in `docs/re-notes/oep-jt-container-and-lsg.md`, all cited to
//! spec sections there.

mod codec;
mod core;
mod lsg;
mod shape;

use std::fmt;

/// A byte cursor with bounds-checked primitive reads, honoring the file's
/// declared byte order.
pub(crate) struct Cursor<'a> {
    pub buf: &'a [u8],
    pub pos: usize,
    pub big: bool,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8], big: bool) -> Self {
        Cursor { buf, pos: 0, big }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Truncated)?;
        if end > self.buf.len() {
            return Err(Error::Truncated);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        if self.big {
            Ok(u16::from_be_bytes([b[0], b[1]]))
        } else {
            Ok(u16::from_le_bytes([b[0], b[1]]))
        }
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("4 bytes");
        Ok(if self.big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(self.u32()? as i32)
    }

    fn u64(&mut self) -> Result<u64, Error> {
        let b: [u8; 8] = self.take(8)?.try_into().expect("8 bytes");
        Ok(if self.big {
            u64::from_be_bytes(b)
        } else {
            u64::from_le_bytes(b)
        })
    }

    fn f32(&mut self) -> Result<f32, Error> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("4 bytes");
        let u = if self.big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        };
        Ok(f32::from_bits(u))
    }

    fn i8(&mut self) -> Result<i8, Error> {
        Ok(self.u8()? as i8)
    }
}

/// Typed, tolerant failure model (P2-M1-T04 lineage): the reader never panics
/// on input; every failure is one of these variants.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The bytes are too short to hold the structure being read.
    Truncated,
    /// The input does not carry the JT file version marker.
    NotJt,
    /// The file header is malformed (bad version length, invalid byte order).
    BadHeader(String),
    /// The block directory is malformed (offset out of range, bad entry
    /// count, unknown block type).
    BadDirectory(String),
    /// An element boundary is inconsistent (declared length overruns the
    /// block, unterminated element stream).
    BadStructure(String),
    /// A documented feature this wedge does not decode was requested (LZMA or
    /// arithmetic payloads, Deering normals, unsupported primitives).
    UnsupportedFeature(String),
    /// Geometry decode failed (bad primitive parameters, degenerate mesh).
    BadGeometry(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => write!(f, "truncated JT input"),
            Error::NotJt => write!(f, "not a JT file (no version marker)"),
            Error::BadHeader(m) => write!(f, "bad JT header: {m}"),
            Error::BadDirectory(m) => write!(f, "bad JT structure: {m}"),
            Error::BadStructure(m) => write!(f, "bad JT element: {m}"),
            Error::UnsupportedFeature(m) => write!(f, "unsupported JT feature: {m}"),
            Error::BadGeometry(m) => write!(f, "bad JT shape: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// An axis-aligned bounding box in MCS (min / max corners).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Minimum corner `[x, y, z]`.
    pub min: [f64; 3],
    /// Maximum corner `[x, y, z]`.
    pub max: [f64; 3],
}

impl BoundingBox {
    /// Extents along each axis.
    pub fn extents(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// Signed volume of the box.
    pub fn volume(&self) -> f64 {
        let e = self.extents();
        e[0] * e[1] * e[2]
    }
}

/// A surface material: RGBA diffuse colour plus an optional name.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialInfo {
    /// Interpolated-source name (from a string property on the material).
    pub name: Option<String>,
    /// Diffuse colour and alpha, each in `[0, 1]`.
    pub diffuse: [f32; 4],
}

/// A facet mesh owned by a part (vertices + triangles).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FacetMesh {
    /// Positions in part-local (LCS) coordinates.
    pub vertices: Vec<[f64; 3]>,
    /// Triangle corner indices into [`FacetMesh::vertices`].
    pub triangles: Vec<[u32; 3]>,
}

impl FacetMesh {
    /// Bounding box over the mesh vertices; `None` when empty.
    pub fn bounds(&self) -> Option<BoundingBox> {
        let mut it = self.vertices.iter();
        let first = *it.next()?;
        let mut min = first;
        let mut max = first;
        for v in it {
            for i in 0..3 {
                if v[i] < min[i] {
                    min[i] = v[i];
                }
                if v[i] > max[i] {
                    max[i] = v[i];
                }
            }
        }
        Some(BoundingBox { min, max })
    }
}

/// A part recovered from the scene graph.
#[derive(Debug, Clone, PartialEq)]
pub struct PartNode {
    /// Object identifier from the file (stable reference).
    pub object_id: i32,
    /// Part name (from a string property atom).
    pub name: String,
    /// Axis-aligned bounding box declared on the partition, when present.
    pub bounding: Option<BoundingBox>,
    /// Facet mesh decoded from the referenced shape LOD segment, if any.
    pub mesh: Option<FacetMesh>,
    /// Optional surface material attached to the part.
    pub material: Option<MaterialInfo>,
}

/// A whole decoded JT model, in vendor-neutral terms.
#[derive(Debug, Clone, PartialEq)]
pub struct JtModel {
    /// Version string from the file header (e.g. `Version 10.0 JT ...`).
    pub version: String,
    /// True when the file was written most-significant-byte-first.
    pub big_endian: bool,
    /// The parts recovered from the scene graph.
    pub parts: Vec<PartNode>,
    /// Non-fatal notes (unknown elements skipped etc).
    pub warnings: Vec<String>,
}

impl JtModel {
    /// True when at least one part carries a non-empty facet mesh.
    pub fn has_mesh(&self) -> bool {
        self.parts.iter().any(|p| {
            p.mesh
                .as_ref()
                .map(|m| !m.triangles.is_empty())
                .unwrap_or(false)
        })
    }
}

// ---------------------------------------------------------------------------
// document mapping (P6 canonical model)
// ---------------------------------------------------------------------------

/// Map a decoded JT model onto the canonical `oep_geo::Document` native model.
///
/// One `Part` per JT part node, one `Solid` `Body` per part carrying an
/// "ImportJt" feature recording the real mesh observables (vertex / triangle
/// counts). Mirrors the STEP adapter's mapping story in this crate.
pub fn to_document(model: &JtModel) -> oep_geo::Document {
    let mut doc = oep_geo::Document::new("mm");
    for (i, part) in model.parts.iter().enumerate() {
        if part.mesh.is_none() {
            continue;
        }
        let mesh = part.mesh.as_ref().expect("mesh present");
        let part_id = format!("oep-jt-part-{}", i + 1);
        let body_id = format!("oep-jt-body-{}", i + 1);
        let feature_id = format!("oep-jt-feature-{}", i + 1);
        let display_name = if part.name.is_empty() {
            "unnamed-part".to_string()
        } else {
            part.name.clone()
        };
        let mut feature = oep_geo::Feature::new(&feature_id, "import", "ImportJt")
            .with_param("vertex_count", mesh.vertices.len() as f64)
            .with_param("triangle_count", mesh.triangles.len() as f64);
        let bbox = mesh.bounds().map(|b| {
            oep_geo::BoundingBox::new(
                [b.min[0], b.min[1], b.min[2]],
                [b.max[0], b.max[1], b.max[2]],
            )
        });
        let body = oep_geo::Body::new(
            &body_id,
            format!("jt-shape-{}", i + 1),
            oep_geo::BodyKind::Solid,
        )
        .with_bbox(bbox.unwrap_or(oep_geo::BoundingBox::new([0.0; 3], [0.0; 3])))
        .with_feature(&feature_id);
        let mut p = oep_geo::Part::new(&part_id, display_name)
            .with_body(&body_id)
            .with_feature(&feature_id);
        if let Some(ref mat) = part.material {
            if let Some(ref n) = mat.name {
                p = p.with_part_number(n.clone());
            }
        }
        feature = feature.with_body(&body_id);
        doc.push_part(p);
        doc.push_body(body);
        doc.push_feature(feature);
    }
    doc
}

/// One-call convenience: [`parse`] then [`to_document`].
pub fn parse_to_document(bytes: &[u8]) -> Result<(JtModel, oep_geo::Document), Error> {
    let model = parse(bytes)?;
    let doc = to_document(&model);
    Ok((model, doc))
}

/// Parse JT bytes into a vendor-neutral [`JtModel`].
///
/// The pipeline implements the documented container spine: file header
/// (version, byte order, block-directory offset, root reference), the block
/// directory mapping, the LSG block element stream (partition/part nodes,
/// string + late-loaded property atoms, property table), and any shape
/// reference decoded by the wedge's shape support. It is tolerant: unknown
/// element types are skipped with warnings recorded on the model, corrupt or
/// unsupported input yields a typed [`Error`], and it never panics and never
/// fabricates geometry.
pub fn parse(bytes: &[u8]) -> Result<JtModel, Error> {
    let header = core::parse_header(bytes)?;

    let entries = core::walk_directory(bytes, header.directory_offset, header.big_endian)?;

    // Locate the root LSG block: prefer an exact root-reference match, fall
    // back to the kind-1 (LSG) block when the reference is absent (tolerant).
    let root = entries
        .iter()
        .find(|e| e.guid == header.root_block)
        .or_else(|| entries.iter().find(|e| e.kind == 1))
        .ok_or_else(|| Error::BadDirectory("no LSG block found".into()))?;
    if root.kind != 1 {
        return Err(Error::BadDirectory(format!(
            "root block kind {} is not the expected logical scene graph",
            root.kind
        )));
    }

    let facts = lsg::parse_lsg(bytes, root, header.big_endian)?;
    let (parts, warnings) = lsg::resolve_parts(&facts, bytes, &entries, header.big_endian)?;

    Ok(JtModel {
        version: header.version,
        big_endian: header.big_endian,
        parts,
        warnings,
    })
}
