//! 2D SVG drawing export (projected-edge wireframe) for the internal mesh.
//!
//! A "2D drawing" of a tessellated part is its orthographic projection onto a
//! plane; this exporter projects onto the XY plane (drops Z, the default
//! "top view") and emits the projected UNIQUE undirected edges as vector line
//! segments inside a single `<path>` element -- a projected-edge wireframe
//! (NOT a hidden-line-removed silhouette; HLR is a later drawing milestone).
//!
//! ## Format contract (byte-for-byte, matches the independent golden encoder
//! [`crates/oep-geo/tools/gen_golden_box_svg.py`])
//!
//! * deterministic first-seen dedup of undirected projected edges keyed by
//!   their two 2D endpoints in canonical order (smaller endpoint first);
//! * edges whose projected endpoints coincide (zero-length in this
//!   projection) are dropped;
//! * `viewBox` covers the projected bounds inflated by a fixed `0.5` margin;
//! * coordinates render in shortest-round-trip `f32` form (integral values
//!   as integer literals, matching Rust's `f32` Display).
//!
//! Errors are typed and never panic, per the sibling exporter discipline.

use std::fmt::Write as _;
use std::io::Write;

use super::base::{bounds2, unique_projected_edges, ExportError};

/// Write the mesh as a 2D SVG document (orthographic XY projected edges).
pub fn write_svg_2d<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    let edges = unique_projected_edges(positions, triangles)?;
    if edges.is_empty() {
        return Err(ExportError::EmptyMesh);
    }

    let (mn, mx) = bounds2(&edges);
    let pad = 0.5f32;
    let vbx = mn[0] - pad;
    let vby = mn[1] - pad;
    let vbw = (mx[0] - mn[0]) + 2.0 * pad;
    let vbh = (mx[1] - mn[1]) + 2.0 * pad;

    let mut d = String::new();
    for [p, q] in &edges {
        write!(d, "M {} {} L {} {} ", p[0], p[1], q[0], q[1])
            .expect("writing to a String cannot fail");
    }
    // strip the trailing space so the path d matches the encoder exactly
    d.pop();

    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vbx} {vby} {vbw} {vbh}\">\n\
         \x20 <g fill=\"none\" stroke=\"black\" stroke-width=\"0.05\">\n\
         \x20   <path d=\"{d}\"/>\n\
         \x20 </g>\n\
         </svg>\n"
    );

    w.write_all(doc.as_bytes())?;
    Ok(())
}
