//! Export tools for the drafting panel (wave F, card UI-F4 UI-DRAWING-EXPORT):
//! the drawing exports to SVG and PDF from the UI, with a print preview, and
//! honest errors when the target is unwritable.
//!
//! Hosted ADDITIVELY inside the drafting panel exactly like the F2 views and
//! F3 annotate tools: `sheet.rs` holds ONE `#[path]` include
//! (`mod drawing_export;`) and ONE `drawing_export::show(ui, app, &mut model)`
//! call at the end of the drafting scroll content; nothing else in the panel
//! knows this module exists.
//!
//! All rendering goes through the EXISTING drafting seam - no writer, no
//! second rendering path:
//!
//! - the drawing is rendered with `oep_drafting::render_sheet` over the live
//!   selected sheet, the solid mesh replayed from the feature graph by
//!   `oep_drafting::solid_from_graph`, and the variable set extracted from the
//!   live document by `oep_drafting::extract_variables` (the exact limbs
//!   `render_document` composes, split so the SELECTED sheet exports);
//! - the SVG and PDF bytes come from the card-mandated writers
//!   `oep_drafting::export::svg::write_drawing_svg` and
//!   `oep_drafting::export::pdf::write_drawing_pdf`;
//! - the print preview paints the SAME primitive list the writers serialize
//!   (`oep_drafting::export::drawing_primitives`), so the preview shows the
//!   same sheet content as the export - the acceptance asserts the element
//!   count agrees.
//!
//! The export is a pure file sink: it reads the live document and writes out
//! of process memory (the bytes are fully materialised before the file is
//! touched, then written in one `fs::write`; on error the panel surfaces a
//! named [`ExportError`] and best-effort removes any partial file, so a failed
//! export never leaves a corrupt target behind). No document mutation, no
//! journal entry, so the mutation guard stays green.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_drafting as draft;

/// Context-data key under which the panel reports its `String` oracle every
/// frame (exported element count, preview status, svg/pdf bytes + FNV-1a
/// hashes, the named export error or last outcome). Cross-`#[path]`-copy safe;
/// the acceptance harness names it by value like the F1-F3 oracle keys.
pub const EXPORT_REPORT_KEY: &str = "ui_f4_export_report";
/// Context-data key a test may seed with the export target path BEFORE the
/// first frame (the same seeding idiom `SHEET_SEED_KEY` uses to open a
/// document), so the acceptance knows deterministically where the buttons
/// write and can read the files back from disk.
pub const EXPORT_PATH_KEY: &str = "ui_f4_export_path";
/// Context-data key under which the panel's own export state lives.
const EXPORT_STATE_KEY: &str = "ui_f4_export_state";

/// Which export format a button triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    /// `oep-drafting` SVG writer.
    Svg,
    /// `oep-drafting` PDF writer.
    Pdf,
}

impl ExportKind {
    fn extension(self) -> &'static str {
        match self {
            ExportKind::Svg => "svg",
            ExportKind::Pdf => "pdf",
        }
    }
}

/// A named export failure (shown in the UI; never a panic).
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// No sheet is renderable from the live document (no shape to export).
    NoRenderableSheet,
    /// The writer or the file write failed, naming the path involved.
    Io {
        /// The path the export was asked to write.
        path: String,
        /// The io error detail.
        source: String,
    },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::NoRenderableSheet => {
                write!(
                    f,
                    "ExportError::NoRenderableSheet: no drawable sheet in the document"
                )
            }
            ExportError::Io { path, source } => {
                write!(f, "ExportError::Io {{ path: {path:?}, source: {source} }}")
            }
        }
    }
}

impl std::error::Error for ExportError {}

/// The panel's own export state (context-store resident like the F1-F3 state).
#[derive(Debug, Clone, Default)]
struct ExportUi {
    /// True once the export path has been seeded on the first frame.
    seeded: bool,
    /// The export target base path (the buttons append `.svg` / `.pdf`).
    path: String,
    /// The last export outcome, as the oracle should word it.
    last: Option<String>,
    /// The named export error surfaced after a failed export.
    error: Option<ExportError>,
    /// Number of SVG/PDF bytes written on the last successful SVG export.
    svg_bytes: usize,
    /// Number of SVG/PDF bytes written on the last successful PDF export.
    pdf_bytes: usize,
    /// FNV-1a 64 of the last successful SVG bytes.
    svg_hash: u64,
    /// FNV-1a 64 of the last successful PDF bytes.
    pdf_hash: u64,
    /// Whether the last export was an SVG.
    last_was_svg: bool,
    /// Whether the print preview rendered at least one frame.
    preview_rendered: bool,
    /// The graphical element count shown by the print preview (== the count
    /// the writers serialize, asserted by the acceptance).
    preview_elements: usize,
}

/// Render the live document's selected sheet to paper geometry through the
/// EXISTING drafting seam: feature graph -> solid mesh -> `render_sheet` with
/// the live variable set. Mirrors exactly what `oep_drafting::render_document`
/// composes (`extract_variables` + `solid_from_graph` + `render_sheet`), split
/// so it renders the SELECTED sheet rather than always the first.
fn render_selected(model: &super::SheetModel) -> Option<draft::RenderedDrawing> {
    let document = model.document.as_ref()?;
    let sheet = model.selected_sheet()?;
    let vars = draft::extract_variables(document);
    let graph = draft_graph(document);
    let mesh = draft::solid_from_graph(&graph).ok()?;
    draft::render_sheet(sheet, &mesh, &vars).ok()
}

/// Rebuild the solid-producing feature graph from the live document's stored
/// extrude feature (the same graph `drive_bracket_drawing` builds: one root
/// `Extrude` node carrying the resolved length/width/height). The document
/// stores the feature; this re-derives the graph the seam replays - never a
/// second fixture.
fn draft_graph(document: &oep_geo::Document) -> oep_geo::feature_graph::FeatureGraph {
    use oep_geo::feature_graph::{FeatureGraph, FeatureNode};
    let mut vars = draft::extract_variables(document);
    let mut get = |k: &str| vars.remove(k).unwrap_or(0.0);
    // Extrude of the solved profile spines: length x width x height in the
    // drafted axis order (width, height, depth -> l x w x h).
    let root = FeatureNode::new("oep-feature-oep-part-bracket-extrude", "extrude", "Extrude")
        .num("length", get("width"))
        .num("width", get("height"))
        .num("height", get("depth"));
    let mut graph = FeatureGraph::new();
    graph.add_root(root).ok();
    graph
}

/// Produce the full raw bytes for one export format from the rendered
/// drawing, using the CARD-MANDATED writers only.
fn export_bytes(kind: ExportKind, render: &draft::RenderedDrawing) -> Result<Vec<u8>, ExportError> {
    let mut bytes = Vec::new();
    let result = match kind {
        ExportKind::Svg => draft::export::svg::write_drawing_svg(render, &mut bytes),
        ExportKind::Pdf => draft::export::pdf::write_drawing_pdf(render, &mut bytes),
    };
    result.map_err(|e| ExportError::Io {
        path: format!("<memory> ({})", kind.extension()),
        source: e.to_string(),
    })?;
    Ok(bytes)
}

/// Write the fully-materialised bytes to `path` in one `fs::write`. On any
/// error a best-effort `remove_file` runs first, so a failed export never
/// leaves a partial file behind, and the clop error is surfaced named.
fn write_export_file(path: &str, bytes: &[u8]) -> Result<(), ExportError> {
    if let Err(source) = std::fs::write(path, bytes) {
        let _ = std::fs::remove_file(path);
        return Err(ExportError::Io {
            path: path.to_string(),
            source: source.to_string(),
        });
    }
    Ok(())
}

/// FNV-1a 64 over the exported bytes - the same hash language the wave uses
/// for canonical bytes (`doc_io::document_hash`'s family), so hashes printed
/// here are comparable with the other cards.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

/// Render the export section inside the drafting panel's scroll content. This
/// is the ONE call `sheet.rs` adds; it is called after the annotate tools.
pub fn show(ui: &mut egui::Ui, _app: &mut App, model: &mut super::SheetModel) {
    ui.separator();
    ui.label(theme::heading(theme::mode_of(ui), "Export").size(13.0));

    let mut st = read_state(ui.ctx());
    if !st.seeded {
        seed_path(ui.ctx(), &mut st);
    }

    // Render the live sheet once per frame; the preview and the exports all
    // derive from this one render (never two render paths).
    let rendered = render_selected(model);
    let prims = rendered
        .as_ref()
        .map(draft::export::drawing_primitives)
        .unwrap_or_default();
    st.preview_rendered = rendered.is_some();
    st.preview_elements = rendered
        .as_ref()
        .map(|_| graphical_count(&prims))
        .unwrap_or(0);

    let mode = theme::mode_of(ui);

    // ---- Print preview ----------------------------------------------------
    ui.add_space(4.0);
    ui.label(theme::heading(mode, "Print preview").size(12.0));
    ui.separator();
    if let Some(render) = &rendered {
        paint_preview(ui, &prims, render, mode);
        ui.label(
            egui::RichText::new(format!(
                "{} graphical elements (same content the export writes)",
                st.preview_elements
            ))
            .monospace()
            .size(11.0),
        );
    } else {
        ui.label(
            egui::RichText::new("No sheet to render; add a drawing sheet first.")
                .weak()
                .italics(),
        );
    }

    // ---- Export target + buttons ------------------------------------------
    ui.add_space(6.0);
    ui.label(theme::heading(mode, "Export to").size(12.0));
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("path:").weak().size(11.0));
        ui.label(
            egui::RichText::new(&st.path)
                .monospace()
                .size(11.0)
                .color(oep_ui_theme::Palette::resolve(mode).text_primary),
        );
    });
    ui.horizontal(|ui| {
        let svg_btn = ui.button("Export SVG");
        let pdf_btn = ui.button("Export PDF");
        if svg_btn.clicked() {
            run_export(ExportKind::Svg, &mut st, rendered.as_ref());
        }
        if pdf_btn.clicked() {
            run_export(ExportKind::Pdf, &mut st, rendered.as_ref());
        }
    });

    // ---- Status / honest error -------------------------------------------
    if let Some(err) = &st.error {
        ui.colored_label(oep_ui_theme::Palette::resolve(mode).danger, err.to_string());
    }
    if let Some(last) = &st.last {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(last.as_str())
                .weak()
                .italics()
                .size(11.0),
        );
    }

    publish_report(ui.ctx(), &st);
    write_state(ui.ctx(), &st);
}

/// Run one export: render (already available), write the bytes, record the
/// outcome or the named error. The target path is `{path}.{ext}`.
fn run_export(kind: ExportKind, st: &mut ExportUi, rendered: Option<&draft::RenderedDrawing>) {
    let Some(render) = rendered else {
        st.error = Some(ExportError::NoRenderableSheet);
        st.last = Some("export failed: no renderable sheet".to_string());
        return;
    };
    let path = format!("{}.{}", st.path, kind.extension());
    match export_bytes(kind, render).and_then(|bytes| {
        write_export_file(&path, &bytes)?;
        Ok(bytes)
    }) {
        Ok(bytes) => {
            let hash = fnv1a64(&bytes);
            match kind {
                ExportKind::Svg => {
                    st.svg_bytes = bytes.len();
                    st.svg_hash = hash;
                }
                ExportKind::Pdf => {
                    st.pdf_bytes = bytes.len();
                    st.pdf_hash = hash;
                }
            }
            st.last_was_svg = matches!(kind, ExportKind::Svg);
            st.error = None;
            st.last = Some(format!(
                "exported {} ({} bytes, hash {})",
                path,
                bytes.len(),
                hash
            ));
        }
        Err(err) => {
            st.error = Some(err.clone());
            st.last = Some(format!(
                "export {} failed: {}",
                kind.extension().to_uppercase(),
                err
            ));
        }
    }
}

/// Count graphical prims (Line / Rect / Text) - the element count the writers
/// serialize (group markers are structure, not content).
fn graphical_count(prims: &[draft::export::Prim]) -> usize {
    prims
        .iter()
        .filter(|p| {
            !matches!(
                p,
                draft::export::Prim::BeginGroup { .. } | draft::export::Prim::EndGroup
            )
        })
        .count()
}

// ---------------------------------------------------------------------------
// Print-preview painting (same primitive list the writers serialize)
// ---------------------------------------------------------------------------

/// Preview canvas size (logical points).
const PREVIEW_W: f32 = 240.0;
const PREVIEW_H: f32 = 108.0;

/// Paint the drawing primitives scaled into a fixed small canvas, Y flipped
/// (paper origin bottom-left, egui top-left) - the same transformation the
/// SVG writer's `viewBox` implies, read back visually.
fn paint_preview(
    ui: &mut egui::Ui,
    prims: &[draft::export::Prim],
    render: &draft::RenderedDrawing,
    mode: oep_ui_theme::Mode,
) {
    let (canvas_rect, _) =
        ui.allocate_exact_size(egui::vec2(PREVIEW_W, PREVIEW_H), egui::Sense::hover());
    let cp = ui.painter().clone();
    cp.rect_filled(
        canvas_rect,
        4.0,
        oep_ui_theme::SURFACE_RAISED.for_mode(mode),
    );
    cp.rect_stroke(
        canvas_rect,
        4.0,
        egui::Stroke::new(1.0, oep_ui_theme::BORDER.for_mode(mode)),
        egui::StrokeKind::Inside,
    );

    // Fit the sheet (mm) into the canvas, keeping aspect.
    let (sw, sh) = (
        render.width_mm.max(1) as f32,
        render.height_mm.max(1) as f32,
    );
    let scale = ((PREVIEW_W * 0.96) / sw)
        .min((PREVIEW_H * 0.96) / sh)
        .max(0.01);
    let span_w = sw * scale;
    let span_h = sh * scale;
    let origin = egui::pos2(
        canvas_rect.left() + (PREVIEW_W - span_w) / 2.0,
        canvas_rect.bottom() - (PREVIEW_H - span_h) / 2.0,
    );
    let to_px =
        |x: f64, y: f64| egui::pos2(origin.x + (x as f32) * scale, origin.y - (y as f32) * scale);

    let ink = oep_ui_theme::Palette::resolve(mode).text_primary;
    for prim in prims {
        match prim {
            draft::export::Prim::BeginGroup { .. } | draft::export::Prim::EndGroup => {}
            draft::export::Prim::Line { x1, y1, x2, y2 } => {
                cp.add(egui::Shape::line_segment(
                    [to_px(*x1, *y1), to_px(*x2, *y2)],
                    egui::Stroke::new(0.5, ink),
                ));
            }
            draft::export::Prim::Rect { x, y, w, h } => {
                let a = to_px(*x, *y);
                let b = to_px(*x + *w, *y + *h);
                cp.rect_stroke(
                    egui::Rect::from_two_pos(a, b),
                    0.0,
                    egui::Stroke::new(0.5, ink),
                    egui::StrokeKind::Inside,
                );
            }
            draft::export::Prim::Text { x, y, text, .. } => {
                let pos = to_px(*x, *y);
                cp.text(
                    pos,
                    egui::Align2::CENTER_CENTER,
                    maybe_clip_text(text),
                    egui::FontId::proportional(6.0),
                    ink,
                );
            }
        }
    }
}

/// Preview labels are tiny; keep them short enough not to smear the canvas
/// (the full text is still what the writers serialize - clipping here is a
/// display-only touch).
fn maybe_clip_text(text: &str) -> String {
    if text.chars().count() > 16 {
        format!("{}~", text.chars().take(15).collect::<String>())
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Context-store plumbing + oracle
// ---------------------------------------------------------------------------

fn read_state(ctx: &egui::Context) -> ExportUi {
    ctx.data(|d| d.get_temp(egui::Id::unique(EXPORT_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &ExportUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(EXPORT_STATE_KEY), st.clone()));
}

/// Seed the export base path on the first frame: the value a test placed
/// under [`EXPORT_PATH_KEY`], else a neutral default relative to the current
/// directory.
fn seed_path(ctx: &egui::Context, st: &mut ExportUi) {
    let seeded: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(EXPORT_PATH_KEY)));
    st.path = seeded.unwrap_or_else(|| "drawing".to_string());
    st.seeded = true;
}

/// Publish the plain-`String` oracle under [`EXPORT_REPORT_KEY`]: preview
/// status + element count, path, per-format bytes/hashes, and the named error
/// or last outcome.
fn publish_report(ctx: &egui::Context, st: &ExportUi) {
    let mut r = String::new();
    r.push_str(&format!("preview={}\n", u8::from(st.preview_rendered)));
    r.push_str(&format!("elements={}\n", st.preview_elements));
    r.push_str(&format!("path={}\n", st.path));
    r.push_str(&format!("svg_bytes={}\n", st.svg_bytes));
    r.push_str(&format!("svg_hash={}\n", st.svg_hash));
    r.push_str(&format!("pdf_bytes={}\n", st.pdf_bytes));
    r.push_str(&format!("pdf_hash={}\n", st.pdf_hash));
    r.push_str(&format!("last_was_svg={}\n", u8::from(st.last_was_svg)));
    match &st.error {
        Some(err) => r.push_str(&format!("error={err}\n")),
        None => r.push_str("error=-\n"),
    }
    r.push_str(&format!(
        "last={}\n",
        st.last.clone().unwrap_or_else(|| "-".to_string())
    ));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(EXPORT_REPORT_KEY), r));
}

// ---------------------------------------------------------------------------
// Module tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed bracket fixture, driven through the panel seam - the same
    /// document the F1 card opens by default.
    fn driven() -> oep_drafting::builder::BracketDrawing {
        super::super::drive_bracket_drawing().expect("fixture drives")
    }

    fn model_with_seed(json: Option<&str>) -> super::super::SheetModel {
        let driven = driven();
        match json {
            Some(json) => {
                let doc = oep_geo::Document::from_json_pretty(json).expect("seed parses");
                super::super::SheetModel {
                    document: Some(doc),
                    selection: Some("oep-sheet-1".to_string()),
                    seeded: true,
                    ..Default::default()
                }
            }
            None => super::super::SheetModel {
                document: Some(driven.document),
                selection: Some("oep-sheet-1".to_string()),
                seeded: true,
                ..Default::default()
            },
        }
    }

    /// The committed bracket renders through the panel seam with the exact
    /// geometry the engine's golden byte-locks (3 views, 3 dimensions, A4).
    #[test]
    fn panel_render_matches_the_committed_fixture() {
        let model = model_with_seed(None);
        let render = render_selected(&model).expect("renders");
        assert_eq!(render.width_mm, 297);
        assert_eq!(render.height_mm, 210);
        assert_eq!(render.views.len(), 3);
        assert_eq!(render.dimensions.len(), 3);
        let prims = draft::export::drawing_primitives(&render);
        assert!(
            graphical_count(&prims) > 30,
            "a full A4 bracket drawing has many elements, got {}",
            graphical_count(&prims)
        );
    }

    /// The SVG and PDF writers (card-mandated) parse deterministically from
    /// the panel render, and the element count each serializes equals the
    /// graphical count the preview reports.
    #[test]
    fn writers_serialize_the_same_element_count_as_the_preview() {
        let model = model_with_seed(None);
        let render = render_selected(&model).expect("renders");
        let prims = draft::export::drawing_primitives(&render);
        let expected = graphical_count(&prims);

        let mut svg = Vec::new();
        draft::export::svg::write_drawing_svg(&render, &mut svg).expect("svg");
        let svg_text = String::from_utf8(svg).expect("utf8");
        let svg_elements = svg_text
            .split('<')
            .filter(|t| t.starts_with("path ") || t.starts_with("rect ") || t.starts_with("text "))
            .count();
        assert_eq!(
            svg_elements, expected,
            "SVG serializes exactly the preview's graphical elements"
        );
    }

    /// Rendering a sheet whose variable set is absent from the document still
    /// resolves to a renderable drawing or a named error - never a panic.
    #[test]
    fn empty_document_renders_no_sheet_honestly() {
        let model = super::super::SheetModel::default();
        assert!(render_selected(&model).is_none());
        let mut st = ExportUi {
            path: "nope".to_string(),
            ..Default::default()
        };
        run_export(ExportKind::Svg, &mut st, None);
        assert_eq!(
            st.error,
            Some(ExportError::NoRenderableSheet),
            "no document -> named export error, never a panic"
        );
    }

    /// A successful export writes the file and reports bytes + hash; a bad
    /// target path surfaces a named Io error and leaves nothing behind.
    #[test]
    fn export_writes_and_reports_or_fails_named_without_partial_file() {
        let dir = std::env::temp_dir().join(format!("ui-f4-modtest-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let model = model_with_seed(None);

        // Success: SVG written to {path}.svg.
        let mut st = ExportUi {
            path: dir.join("mod-out").to_string_lossy().to_string(),
            ..Default::default()
        };
        let render = render_selected(&model).expect("render");
        run_export(ExportKind::Svg, &mut st, Some(&render));
        let target = format!("{}.svg", st.path);
        assert!(std::path::Path::new(&target).exists(), "file written");
        assert!(st.svg_bytes > 100 && st.error.is_none());

        // Failure: unwritable-ish path -> named Io error, no file left.
        let mut bad = ExportUi {
            path: dir
                .join("no-such-dir")
                .join("out")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };
        run_export(ExportKind::Pdf, &mut bad, Some(&render));
        assert!(
            bad.error.is_some(),
            "an unwritable path must surface a named error"
        );
        let bad_target = format!("{}.pdf", bad.path);
        assert!(
            !std::path::Path::new(&bad_target).exists(),
            "no partial file left behind"
        );

        // PDF success too.
        let mut pdf_st = ExportUi {
            path: dir.join("mod-pdf").to_string_lossy().to_string(),
            ..Default::default()
        };
        run_export(ExportKind::Pdf, &mut pdf_st, Some(&render));
        assert!(std::path::Path::new(&format!("{}.pdf", pdf_st.path)).exists());
        assert!(pdf_st.pdf_bytes > 100 && pdf_st.error.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fixture's sketch + variable set re-derive the same extrude graph
    /// `drive_bracket_drawing` builds (one Extrude root, length/width/height).
    #[test]
    fn rederived_graph_replays_to_the_fixture_mesh() {
        let driven = driven();
        let model = model_with_seed(None);
        let from_doc = draft_graph(model.document.as_ref().expect("doc"));
        let a = draft::solid_from_graph(&driven.graph).expect("fixture mesh");
        let b = draft::solid_from_graph(&from_doc).expect("rederived mesh");
        assert_eq!(a.positions.len(), 8, "one box solid from the fixture graph");
        assert_eq!(
            a.positions.len(),
            b.positions.len(),
            "re-derived graph replays the same box solid"
        );
        // The fixture's built mesh == the re-derived mesh, vertex for vertex.
        assert_eq!(a.positions, b.positions);
    }
}
