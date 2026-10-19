//! Wavefront OBJ export (P6-M4-T01).
//!
//! Writes an indexed triangle mesh (`positions` + `triangles` slices) as a
//! minimal, dependency-free Wavefront OBJ text file: one `v x y z` record per
//! position (in input order) and one `f a b c` record per triangle (1-based,
//! space-separated indices, no per-corner options). A comment header records
//! the exporter and the primitive counts.
//!
//! `v` coordinates render in shortest-round-trip `f32` form (integer literals
//! for integral values), matching the independent golden encoder
//! `tools/gen_golden_box_obj.py`. Errors are typed and never panic.

use std::io::Write;

use super::base::ExportError;

/// Write the mesh as a Wavefront OBJ document into `w`.
pub fn write_obj<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    if triangles.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    writeln!(w, "# oep-geo -- Wavefront OBJ export (P6-M4-T01)")?;
    writeln!(
        w,
        "# {} vertices, {} triangles (reference tessellation)",
        positions.len(),
        triangles.len()
    )?;
    for p in positions {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(ExportError::NonFiniteVertex(0));
        }
        writeln!(w, "v {} {} {}", p[0], p[1], p[2])?;
    }
    for tri in triangles {
        for &idx in tri {
            if idx as usize >= positions.len() {
                return Err(ExportError::IndexOutOfRange {
                    index: idx,
                    len: positions.len(),
                });
            }
        }
        writeln!(w, "f {} {} {}", tri[0] + 1, tri[1] + 1, tri[2] + 1)?;
    }
    Ok(())
}
