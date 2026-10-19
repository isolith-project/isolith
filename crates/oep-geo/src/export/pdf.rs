//! PDF 1.4 vector drawing export (projected-edge wireframe, one page).
//!
//! Produces a minimal, valid PDF 1.4 whose single page's content stream draws
//! the SAME 2D projected-edge wireframe as the SVG export
//! ([`super::svg::write_svg_2d`]). The projection is an orthographic XY
//! projection (drop Z) of the unique undirected mesh edges.
//!
//! ## Format contract (byte-for-byte, matches
//! [`crates/oep-geo/tools/gen_golden_box_pdf.py`])
//!
//! * 4 objects only: Catalog, Pages, Page, Content stream (no compression);
//! * `MediaBox [0 0 W H]` where `W`/`H` = projected bounds + `1.0` unit
//!   margin, and all projected coordinates are translated so the view is
//!   inside `[0, W] x [0, H]` (offsets `-min + 0.5`);
//! * content stream: one `x1 y1 m x2 y2 l S\n` operator per unique projected
//!   edge;
//! * xref table with byte offsets, trailer and `startxref` exactly as the
//!   independent encoder computes them (so the file is byte-identical).
//!
//! PDF numbers use the same shortest-round-trip `f32` formatter as the other
//! exporters (integral values as `"1"`, `"0"`). Errors are typed and never
//! panic.

use std::io::Write;

use super::base::{bounds2, unique_projected_edges, ExportError};

/// Write the mesh as a one-page PDF 1.4 drawing (XY projected edges).
pub fn write_pdf_drawing<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), ExportError> {
    let edges = unique_projected_edges(positions, triangles)?;
    if edges.is_empty() {
        return Err(ExportError::EmptyMesh);
    }

    let (mn, mx) = bounds2(&edges);
    let page_w = (mx[0] - mn[0]) + 1.0f32;
    let page_h = (mx[1] - mn[1]) + 1.0f32;
    let tx = -mn[0] + 0.5f32;
    let ty = -mn[1] + 0.5f32;

    // Content stream: one `x1 y1 m x2 y2 l S` operator per edge, translated.
    let mut content = String::new();
    for [p, q] in &edges {
        content.push_str(&format!(
            "{0} {1} m {2} {3} l S\n",
            p[0] + tx,
            p[1] + ty,
            q[0] + tx,
            q[1] + ty
        ));
    }

    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R >>",
            page_w, page_h
        ),
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            content.len(),
            content
        ),
    ];

    // Assemble the file byte-for-byte, recording each object's offset.
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(body.len());
        writeln!(body, "{} 0 obj", i + 1).expect("write to Vec<u8>");
        body.extend_from_slice(obj.as_bytes());
        body.extend_from_slice(b"\nendobj\n");
    }

    let xref_pos = body.len();
    body.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    body.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        body.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    body.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_pos
        )
        .as_bytes(),
    );

    w.write_all(&body)?;
    Ok(())
}
