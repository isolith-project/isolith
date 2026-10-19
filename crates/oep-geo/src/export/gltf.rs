//! glTF 2.0 / GLB mesh export (P6-M4-T01).
//!
//! Writes an indexed triangle mesh (`positions` + `triangles` slices) to a
//! **binary glTF 2.0 container (GLB)**, or the equivalent `.gltf` (JSON) +
//! `.bin` (buffer) pair. The layout exactly follows the P2-M2-T03 contract
//! (`crates/oep-cgr/src/gltf.rs`) so both exporters are byte-interchangeable
//! and the same committed golden (`tests/golden/box-reference.glb`, generated
//! by the independent encoder `tools/gen_golden_box_glb.py`) applies to both.
//!
//! ## GLB layout (Khronos glTF 2.0 spec §5.2)
//!
//! * `u32` magic `0x46546C67` ("glTF"), `u32` version `2`, `u32` total length;
//! * JSON chunk: `u32` length + `u32` type `0x4E4F534A` ("JSON") + JSON
//!   space-padded to a 4-byte multiple;
//! * BIN chunk: `u32` length + `u32` type `0x004E4942` ("BIN\0") + buffer
//!   (positions then indices) zero-padded to a 4-byte multiple.
//!
//! JSON numbers use shortest-round-trip `f32` formatting with integer
//! literals for integral values, matching the independent encoder.

use std::fmt::Write as _;
use std::io::Write;

use super::base::{cross, ExportError};

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
/// glTF `TRIANGLES` primitive mode.
const MODE_TRIANGLES: u32 = 4;

/// Validate a triangle (bounds, finiteness, non-degenerate).
fn check_triangle(positions: &[[f32; 3]], tri: &[u32; 3]) -> Result<(), ExportError> {
    let pts = super::base::triangle_positions(positions, tri)?;
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
    if mag <= super::base::DEGENERATE_EPS {
        return Err(ExportError::DegenerateTriangle);
    }
    Ok(())
}

fn push_f32(s: &mut String, v: f32) {
    write!(s, "{v}").expect("writing to a String cannot fail");
}

/// glTF `UNSIGNED_INT` accessor component type (32-bit indices).
const COMPONENT_UINT: u32 = 5_125;

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
fn build_json(positions: &[[f32; 3]], triangles: &[[u32; 3]], buffer_uri: Option<&str>) -> String {
    let pos_bytes = 12 * positions.len();
    let idx_component = index_component(positions.len());
    let idx_bytes = index_width(idx_component) * 3 * triangles.len();

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

/// Build the binary buffer payload: `positions` as LE f32 triples then
/// the index list (u16).
fn build_bin(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<u8> {
    let mut bin = Vec::with_capacity(12 * positions.len() + 3 * triangles.len() * 2);
    for p in positions {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    for tri in triangles {
        for &i in tri {
            bin.extend_from_slice(&(i as u16).to_le_bytes());
        }
    }
    bin
}

fn pad_to_four(mut bytes: Vec<u8>, fill: u8) -> Vec<u8> {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(fill);
    }
    bytes
}

/// Write the mesh as a **binary glTF 2.0 container (GLB)** into `w`.
pub fn write_glb<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
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

/// Write the `.gltf` JSON document (referencing an external `.bin`) into
/// `gltf_out`. The caller then writes `<bin_uri>` from [`write_gltf_bin`].
pub fn write_gltf_json(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    bin_uri: &str,
    gltf_out: &mut String,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    for tri in triangles {
        check_triangle(positions, tri)?;
    }
    *gltf_out = build_json(positions, triangles, Some(bin_uri));
    Ok(())
}

/// Write the `.bin` buffer payload referenced by [`write_gltf_json`] into
/// `bin_out`.
pub fn write_gltf_bin<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    bin_out: &mut W,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    for tri in triangles {
        check_triangle(positions, tri)?;
    }
    bin_out.write_all(&build_bin(positions, triangles))?;
    Ok(())
}
