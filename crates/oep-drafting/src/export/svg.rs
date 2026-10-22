//! SVG writer for a rendered drawing (vector output, byte-deterministic).
//!
//! Emits an XML 1.0 SVG document sized to the sheet in millimetres. Drawing
//! coordinates are the paper model's own Y-up millimetre space (the SAME
//! coordinates the PDF export uses), so a third-angle sheet literally renders
//! top-above-front and the output is orientation-consistent with the PDF.
//! Fonts are the generic Helvetica/stem family, sizes in mm-equivalent `px`
//! (1:1 with the paper millimetre space). Every number is Rust's shortest
//! round-trip f64 form, so the output is byte-stable for a given input.
//!
//! Unlike the P6 mesh exporters, this document draws the FULL drawing (views,
//! title block, dimension annotations), not just a wireframe.

use std::fmt::Write as _;
use std::io::Write;

use super::metrics::text_width;
use super::{drawing_primitives, Prim};
use crate::render::RenderedDrawing;

/// Write the drawing as an SVG document to `w`.
pub fn write_drawing_svg(dr: &RenderedDrawing, w: &mut dyn Write) -> std::io::Result<()> {
    let (sw, sh) = (dr.width_mm as f64, dr.height_mm as f64);
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{sw}mm\" height=\"{sh}mm\" viewBox=\"0 0 {sw} {sh}\">\n"
    ));
    s.push_str(" <g fill=\"none\" stroke=\"black\" stroke-width=\"0.35\">\n");

    for prim in drawing_primitives(dr) {
        match prim {
            Prim::BeginGroup { name } => {
                writeln!(s, "  <g data-view=\"{name}\">").unwrap();
            }
            Prim::EndGroup => {
                s.push_str("  </g>\n");
            }
            Prim::Line { x1, y1, x2, y2 } => {
                writeln!(s, "  <path d=\"M {x1} {y1} L {x2} {y2}\"/>").unwrap();
            }
            Prim::Rect { x, y, w, h } => {
                writeln!(
                    s,
                    "  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>"
                )
                .unwrap();
            }
            Prim::Text { x, y, text, size } => {
                // Left-align the text by shifting the centre anchor by half its
                // measured width (shared Helvetica metrics with the PDF export).
                let x0 = x - text_width(&text, size) / 2.0;
                writeln!(
                    s,
                    "  <text x=\"{x0}\" y=\"{y}\" font-family=\"Helvetica,Arial,sans-serif\" font-size=\"{size}\" fill=\"black\" stroke=\"none\">{text}</text>"
                )
                .unwrap();
            }
        }
    }

    s.push_str(" </g>\n</svg>\n");
    w.write_all(s.as_bytes())
}
