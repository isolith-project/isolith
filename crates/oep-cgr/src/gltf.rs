//! glTF 2.0 / GLB mesh export for the internal tessellation mesh (P2-M2-T03).
//!
//! Writes an indexed triangle mesh (`positions: &[[f32; 3]]`,
//! `triangles: &[[u32; 3]]`) to a **binary glTF 2.0 container (GLB)**, or the
//! equivalent `.gltf` (JSON) + `.bin` (buffer) pair. The coordinates are the
//! internal mesh representation named by P2-M2-T01 (vertices/indices/
//! triangles); P2-M2-T02 added STL on the same input.
//!
//! ## Format contract (public Khronos glTF 2.0 spec)
//!
//! **GLB** layout (spec §5.2 — Binary glTF Layout):
//!   * a `u32` magic `0x46546C67` (`"glTF"`), a `u32` version `2`, and a
//!     `u32` total length (all little-endian);
//!   * a JSON chunk: 4-byte length + 4-byte type `0x4E4F534A` (`"JSON"`)
//!     + the JSON document right-padded with spaces to a 4-byte multiple;
//!   * a BIN chunk: 4-byte length + 4-byte type `0x004E4942` (`"BIN\0"`)
//!     + the binary buffer (positions then indices) zero-padded to a
//!     4-byte multiple, referenced as `buffers[0]`.
//!
//! The JSON document embeds the exact data layout the golden generator locks:
//!   * `buffers[0].byteLength` = `12*V + index_width*3*T`;
//!   * `bufferViews[0]`: positions, target `34962` (`ARRAY_BUFFER`);
//!   * `bufferViews[1]`: indices, target `34963` (`ELEMENT_ARRAY_BUFFER`);
//!   * `accessors[0]`: `POSITION`, `VEC3`, `FLOAT` (`5126`), count `V`,
//!     with computed `min`/`max`;
//!   * `accessors[1]`: indices, `SCALAR`, `UNSIGNED_SHORT` (`5123`, when
//!     `V < 65536`) or `UNSIGNED_INT` (`5125`);
//!   * one `TRIANGLES` primitive (`mode: 4`), one node, one scene.
//!
//! Numbers in the JSON use shortest round-trip `f32` formatting with integer
//! literals for integral values (`0` not `0.0`), matching the independent
//! encoder `tools/gen_golden_box_glb.py` so the committed golden compares
//! byte-for-byte.
//!
//! ## Golden-file discipline
//!
//! `tests/golden/box-reference.glb` is produced by
//! `tools/gen_golden_box_glb.py` — an INDEPENDENT spec encoder, NOT this
//! writer — so `tests/golden_gltf.rs` proves byte-for-byte conformance to the
//! reference tessellation (`corpus/data/box.stl`, CC0-1.0 P0-M4-T02 fixture).
//!
//! Errors are typed and never panic, mirroring `stl`: indices are bounds
//! checked (`GltfError::IndexOutOfRange`), non-finite positions rejected
//! (`GltfError::NonFiniteVertex`), degenerate zero-area triangles rejected
//! (`GltfError::DegenerateTriangle`), IO failures surface as
//! `GltfError::Io`. The exporter is safe to run over untrusted geometry
//! (fuzz-friendly, tolerant-failure heritage P2-M1-T04).

use std::fmt::{self, Write as _};
use std::io::Write;

/// A triangle with zero area under the right-hand rule (duplicate or
/// co-linear vertices) — would produce a degenerate GLB primitive.
const DEGENERATE_EPS: f32 = 1e-12;

/// GLB magic: little-endian `0x46546C67` == `"glTF"`.
const GLB_MAGIC: u32 = 0x4654_6C67;
/// GLB version.
const GLB_VERSION: u32 = 2;
/// JSON chunk type: little-endian `0x4E4F534A` == `"JSON"`.
const CHUNK_JSON: u32 = 0x4E4F_534A;
/// BIN chunk type: little-endian `0x004E4942` == `"BIN\0"`.
const CHUNK_BIN: u32 = 0x004E_4942;
/// glTF `ARRAY_BUFFER` target.
const TARGET_ARRAY_BUFFER: u32 = 34_962;
/// glTF `ELEMENT_ARRAY_BUFFER` target.
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34_963;
/// glTF `FLOAT` accessor component type.
const COMPONENT_FLOAT: u32 = 5_126;
/// glTF `UNSIGNED_SHORT` accessor component type (16-bit indices).
const COMPONENT_USHORT: u32 = 5_123;
/// glTF `UNSIGNED_INT` accessor component type (32-bit indices).
const COMPONENT_UINT: u32 = 5_125;
/// glTF `TRIANGLES` primitive mode.
const MODE_TRIANGLES: u32 = 4;

/// Errors produced during glTF/GLB export.
#[derive(Debug)]
pub enum GltfError {
    /// A triangle index referenced a vertex position that does not exist.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of positions in the mesh.
        len: usize,
    },
    /// A vertex position is NaN or infinite.
    NonFiniteVertex(usize),
    /// A triangle is degenerate (zero area) and cannot be exported.
    DegenerateTriangle,
    /// Underlying IO failure while writing.
    Io(std::io::Error),
}

impl fmt::Display for GltfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GltfError::IndexOutOfRange { index, len } => {
                write!(f, "triangle index {index} out of range (positions: {len})")
            }
            GltfError::NonFiniteVertex(i) => write!(f, "vertex {i} not finite"),
            GltfError::DegenerateTriangle => write!(f, "degenerate triangle (zero area)"),
            GltfError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for GltfError {}

impl From<std::io::Error> for GltfError {
    fn from(e: std::io::Error) -> Self {
        GltfError::Io(e)
    }
}

/// Cross product `u x v` as an f32 triple.
fn cross(u: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Resolve a triangle's three positions, bounds-checked and finite-checked.
fn triangle_positions(positions: &[[f32; 3]], tri: &[u32; 3]) -> Result<[[f32; 3]; 3], GltfError> {
    let mut out = [[0.0f32; 3]; 3];
    for (k, &idx) in tri.iter().enumerate() {
        let i = idx as usize;
        if i >= positions.len() {
            return Err(GltfError::IndexOutOfRange {
                index: idx,
                len: positions.len(),
            });
        }
        let v = positions[i];
        if !v.iter().all(|c| c.is_finite()) {
            return Err(GltfError::NonFiniteVertex(i));
        }
        out[k] = v;
    }
    Ok(out)
}

/// Validate a triangle (bounds, finiteness, non-degenerate).
fn check_triangle(positions: &[[f32; 3]], tri: &[u32; 3]) -> Result<(), GltfError> {
    let pts = triangle_positions(positions, tri)?;
    let u = [
        pts[1][0] - pts[0][0],
        pts[1][1] - pts[0][1],
        pts[1][2] - pts[0][2],
    ];
    let v = [
        pts[2][0] - pts[0][0],
        pts[2][1] - pts[0][1],
        pts[2][2] - pts[0][2],
    ];
    let n = cross(u, v);
    let mag = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if mag <= DEGENERATE_EPS {
        return Err(GltfError::DegenerateTriangle);
    }
    Ok(())
}

/// Append one `f32` as a shortest round-trip literal; integral values render
/// without a decimal point (`0` not `0.0`), matching `gen_golden_box_glb.py`.
fn push_f32(s: &mut String, v: f32) {
    write!(s, "{v}").expect("writing to a String cannot fail");
}

/// Component type for the indices accessor given the vertex count.
fn index_component(vertex_count: usize) -> u32 {
    if vertex_count >= 65_536 {
        COMPONENT_UINT
    } else {
        COMPONENT_USHORT
    }
}

/// Width in bytes of one index for [`index_component`].
fn index_width(component: u32) -> usize {
    match component {
        COMPONENT_USHORT => 2,
        _ => 4,
    }
}

/// Build the compact glTF 2.0 JSON document for this mesh.
///
/// With `buffer_uri == None` (GLB form) `buffers[0]` carries only
/// `byteLength` and is served by the BIN chunk; with `Some(uri)` (.gltf form)
/// the same document references an external buffer. Everything else is
/// identical. Byte-compatible with `build_json` in
/// `tools/gen_golden_box_glb.py` for the reference box.
fn build_json(positions: &[[f32; 3]], triangles: &[[u32; 3]], buffer_uri: Option<&str>) -> String {
    let pos_bytes = 12 * positions.len();
    let idx_component = index_component(positions.len());
    let idx_bytes = index_width(idx_component) * 3 * triangles.len();

    // min/max over all three axes (required by glTF for POSITION accessors).
    let (min_s, max_s) = if positions.is_empty() {
        ("[]".to_string(), "[]".to_string())
    } else {
        let mut mn = [f32::INFINITY; 3];
        let mut mx = [f32::NEG_INFINITY; 3];
        for p in positions {
            for a in 0..3 {
                mn[a] = mn[a].min(p[a]);
                mx[a] = mx[a].max(p[a]);
            }
        }
        let mut mn_s = String::new();
        let mut mx_s = String::new();
        for a in 0..3 {
            if a > 0 {
                mn_s.push(',');
                mx_s.push(',');
            }
            push_f32(&mut mn_s, mn[a]);
            push_f32(&mut mx_s, mx[a]);
        }
        (format!("[{mn_s}]"), format!("[{mx_s}]"))
    };

    let mut s = String::with_capacity(512 + 16 * positions.len() + 8 * triangles.len());
    s.push_str("{\"asset\":{\"version\":\"2.0\"},");
    match buffer_uri {
        Some(uri) => write!(
            s,
            "\"buffers\":[{{\"byteLength\":{},\"uri\":\"{uri}\"}}]",
            pos_bytes + idx_bytes
        )
        .expect("write to String"),
        None => write!(
            s,
            "\"buffers\":[{{\"byteLength\":{}}}]",
            pos_bytes + idx_bytes
        )
        .expect("write to String"),
    }
    write!(
        s,
        ",\"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{pos_bytes},\
         \"target\":{TARGET_ARRAY_BUFFER}}},{{\"buffer\":0,\"byteOffset\":{pos_bytes},\
         \"byteLength\":{idx_bytes},\"target\":{TARGET_ELEMENT_ARRAY_BUFFER}}}]"
    )
    .expect("write to String");
    write!(
        s,
        ",\"accessors\":[{{\"bufferView\":0,\"byteOffset\":0,\"componentType\":{COMPONENT_FLOAT},\
         \"count\":{},\"type\":\"VEC3\",\"min\":{min_s},\"max\":{max_s}}},\
         {{\"bufferView\":1,\"byteOffset\":0,\"componentType\":{idx_component},\
         \"count\":{},\"type\":\"SCALAR\"}}]",
        positions.len(),
        3 * triangles.len()
    )
    .expect("write to String");
    write!(
        s,
        ",\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\
         \"indices\":1,\"mode\":{MODE_TRIANGLES}}}]}}],"
    )
    .expect("write to String");
    s.push_str("\"nodes\":[{\"mesh\":0}],\"scenes\":[{\"nodes\":[0]}],\"scene\":0}");
    s
}

/// Build the binary buffer payload: `positions` as LE f32 triples, then the
/// index list in the width implied by [`index_component`].
fn build_bin(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<u8> {
    let idx_component = index_component(positions.len());
    let mut bin = Vec::with_capacity(12 * positions.len() + 3 * triangles.len() * 4);
    for p in positions {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let width = index_width(idx_component);
    for tri in triangles {
        for &i in tri {
            match width {
                2 => bin.extend_from_slice(&(i as u16).to_le_bytes()),
                _ => bin.extend_from_slice(&i.to_le_bytes()),
            }
        }
    }
    bin
}

/// Pad `bytes` with `fill` copies so its length is a multiple of 4.
fn pad_to_four(mut bytes: Vec<u8>, fill: u8) -> Vec<u8> {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(fill);
    }
    bytes
}

/// Write the mesh as a **binary glTF 2.0 container (GLB)** into `w`.
///
/// `buffers[0]` carries no `uri` and is served by the GLB BIN chunk. Any GLB
/// reader (Blender, three.js, gltf-viewer) can open the output directly.
pub fn write_glb<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), GltfError> {
    for tri in triangles {
        check_triangle(positions, tri)?;
    }
    let json = build_json(positions, triangles, None);
    let json_chunk = pad_to_four(json.into_bytes(), b' ');
    let bin = pad_to_four(build_bin(positions, triangles), 0u8);

    let total = 12 + 8 + json_chunk.len() + 8 + bin.len();
    let mut head = Vec::with_capacity(total);
    head.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    head.extend_from_slice(&GLB_VERSION.to_le_bytes());
    head.extend_from_slice(&(total as u32).to_le_bytes());
    head.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    head.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    head.extend_from_slice(&json_chunk);
    head.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    head.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    head.extend_from_slice(&bin);
    w.write_all(&head)?;
    Ok(())
}

/// Write the `.gltf` JSON document (referencing an external `.bin` named by
/// `bin_uri`) into `gltf_out`. The caller then writes `<bin_uri>` from
/// [`write_gltf_bin`]; together the pair carries the same buffer layout,
/// accessors and geometry as [`write_glb`].
pub fn write_gltf_json(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    bin_uri: &str,
    gltf_out: &mut String,
) -> Result<(), GltfError> {
    for tri in triangles {
        check_triangle(positions, tri)?;
    }
    *gltf_out = build_json(positions, triangles, Some(bin_uri));
    Ok(())
}

/// Write the `.bin` buffer payload referenced by [`write_gltf_json`] into
/// `bin_out` (the array of LE f32 positions followed by the index list, in
/// the width implied by the vertex count).
pub fn write_gltf_bin<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    bin_out: &mut W,
) -> Result<(), GltfError> {
    for tri in triangles {
        check_triangle(positions, tri)?;
    }
    bin_out.write_all(&build_bin(positions, triangles))?;
    Ok(())
}
