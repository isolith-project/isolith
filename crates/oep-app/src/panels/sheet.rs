//! Drafting panel (wave F, card UI-F1 UI-SHEET): drawing sheets are
//! first-class objects - size, scale, projection angle and multiple sheets -
//! with a sheet tree the user can select.
//!
//! The panel hosts a live canonical drawing document (an `oep_geo::Document`,
//! seeded from the committed bracket fixture via the EXISTING drafting seam
//! [`oep_drafting::builder::drive_bracket_drawing`], never a second fixture)
//! and renders:
//!
//! - a **sheet tree**: one selectable row per sheet of the drawing (its name,
//!   size, scale and projection), so multiple sheets coexist and each is
//!   independently selectable;
//! - **sheet properties** for the selected sheet: paper size (A3 / A4), the
//!   drawing scale (1:1 / 1:2) and the first-/third-angle projection
//!   convention. Each edit is ONE journaled command through the UI-B2 routed
//!   [`doc_io::EditHistory`] seam (the same write path the model tree and
//!   assembly tree use), writes the canonical `oep_geo` sheet record, and
//!   therefore round-trips byte-identically through save and reopen;
//! - a **view-placement legality** rule: whether a requested placement of a
//!   standard view is legal depends on the sheet's projection. Under
//!   third-angle the top view must sit ABOVE the front view; the first-angle
//!   top placement (below the front) is REFUSED with a named error shown in
//!   the UI. The rule mirrors the EXISTING layout seam's documented
//!   convention ([`oep_drafting::layout`]: top/right fold the opposite way
//!   per projection), and a module test below re-derives it from
//!   [`oep_drafting::layout::layout_views`] so the UI rule can never drift
//!   from the seam it claims to speak for.
//!
//! F2-F4 sub-panels (views, annotate, export) are hosted ADDITIVELY here via
//! `#[path]` includes as those cards land, exactly like the assembly tree
//! hosts the E2-E4 panels. Their include + ONE call each are added by their
//! own cards; nothing in this file references a file that does not exist yet.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_geo::{Document, ProjectionType, SheetSize, ViewKind};

// The routed, undoable document seam (wave B, card UI-B2), included verbatim
// the way every other panel includes it: every sheet edit is ONE journaled
// command through `doc_io::EditHistory::apply`, so edits are undoable and the
// mutation guard stays green.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../doc_io.rs"]
mod doc_io;

// Wave F (card UI-F2 UI-VIEWS): the view placement tools (base / projected /
// section / detail placement with base-alignment). A `#[path]` child module
// exactly like the assembly panel hosts its E2-E4 tools; `show` renders it
// with ONE call below, keeping both sides touching nothing else in this file.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "views.rs"]
mod views;

// Wave F (card UI-F3 UI-ANNOTATE): the annotation tools - dimension, leader
// and note placement plus the document-bound title block. A `#[path]` child
// module exactly like `views` above; `show` renders it with ONE call below.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "annotate.rs"]
mod annotate;

// Wave F (card UI-F4 UI-DRAWING-EXPORT): SVG and PDF export from the UI with
// a print preview and honest errors. A `#[path]` child module exactly like
// `views` and `annotate` above; `show` renders it with ONE call below.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "drawing_export.rs"]
mod drawing_export;

/// Context-data key under which the panel reports its `String` oracle every
/// frame (sheet count, selection, per-sheet properties, placement legality,
/// document hash). Cross-`#[path]`-copy safe; the acceptance harness names it
/// by value exactly like the E-wave oracle keys.
pub const SHEET_REPORT_KEY: &str = "ui_f1_sheet_report";
/// Context-data key under which the panel publishes the live canonical
/// drawing document JSON every frame (what a save would write) - the
/// round-trip acceptance re-seeds this bytes into a fresh harness and
/// asserts the sheet record comes back byte-identical.
pub const SHEET_DOC_JSON_KEY: &str = "ui_f1_sheet_doc_json";
/// Context-data key a test may seed with the canonical `.oepdoc` JSON the
/// panel should open before its first frame (exactly like the tree's seed
/// key). Absent, the committed bracket drawing fixture is opened instead.
pub const SHEET_SEED_KEY: &str = "ui_f1_sheet_seed";
/// Context-data key under which the panel's own typed model lives (tests read
/// the `String` oracle instead, not this).
const SHEET_STATE_KEY: &str = "ui_f1_sheet_state";

/// Journal token recorded for one sheet-size edit.
pub const SIZE_TOKEN: &str = "sheet:size";
/// Journal token recorded for one sheet-scale edit.
pub const SCALE_TOKEN: &str = "sheet:scale";
/// Journal token recorded for one projection-angle edit.
pub const PROJECTION_TOKEN: &str = "sheet:projection";
/// Journal token recorded for adding a sheet.
pub const ADD_TOKEN: &str = "sheet:add";

/// The standard-view placement vocabulary (where a view sits relative to the
/// front view on the sheet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPlacement {
    /// The front view itself: the anchor placement on any sheet.
    FrontAnchor,
    /// Above the front view (third-angle top placement).
    AboveFront,
    /// Below the front view (first-angle top placement).
    BelowFront,
    /// Right of the front view (third-angle right placement).
    RightOfFront,
    /// Left of the front view (first-angle right placement).
    LeftOfFront,
}

impl ViewPlacement {
    /// The human label used in the UI and error text.
    pub fn label(self) -> &'static str {
        match self {
            ViewPlacement::FrontAnchor => "the front anchor",
            ViewPlacement::AboveFront => "above the front view",
            ViewPlacement::BelowFront => "below the front view",
            ViewPlacement::RightOfFront => "right of the front view",
            ViewPlacement::LeftOfFront => "left of the front view",
        }
    }
}

/// A named placement-refusal error (ever shown in the UI; never a panic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetError {
    /// The requested placement of `kind` is not the convention's placement
    /// for the sheet's projection.
    IllegalPlacement {
        /// The sheet's projection angle.
        projection: ProjectionType,
        /// Which standard view the user tried to place.
        kind: ViewKind,
        /// The placement the user requested.
        requested: ViewPlacement,
        /// The sole legal placement for that kind under `projection`.
        legal: ViewPlacement,
    },
}

/// The human label of a `ViewKind` ("front" / "top" / "right").
pub fn kind_label(kind: ViewKind) -> &'static str {
    match kind {
        ViewKind::Front => "front",
        ViewKind::Top => "top",
        ViewKind::Right => "right",
    }
}

impl std::fmt::Display for SheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SheetError::IllegalPlacement {
                projection,
                kind,
                requested,
                legal,
            } => write!(
                f,
                "SheetError::IllegalPlacement: cannot place the {} view {}. \
                 On a {} sheet the sole legal placement for the {} view \
                 is {}.",
                kind_label(*kind),
                requested.label(),
                projection_label(*projection),
                kind_label(*kind),
                legal.label(),
            ),
        }
    }
}

impl std::error::Error for SheetError {}

/// The human label of a projection convention.
pub fn projection_label(p: ProjectionType) -> &'static str {
    match p {
        ProjectionType::ThirdAngle => "third-angle",
        ProjectionType::FirstAngle => "first-angle",
    }
}

/// The single legal [`ViewPlacement`] of a standard view on a sheet with the
/// given projection. Mirrors the EXISTING layout seam's documented
/// convention: third-angle puts TOP above FRONT and RIGHT to the right;
/// first-angle folds them to TOP below FRONT and RIGHT to the left.
pub fn legal_placement(projection: ProjectionType, kind: ViewKind) -> ViewPlacement {
    match (projection, kind) {
        (_, ViewKind::Front) => ViewPlacement::FrontAnchor,
        (ProjectionType::ThirdAngle, ViewKind::Top) => ViewPlacement::AboveFront,
        (ProjectionType::FirstAngle, ViewKind::Top) => ViewPlacement::BelowFront,
        (ProjectionType::ThirdAngle, ViewKind::Right) => ViewPlacement::RightOfFront,
        (ProjectionType::FirstAngle, ViewKind::Right) => ViewPlacement::LeftOfFront,
    }
}

/// Check a requested view placement against the sheet's projection. `Ok(())`
/// for the convention's own placement of that view; a named
/// [`SheetError::IllegalPlacement`] otherwise - e.g. under third-angle, the
/// first-angle top placement (below the front) is refused.
pub fn check_placement(
    projection: ProjectionType,
    kind: ViewKind,
    requested: ViewPlacement,
) -> Result<(), SheetError> {
    let legal = legal_placement(projection, kind);
    if legal == requested {
        Ok(())
    } else {
        Err(SheetError::IllegalPlacement {
            projection,
            kind,
            requested,
            legal,
        })
    }
}

/// The model-unit-per-paper-unit view scale for a "1:N" sheet-scale callout:
/// a 1:2 sheet draws the model at half size, so `paper = model / 2`
/// (`oep-drafting::layout::paper_extent`), i.e. view scale `2.0`. 1:1 -> 1.0.
pub fn ratio_to_view_scale(ratio: &str) -> f64 {
    if ratio == "1:2" {
        2.0
    } else {
        1.0
    }
}

/// The "1:N" callout for a view's numeric scale (the inverse of
/// [`ratio_to_view_scale`]); anything other than exactly 2.0 reads as 1:1.
#[cfg(test)]
pub fn view_scale_to_ratio(scale: f64) -> String {
    if (scale - 2.0).abs() < 1e-9 {
        "1:2".to_string()
    } else {
        "1:1".to_string()
    }
}

/// The sheet's nominal scale callout, from its title block.
pub fn sheet_scale(sheet: &oep_geo::Sheet) -> String {
    sheet
        .title_block
        .scale
        .clone()
        .unwrap_or_else(|| "1:1".to_string())
}

/// One sheet-property change, applied to the selected sheet of the live
/// document through the routed journal. `mutate` is the panel's only writer.
fn apply_sheet_edit<M>(
    history: &mut doc_io::EditHistory,
    document: &mut Document,
    token: &'static str,
    selected: &str,
    mutate: M,
) where
    M: FnOnce(&mut oep_geo::Sheet),
{
    let selected = selected.to_string();
    history.apply(document, token, None, move |d| {
        if let Some(drawing) = d.drawings.first_mut() {
            if let Some(sheet) = drawing.sheets.iter_mut().find(|s| s.id == selected) {
                mutate(sheet);
            }
        }
    });
}

/// The panel's live drawing state, resident in the egui context store.
#[derive(Debug, Clone, Default)]
struct SheetModel {
    /// The canonical drawing document (part + drawing record).
    document: Option<Document>,
    /// The selected sheet id (`None` = nothing selected).
    selection: Option<String>,
    /// The routed, undoable history (UI-B2 journal over the document).
    history: doc_io::EditHistory,
    /// True once the document has been seeded.
    seeded: bool,
    /// Result of the last view-placement check: `Ok(summary)` or the error.
    last_placement: Option<Result<String, SheetError>>,
    /// One-line description of the last action (oracle readout).
    last: Option<String>,
}

impl SheetModel {
    fn selected_sheet(&self) -> Option<&oep_geo::Sheet> {
        let doc = self.document.as_ref()?;
        let drawing = doc.drawings.first()?;
        let id = self.selection.as_deref()?;
        drawing.sheets.iter().find(|s| s.id == id)
    }

    fn sheet_count(&self) -> usize {
        self.document
            .as_ref()
            .and_then(|d| d.drawings.first())
            .map(|dr| dr.sheets.len())
            .unwrap_or(0)
    }
}

/// Seed the live document on the first frame: the canonical JSON a test
/// placed under [`SHEET_SEED_KEY`] if any (equivalent to opening that drawing
/// document), else the COMMITTED BRACKET fixture driven through the existing
/// drafting seam (`drive_bracket_drawing` - the engine's own golden for the
/// bracket drawing). A failed parse leaves an honest empty tree.
fn seed_model(ctx: &egui::Context, model: &mut SheetModel) {
    let seed: Option<String> = ctx.data(|d| d.get_temp::<String>(egui::Id::unique(SHEET_SEED_KEY)));
    if let Some(json) = seed {
        if let Ok(doc) = Document::from_json_pretty(&json) {
            if doc.validate().is_empty() {
                model.history.clear();
                model.document = Some(doc);
                model.last = Some("opened drawing document".to_string());
                model.seeded = true;
                model.selection = model
                    .document
                    .as_ref()
                    .and_then(|d| d.drawings.first())
                    .and_then(|dr| dr.sheets.first())
                    .map(|s| s.id.clone());
                return;
            }
        }
    }
    if let Ok(driven) = drive_bracket_drawing() {
        model.document = Some(driven.document);
        model.last = Some("opened committed bracket drawing".to_string());
        model.selection = model
            .document
            .as_ref()
            .and_then(|d| d.drawings.first())
            .and_then(|dr| dr.sheets.first())
            .map(|s| s.id.clone());
    }
    model.seeded = true;
}

/// Drive the committed bracket drawing through the EXISTING drafting seam
/// (`oep_drafting::builder::drive_bracket_drawing`, the engine's golden for
/// "bracket drawing": A4 sheet, three standard views, three variable-driven
/// dimensions). Same sketch + variable set as the committed golden generator.
fn drive_bracket_drawing(
) -> Result<oep_drafting::builder::BracketDrawing, oep_sketch::solve::SketchError> {
    use oep_drafting::builder::drive_bracket_drawing as drive;
    use oep_sketch::sketch::{DimExpr, Sketch};
    use oep_sketch::varset::VariableSet;

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

    let mut vars = VariableSet::new();
    vars.set("width", 80.0);
    vars.set("height", 40.0);
    vars.set("depth", 6.0);

    drive(
        "oep-part-bracket",
        "bracket",
        &s,
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
}

/// Render the drafting panel content inside its already-positioned region (the
/// properties column's lower portion).
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    ui.label(theme::heading(theme::mode_of(ui), "Drafting").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    let mut model = read_model(ui.ctx());
    if !model.seeded {
        seed_model(ui.ctx(), &mut model);
    }

    let mode = theme::mode_of(ui);
    let palette = oep_ui_theme::Palette::resolve(mode);

    egui::ScrollArea::vertical()
        .id_salt("ui_f1_drafting")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ---- Sheet tree -------------------------------------------------
            ui.label(theme::heading(mode, "Sheet tree").size(12.0));
            let sheets: Vec<oep_geo::Sheet> = model
                .document
                .as_ref()
                .and_then(|d| d.drawings.first())
                .map(|dr| dr.sheets.clone())
                .unwrap_or_default();
            if sheets.is_empty() {
                ui.label(
                    egui::RichText::new("No sheets in the drawing")
                        .weak()
                        .italics(),
                );
            }
            for sheet in &sheets {
                let selected = model.selection.as_deref() == Some(sheet.id.as_str());
                let label = format!(
                    "{} · {} · {} · scale {}",
                    sheet.name,
                    sheet.size.name(),
                    projection_label(sheet.projection),
                    sheet_scale(sheet),
                );
                if ui.selectable_label(selected, &label).clicked() {
                    model.selection = Some(sheet.id.clone());
                    model.last = Some(format!("selected {}", sheet.name));
                }
            }

            // ---- Toolbar -----------------------------------------------------
            ui.add_space(6.0);
            if ui.button("Add sheet").clicked() {
                if let Some(document) = model.document.as_mut() {
                    // One journaled command: append a fresh sheet (default A4,
                    // third-angle, no views) to the drawing and select it. The
                    // new id is deterministic from the drawing's sheet count,
                    // computed before the apply (the journaled mutation only).
                    let added_id = {
                        let history = &mut model.history;
                        let drawing_len = document
                            .drawings
                            .first()
                            .map(|dr| dr.sheets.len())
                            .unwrap_or(0);
                        let (id, name) = (
                            format!("oep-sheet-{}", drawing_len + 1),
                            format!("Sheet {}", drawing_len + 1),
                        );
                        history.apply(document, ADD_TOKEN, None, move |d| {
                            if let Some(drawing) = d.drawings.first_mut() {
                                drawing.sheets.push(oep_geo::Sheet::new(id, name));
                            }
                        });
                        format!("oep-sheet-{}", drawing_len + 1)
                    };
                    model.selection = Some(added_id);
                    model.last = Some("added a sheet".to_string());
                }
            }

            // ---- Selected sheet properties ----------------------------------
            if let Some(selected) = model.selection.clone() {
                if let Some(sheet) = model.selected_sheet().cloned() {
                    ui.add_space(8.0);
                    ui.label(theme::heading(mode, "Sheet properties").size(12.0));
                    ui.separator();
                    ui.add_space(4.0);

                    let current_scale = sheet_scale(&sheet);

                    // Size: A3 / A4.
                    ui.horizontal_wrapped(|ui| {
                        ui.label("size:");
                        for (label, size) in [
                            ("A3", SheetSize::A3_LANDSCAPE),
                            ("A4", SheetSize::A4_LANDSCAPE),
                        ] {
                            let is = sheet.size == size;
                            if ui.selectable_label(is, label).clicked() && !is {
                                if let Some(document) = model.document.as_mut() {
                                    apply_sheet_edit(
                                        &mut model.history,
                                        document,
                                        SIZE_TOKEN,
                                        &selected,
                                        |s| s.size = size,
                                    );
                                    model.last = Some(format!("size -> {label} ({})", size.name()));
                                }
                            }
                        }
                    });
                    // Scale: 1:1 / 1:2.
                    ui.horizontal_wrapped(|ui| {
                        ui.label("scale:");
                        for ratio in ["1:1", "1:2"] {
                            let is = current_scale == ratio;
                            if ui.selectable_label(is, ratio).clicked() && !is {
                                if let Some(document) = model.document.as_mut() {
                                    apply_sheet_edit(
                                        &mut model.history,
                                        document,
                                        SCALE_TOKEN,
                                        &selected,
                                        |s| {
                                            s.title_block.scale = Some(ratio.to_string());
                                            let view_scale = ratio_to_view_scale(ratio);
                                            for v in &mut s.views {
                                                v.scale = view_scale;
                                            }
                                        },
                                    );
                                    model.last = Some(format!("scale -> {ratio}"));
                                }
                            }
                        }
                    });
                    // Projection: first/third angle.
                    ui.horizontal_wrapped(|ui| {
                        ui.label("projection:");
                        for (label, proj) in [
                            ("first-angle", ProjectionType::FirstAngle),
                            ("third-angle", ProjectionType::ThirdAngle),
                        ] {
                            let is = sheet.projection == proj;
                            if ui.selectable_label(is, label).clicked() && !is {
                                if let Some(document) = model.document.as_mut() {
                                    apply_sheet_edit(
                                        &mut model.history,
                                        document,
                                        PROJECTION_TOKEN,
                                        &selected,
                                        |s| s.projection = proj,
                                    );
                                    model.last =
                                        Some(format!("projection -> {}", projection_label(proj)));
                                }
                            }
                        }
                    });

                    // ---- View-placement legality ----------------------------
                    ui.add_space(8.0);
                    ui.label(theme::heading(mode, "View placement legality").size(12.0));
                    ui.separator();
                    ui.add_space(4.0);
                    let projection = sheet.projection;
                    ui.label(format!(
                        "On this {} sheet: top must be {}, right must be {}.",
                        projection_label(projection),
                        legal_placement(projection, ViewKind::Top).label(),
                        legal_placement(projection, ViewKind::Right).label(),
                    ));
                    if ui
                        .button("Place top above (third-angle placement)")
                        .clicked()
                    {
                        model.last_placement = Some(
                            check_placement(projection, ViewKind::Top, ViewPlacement::AboveFront)
                                .map(|_| "top above front: legal".to_string()),
                        );
                    }
                    if ui
                        .button("Place top below (first-angle placement)")
                        .clicked()
                    {
                        model.last_placement = Some(
                            check_placement(projection, ViewKind::Top, ViewPlacement::BelowFront)
                                .map(|_| "top below front: legal".to_string()),
                        );
                    }
                    if let Some(result) = &model.last_placement {
                        match result {
                            Ok(line) => {
                                ui.label(line);
                            }
                            Err(err) => {
                                ui.colored_label(palette.danger, err.to_string());
                            }
                        }
                    }
                }
            } else {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Select a sheet to edit its properties")
                        .weak()
                        .italics(),
                );
            }

            // Wave F (card UI-F2 UI-VIEWS): the ONE additive call of the view
            // placement tools, rendered after the sheet controls (so those keep
            // their exact layout). Everything below stays placed on the same
            // selected sheet through the same journaled model.
            views::show(ui, app, &mut model);

            // Wave F (card UI-F3 UI-ANNOTATE): the ONE additive call of the
            // annotation tools (dimensions, leaders, notes, document-bound
            // title block), rendered after the view placement tools.
            annotate::show(ui, app, &mut model);

            // Wave F (card UI-F4 UI-DRAWING-EXPORT): the ONE additive call of
            // the export tools (print preview + SVG/PDF export + honest
            // errors), rendered after the annotation tools.
            drawing_export::show(ui, app, &mut model);
        });

    publish_report(ui.ctx(), &model);
    write_model(ui.ctx(), &model);
    app.status.mode = format!(
        "drafting · {} sheet{}",
        model.sheet_count(),
        if model.sheet_count() == 1 { "" } else { "s" }
    );
}

// --- context-store plumbing (read/write the panel model) ---

fn read_model(ctx: &egui::Context) -> SheetModel {
    ctx.data(|d| d.get_temp(egui::Id::unique(SHEET_STATE_KEY)))
        .unwrap_or_default()
}

fn write_model(ctx: &egui::Context, model: &SheetModel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(SHEET_STATE_KEY), model.clone()));
}

/// Publish the plain-`String` report oracle plus the live canonical document
/// JSON under [`SHEET_DOC_JSON_KEY`].
fn publish_report(ctx: &egui::Context, model: &SheetModel) {
    let document = model.document.as_ref();
    let hash = document.map(doc_io::document_hash).unwrap_or(0);
    let sheets: Vec<oep_geo::Sheet> = document
        .and_then(|d| d.drawings.first())
        .map(|dr| dr.sheets.clone())
        .unwrap_or_default();

    let sel = model.selection.clone().unwrap_or_else(|| "-".to_string());
    let mut report = String::new();
    report.push_str(&format!("open={}\n", u8::from(document.is_some())));
    report.push_str(&format!("sheets={}\n", sheets.len()));
    report.push_str(&format!("sel={sel}\n"));
    if let Some(sheet) = model.selected_sheet() {
        report.push_str(&format!(
            "size={}x{}\n",
            sheet.size.width_mm, sheet.size.height_mm
        ));
        report.push_str(&format!("size_name={}\n", sheet.size.name()));
        report.push_str(&format!("scale={}\n", sheet_scale(sheet)));
        report.push_str(&format!(
            "projection={}\n",
            projection_label(sheet.projection)
        ));
        report.push_str(&format!("views={}\n", sheet.views.len()));
    }
    match &model.last_placement {
        Some(Ok(line)) => {
            report.push_str("placement_status=legal\nplacement_error=-\n");
            report.push_str(&format!("placement={line}\n"));
        }
        Some(Err(err)) => {
            report.push_str("placement_status=refused\n");
            report.push_str(&format!("placement_error={err}\n"));
            report.push_str(&format!("placement=-{err}\n"));
        }
        None => {
            report.push_str("placement_status=-\nplacement_error=-\nplacement=-\n");
        }
    }
    report.push_str(&format!("journal={}\n", model.history.len()));
    report.push_str(&format!("doc_hash={hash}\n"));
    report.push_str(&format!("last={}\n", model.last.as_deref().unwrap_or("-")));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(SHEET_REPORT_KEY), report));

    if let Some(document) = document {
        if let Ok(json) = document.to_json_pretty() {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(SHEET_DOC_JSON_KEY), json);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The UI legality rule agrees with the EXISTING layout seam: for every
    /// projection and every standard view, the legal placement is exactly the
    /// arrangement `oep_drafting::layout::layout_views` computes (re-derived,
    /// never copied into the rule by hand).
    #[test]
    fn legal_placement_matches_layout_seam() {
        use oep_drafting::layout::layout_views;
        // Lay a tiny front + the candidate kind out per the seam in a big
        // blank region, then read where the kind sits relative to front.
        let locate = |projection: ProjectionType, kind: ViewKind| -> ViewPlacement {
            let views = [(ViewKind::Front, 1.0, 1.0, 1.0), (kind, 1.0, 1.0, 1.0)];
            let placed = layout_views(projection, &views, 1000.0, 1000.0);
            let front = placed.iter().find(|p| p.kind == ViewKind::Front).unwrap();
            let other = placed.iter().find(|p| p.kind == kind).unwrap();
            let (dx, dy) = (other.cx - front.cx, other.cy - front.cy);
            match kind {
                ViewKind::Front => ViewPlacement::FrontAnchor,
                ViewKind::Top => {
                    if dy > 0.0 {
                        ViewPlacement::AboveFront
                    } else {
                        ViewPlacement::BelowFront
                    }
                }
                ViewKind::Right => {
                    if dx > 0.0 {
                        ViewPlacement::RightOfFront
                    } else {
                        ViewPlacement::LeftOfFront
                    }
                }
            }
        };
        for projection in [ProjectionType::ThirdAngle, ProjectionType::FirstAngle] {
            for kind in [ViewKind::Front, ViewKind::Top, ViewKind::Right] {
                assert_eq!(
                    legal_placement(projection, kind),
                    locate(projection, kind),
                    "rule for {projection:?} {kind:?} drifted from the layout seam"
                );
            }
        }
    }

    /// Third-angle REFUSES the first-angle top placement with the NAMED error;
    /// first-angle refuses the third-angle top placement similarly. The
    /// projection angle therefore changes which view placements are legal.
    #[test]
    fn third_angle_forbids_first_angle_top_placement() {
        let err = check_placement(
            ProjectionType::ThirdAngle,
            ViewKind::Top,
            ViewPlacement::BelowFront,
        )
        .expect_err("third-angle must refuse the first-angle top placement");
        let text = err.to_string();
        assert!(text.contains("SheetError::IllegalPlacement"), "{text}");
        assert!(text.contains("top"), "{text}");
        assert!(text.contains("third-angle"), "{text}");
        assert!(text.contains("below the front view"), "{text}");

        // The projection flips legality: first-angle accepts the same request.
        assert!(check_placement(
            ProjectionType::FirstAngle,
            ViewKind::Top,
            ViewPlacement::BelowFront
        )
        .is_ok());

        // And the reverse: third-angle accepts its own top placement.
        assert!(check_placement(
            ProjectionType::ThirdAngle,
            ViewKind::Top,
            ViewPlacement::AboveFront
        )
        .is_ok());
    }

    /// Scale mapping matches the layout seam's paper extent convention:
    /// 1:2 -> view scale 2.0 (paper = model / 2), 1:1 -> 1.0, round trip.
    #[test]
    fn scale_mapping_round_trips() {
        assert_eq!(ratio_to_view_scale("1:1"), 1.0);
        assert_eq!(ratio_to_view_scale("1:2"), 2.0);
        assert_eq!(view_scale_to_ratio(1.0), "1:1");
        assert_eq!(view_scale_to_ratio(2.0), "1:2");
    }

    /// The bracket fixture carries the committed sheet record the acceptance
    /// edits (A4 landscape, third-angle, three views) and the sheet properties
    /// round-trip through the canonical document bytes.
    #[test]
    fn bracket_fixture_round_trips_through_canonical_bytes() {
        let driven = drive_bracket_drawing().expect("drives");
        let doc = driven.document;
        let sheet = &doc.drawings[0].sheets[0];
        assert_eq!(sheet.size, SheetSize::A4_LANDSCAPE);
        assert_eq!(sheet.projection, ProjectionType::ThirdAngle);
        assert_eq!(sheet.views.len(), 3);
        assert_eq!(sheet_scale(sheet), "1:1");

        // Set the acceptance values, save (canonical JSON), reopen (parse),
        // and every sheet field is byte-identical.
        let mut edited = doc.clone();
        let drawing = edited.drawings.first_mut().unwrap();
        let sheet = drawing.sheets.first_mut().unwrap();
        sheet.size = SheetSize::A3_LANDSCAPE;
        sheet.projection = ProjectionType::FirstAngle;
        sheet.title_block.scale = Some("1:2".to_string());
        for v in &mut sheet.views {
            v.scale = 2.0;
        }
        let saved = edited.to_json_pretty().expect("serialize");
        let reopened = Document::from_json_pretty(&saved).expect("reopen");
        assert_eq!(
            reopened, edited,
            "sheet record must round-trip byte-identical"
        );
    }
}
