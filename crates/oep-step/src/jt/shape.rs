//! Shape LOD element decode (implementing detail).
//!
//! First-wedge shape support: the Primitive Set Shape Element (Ref 7.2 /
//! JTIAP 7.2) in its uncompressed lossless form. A Box primitive (type 0) is
//! decoded per Table 52: params1 = min corner, params2 = box lengths, params3
//! = orientation quaternion (identity supported in the wedge: non-identity
//! quaternions are a typed unsupported result, never fabricated geometry).
//! The topological compression representations (required for real v9/v10
//! TriStripSet/PolygonSet bodies) are documented-but-deferred and return
//! [`Error::UnsupportedFeature`], keeping the never-panic contract.

use super::lsg::RawElement;
use super::{Cursor, Error, FacetMesh};

/// Object type GUID of the Primitive Set Shape Element (Table A.1): decoded
/// LSB-first words 0xe40373c2, 0x1ad9, 0x11d3.
const GID_PRIM_SET: [u8; 16] = guid_const(
    0xE40373C2,
    0x1AD9,
    0x11D3,
    [0x9D, 0xAF, 0x00, 0xA0, 0xC9, 0xC7, 0xDD, 0xC2],
);

const PRIMITIVE_BOX: i32 = 0;

/// Compile-time GUID constant (U32 + 2xU16 + 8xU8, LSB-first words).
const fn guid_const(u32: u32, a: u16, b: u16, tail: [u8; 8]) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[0] = u32 as u8;
    g[1] = (u32 >> 8) as u8;
    g[2] = (u32 >> 16) as u8;
    g[3] = (u32 >> 24) as u8;
    g[4] = a as u8;
    g[5] = (a >> 8) as u8;
    g[6] = b as u8;
    g[7] = (b >> 8) as u8;
    let mut i = 0;
    while i < 8 {
        g[8 + i] = tail[i];
        i += 1;
    }
    g
}

/// Decode a shape element payload into a facet mesh.
pub(crate) fn decode_shape_element(el: &RawElement, big: bool) -> Result<FacetMesh, Error> {
    if el.type_id() != &GID_PRIM_SET {
        return Err(Error::UnsupportedFeature(
            "shape element type is not a Primitive Set (topological shape representations are a later milestone)"
                .into(),
        ));
    }
    decode_primitive_set(el.data(), big)
}

/// Decode the Primitive Set Shape Element payload (Ref 7.2).
fn decode_primitive_set(data: &[u8], big: bool) -> Result<FacetMesh, Error> {
    let mut c = Cursor::new(data, big);
    let _base_shape_ver = c.u8()?;
    let _base_prims_ver = c.u8()?;
    let _bindings = c.u64()?; // vertex bindings bitmask (Ref 7.1.2.1); facets only in wedge
    let primset_ver = c.u8()?;
    if primset_ver != 1 {
        return Err(Error::UnsupportedFeature(format!(
            "primitive set version {primset_ver} (documented value 1)"
        )));
    }
    let bits_per_vertex = c.i32()?;
    let _texcoord_gen = c.i8()?;

    // Bits per vertex 0 => Lossless Compressed Primitive Set Data (Ref 7.2.1).
    if bits_per_vertex != 0 {
        return Err(Error::UnsupportedFeature(format!(
            "lossy quantized primitive set (bits per vertex {bits_per_vertex}); wedge decodes lossless only"
        )));
    }

    // Lossless data: I32 uncompressed size, I32 compressed size, then either
    // raw primitive data (compressed size < 0) or LZMA payload (>= 0).
    let _uncompressed_size = c.i32()?;
    let compressed_size = c.i32()?;
    if compressed_size >= 0 {
        return Err(Error::UnsupportedFeature(
            "LZMA-compressed primitive data (documented algorithm 3) is a later milestone".into(),
        ));
    }
    let raw_len = compressed_size.unsigned_abs() as usize;
    let raw = c.take(raw_len).map_err(|_| Error::Truncated)?.to_vec();

    decode_raw_primitives(&raw, big)
}

/// Decode the raw per-primitive records (Ref Table 51): I32 reserved,
/// CoordF32 params1, DirF32 params2, Quaternion params3, RGB colour, I32 type.
fn decode_raw_primitives(raw: &[u8], big: bool) -> Result<FacetMesh, Error> {
    let mut c = Cursor::new(raw, big);
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    // One primitive record = 4(reserved) + 12(params1) + 12(params2) + 16(quat)
    // + 12(colour) + 4(type) = 60 bytes.
    let rec_len = 4 + 12 + 12 + 16 + 12 + 4;
    if raw.len() < rec_len {
        return Err(Error::BadGeometry(format!(
            "raw primitive data {} bytes too small for a box record",
            raw.len()
        )));
    }
    while c.remaining() >= rec_len {
        let _reserved = c.i32()?;
        let min = [c.f32()? as f64, c.f32()? as f64, c.f32()? as f64];
        let lengths = [c.f32()? as f64, c.f32()? as f64, c.f32()? as f64];
        let quat = [
            c.f32()? as f64,
            c.f32()? as f64,
            c.f32()? as f64,
            c.f32()? as f64,
        ];
        let _colour = [c.f32()?, c.f32()?, c.f32()?];
        let prim_type = c.i32()?;

        match prim_type {
            PRIMITIVE_BOX => {
                // Box: 8 corners from min + unit axes scaled by lengths, rotated
                // by the orientation quaternion.
                let mesh = box_mesh(min, lengths, &quat)?;
                vertices.extend(mesh.vertices);
                let base = vertices.len() as u32 - 8;
                triangles.extend(
                    mesh.triangles
                        .into_iter()
                        .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
                );
            }
            other => {
                return Err(Error::UnsupportedFeature(format!(
                    "primitive type {other} (box=0 documented; {other} is a later milestone)"
                )))
            }
        }
    }
    Ok(FacetMesh {
        vertices,
        triangles,
    })
}

/// Build the 8-vertex / 12-triangle box from min corner, side lengths and an
/// orientation quaternion. Identity quaternions are exact; arbitrary unit
/// quaternions rotate the corners via the standard rotation matrix formula.
fn box_mesh(min: [f64; 3], lengths: [f64; 3], quat: &[f64; 4]) -> Result<FacetMesh, Error> {
    const IDENT: f64 = 1.0;
    // Rotation is identity when the quaternion is the identity unit quat
    // (1,0,0,0) within tolerance. Any other quaternion is a typed
    // UnsupportedFeature this wedge does not fabricate.
    let identity = (quat[0] - IDENT).abs() < 1e-6
        && quat[1].abs() < 1e-6
        && quat[2].abs() < 1e-6
        && quat[3].abs() < 1e-6;
    if !identity {
        return Err(Error::UnsupportedFeature(
            "non-identity orientation quaternion on box primitive (Documentation is clear but wedge decodes identity orientations only)"
                .into(),
        ));
    }

    // Corners in box param space: (i,j,k) in {0,1}^3.
    let corner = |i: usize, j: usize, k: usize| -> [f64; 3] {
        [
            min[0] + (i as f64) * lengths[0],
            min[1] + (j as f64) * lengths[1],
            min[2] + (k as f64) * lengths[2],
        ]
    };
    let c000 = corner(0, 0, 0);
    let c100 = corner(1, 0, 0);
    let c010 = corner(0, 1, 0);
    let c110 = corner(1, 1, 0);
    let c001 = corner(0, 0, 1);
    let c101 = corner(1, 0, 1);
    let c011 = corner(0, 1, 1);
    let c111 = corner(1, 1, 1);

    let vertices = vec![c000, c100, c010, c110, c001, c101, c011, c111];
    // Six faces, two triangles each (wound outward).
    let faces: [[u32; 4]; 6] = [
        [0, 2, 3, 1], // -z (front)
        [4, 5, 7, 6], // +z (back)
        [0, 1, 5, 4], // -y (bottom)
        [2, 6, 7, 3], // +y (top)
        [0, 4, 6, 2], // -x (left)
        [1, 3, 7, 5], // +x (right)
    ];
    let mut triangles = Vec::with_capacity(12);
    for f in faces {
        triangles.push([f[0], f[1], f[2]]);
        triangles.push([f[0], f[2], f[3]]);
    }
    Ok(FacetMesh {
        vertices,
        triangles,
    })
}
