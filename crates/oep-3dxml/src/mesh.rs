//! P3-M1-T03 — mesh rep -> internal mesh reader.
//!
//! Reads the tessellated-ASCII branch of 3DXML: the `.3DRep` member markup
//! (an `<XMLRepresentation>` document whose `xsi:type="PolygonalRepType"`
//! reps carry the actual triangle geometry) and returns ONE internal [`Mesh`]
//! per geometry-bearing polygonal rep, in document order.
//!
//! ## Clean-room / license posture
//!
//! All format facts (element spine `XMLRepresentation` / `Root` /
//! `Rep xsi:type="PolygonalRepType"`, the `VertexBuffer > Positions`
//! comma/space text encoding, `Face` `triangles` / `strips` attributes)
//! come ONLY from observed bytes of public MIT/Apache-2.0 sample files —
//! recorded in `docs/re-notes/3dxml-hex-notebook.md` (P1-M2-T04, section
//! 4.2) — and from the Dassault 3DXML specification held privately under its
//! free internal-use license (P1-M1-T02). Nothing from that licensed spec is
//! reproduced here. No vendor library reads for us.
//!
//! ## Face encodings (both observed, both decoded)
//!
//! 1. `triangles="i j k i j k ..."` — a space-separated list of vertex-index
//!    triplets, 3 indices per triangle (Abaqus-exporter branch: cube1/blocks).
//! 2. `strips="r1,r2,..."` — comma-separated runs, each run a
//!    space-separated list of vertex indices forming a triangle strip; a run
//!    of N indices decodes to N-2 triangles (CATIA V5-exporter branch:
//!    machinist/cycloidal/FM1). Our own writer emits ONE 3-index run per
//!    triangle, which decodes to exactly that triangle unambiguously.
//!
//! The strip winding for runs of length >3 is `[uncertain]` (no vendor
//! viewer in the clean-room); the pinned convention is the standard OpenGL
//! one — triangle k of a run uses indices `(i[k], i[k+1], i[k+2])` when k is
//! even and `(i[k+1], i[k], i[k+2])` when k is odd. Triangle counts, index
//! bounds and bounding boxes are convention-independent and are asserted
//! exactly against real material.
//!
//! ## Tolerant-failure model (P2-M1-T04)
//!
//! Reps that carry no geometry (silhouette `<Edges><Polyline>` only, or an
//! unobserved `xsi:type` such as `TessellatedRepType`) are SKIPPED — they
//! produce no mesh and never an error. Malformed or inconsistent input is a
//! typed [`MeshError`], never a panic.
use std::fmt;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Internal indexed-triangle mesh: vertex positions + triangles.
///
/// Exposes `positions()` / `triangles()` as plain slices — the exact shape
/// the P3-M4 ASCII writer consumes (`&[[f32; 3]]` / `&[[u32; 3]]`), so
/// reader output feeds the writer directly (P3-M4-T02 round-trip).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// The vertex positions (one `[x, y, z]` per vertex).
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// The triangles as index triplets into [`Mesh::positions`].
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
}

/// Errors produced by [`parse_rep`]. Display + [`std::error::Error`], never
/// used to panic.
#[derive(Debug)]
pub enum MeshError {
    /// The bytes are not well-formed XML (including truncated input).
    MalformedXml(String),
    /// Well-formed XML, but the root element is not `XMLRepresentation`.
    NotXmlRepresentation,
    /// A `<Positions>` triple did not contain exactly three numbers.
    BadPosition(String),
    /// A triangle index referenced a position that does not exist.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of positions in the rep.
        positions: usize,
    },
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::MalformedXml(e) => write!(f, "malformed XML: {e}"),
            MeshError::NotXmlRepresentation => {
                write!(f, "not a 3DXML XMLRepresentation document")
            }
            MeshError::BadPosition(s) => write!(f, "malformed position triple: {s:?}"),
            MeshError::IndexOutOfRange { index, positions } => {
                write!(
                    f,
                    "triangle index {index} out of range (positions: {positions})"
                )
            }
        }
    }
}

impl std::error::Error for MeshError {}

/// Is an attribute's *local* name `type` with a value naming a polygonal rep?
fn polygonal_type(start: &BytesStart<'_>) -> Option<bool> {
    let mut saw_type = false;
    for attr in start.attributes().flatten() {
        if attr.key.local_name().as_ref() != b"type" {
            continue;
        }
        saw_type = true;
        let val = attr.unescape_value().unwrap_or_default();
        if val.ends_with("PolygonalRepType") {
            return Some(true);
        }
    }
    if saw_type {
        Some(false)
    } else {
        None
    }
}

/// Decode a comma-separated `x y z,x y z,...` `<Positions>` text block.
fn decode_positions(text: &str) -> Result<Vec<[f32; 3]>, MeshError> {
    let mut out = Vec::new();
    for chunk in text.split(',') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let mut it = chunk.split_whitespace();
        let x = it.next().and_then(|v| v.parse::<f32>().ok());
        let y = it.next().and_then(|v| v.parse::<f32>().ok());
        let z = it.next().and_then(|v| v.parse::<f32>().ok());
        match (x, y, z, it.next()) {
            (Some(x), Some(y), Some(z), None) => out.push([x, y, z]),
            _ => {
                return Err(MeshError::BadPosition(chunk.to_string()));
            }
        }
    }
    Ok(out)
}

/// Decode one strip run (N indices -> N-2 triangles, pinned OpenGL winding).
fn decode_strip_run(run: &[u32], out: &mut Vec<[u32; 3]>) {
    if run.len() < 3 {
        return;
    }
    for k in 0..run.len() - 2 {
        if k % 2 == 0 {
            out.push([run[k], run[k + 1], run[k + 2]]);
        } else {
            out.push([run[k + 1], run[k], run[k + 2]]);
        }
    }
}

/// Decode the triangles carried by a `<Face ...>` element.
fn decode_face_triangles(start: &BytesStart<'_>) -> Vec<[u32; 3]> {
    let mut tris = Vec::new();
    for attr in start.attributes().flatten() {
        let name = attr.key.local_name();
        if name.as_ref() == b"triangles" {
            let val = attr.unescape_value().unwrap_or_default();
            let nums: Vec<u32> = val
                .split_whitespace()
                .filter_map(|t| t.parse::<u32>().ok())
                .collect();
            for tri in nums.chunks(3) {
                if let [a, b, c] = tri {
                    tris.push([*a, *b, *c]);
                }
            }
        } else if name.as_ref() == b"strips" {
            let val = attr.unescape_value().unwrap_or_default();
            for run in val.split(',') {
                let inds: Vec<u32> = run
                    .split_whitespace()
                    .filter_map(|t| t.parse::<u32>().ok())
                    .collect();
                decode_strip_run(&inds, &mut tris);
            }
        }
    }
    tris
}

/// Finalize one polygonal rep: validate, skip geometry-less reps.
fn finalize_rep(
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
) -> Result<Option<Mesh>, MeshError> {
    if positions.is_empty() || triangles.is_empty() {
        // edge-only / empty rep (silhouette) — skipped, not an error
        return Ok(None);
    }
    for tri in &triangles {
        for &i in tri {
            if i as usize >= positions.len() {
                return Err(MeshError::IndexOutOfRange {
                    index: i,
                    positions: positions.len(),
                });
            }
        }
    }
    Ok(Some(Mesh {
        positions,
        triangles,
    }))
}

/// Accumulator for one polygonal rep while it is being parsed.
struct RepAcc {
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl RepAcc {
    fn new() -> Self {
        RepAcc {
            positions: Vec::new(),
            triangles: Vec::new(),
        }
    }
}

/// Parse a tessellated-ASCII `.3DRep` markup and return one internal [`Mesh`]
/// per geometry-bearing `PolygonalRepType` rep, in document order.
///
/// Both face encodings are decoded (`triangles=` and `strips=`, section
/// header). Reps without geometry are skipped; malformed input is a typed
/// [`MeshError`]; this function never panics.
pub fn parse_rep(input: &str) -> Result<Vec<Mesh>, MeshError> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);

    // Stack of open polygonal reps (innermost last), each with its collected
    // geometry. Non-polygonal reps (BagRepType wrappers) are ignored.
    let mut stack: Vec<RepAcc> = Vec::new();
    // Which open `<Rep>` elements are polygonal (parallel to `stack`).
    let mut rep_flags: Vec<bool> = Vec::new();
    let mut saw_root = false;
    let mut collect_positions = false;
    let mut positions_buf = String::new();
    let mut meshes: Vec<Mesh> = Vec::new();

    loop {
        let ev = reader
            .read_event()
            .map_err(|e| MeshError::MalformedXml(e.to_string()))?;
        match ev {
            Event::Start(e) => {
                let qname = e.name();
                let name = qname.as_ref();
                if !saw_root {
                    if name == b"XMLRepresentation" {
                        saw_root = true;
                    } else {
                        return Err(MeshError::NotXmlRepresentation);
                    }
                }
                if name == b"Rep" {
                    let flag = polygonal_type(&e).unwrap_or(false);
                    rep_flags.push(flag);
                    if flag {
                        stack.push(RepAcc::new());
                    }
                } else if name == b"Positions" && !stack.is_empty() {
                    collect_positions = true;
                    positions_buf.clear();
                } else if name == b"Face" && !stack.is_empty() {
                    let tris = decode_face_triangles(&e);
                    if let Some(top) = stack.last_mut() {
                        top.triangles.extend(tris);
                    }
                }
            }
            Event::Empty(e) => {
                let qname = e.name();
                let name = qname.as_ref();
                if !saw_root {
                    if name == b"XMLRepresentation" {
                        saw_root = true;
                    } else {
                        return Err(MeshError::NotXmlRepresentation);
                    }
                }
                if name == b"Face" && !stack.is_empty() {
                    let tris = decode_face_triangles(&e);
                    if let Some(top) = stack.last_mut() {
                        top.triangles.extend(tris);
                    }
                }
            }
            Event::Text(t) => {
                if collect_positions {
                    let txt = std::str::from_utf8(t.as_ref())
                        .map_err(|e| MeshError::MalformedXml(e.to_string()))?;
                    positions_buf.push_str(txt);
                }
            }
            Event::End(e) => {
                let qname = e.name();
                let name = qname.as_ref();
                if name == b"Positions" && collect_positions {
                    collect_positions = false;
                    let pos = decode_positions(&positions_buf)?;
                    if let Some(top) = stack.last_mut() {
                        top.positions.extend(pos);
                    }
                } else if name == b"Rep" {
                    let flag = rep_flags.pop();
                    if flag == Some(true) {
                        let acc = stack.pop().expect("flag/stack in sync");
                        if let Some(mesh) = finalize_rep(acc.positions, acc.triangles)? {
                            meshes.push(mesh);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !saw_root {
        return Err(MeshError::NotXmlRepresentation);
    }
    Ok(meshes)
}
