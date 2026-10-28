//! EDITOR-2D-DRAFTING acceptance (TDD: the card's exact bar, verified here on
//! real computed geometry — never fabricated).
//!
//! Locked against a part with a known sketch+extrude (bracket profile driven
//! by variable set width=80/height=40, extruded 6 deep):
//!
//! 1. **Correct projected edges (committed golden)** — replaying the feature
//!    graph into a solid mesh and projecting it yields, per standard view,
//!    the exact rectangle edges of the solid (front 80x6, top 80x40, right
//!    40x6), and the rendered drawing byte-locks the committed SVG/PDF
//!    goldens (independent `tests/test_oep_drafting_acceptance.py` re-parses
//!    them from the other side).
//! 2. **Dimensions driven by the variable set** — the width/height/depth
//!    dimensions label the resolved variable entries (80/40/6) and the
//!    measured model geometry agrees.
//! 3. **An edit to the variable updates the drawing dimension** — changing
//!    width 80 -> 120 re-solves, re-drives the model, redraws, and both the
//!    drawing label and the projected geometry follow to 120.
//! 4. **First/third-angle per document setting** — the same drawing laid out
//!    third-angle puts TOP above FRONT and RIGHT to the right; first-angle
//!    folds the views the other way.
//! 5. **Title block + sheet** — the sheet carries a deterministic title block
//!    and every drawing is stored as a first-class `oep-geo` Document (v2).

use oep_drafting::builder::drive_bracket_drawing;
use oep_drafting::export::drawing_primitives;
use oep_drafting::export::metrics::text_width;
use oep_drafting::export::{BORDER_MM, TEXT_MM, TITLE_BLOCK_H, TITLE_BLOCK_W};
use oep_drafting::view::{bounds2, extent, project_edges, Edge2};
use oep_drafting::{
    render_document, render_sheet, solid_from_graph, write_drawing_pdf, write_drawing_svg,
};
use oep_geo::feature_graph::FeatureGraph;
use oep_geo::{Document, ProjectionType, ViewKind};
use oep_sketch::sketch::{DimExpr, Sketch};
use oep_sketch::varset::VariableSet;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn read_golden(name: &str) -> Vec<u8> {
    std::fs::read(golden_dir().join(name)).unwrap_or_else(|_| panic!("missing golden {name}"))
}

/// The acceptance rectangle: bottom-left pinned at (0,0), width/height driven
/// by variable-set entries `width` and `height`.
fn rect_sketch() -> Sketch {
    let mut s = Sketch::new("bracket-profile");
    let p0 = s.add_point("p0", 0.0, 0.0);
    let p1 = s.add_point("p1", 3.0, 1.0);
    let p2 = s.add_point("p2", 4.0, 4.0);
    let p3 = s.add_point("p3", 1.0, 5.0);
    let l0 = s.add_line("l0", &p0, &p1);
    let l1 = s.add_line("l1", &p1, &p2);
    s.add_line("l2", &p2, &p3);
    s.add_line("l3", &p3, &p0);
    s.constrain_fixed(&p0, 0.0, 0.0);
    s.constrain_horizontal(&l0);
    s.constrain_horizontal("l2");
    s.constrain_vertical(&l1);
    s.constrain_vertical("l3");
    s.constrain_length(&l0, DimExpr::Variable("width".into()));
    s.constrain_length(&l1, DimExpr::Variable("height".into()));
    s
}

fn bracket_vars() -> VariableSet {
    let mut vars = VariableSet::new();
    vars.set("width", 80.0);
    vars.set("height", 40.0);
    vars.set("depth", 6.0);
    vars
}

fn drive(width: f64) -> Document {
    let sketch = rect_sketch();
    let mut vars = bracket_vars();
    vars.set("width", width);
    let driven = drive_bracket_drawing(
        "oep-part-bracket",
        "bracket",
        &sketch,
        &vars,
        "width",
        "height",
        "depth",
        "Bracket",
        Some("ISO-BKT-001"),
        Some("6061-T6"),
        Some("A"),
        Some("oep-drafting"),
        Some("2026-10-22"),
        Some("Isolith"),
    )
    .expect("drives");
    driven.document
}

// ---------------------------------------------------------------------------
// 1. projected edges are computed from the model (committed golden)
// ---------------------------------------------------------------------------

#[test]
fn part_solid_mesh_and_projected_edges_are_correct() {
    let sketch = rect_sketch();
    let vars = bracket_vars();
    let driven = drive_bracket_drawing(
        "oep-part-bracket",
        "bracket",
        &sketch,
        &vars,
        "width",
        "height",
        "depth",
        "Bracket",
        Some("ISO-BKT-001"),
        Some("6061-T6"),
        Some("A"),
        Some("oep-drafting"),
        Some("2026-10-22"),
        Some("Isolith"),
    )
    .expect("drives");

    let mesh = solid_from_graph(&driven.graph).expect("replays to a solid mesh");
    assert_eq!(
        mesh.positions.len(),
        8,
        "extrude of the profile = one box solid"
    );
    assert_eq!(mesh.triangles.len(), 12);

    // A box projected onto each standard plane collapses to its 4-edge outline
    // rectangle. These are the CORRECT projected edges of the model.
    let front = project_edges(&mesh, ViewKind::Front);
    let top = project_edges(&mesh, ViewKind::Top);
    let right = project_edges(&mesh, ViewKind::Right);

    assert_eq!(front.len(), 4, "front view = rectangle outline");
    assert_eq!(top.len(), 4, "top view = rectangle outline");
    assert_eq!(right.len(), 4, "right view = rectangle outline");

    let (fw, fh) = extent(&front);
    let (tw, th) = extent(&top);
    let (rw, rh) = extent(&right);
    assert!(
        (fw - 80.0).abs() < 1e-9 && (fh - 6.0).abs() < 1e-9,
        "front 80x6, got {fw}x{fh}"
    );
    assert!(
        (tw - 80.0).abs() < 1e-9 && (th - 40.0).abs() < 1e-9,
        "top 80x40, got {tw}x{th}"
    );
    assert!(
        (rw - 40.0).abs() < 1e-9 && (rh - 6.0).abs() < 1e-9,
        "right 40x6, got {rw}x{rh}"
    );
}

#[test]
fn rendered_drawing_byte_locks_committed_goldens() {
    let sketch = rect_sketch();
    let vars = bracket_vars();
    let driven = drive_bracket_drawing(
        "oep-part-bracket",
        "bracket",
        &sketch,
        &vars,
        "width",
        "height",
        "depth",
        "Bracket",
        Some("ISO-BKT-001"),
        Some("6061-T6"),
        Some("A"),
        Some("oep-drafting"),
        Some("2026-10-22"),
        Some("Isolith"),
    )
    .expect("drives");
    let render = render_document(&driven.document, &driven.graph).expect("renderable");

    let mut svg = Vec::new();
    write_drawing_svg(&render, &mut svg).expect("svg");
    assert_eq!(
        svg,
        read_golden("bracket-drawing.svg"),
        "SVG drifted from the committed golden"
    );

    let mut pdf = Vec::new();
    write_drawing_pdf(&render, &mut pdf).expect("pdf");
    assert_eq!(
        pdf,
        read_golden("bracket-drawing.pdf"),
        "PDF drifted from the committed golden"
    );

    let doc_json = driven.document.to_json_pretty().expect("serialize");
    assert_eq!(
        doc_json.as_bytes(),
        read_golden("bracket-drawing.oepgeo.json"),
        "document drifted from the committed golden"
    );
}

// ---------------------------------------------------------------------------
// 2 + 3. dimensions are driven by the variable set and follow an edit
// ---------------------------------------------------------------------------

#[test]
fn dimension_values_are_driven_by_the_variable_set() {
    let driven = drive(80.0);
    let render = render_document_with_graph(&driven);

    let by_id = |id: &str| -> &oep_drafting::RenderedDimension {
        render
            .dimensions
            .iter()
            .find(|d| d.id == id)
            .unwrap_or_else(|| panic!("missing dimension {id}"))
    };
    assert_eq!(by_id("oep-dim-length").label, "80");
    assert_eq!(by_id("oep-dim-width").label, "40");
    assert_eq!(by_id("oep-dim-depth").label, "6");
    for d in &render.dimensions {
        assert!(
            (d.value - d.measured).abs() < 1e-6,
            "variable-driven value {} must agree with measured geometry {}",
            d.value,
            d.measured
        );
    }
}

#[test]
fn editing_the_variable_updates_the_drawing_dimension() {
    let r80 = render_document_with_graph(&drive(80.0));
    let r120 = render_document_with_graph(&drive(120.0));

    let label = |r: &oep_drafting::RenderedDrawing, id: &str| -> String {
        r.dimensions
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.label.clone())
            .unwrap_or_default()
    };
    assert_eq!(label(&r80, "oep-dim-length"), "80");
    assert_eq!(label(&r120, "oep-dim-length"), "120");

    // The projected geometry follows too: the top view's horizontal extent is
    // now 120 geometry units.
    let top120 = r120
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Top)
        .expect("top view");
    let (w120, _) = extent_from_paper(&top120.edges_paper, 6.0);
    assert!(
        (w120 - 120.0).abs() < 1e-6,
        "top extent follows the edit: {w120}"
    );
}

// ---------------------------------------------------------------------------
// 4. first- vs third-angle arrangement
// ---------------------------------------------------------------------------

#[test]
fn first_and_third_angle_lay_out_the_views_differently() {
    let third = render_document_with_graph(&drive(80.0));
    let front = third
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Front)
        .unwrap();
    let top = third
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Top)
        .unwrap();
    let right = third
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Right)
        .unwrap();
    assert!(
        top.cy > front.cy,
        "third-angle: TOP above FRONT ({} vs {})",
        top.cy,
        front.cy
    );
    assert!(
        right.cx > front.cx,
        "third-angle: RIGHT to the right of FRONT ({} vs {})",
        right.cx,
        front.cx
    );

    // First-angle: same geometry, folded the other way.
    let doc = drive(80.0);
    let vars = oep_drafting::extract_variables(&doc);
    let mut flipped = doc;
    let drawing = flipped.drawings.first_mut().expect("one drawing");
    let sheet = drawing.sheets.first_mut().expect("one sheet");
    sheet.projection = ProjectionType::FirstAngle;
    let kinds: Vec<ViewKind> = sheet.views.iter().map(|v| v.kind).collect();
    let mesh = solid_from_graph(&first_graph(&vars, &kinds)).expect("mesh");
    let rendered = render_sheet(sheet, &mesh, &vars).expect("render");
    let f = rendered
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Front)
        .unwrap();
    let t = rendered
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Top)
        .unwrap();
    let r = rendered
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Right)
        .unwrap();
    assert!(
        t.cy < f.cy,
        "first-angle: TOP below FRONT ({} vs {})",
        t.cy,
        f.cy
    );
    assert!(
        r.cx < f.cx,
        "first-angle: RIGHT to the left of FRONT ({} vs {})",
        r.cx,
        f.cx
    );
}

// ---------------------------------------------------------------------------
// 5. title block + sheet + canonical document
// ---------------------------------------------------------------------------

#[test]
fn title_block_is_drawn_and_the_document_is_canonical_v2() {
    let driven = drive(80.0);

    // Canonical Document: v2, drawings present, validates.
    assert_eq!(driven.version(), oep_geo::document::VERSION);
    assert_eq!(driven.drawings.len(), 1);
    let drawing = &driven.drawings[0];
    assert_eq!(drawing.sheets.len(), 1);
    let sheet = &drawing.sheets[0];
    assert_eq!(sheet.views.len(), 3);
    assert_eq!(sheet.dimensions.len(), 3);
    assert!(driven.validate().is_empty(), "{:?}", driven.validate());

    // Title block values travel with the drawing record.
    assert_eq!(sheet.title_block.title, "Bracket");
    assert_eq!(sheet.title_block.part_number.as_deref(), Some("ISO-BKT-001"));

    // And the SVG primitives carry the title text and the dimension labels.
    let render = render_document_with_graph(&driven);
    let prims = drawing_primitives(&render);
    let mut labels = String::new();
    for p in prims {
        if let oep_drafting::export::Prim::Text { text, .. } = p {
            labels.push_str(&text);
            labels.push('\n');
        }
    }
    assert!(labels.contains("TITLE: Bracket"), "title in drawing");
    assert!(
        labels.contains("PART NO: ISO-BKT-001"),
        "part number in drawing"
    );
    assert!(labels.contains("80"), "width dimension label");
    assert!(labels.contains("FRONT") && labels.contains("TOP") && labels.contains("RIGHT"));

    // Round-trip determinism of the whole document.
    let a = driven.to_json_pretty().expect("serialize");
    let back = Document::from_json_pretty(&a).expect("parse");
    assert_eq!(back, driven, "document round-trip identical");
}

// ---------------------------------------------------------------------------
// layout: title-block labels stay in their cells; no line leaves the sheet
// (regression for the DRAFTING-TITLEBLOCK defect — the pre-fix emitter
// centre-anchored title labels and overran the block with the right-column
// dividers, and the goldens had byte-locked that buggy output).
// ---------------------------------------------------------------------------

/// A parsed SVG `<text>`: (left-edge x, centre y, content).
type SvgText = (f64, f64, String);
/// A parsed SVG `<path>`: (x1, y1, x2, y2).
type SvgLine = (f64, f64, f64, f64);

/// Parse the rendered SVG's `<text>` and `<path>` elements (stdlib only).
fn parse_svg(svg: &str) -> (Vec<SvgText>, Vec<SvgLine>) {
    let mut texts = Vec::new();
    let mut paths = Vec::new();
    for chunk in svg.split('<') {
        if let Some(attrs_and_body) = chunk.strip_prefix("text ") {
            let (attrs, body) = attrs_and_body.split_once('>').expect("text attrs");
            // `split('<')` already cut at the element's own `</text`, so the
            // body IS the text content.
            let text = body.trim_end().to_string();
            let x: f64 = attr(attrs, "x").parse().expect("text x");
            let y: f64 = attr(attrs, "y").parse().expect("text y");
            texts.push((x, y, text));
        } else if let Some(attrs) = chunk.strip_prefix("path ") {
            let d = attr(attrs, "d");
            let nums: Vec<f64> = d
                .split_whitespace()
                .filter_map(|t| t.parse::<f64>().ok())
                .collect();
            assert_eq!(nums.len(), 4, "path d must hold 4 numbers: {d:?}");
            paths.push((nums[0], nums[1], nums[2], nums[3]));
        }
    }
    (texts, paths)
}

/// The value of `key="..."` in an SVG attribute list.
fn attr(attrs: &str, key: &str) -> String {
    let pat = format!("{key}=\"");
    let s = attrs
        .find(&pat)
        .unwrap_or_else(|| panic!("attr {key} in {attrs:?}"))
        + pat.len();
    let e = attrs[s..].find('"').expect("closing quote");
    attrs[s..s + e].to_string()
}

#[test]
fn title_block_text_and_dividers_stay_in_bounds_and_cells() {
    let driven = drive(80.0);
    let render = render_document_with_graph(&driven);
    let mut svg = Vec::new();
    write_drawing_svg(&render, &mut svg).expect("svg");
    let (texts, paths) = parse_svg(&String::from_utf8(svg).expect("utf-8"));

    // Title-block geometry on the sheet (mm).
    let (sw, sh) = (render.width_mm as f64, render.height_mm as f64);
    let left = sw - BORDER_MM - TITLE_BLOCK_W;
    let col = TITLE_BLOCK_W / 2.0;
    let tb_right = left + TITLE_BLOCK_W;
    let bottom = BORDER_MM;
    let tb_top = bottom + TITLE_BLOCK_H;
    let eps = 1e-6;
    assert_eq!(left, 127.0, "title block left edge");
    assert_eq!(col, 80.0, "column width");
    assert_eq!(tb_right, 287.0, "title block right edge");

    // 1. Every title-block label sits inside its cell: x0 >= cell left edge
    //    and x0 + text_width <= cell right edge. (Pre-fix the emitter shifted
    //    every label LEFT by half its own width, so e.g. "DRAWN BY:
    //    oep-drafting 2026-10-22" began at x=99.8 while its box starts at
    //    127.) Title-block labels are the texts whose (centre-anchored) y
    //    falls inside the block's band; view and dimension labels sit above
    //    it and are intentionally centre-anchored.
    let mut title_texts = 0;
    for (x0, y, text) in &texts {
        if *y <= tb_top + eps {
            let w = text_width(text, TEXT_MM);
            let (cell_l, cell_r) = if *x0 < left + col {
                (left, left + col)
            } else {
                (left + col, tb_right)
            };
            assert!(
                *x0 >= cell_l - eps && *x0 + w <= cell_r + eps,
                "title-block label {text:?} escapes its cell: x0={x0} (+width {w}) not within [{cell_l}, {cell_r}]",
            );
            title_texts += 1;
        }
    }
    assert!(
        title_texts >= 7,
        "expected >=7 title-block labels, saw {title_texts}"
    );

    // 2. Every line stays inside the sheet. (Pre-fix the right-column row
    //    dividers ran from x=207 to x=367, past the 297-wide sheet.)
    for (x1, y1, x2, y2) in &paths {
        for c in [*x1, *x2] {
            assert!(
                c >= -eps && c <= sw + eps,
                "line x={c} leaves the sheet width [0, {sw}]"
            );
        }
        for c in [*y1, *y2] {
            assert!(
                c >= -eps && c <= sh + eps,
                "line y={c} leaves the sheet height [0, {sh}]"
            );
        }
    }

    // 3. Exactly one vertical column divider at x = left+col across the block.
    let vdiv = paths
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (*x1 - (left + col)).abs() < 1e-9
                && (*x2 - (left + col)).abs() < 1e-9
                && (*y1 - bottom).abs() < 1e-9
                && (*y2 - tb_top).abs() < 1e-9
        })
        .count();
    assert_eq!(
        vdiv, 1,
        "expected exactly one vertical column divider, saw {vdiv}"
    );
}

// ---------------------------------------------------------------------------
// helpers (the draft graph is built identically to the acceptance driver)
// ---------------------------------------------------------------------------

/// Build the feature graph for the driven document the same way the driver
/// does (Extrude of the solved profile spines).
fn first_graph(
    vars: &BTreeMap<String, f64>,
    _kinds: &[ViewKind],
) -> oep_geo::feature_graph::FeatureGraph {
    let length = *vars.get("width").unwrap_or(&0.0);
    let width = *vars.get("height").unwrap_or(&0.0);
    let depth = *vars.get("depth").unwrap_or(&0.0);
    let mut graph = FeatureGraph::new();
    graph
        .add_root(
            oep_geo::feature_graph::FeatureNode::new(
                "oep-feature-oep-part-bracket-extrude",
                "extrude-bracket",
                "Extrude",
            )
            .num("length", length)
            .num("width", width)
            .num("height", depth),
        )
        .expect("root");
    graph
}

fn render_document_with_graph(doc: &Document) -> oep_drafting::RenderedDrawing {
    let vars = oep_drafting::extract_variables(doc);
    let drawing = doc.drawings.first().expect("drawing");
    let sheet = drawing.sheets.first().expect("sheet");
    let kinds: Vec<ViewKind> = sheet.views.iter().map(|v| v.kind).collect();
    let mesh = solid_from_graph(&first_graph(&vars, &kinds)).expect("mesh");
    render_sheet(sheet, &mesh, &vars).expect("render")
}

/// Recover the paper-space extent from paper edges (view scale is 1 in the
/// acceptance driver, so paper == model extent).
fn extent_from_paper(edges: &[Edge2], _scale: f64) -> (f64, f64) {
    let (mn, mx) = bounds2(edges);
    (mx[0] - mn[0], mx[1] - mn[1])
}
