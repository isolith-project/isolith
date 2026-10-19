//! STL export (binary + ASCII) for the internal tessellation mesh (P6-M4-T01).
//!
//! Writes an indexed triangle mesh (`positions` + `triangles` slices) as
//! **binary STL** or **ASCII STL**, exactly following the P2-M2-T02 format
//! contract (`crates/oep-cgr/src/stl.rs`) so both exporters are
//! byte-interchangeable on the same mesh.
//!
//! ## Format contract
//!
//! **Binary STL**:
//!   * 80-byte header, zero-filled;
//!   * `u32` little-endian triangle count;
//!   * per triangle (50 bytes): facet normal `3 x f32`, the 3 vertices
//!     `9 x f32`, `u16` attribute byte count (0).
//!
//! The facet normal is the right-hand-rule unit normal
//! `normalize(cross(v1 - v0, v2 - v0))`. Consumers MUST NOT trust STL normals
//! (the format says readers recompute); we still write them for interchange.
//!
//! **ASCII STL**: `solid <name>` ... `endsolid <name>` with one
//! `facet`/`outer loop`/`endloop`/`endfacet` block per triangle; numbers in
//! shortest-round-trip form.
//!
//! Errors are typed and never panic, per the sibling exporter discipline.

use std::io::Write;

use super::base::{facet_normal, triangle_positions, ExportError};

/// Write the mesh as **binary** STL into `w` (byte-for-byte stable).
pub fn write_binary_stl<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    let mut out = Vec::with_capacity(84 + 50 * triangles.len());
    out.resize(80, 0u8); // header
    out.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for tri in triangles {
        let pts = triangle_positions(positions, tri)?;
        let normal = facet_normal(pts[0], pts[1], pts[2]).ok_or(ExportError::DegenerateTriangle)?;
        for c in normal {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for p in pts {
            for c in p {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    w.write_all(&out)?;
    Ok(())
}

/// Write the mesh as **ASCII** STL (legacy `solid` syntax) into `w`.
pub fn write_ascii_stl<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    w.write_all(b"solid oep-geo mesh\n")?;
    for tri in triangles {
        let pts = triangle_positions(positions, tri)?;
        let normal = facet_normal(pts[0], pts[1], pts[2]).ok_or(ExportError::DegenerateTriangle)?;
        writeln!(
            w,
            "  facet normal {} {} {}\n    outer loop",
            normal[0], normal[1], normal[2]
        )?;
        for p in pts {
            writeln!(w, "      vertex {} {} {}", p[0], p[1], p[2])?;
        }
        writeln!(w, "    endloop\n  endfacet")?;
    }
    w.write_all(b"endsolid oep-geo mesh\n")?;
    Ok(())
}
