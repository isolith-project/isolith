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

// The UI-B8 feature-from-sketch panel, included verbatim so this panel's
// per-feature parameter editor reuses the B8 preview lane: the same
// `committed_geometry` / `build_clone` derivation that turns the committed
// sketch profile into a marked-preview document clone and its derived solid.
// Like the sibling `#[path]` copies, this is a fresh module compile; only the
// pure derivation functions are called here (never its typed dialog state),
// so no cross-copy value is shared. `duplicate_mod` is the same allowance the
// B4/B7/B8 tests make for their second copies.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "features.rs"]
mod feature_preview;

// The routed, undoable document seam (wave B, card UI-B2), included verbatim
// the way `panels/tree.rs` includes it: the C2 parameter editor owns its own
// live model document plus an `EditHistory`, so every Apply is ONE journaled
// command and Cancel leaves the document byte-identical. The two copies never
// share a stored typed value (each panel keeps its own document + history).
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../doc_io.rs"]
mod doc_io;

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

    // UI-C2 UI-PROPERTY-PANEL: the per-feature parameter editor (below the
    // B7 dimension editor). Reads the one shared selection id the tree panel
    // publishes; editing the selected feature's depth / angle / direction /
    // symmetric shows a live preview through the B8 derivation lane, refuses
    // invalid values (document untouched, field+reason named), and separates
    // Apply (one journal entry) from Cancel (discard).
    c2_parameter_editor(ui, _app);
}

// ---------------------------------------------------------------------------
// UI-C2 UI-PROPERTY-PANEL: per-feature parameter editor
// ---------------------------------------------------------------------------
//
// The right column edits the SELECTED feature's parameters (depth, angle,
// direction, symmetric) with a live preview. Details of this card's contract:
//
// - The panel keeps its own authoritative live model document + a
//   `doc_io::EditHistory` (the UI-B2 journal), seeded from the same source the
//   tree panel uses (`ui_c1_tree_seed` `.oepdoc` JSON, else the committed demo
//   STEP) so the shared selection id resolves to the same feature. Every Apply
//   runs `EditHistory::apply` (ONE journaled command); Cancel never touches the
//   journal, so its document hash is byte-identical before and after.
// - The live preview reuses the UI-B8 derivation lane verbatim (this file's
//   `feature_preview` copy of `features.rs`): the committed sketch profile from
//   the sketch oracle plus the feature's kind and the edited amount produce a
//   marked-preview document clone whose derived volume/face count and a
//   rasterized frame hash are published. A changed depth therefore moves the
//   volume predictably (solid = area x depth) without touching the document.
// - Invalid text leaves the document untouched and names the FIELD and the
//   REASON (e.g. `depth` / "must be a positive number") in the oracle.
// - Apply and Cancel differ by the journal: Apply commits exactly one entry
//   (oracle `journal` and `doc_hash` move), Cancel discards (both unchanged).

/// Context-data key under which the C2 parameter editor's typed state lives
/// (this module's copy only; tests read the `String` oracle instead).
const PARAM_STATE_KEY: &str = "ui_c2_param_state";
/// Context-data key under which the C2 oracle (a plain `String`) is published
/// every frame. Cross-`#[path]`-copy safe.
pub const PARAM_REPORT_KEY: &str = "ui_c2_property_report";
/// The one shared selection id (must match `tree::TREE_SELECTION_KEY`; a local
/// literal avoids depending on the parent module across `#[path]` copies).
const TREE_SELECTION_KEY: &str = "isolith.ui.tree.selection";
/// The tree panel's `.oepdoc` seed key (must match `tree::TREE_SEED_KEY`) so a
/// test seeds the property panel's own document copy like the tree's.
const TREE_SEED_KEY: &str = "ui_c1_tree_seed";
/// Journal token for one applied parameter edit (the UI-B2 journal's token is
/// a name, hashes before/after; the panel's editing language, like `tree:`).
pub const PARAM_TOKEN: &str = "feat:param";

/// The four parameter keys the editor writes into `Feature.parameters`.
pub const PARAM_DEPTH: &str = "depth";
pub const PARAM_ANGLE: &str = "angle";
pub const PARAM_DIRECTION: &str = "direction";
pub const PARAM_SYMMETRIC: &str = "symmetric";

/// The accepted `direction` axis tokens for a solid feature.
pub const DIRECTION_TOKENS: [&str; 6] = ["+Z", "-Z", "+X", "-X", "+Y", "-Y"];
/// Default depth when the selected feature carries no `depth` parameter.
pub const DEFAULT_DEPTH: f64 = 10.0;
/// Default angle when the selected feature carries no `angle` parameter.
pub const DEFAULT_ANGLE: f64 = std::f64::consts::FRAC_PI_2;
/// Default direction when the selected feature carries none.
pub const DEFAULT_DIRECTION: &str = "+Z";

/// The on-screen rects of the C2 parameter editor widgets, laid out below the
/// B7 dimension editor. Public so the acceptance harness clicks the exact
/// rects the renderer uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamWidgetRects {
    /// `depth` text input.
    pub depth: egui::Rect,
    /// `angle` text input.
    pub angle: egui::Rect,
    /// `direction` text input.
    pub direction: egui::Rect,
    /// `symmetric` checkbox.
    pub symmetric: egui::Rect,
    /// Apply button.
    pub apply: egui::Rect,
    /// Cancel button.
    pub cancel: egui::Rect,
}

/// Fixed widget offsets/layout for the C2 editor (below the B7 block).
const PARAM_X: f32 = 8.0;
const PARAM_Y0: f32 = 166.0;
const PARAM_ROW: f32 = 32.0;
const PARAM_FIELD_W: f32 = 120.0;
const PARAM_FIELD_H: f32 = 22.0;
const PARAM_BTN_W: f32 = 88.0;
const PARAM_BTN_H: f32 = 26.0;

/// Build the deterministic widget rects for the C2 parameter editor, anchored
/// in the properties region below the B7 dimension editor.
pub fn param_widget_rects(region: egui::Rect) -> ParamWidgetRects {
    let x = region.min.x + PARAM_X;
    let y0 = region.min.y + PARAM_Y0;
    let field = |row: usize| {
        egui::Rect::from_min_size(
            egui::pos2(x, y0 + PARAM_ROW * row as f32),
            egui::vec2(PARAM_FIELD_W, PARAM_FIELD_H),
        )
    };
    let yb = y0 + PARAM_ROW * 4.0;
    ParamWidgetRects {
        depth: field(0),
        angle: field(1),
        direction: field(2),
        symmetric: field(3),
        apply: egui::Rect::from_min_size(egui::pos2(x, yb), egui::vec2(PARAM_BTN_W, PARAM_BTN_H)),
        cancel: egui::Rect::from_min_size(
            egui::pos2(x + PARAM_BTN_W + 12.0, yb),
            egui::vec2(PARAM_BTN_W, PARAM_BTN_H),
        ),
    }
}

/// The parsed, committed parameter set for the selected feature.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureParams {
    /// Extrude depth in mm.
    pub depth: f64,
    /// Revolve angle in radians.
    pub angle: f64,
    /// Axis token (`+Z`, `-Z`, ...).
    pub direction: String,
    /// Symmetric about the sketch plane.
    pub symmetric: bool,
}

impl Default for FeatureParams {
    fn default() -> Self {
        Self {
            depth: DEFAULT_DEPTH,
            angle: DEFAULT_ANGLE,
            direction: DEFAULT_DIRECTION.to_string(),
            symmetric: false,
        }
    }
}

/// Read a numeric parameter from a feature's parameter map.
fn param_f64(feature: &oep_import::Feature, key: &str) -> Option<f64> {
    feature.parameters.get(key).and_then(|v| v.as_f64())
}

/// Parse a numeric parameter from raw user text. `None` when absent in the
/// feature; invalid free text is a named refusal, never a silent fallback.
fn parse_num(text: &str, key: &str) -> Result<Option<f64>, (String, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        Ok(v) => Err((key.to_string(), format!("must be a finite number, got {v}"))),
        Err(_) => Err((
            key.to_string(),
            format!("must be a number, got '{trimmed}'"),
        )),
    }
}

/// Validate the four parameter drafts; returns the committed set. An invalid
/// draft yields the named field + reason (the card: invalid values leave the
/// document untouched and name the field and reason).
pub fn validate_params(
    depth: &str,
    angle: &str,
    direction: &str,
    symmetric: bool,
) -> Result<FeatureParams, (String, String)> {
    let depth = parse_num(depth, PARAM_DEPTH)?;
    let angle = parse_num(angle, PARAM_ANGLE)?;
    let dir = direction.trim();
    if dir.is_empty() {
        return Err((
            PARAM_DIRECTION.to_string(),
            "must pick an axis token".to_string(),
        ));
    }
    if !DIRECTION_TOKENS.contains(&dir) {
        return Err((
            PARAM_DIRECTION.to_string(),
            format!("must be one of {}, got '{dir}'", DIRECTION_TOKENS.join(" ")),
        ));
    }
    let depth = depth.unwrap_or(DEFAULT_DEPTH);
    let angle = angle.unwrap_or(DEFAULT_ANGLE);
    if depth <= 0.0 {
        return Err((
            PARAM_DEPTH.to_string(),
            format!("must be a positive number, got {depth}"),
        ));
    }
    if !(0.0..=std::f64::consts::TAU).contains(&angle) {
        return Err((
            PARAM_ANGLE.to_string(),
            format!("must be in [0, 2π], got {angle}"),
        ));
    }
    Ok(FeatureParams {
        depth,
        angle,
        direction: dir.to_string(),
        symmetric,
    })
}

/// The live preview state for the selected feature: the derived solid's own
/// volume and face count (the B8 lane's readout) plus a rasterized frame hash.
#[derive(Debug, Clone, Default)]
pub struct ParamPreview {
    /// Whether a preview solid is currently derived.
    pub present: bool,
    /// Derived solid volume (the document's own mass property).
    pub volume: f64,
    /// Face count of the derived mesh.
    pub faces: usize,
    /// FNV-1a hash of the rasterized preview frame.
    pub frame_hash: u64,
}

/// The C2 parameter editor's persistent panel state (this module's copy).
#[derive(Debug, Clone)]
struct ParamPanel {
    /// The editorial live document (own copy, seeded like the tree).
    document: Option<oep_import::Document>,
    /// The UI-B2 routed journal over that document (Apply = one entry).
    history: doc_io::EditHistory,
    /// Whether the document has been seeded.
    seeded: bool,
    /// The feature id currently being edited (mirror of the shared selection).
    selected: String,
    /// Draft text of the depth field.
    depth: String,
    /// Draft text of the angle field.
    angle: String,
    /// Draft text of the direction field.
    direction: String,
    /// Draft value of the symmetric checkbox.
    symmetric: bool,
    /// Last successful Apply (a committed parameter set) for cancel-restore.
    applied: FeatureParams,
    /// The live preview, rebuilt on any valid edit.
    preview: ParamPreview,
    /// Named refusal: (field, reason). Empty reason = no error.
    field_error: (String, String),
    /// One-line description of the last action (oracle readout).
    last: Option<String>,
}

impl Default for ParamPanel {
    fn default() -> Self {
        Self {
            document: None,
            history: doc_io::EditHistory::new(),
            seeded: false,
            selected: String::new(),
            depth: String::new(),
            angle: String::new(),
            direction: DEFAULT_DIRECTION.to_string(),
            symmetric: false,
            applied: FeatureParams::default(),
            preview: ParamPreview::default(),
            field_error: (String::new(), String::new()),
            last: None,
        }
    }
}

fn read_param_panel(ctx: &egui::Context) -> ParamPanel {
    ctx.data(|d| d.get_temp(egui::Id::unique(PARAM_STATE_KEY)))
        .unwrap_or_default()
}

fn write_param_panel(ctx: &egui::Context, panel: &ParamPanel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(PARAM_STATE_KEY), panel.clone()));
}

/// Read the shared selection id published by the tree panel.
fn shared_selection(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::unique(TREE_SELECTION_KEY)))
        .unwrap_or_default()
}

/// Seed the panel's live document from the same source the tree uses: the
/// canonical `.oepdoc` JSON a test placed under `TREE_SEED_KEY`, else the
/// committed demo STEP through the app's own import path.
fn seed_panel_document(ctx: &egui::Context, panel: &mut ParamPanel) {
    let seed: Option<String> = ctx.data(|d| d.get_temp::<String>(egui::Id::unique(TREE_SEED_KEY)));
    if let Some(json) = seed {
        if let Ok(doc) = oep_import::Document::from_json_pretty(&json) {
            panel.history.clear();
            panel.document = Some(doc);
            panel.seeded = true;
            return;
        }
    }
    if let Some(path) = doc_io::demo_step_path() {
        panel.document = doc_io::import_step(&path).ok();
    }
    panel.seeded = true;
}

/// The feature selected by `id` in the panel's live document, if any.
fn selected_feature<'a>(panel: &'a ParamPanel, id: &str) -> Option<&'a oep_import::Feature> {
    if id.is_empty() {
        return None;
    }
    panel
        .document
        .as_ref()
        .and_then(|d| d.features.iter().find(|f| f.id == id))
}

/// The editable parameter set for a feature: values already in its parameter
/// map, falling back to the canonical defaults for absent keys.
fn params_of(feature: &oep_import::Feature) -> FeatureParams {
    FeatureParams {
        depth: param_f64(feature, PARAM_DEPTH).unwrap_or(DEFAULT_DEPTH),
        angle: param_f64(feature, PARAM_ANGLE).unwrap_or(DEFAULT_ANGLE),
        direction: feature
            .parameters
            .get(PARAM_DIRECTION)
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_DIRECTION)
            .to_string(),
        symmetric: feature
            .parameters
            .get(PARAM_SYMMETRIC)
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

/// The feature kind as a string (e.g. `Extrude`, `Revolve`, `ImportStep`) or
/// empty when the selection is not a feature (a part/body row).
fn feature_kind(panel: &ParamPanel, id: &str) -> String {
    selected_feature(panel, id)
        .map(|f| f.kind.clone())
        .unwrap_or_default()
}

/// Whether this feature kind has a solid-deriving amount this editor models
/// (Extrude uses `depth`, Revolve uses `angle`).
fn is_editable_kind(kind: &str) -> bool {
    matches!(kind, "Extrude" | "Revolve")
}

/// Derive the live preview through the UI-B8 lane: the committed sketch
/// profile (from the sketch oracle) plus the feature kind and the edited
/// amount produce a marked-preview document clone whose volume / face count /
/// rasterized frame hash are returned. `None` when no closed profile is
/// committed yet (an honest "draw a profile" state, never fake geometry).
fn derive_preview(ctx: &egui::Context, kind: &str, params: &FeatureParams) -> ParamPreview {
    let report: String = ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default();
    let Some(sk) = feature_preview::committed_geometry(&report) else {
        return ParamPreview::default();
    };
    let (fkind, amount) = match kind {
        "Extrude" => (feature_preview::FeatureKind::Extrude, params.depth),
        "Revolve" => (feature_preview::FeatureKind::Revolve, params.angle),
        _ => return ParamPreview::default(),
    };
    let Ok(session) = feature_preview::build_clone(&sk, fkind, amount) else {
        return ParamPreview::default();
    };
    let volume = session.props().volume.unwrap_or(0.0);
    let faces = session.mesh().map(|m| m.triangles.len()).unwrap_or(0);
    // A deterministic preview frame fingerprint through the same raster path
    // B8 uses (the viewport shows the applied solid; this is its hash).
    let scene = session.build_scene().ok();
    let hash = scene
        .as_ref()
        .map(|s| {
            let cam = s
                .bounds
                .as_ref()
                .map(oep_viewer::Camera::framed)
                .unwrap_or_default();
            let img = oep_viewer::render::render(s, &cam, &[], 96, 96);
            oep_viewer::backend::frame_hash(&img)
        })
        .unwrap_or(0);
    ParamPreview {
        present: true,
        volume,
        faces,
        frame_hash: hash,
    }
}

/// Load the selected feature into the editor: read its committed params into
/// the drafts and rebuild the preview. Keeps the selection empty-state honest.
fn load_feature(ctx: &egui::Context, panel: &mut ParamPanel) {
    panel.field_error = (String::new(), String::new());
    panel.depth.clear();
    panel.angle.clear();
    panel.preview = ParamPreview::default();
    if let Some(feature) = selected_feature(panel, &panel.selected) {
        let committed = params_of(feature);
        let (name, kind) = (feature.name.clone(), feature.kind.clone());
        panel.applied = committed.clone();
        panel.depth = format!("{}", committed.depth);
        panel.angle = format!("{}", committed.angle);
        panel.direction = committed.direction.clone();
        panel.symmetric = committed.symmetric;
        panel.last = Some(format!("editing {name} ({kind})"));
        if is_editable_kind(&kind) {
            panel.preview = derive_preview(ctx, &kind, &committed);
        }
    } else {
        panel.applied = FeatureParams::default();
        panel.direction = DEFAULT_DIRECTION.to_string();
        panel.last = Some(if panel.selected.is_empty() {
            "no feature selected".to_string()
        } else {
            "selection is not a feature".to_string()
        });
    }
}

/// One frame of the C2 editor: read the shared selection, render the four
/// parameter fields + Apply/Cancel, re-derive the live preview on any valid
/// edit, and publish the report oracle.
pub fn c2_parameter_editor(ui: &mut egui::Ui, _app: &mut App) {
    let ctx = ui.ctx().clone();
    let mut panel = read_param_panel(&ctx);
    if !panel.seeded {
        seed_panel_document(&ctx, &mut panel);
    }

    let selection = shared_selection(&ctx);
    if panel.selected != selection {
        panel.selected = selection;
        load_feature(&ctx, &mut panel);
    }

    let kind = feature_kind(&panel, &panel.selected);
    let editable = is_editable_kind(&kind);
    let region = ui.max_rect();
    let rects = param_widget_rects(region);

    if panel.selected.is_empty() {
        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("Select a feature to edit its parameters")
                .weak()
                .italics(),
        );
    } else if !editable {
        ui.add_space(18.0);
        ui.label(
            egui::RichText::new(format!("No editable parameters for {kind}"))
                .weak()
                .italics(),
        );
    } else {
        ui.add_space(10.0);
        ui.label(theme::heading(theme::mode_of(ui), "Feature parameters").size(12.0));
        ui.separator();
        ui.add_space(4.0);

        let p = oep_ui_theme::Palette::resolve(theme::mode_of(ui));
        let label = |ui: &mut egui::Ui, rect: egui::Rect, text: &str| {
            ui.painter().text(
                egui::pos2(rect.min.x, rect.min.y - 2.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::proportional(9.0),
                p.text_muted,
            );
        };
        label(ui, rects.depth, "depth (mm)");
        label(ui, rects.angle, "angle (rad)");
        label(ui, rects.direction, "direction");
        label(ui, rects.symmetric, "symmetric");

        let mut depth = panel.depth.clone();
        let mut angle = panel.angle.clone();
        let mut direction = panel.direction.clone();
        let mut symmetric = panel.symmetric;

        let d = ui.put(
            rects.depth,
            egui::TextEdit::singleline(&mut depth)
                .hint_text("10.0")
                .font(egui::TextStyle::Monospace),
        );
        let a = ui.put(
            rects.angle,
            egui::TextEdit::singleline(&mut angle)
                .hint_text("1.5708")
                .font(egui::TextStyle::Monospace),
        );
        let dir = ui.put(
            rects.direction,
            egui::TextEdit::singleline(&mut direction)
                .hint_text("+Z")
                .font(egui::TextStyle::Monospace),
        );
        let s = ui.put(
            rects.symmetric,
            egui::Checkbox::new(&mut symmetric, "symmetric"),
        );

        let apply = ui.put(
            rects.apply,
            egui::Button::new(egui::RichText::new("Apply").size(11.0)),
        );
        let cancel = ui.put(
            rects.cancel,
            egui::Button::new(egui::RichText::new("Cancel").size(11.0)),
        );

        let edited = d.changed() || a.changed() || dir.changed() || s.changed();

        // Apply: validate + journal ONE entry. Invalid text refuses with a
        // named field+reason and never touches the document or journal.
        if apply.clicked() {
            match validate_params(&depth, &angle, &direction, symmetric) {
                Ok(next) => {
                    let id = panel.selected.clone();
                    let kind_spec = kind.clone();
                    if let Some(document) = panel.document.as_mut() {
                        let before = doc_io::document_hash(document);
                        panel.history.apply(document, PARAM_TOKEN, None, |d| {
                            if let Some(f) = d.features.iter_mut().find(|f| f.id == id) {
                                f.parameters
                                    .insert(PARAM_DEPTH.to_string(), next.depth.into());
                                f.parameters
                                    .insert(PARAM_ANGLE.to_string(), next.angle.into());
                                f.parameters.insert(
                                    PARAM_DIRECTION.to_string(),
                                    next.direction.clone().into(),
                                );
                                f.parameters
                                    .insert(PARAM_SYMMETRIC.to_string(), next.symmetric.into());
                            }
                        });
                        let after = doc_io::document_hash(document);
                        panel.field_error = (String::new(), String::new());
                        panel.applied = next.clone();
                        panel.depth = format!("{}", next.depth);
                        panel.angle = format!("{}", next.angle);
                        panel.direction = next.direction.clone();
                        panel.symmetric = next.symmetric;
                        panel.preview = derive_preview(&ctx, &kind_spec, &next);
                        panel.last = Some(format!(
                            "applied {}: depth {} angle {} (hash {before} -> {after})",
                            kind_spec, next.depth, next.angle
                        ));
                    }
                }
                Err((field, reason)) => {
                    panel.field_error = (field.clone(), reason.clone());
                    panel.last = Some(format!("refused {field}: {reason}"));
                }
            }
            ctx.request_repaint();
        }

        // Cancel: discard the drafts, restore the committed set, leave the
        // document and journal untouched.
        if cancel.clicked() {
            panel.depth = format!("{}", panel.applied.depth);
            panel.angle = format!("{}", panel.applied.angle);
            panel.direction = panel.applied.direction.clone();
            panel.symmetric = panel.applied.symmetric;
            panel.field_error = (String::new(), String::new());
            panel.preview = derive_preview(&ctx, &kind, &panel.applied);
            panel.last = Some("cancelled parameter edit".to_string());
            ctx.request_repaint();
        }

        // Live preview: any valid edit to the editable amount re-derives the
        // derived solid; an invalid one is a named refusal and nothing else.
        if edited && !apply.clicked() && !cancel.clicked() {
            match validate_params(&depth, &angle, &direction, symmetric) {
                Ok(next) => {
                    panel.field_error = (String::new(), String::new());
                    panel.depth = depth;
                    panel.angle = angle;
                    panel.direction = direction;
                    panel.symmetric = symmetric;
                    panel.preview = derive_preview(&ctx, &kind, &next);
                }
                Err((field, reason)) => {
                    // Keep the drafts as typed (the field holds the text), the
                    // document is untouched; name the field + reason.
                    panel.depth = depth;
                    panel.angle = angle;
                    panel.direction = direction;
                    panel.symmetric = symmetric;
                    panel.field_error = (field.clone(), reason.clone());
                    panel.last = Some(format!("refused {field}: {reason}"));
                }
            }
        }

        // Preview + error readout.
        let p2 = oep_ui_theme::Palette::resolve(theme::mode_of(ui));
        if panel.preview.present {
            ui.painter().text(
                egui::pos2(rects.depth.min.x, rects.apply.max.y + 6.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "volume {} · {} faces · frame {}",
                    panel.preview.volume, panel.preview.faces, panel.preview.frame_hash
                ),
                egui::FontId::monospace(9.0),
                p2.text_muted,
            );
        } else {
            ui.painter().text(
                egui::pos2(rects.depth.min.x, rects.apply.max.y + 6.0),
                egui::Align2::LEFT_TOP,
                "no profile yet - draw a closed sketch",
                egui::FontId::monospace(9.0),
                p2.text_muted,
            );
        }
        let (field, reason) = &panel.field_error;
        if !field.is_empty() {
            ui.painter().text(
                egui::pos2(rects.depth.min.x, rects.apply.max.y + 20.0),
                egui::Align2::LEFT_TOP,
                format!("{field}: {reason}"),
                egui::FontId::monospace(9.0),
                panel_danger(ui),
            );
        }
    }

    write_param_panel(&ctx, &panel);
    publish_param_oracle(&ctx, &panel, &kind, editable, rects);
}

/// The danger token for the error readout line.
fn panel_danger(ui: &egui::Ui) -> egui::Color32 {
    oep_ui_theme::Palette::resolve(theme::mode_of(ui)).danger
}

/// Publish the C2 `String` oracle: selection, kind, drafts, committed set,
/// preview, journal, document hash, and the named refusal if any.
fn publish_param_oracle(
    ctx: &egui::Context,
    panel: &ParamPanel,
    kind: &str,
    editable: bool,
    _rects: ParamWidgetRects,
) {
    let doc_hash = panel
        .document
        .as_ref()
        .map(doc_io::document_hash)
        .unwrap_or(0);
    let journal = panel.history.len();
    let sel = if panel.selected.is_empty() {
        "-".to_string()
    } else {
        panel.selected.clone()
    };
    let mut out = String::new();
    out.push_str(&format!("sel={sel}\n"));
    out.push_str(&format!("kind={kind}\n"));
    out.push_str(&format!("editable={}\n", u8::from(editable)));
    out.push_str(&format!("depth={}\n", panel.depth));
    out.push_str(&format!("angle={}\n", panel.angle));
    out.push_str(&format!("direction={}\n", panel.direction));
    out.push_str(&format!("symmetric={}\n", u8::from(panel.symmetric)));
    out.push_str(&format!(
        "app_depth={:.4}\napp_angle={:.4}\napp_direction={}\napp_symmetric={}\n",
        panel.applied.depth,
        panel.applied.angle,
        panel.applied.direction,
        u8::from(panel.applied.symmetric)
    ));
    out.push_str(&format!("preview={}\n", u8::from(panel.preview.present)));
    out.push_str(&format!("volume={:.4}\n", panel.preview.volume));
    out.push_str(&format!("faces={}\n", panel.preview.faces));
    out.push_str(&format!("frame_hash={}\n", panel.preview.frame_hash));
    out.push_str(&format!(
        "error_field={}\n",
        if panel.field_error.0.is_empty() {
            "-"
        } else {
            &panel.field_error.0
        }
    ));
    out.push_str(&format!(
        "error={}\n",
        if panel.field_error.1.is_empty() {
            "-"
        } else {
            &panel.field_error.1
        }
    ));
    out.push_str(&format!("journal={journal}\n"));
    out.push_str(&format!("doc_hash={doc_hash}\n"));
    out.push_str(&format!("last={}\n", panel.last.as_deref().unwrap_or("-")));
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(PARAM_REPORT_KEY), out);
    });
}
