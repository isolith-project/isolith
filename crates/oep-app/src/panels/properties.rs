//! Properties panel (wave A, card UI-A4; wave B, card UI-B7 UI-DIMENSIONS).
//!
//! Renders the right "properties" region of the editor shell plus the
//! UI-B7 dimension editor: here is where a dimension on the committed sketch
//! geometry is driven by a name from the variable table and edited as an
//! expression (`width` 80 becomes `width*1.5`). The expression input lives in
//! this panel per the card's file list; the dimension engine is egui-free in
//! `src/sketch/dimensions.rs` (included verbatim via `#[path]`, the same
//! seam-splitting pattern the draw tools and constraints use).
//!
//! Interaction:
//!
//! - **Place**: a "Place dimension" button measures the most recently
//!   committed sketch line from the sketch `String` oracle
//!   ([`SKETCH_REPORT_KEY`], cross-`#[path]`-copy safe) and registers the
//!   variable-table entry `width`; the panel then drives the model and reports
//!   its volume (the document's own mass property).
//! - **Edit**: the expression field (a real `TextEdit`) holds the dimension's
//!   editable expression. Hitting Enter or the Apply button resolves it
//!   through the existing variable table ([`dimensions::edit`]); an invalid
//!   expression is refused and the named [`dimensions::DimensionError`] is
//!   shown here, with the document and variable table untouched.
//!
//! State and oracle: the typed [`DimDimensionPanel`] lives in the egui
//! context data store under [`DIMENSION_PANEL_KEY`] (like the B6 constraint
//! panel, only this pane touches the typed value), and a plain `String`
//! report under [`DIMENSION_REPORT_KEY`] is published every frame so tests
//! and other cross-`#[path]` consumers read the same values without a shared
//! `TypeId`.

use eframe::egui;

use crate::app::App;
use crate::theme;

// The UI-B7 dimension engine (egui-free; `src/sketch/dimensions.rs`).
#[path = "../sketch/dimensions.rs"]
#[allow(clippy::duplicate_mod)]
pub mod dimensions;

/// The sketch panel's `String` report oracle (must match
/// `viewport::sketch::SKETCH_REPORT_KEY`; a local literal avoids depending on
/// the parent module across `#[path]` compilations).
const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";
/// Context-data key under which the typed dimension panel lives. Only this
/// pane (in its own module copy) reads/writes the typed value; tests read the
/// `String` oracle instead.
const DIMENSION_PANEL_KEY: &str = "ui_b7_dimension_panel";
/// Context-data key under which the `String` dimension oracle is published.
pub const DIMENSION_REPORT_KEY: &str = "ui_b7_dimension_report";
/// Context-data key under which the expression input's draft text lives
/// (published as part of the oracle too, so a test can inspect it).
const EXPR_DRAFT_KEY: &str = "ui_b7_dimension_expr_draft";

/// Fixed sizes of the dimension-editor widgets (the test clicks the same
/// rects via [`dimension_widget_rects`]).
const PLACE_SIZE: egui::Vec2 = egui::vec2(150.0, 24.0);
const EXPR_SIZE: egui::Vec2 = egui::vec2(200.0, 24.0);
const APPLY_SIZE: egui::Vec2 = egui::vec2(80.0, 24.0);

/// The on-screen rects of the three dimension-editor widgets, laid out at the
/// top of the properties region. Public so the acceptance harness clicks the
/// exact rects the renderer uses (the same determinism contract as the B6
/// constraint toolbar).
pub fn dimension_widget_rects(region: egui::Rect) -> DimensionWidgetRects {
    let x = region.min.x + 8.0;
    let y = region.min.y + 56.0;
    DimensionWidgetRects {
        place: egui::Rect::from_min_size(egui::pos2(x, y), PLACE_SIZE),
        expr: egui::Rect::from_min_size(egui::pos2(x, y + 34.0), EXPR_SIZE),
        apply: egui::Rect::from_min_size(egui::pos2(x + 206.0, y + 34.0), APPLY_SIZE),
    }
}

/// The three interactive widget rects of the dimension editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionWidgetRects {
    /// "Place dimension" button.
    pub place: egui::Rect,
    /// Expression input field.
    pub expr: egui::Rect,
    /// "Apply" button.
    pub apply: egui::Rect,
}

/// Read the typed dimension panel from the egui context data store.
fn read_panel(ctx: &egui::Context) -> dimensions::DimensionPanel {
    ctx.data(|d| d.get_temp(egui::Id::unique(DIMENSION_PANEL_KEY)))
        .unwrap_or_default()
}

/// Write the typed dimension panel back into the egui context data store.
fn write_panel(ctx: &egui::Context, panel: &dimensions::DimensionPanel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(DIMENSION_PANEL_KEY), panel.clone()));
}

/// Read the expression draft (the live text in the expression field).
fn read_draft(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::unique(EXPR_DRAFT_KEY)))
        .unwrap_or_default()
}

fn write_draft(ctx: &egui::Context, text: &str) {
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(EXPR_DRAFT_KEY), text.to_string());
    });
}

/// Build the `String` oracle lines for the current dimension state: the
/// placed dimension (count, name, target, value), the dimension's expression,
/// the driven volume and the last named error if any.
fn oracle_lines(panel: &dimensions::DimensionPanel, draft: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("dims={}\n", panel.dims.len()));
    if let Some(d) = panel.dimension() {
        out.push_str(&format!("dim_name={}\n", d.name));
        out.push_str(&format!("dim_target={}\n", d.target));
        out.push_str(&format!("dim_value={:.4}\n", d.value));
        out.push_str(&format!("dim_expr={}\n", d.expr));
    } else {
        out.push_str("dim_expr=-\n");
    }
    // The live text in the expression input (what the user is typing).
    out.push_str(&format!("draft={draft}\n"));
    match panel.volume {
        Some(v) => out.push_str(&format!("volume={v:.4}\nvolume_ok=1\n")),
        None => {
            out.push_str("volume=-\nvolume_ok=0\n");
        }
    }
    match panel.error() {
        Some(e) => out.push_str(&format!("dim_error={e}\n")),
        None => out.push_str("dim_error=-\n"),
    }
    out.push_str(&format!(
        "dim_last={}\n",
        panel.last.as_deref().unwrap_or("-")
    ));
    out
}

/// Publish the dimension oracle (String) into the egui context data store.
fn write_oracle(ctx: &egui::Context, panel: &dimensions::DimensionPanel, draft: &str) {
    ctx.data_mut(|d| {
        d.insert_temp::<String>(
            egui::Id::unique(DIMENSION_REPORT_KEY),
            oracle_lines(panel, draft),
        );
    });
}

/// Draw the properties panel content inside its already-positioned region.
/// The UI-B7 dimension editor sits at the top of the region (deterministic
/// rects); the rest stays the A4-style panel content.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    let mode = theme::mode_of(ui);
    ui.label(theme::heading(mode, "Properties").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    let ctx = ui.ctx().clone();
    let region = ui.max_rect();
    let rects = dimension_widget_rects(region);
    let mut panel = read_panel(&ctx);
    let mut draft = read_draft(&ctx);

    // The sketch oracle: what the dimension measures and where it is placed.
    let sketch_report = ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default();

    // Determine whether a sketch line is available (the cloud can place a
    // dimension on it). The oracle's doclines= field is the committed line count.
    let has_line = sketch_report
        .lines()
        .any(|l| l.strip_prefix("doclines=").is_some_and(|v| v != "0"));

    // Place dimension: measure the last committed line through the engine and
    // drive the model immediately.
    let button = egui::Button::new(egui::RichText::new("Place dimension").size(12.0));
    let placed = ui.put(rects.place, button).clicked();
    if placed && has_line {
        let (next, _msg) = dimensions::place(panel, &sketch_report);
        panel = next;
    }
    let muted = oep_ui_theme::Palette::resolve(mode).text_muted;
    ui.painter().text(
        egui::pos2(rects.place.min.x, rects.place.max.y + 4.0),
        egui::Align2::LEFT_TOP,
        if has_line {
            "attaches to the last sketch line"
        } else {
            "draw a line in the sketch first"
        },
        egui::FontId::proportional(9.0),
        muted,
    );

    // Expression input + apply: only meaningful once a dimension exists.
    if panel.dimension().is_some() {
        let expr_response = ui.put(
            rects.expr,
            egui::TextEdit::singleline(&mut draft)
                .hint_text("e.g. width*1.5")
                .desired_width(EXPR_SIZE.x - 6.0),
        );
        let apply = ui
            .put(
                rects.apply,
                egui::Button::new(egui::RichText::new("Apply").size(12.0)),
            )
            .clicked();
        let submitted = expr_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (apply || submitted) && !draft.is_empty() {
            let (next, _msg) = dimensions::edit(panel, &draft);
            panel = next;
        }
    } else {
        // Reserve the vertical space so widget rects stay stable across
        // frames (the dimensions editor appears without jittering layout).
        ui.add_space(56.0);
    }

    // Draw the honest state: the dimension summary, the driven volume (from
    // the document's own mass properties) and any named error.
    if let Some(d) = panel.dimension() {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(format!("{} = {:.2}", d.name, d.value));
            ui.label(format!("(line {})", d.target));
        });
    }
    if let Some(vol) = panel.volume() {
        ui.label(format!("volume {vol:.1}"));
    }
    if let Some(err) = panel.error() {
        let danger = oep_ui_theme::Palette::resolve(mode).danger;
        ui.colored_label(danger, format!("dimension error: {err}"));
    }

    write_draft(&ctx, &draft);
    write_panel(&ctx, &panel);
    write_oracle(&ctx, &panel, &draft);
}
