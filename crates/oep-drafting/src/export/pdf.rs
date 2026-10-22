//! PDF 1.4 writer for a rendered drawing (vector output, byte-deterministic).
//!
//! Produces a minimal valid PDF 1.4 whose single page draws the SAME
//! primitive model as the SVG export ([`super::svg::write_drawing_svg`]):
//! border, title block, projected-view edges and dimension annotations.
//! Paper millimetres map 1:1 onto PDF user units (origin bottom-left, Y up),
//! so no coordinate transform is needed. Text uses the PDF base-14 Helvetica
//! font with WinAnsiEncoding (our labels are ASCII), positioned with the
//! shared text metrics so centring matches the SVG export.
//!
//! File layout mirrors the P6 exporters' minimal hand-rolled structure:
//! numbered objects, an xref table with real byte offsets, trailer and
//! `startxref`. No compression, no external resources. Numbers are Rust's
//! shortest round-trip f64 form, so a given drawing produces byte-identical
//! output.

use std::fmt::Write as _;
use std::io::Write;

use super::metrics::text_width;
use super::{drawing_primitives, Prim};
use crate::render::RenderedDrawing;

/// Escape a text string for a PDF literal (parens and backslash).
fn pdf_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 32 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Write the drawing as a one-page PDF 1.4 document to `w`.
pub fn write_drawing_pdf(dr: &RenderedDrawing, w: &mut dyn Write) -> std::io::Result<()> {
    let (sw, sh) = (dr.width_mm as f64, dr.height_mm as f64);

    // Content stream.
    let mut content = String::new();
    content.push_str("0.35 w\n");
    for prim in drawing_primitives(dr) {
        match prim {
            // Group markers are an SVG structure aid; PDF draws geometry only.
            Prim::BeginGroup { .. } | Prim::EndGroup => {}
            Prim::Line { x1, y1, x2, y2 } => {
                writeln!(content, "{x1} {y1} m {x2} {y2} l S").unwrap();
            }
            Prim::Rect { x, y, w, h } => {
                writeln!(content, "{x} {y} {w} {h} re S").unwrap();
            }
            Prim::Text { x, y, text, size } => {
                // Centre at (x, y): left-align by half the text width.
                let x0 = x - text_width(&text, size) / 2.0;
                // Baseline slightly below the centre anchor so the label reads
                // centred on the line (cap height ~ 0.7 em).
                let y0 = y - 0.35 * size;
                writeln!(
                    content,
                    "BT /F1 {size} Tf {x0} {y0} Td ({}) Tj ET",
                    pdf_escape(&text)
                )
                .unwrap();
            }
        }
    }

    let font = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>";
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {sw} {sh}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
        ),
        font.to_string(),
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            content.len(),
            content
        ),
    ];

    // Assemble the file byte-by-byte, recording each object's byte offset.
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(body.len());
        writeln!(body, "{} 0 obj", i + 1).unwrap();
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

    w.write_all(&body)
}
