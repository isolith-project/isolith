//! Vector export of a rendered drawing: a small primitive model shared by the
//! SVG and PDF writers.
//!
//! Paper coordinates are millimetres, origin at the sheet bottom-left, Y up.
//! The writers adapt to their own coordinate systems (SVG flips Y, PDF keeps
//! it). Both are deterministic (byte-for-byte for a given input) and typed —
//! text is ASCII-only so both stay unambiguous and byte-stable.

mod metrics;

pub mod pdf;
pub mod svg;

pub use self::pdf::write_drawing_pdf;
pub use self::svg::write_drawing_svg;

use crate::render::RenderedDrawing;
use oep_geo::TitleBlock;

/// Border margin around the sheet, mm (mirrors `render::BORDER_MM`).
pub const BORDER_MM: f64 = crate::render::BORDER_MM;
/// Title block width, mm.
pub const TITLE_BLOCK_W: f64 = crate::render::TITLE_BLOCK_W;
/// Title block height, mm.
pub const TITLE_BLOCK_H: f64 = crate::render::TITLE_BLOCK_H;
/// Label font size, mm.
pub const TEXT_MM: f64 = crate::render::TEXT_MM;
/// Tick length across dimension line ends, mm.
pub const TICK_MM: f64 = 2.0;

/// A drawable primitive in paper coordinates (mm, Y up).
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    /// Open a named group (e.g. one view's edge set). SVG emits a `<g
    /// data-view="...">`; PDF ignores the marker (no-op).
    BeginGroup {
        /// The group's name/label.
        name: String,
    },
    /// Close the current group.
    EndGroup,
    /// A line segment.
    Line {
        /// Start x.
        x1: f64,
        /// Start y.
        y1: f64,
        /// End x.
        x2: f64,
        /// End y.
        y2: f64,
    },
    /// An axis-aligned rectangle (x/y bottom-left corner + size).
    Rect {
        /// Bottom-left x.
        x: f64,
        /// Bottom-left y.
        y: f64,
        /// Width.
        w: f64,
        /// Height.
        h: f64,
    },
    /// A centred-aligned text label (x/y is the CENTRE anchor, Y up).
    Text {
        /// Centre x.
        x: f64,
        /// Centre y.
        y: f64,
        /// The text.
        text: String,
        /// Font size.
        size: f64,
    },
}

/// Build the full primitive list for a rendered drawing: border frame, title
/// block, each view's projected edges + label, and each dimension line + label.
pub fn drawing_primitives(dr: &RenderedDrawing) -> Vec<Prim> {
    let mut out = Vec::new();
    let (w, h) = (dr.width_mm as f64, dr.height_mm as f64);

    // Border frame.
    out.push(Prim::Rect {
        x: BORDER_MM,
        y: BORDER_MM,
        w: w - 2.0 * BORDER_MM,
        h: h - 2.0 * BORDER_MM,
    });

    // Title block.
    out.extend(title_block_primitives(&dr.title_block, w, h));

    // Views (each grouped so the projected edges are independently verifiable).
    for v in &dr.views {
        out.push(Prim::BeginGroup {
            name: v.label.clone(),
        });
        for e in &v.edges_paper {
            out.push(Prim::Line {
                x1: e[0][0],
                y1: e[0][1],
                x2: e[1][0],
                y2: e[1][1],
            });
        }
        out.push(Prim::EndGroup);
        // View label anchored above-left of the view bbox.
        out.push(Prim::Text {
            x: v.cx - v.extent_w / 2.0,
            y: v.cy + v.extent_h / 2.0 + 3.0,
            text: v.label.clone(),
            size: TEXT_MM,
        });
    }

    // Dimensions.
    for d in &dr.dimensions {
        out.push(Prim::Line {
            x1: d.ax,
            y1: d.ay,
            x2: d.bx,
            y2: d.by,
        });
        // End ticks across the dimension line.
        let tick = TICK_MM;
        let (tx, ty) = match d.kind {
            oep_geo::DimensionKind::Horizontal => (0.0, tick),
            oep_geo::DimensionKind::Vertical => (tick, 0.0),
        };
        out.push(Prim::Line {
            x1: d.ax - tx,
            y1: d.ay - ty,
            x2: d.ax + tx,
            y2: d.ay + ty,
        });
        out.push(Prim::Line {
            x1: d.bx - tx,
            y1: d.by - ty,
            x2: d.bx + tx,
            y2: d.by + ty,
        });
        // Label centred on the dimension line.
        out.push(Prim::Text {
            x: (d.ax + d.bx) / 2.0,
            y: (d.ay + d.by) / 2.0 + TEXT_MM,
            text: d.label.clone(),
            size: TEXT_MM,
        });
    }

    out
}

/// The title block table: an outer frame plus "LABEL: value" rows.
fn title_block_primitives(tb: &TitleBlock, sheet_w: f64, _sheet_h: f64) -> Vec<Prim> {
    let left = sheet_w - BORDER_MM - TITLE_BLOCK_W;
    let bottom = BORDER_MM;
    let mut out = vec![Prim::Rect {
        x: left,
        y: bottom,
        w: TITLE_BLOCK_W,
        h: TITLE_BLOCK_H,
    }];

    // Rows: title spans the full width; the rest share two columns.
    let row_h = TITLE_BLOCK_H / 4.0;
    let col = TITLE_BLOCK_W / 2.0;
    let mut row = |label: &str, value: &str, x: f64, y_top: f64| {
        let y = y_top - row_h;
        out.push(Prim::Line {
            x1: x,
            y1: y,
            x2: x + TITLE_BLOCK_W,
            y2: y,
        });
        out.push(Prim::Text {
            x: x + 2.0,
            y: y + row_h / 2.0,
            text: format!("{label}: {value}"),
            size: TEXT_MM,
        });
    };

    let top = bottom + TITLE_BLOCK_H;
    // Title row (full width).
    row("TITLE", &tb.title, left, top);
    // Material | scale row.
    row(
        "MATERIAL",
        tb.material.as_deref().unwrap_or(""),
        left,
        top - row_h,
    );
    row(
        "SCALE",
        tb.scale.as_deref().unwrap_or(""),
        left + col,
        top - row_h,
    );
    // Part no | revision row.
    row(
        "PART NO",
        tb.part_number.as_deref().unwrap_or(""),
        left,
        top - 2.0 * row_h,
    );
    row(
        "REV",
        tb.revision.as_deref().unwrap_or(""),
        left + col,
        top - 2.0 * row_h,
    );
    // Drawn by/date | sheet row.
    let drawn = match (&tb.drawn_by, &tb.date) {
        (Some(d), Some(dt)) => format!("{d} {dt}"),
        (Some(d), None) => d.clone(),
        (None, Some(dt)) => dt.clone(),
        (None, None) => String::new(),
    };
    row("DRAWN BY", &drawn, left, top - 3.0 * row_h);
    row(
        "SHEET",
        tb.sheet.as_deref().unwrap_or(""),
        left + col,
        top - 3.0 * row_h,
    );
    if let Some(company) = &tb.company {
        out.push(Prim::Text {
            x: left + col + 8.0,
            y: top - 3.0 * row_h + row_h / 2.0,
            text: company.clone(),
            size: TEXT_MM,
        });
    }

    // Vertical separator between the two columns.
    out.push(Prim::Line {
        x1: left + col,
        y1: bottom,
        x2: left + col,
        y2: top,
    });

    out
}
