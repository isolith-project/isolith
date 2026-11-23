//! Kernel-feature dialogs (wave C, card UI-C6 UI-KERNEL-FEATURES).
//!
//! Fillet, chamfer, shell and boolean are DIALOGS over the oep-geo kernel
//! seam (the typed surface that fronts the oep-engine-occ OCCT binding). The
//! card's behaviour: with the kernel available the geometry is real, and
//! without it the app says the feature is unavailable instead of drawing fake
//! geometry.
//!
//! ## Availability is detected, never assumed
//!
//! [`probe_kernel`] opens a real `oep_geo` session at runtime and the panel
//! keeps that session (plus its committed `box-reference` fixture) only while
//! it exists. oep-geo's seam is typed on BOTH sides of the engine: with the
//! `kernel` feature off (the workspace default, no OCCT linked) `sys_new`
//! returns [`KernelError::EngineUnavailable`]; with it on, a real session
//! opens. The dialogs therefore branch on what detection actually found, not
//! on a compile-time guess:
//!
//! - **kernel available** -- fillet / chamfer / boolean run real B-Rep
//!   operations against the committed fixture (the corpus `box-reference`
//!   unit cube, corner (0,0,0)-(1,1,1), exactly as `oep-geo`'s own seam
//!   acceptance uses it) and report the seam's measured topology. Fillet of
//!   radius 0.2 on one cube edge yields face count 7 (the committed
//!   `kernel-seam-golden.json` number), asserted exactly by the acceptance.
//! - **kernel unavailable** -- every dialog says the feature is unavailable
//!   with an honest reason and publishes `geometry=0` / `faces=-`: no
//!   face count, volume or edge number is ever fabricated.
//! - **shell** -- there is NO shell/thicken operation in the seam today, so
//!   its dialog honestly reports "no kernel shell operation in the seam" even
//!   when a kernel session is open. This is the detected-unavailable state,
//!   never fake geometry.
//!
//! The panel publishes its state as a plain `String` oracle every frame under
//! [`KERNEL_REPORT_KEY`] (cross-`#[path]`-copy safe) and keeps its typed state
//! in the egui context store under [`KERNEL_STATE_KEY`], the same seam-split
//! the B7/C2/C3 panels use: only this pane touches the typed value, tests read
//! the `String` oracle.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_geo::{
    make_box, solid_boolean, solid_chamfer, solid_edges, solid_fillet, sys_new, BooleanOp, EdgeRef,
    Solid, SysHandle, Tolerance,
};

/// Context-data key under which this panel's typed state lives (this module's
/// copy only; tests read the `String` oracle instead).
const KERNEL_STATE_KEY: &str = "ui_c6_kernel_state";
/// Context-data key under which the `String` oracle is published every frame.
pub const KERNEL_REPORT_KEY: &str = "ui_c6_kernel_report";

/// The committed fixture: the corpus `box-reference` unit cube, exactly as the
/// oep-geo seam acceptance builds it.
const FIXTURE_MIN: (f64, f64, f64) = (0.0, 0.0, 0.0);
const FIXTURE_MAX: (f64, f64, f64) = (1.0, 1.0, 1.0);
/// The boolean operand B: the half-overlapping second unit box the seam golden
/// `union`/`difference`/`intersection` entries use (overlap 0.5 in x).
const FIXTURE_B_MIN: (f64, f64, f64) = (0.5, 0.0, 0.0);
const FIXTURE_B_MAX: (f64, f64, f64) = (1.5, 1.0, 1.0);
/// Default fillet radius / chamfer distance (the seam golden fixture amount).
const DEFAULT_AMOUNT: f64 = 0.2;
/// Caller-owned geometric tolerance (the seam default).
const KERNEL_TOLERANCE: f64 = 1e-6;

/// The four kernel-feature dialogs this panel renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelDialog {
    /// Fillet: rounds a selected fixture edge by a radius.
    Fillet,
    /// Chamfer: bevels a selected fixture edge by a symmetric distance.
    Chamfer,
    /// Boolean: unions / differences / intersects the two fixture solids.
    Boolean,
    /// Shell: hollows a solid - honestly not in the kernel seam today.
    Shell,
}

impl KernelDialog {
    /// Every dialog, in render order (a test iterates them).
    pub const ALL: [KernelDialog; 4] = [
        KernelDialog::Fillet,
        KernelDialog::Chamfer,
        KernelDialog::Boolean,
        KernelDialog::Shell,
    ];

    /// The dialog's on-screen label.
    pub fn label(self) -> &'static str {
        match self {
            KernelDialog::Fillet => "Fillet",
            KernelDialog::Chamfer => "Chamfer",
            KernelDialog::Boolean => "Boolean",
            KernelDialog::Shell => "Shell",
        }
    }

    /// The name of the parameter field (empty for shell, which takes none).
    fn field_name(self) -> &'static str {
        match self {
            KernelDialog::Fillet => "radius",
            KernelDialog::Chamfer => "distance",
            KernelDialog::Boolean => "op",
            KernelDialog::Shell => "",
        }
    }
}

/// The three accepted boolean operation tokens (the seam's [`BooleanOp`]s).
pub const BOOLEAN_OP_TOKENS: [&str; 3] = ["union", "difference", "intersection"];

/// The measured result of one dialog run: only ever filled with real numbers
/// straight from the kernel seam. `geometry=true` iff a kernel operation ran
/// and produced a measured solid.
#[derive(Debug, Clone, PartialEq)]
pub struct KernelResult {
    /// Whether real geometry was produced by the last run.
    pub geometry: bool,
    /// Measured faces (real seam output; `None` when no geometry ran).
    pub faces: Option<u64>,
    /// Measured volume (real seam output).
    pub volume: Option<f64>,
    /// Measured edges (real seam output).
    pub edges: Option<u64>,
    /// Measured vertices (real seam output).
    pub verts: Option<u64>,
    /// One-line outcome (also shown on screen).
    pub message: String,
}

impl KernelResult {
    /// The honest no-geometry result: `message` names why nothing ran.
    pub fn no_geometry(message: impl Into<String>) -> Self {
        Self {
            geometry: false,
            faces: None,
            volume: None,
            edges: None,
            verts: None,
            message: message.into(),
        }
    }

    /// Empty default (before any run).
    fn none() -> Self {
        Self::no_geometry(String::new())
    }
}

/// The kernel-feature panel's persistent state (this module's copy). The panel
/// owns the detection result and, when a kernel session is open, that session
/// and its fixture solids -- so a run reuses the committed geometry instead of
/// rebuilding it every frame.
#[derive(Debug, Clone)]
pub struct KernelPanel {
    /// True once availability has been detected (never re-probed mid-run).
    probed: bool,
    /// Whether a real kernel session is open.
    available: bool,
    /// Honest reason when `available` is false (empty when true).
    reason: String,
    /// The open kernel session (present exactly when `available`).
    sys: Option<SysHandle>,
    /// Fixture solid A (the box-reference unit cube).
    solid_a: Option<Solid>,
    /// Fixture solid B (the half-overlapping boolean operand).
    solid_b: Option<Solid>,
    /// Edge references of fixture A (the seam's own occurrence list: 24 rows
    /// for a cube) -- what fillet / chamfer name their target.
    edges: Vec<EdgeRef>,
    /// Draft text of the active dialog's parameter field.
    radius: String,
    /// Draft text of the chamfer distance field.
    distance: String,
    /// Draft text of the boolean op field.
    op: String,
    /// Most recent dialog run outcome (real seam numbers when one ran).
    result: KernelResult,
    /// Named refusal on the last validated input: (field, reason).
    field_error: (String, String),
}

impl Default for KernelPanel {
    fn default() -> Self {
        Self {
            probed: false,
            available: false,
            reason: String::new(),
            sys: None,
            solid_a: None,
            solid_b: None,
            edges: Vec::new(),
            radius: String::new(),
            distance: String::new(),
            op: "union".to_string(),
            result: KernelResult::none(),
            field_error: (String::new(), String::new()),
        }
    }
}

fn read_panel(ctx: &egui::Context) -> KernelPanel {
    ctx.data(|d| d.get_temp(egui::Id::unique(KERNEL_STATE_KEY)))
        .unwrap_or_default()
}

fn write_panel(ctx: &egui::Context, panel: &KernelPanel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(KERNEL_STATE_KEY), panel.clone()));
}

/// The one tolerance used for every seam call.
fn tol() -> Tolerance {
    Tolerance::new(KERNEL_TOLERANCE).expect("valid kernel tolerance")
}

/// DETECT kernel availability at runtime and, when a real session opened,
/// build the committed box-reference fixture inside it. Runs once per panel
/// lifetime: the result is cached in [`KernelPanel::probed`]. Never assumes
/// the engine -- oep-geo's typed seam reports `EngineUnavailable` on stub
/// builds and this panel turns that into the honest unavailable state.
fn probe_kernel(panel: &mut KernelPanel) {
    panel.probed = true;
    match sys_new() {
        Ok(sys) => match make_box(
            &sys,
            FIXTURE_MIN.0,
            FIXTURE_MIN.1,
            FIXTURE_MIN.2,
            FIXTURE_MAX.0,
            FIXTURE_MAX.1,
            FIXTURE_MAX.2,
            tol(),
        ) {
            Ok(solid_a) => {
                let edges = solid_edges(&sys, &solid_a, tol()).unwrap_or_default();
                let solid_b = make_box(
                    &sys,
                    FIXTURE_B_MIN.0,
                    FIXTURE_B_MIN.1,
                    FIXTURE_B_MIN.2,
                    FIXTURE_B_MAX.0,
                    FIXTURE_B_MAX.1,
                    FIXTURE_B_MAX.2,
                    tol(),
                )
                .ok();
                panel.sys = Some(sys);
                panel.solid_a = Some(solid_a);
                panel.solid_b = solid_b;
                panel.edges = edges;
                panel.available = true;
                panel.reason.clear();
            }
            Err(e) => {
                panel.available = false;
                panel.reason = format!("fixture build failed: {e}");
            }
        },
        Err(e) => {
            panel.available = false;
            panel.reason = e.to_string();
        }
    }
}

/// Parse a positive finite amount (radius/distance) from raw text. `None`
/// means the field was EMPTY (default applies); an invalid value is a named
/// refusal (field, reason), never a silent fallback.
fn parse_amount(text: &str, field: &str, default: f64) -> Result<f64, (String, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() && v > 0.0 => Ok(v),
        Ok(v) => Err((
            field.to_string(),
            format!("must be a positive number, got {v}"),
        )),
        Err(_) => Err((
            field.to_string(),
            format!("must be a number, got '{trimmed}'"),
        )),
    }
}

/// Validate and run the FILET dialog: round fixture edge 0 of the unit cube by
/// `radius` and report the new solid's real measured topology.
fn run_fillet(panel: &mut KernelPanel) {
    let radius = match parse_amount(&panel.radius, "radius", DEFAULT_AMOUNT) {
        Ok(v) => v,
        Err(e) => {
            panel.field_error = e;
            return;
        }
    };
    let Some(sys) = panel.sys else {
        return;
    };
    let first_edge = panel.edges.first().copied();
    let subject = panel.solid_a.as_ref();
    let (Some(first_edge), Some(_subject)) = (first_edge, subject) else {
        panel.result = KernelResult::no_geometry("no fixture edge to fillet");
        return;
    };
    match solid_fillet(&sys, &[first_edge], radius, tol()) {
        Ok(solid) => {
            let m = solid.measures;
            panel.result = KernelResult {
                geometry: true,
                faces: Some(m.face_count),
                volume: Some(m.volume),
                edges: Some(m.edge_count),
                verts: Some(m.vertex_count),
                message: format!(
                    "fillet radius {radius} on fixture edge -> {} faces",
                    m.face_count
                ),
            };
        }
        Err(e) => {
            panel.result = KernelResult::no_geometry(format!("fillet failed: {e}"));
        }
    }
    panel.field_error = (String::new(), String::new());
}

/// Validate and run the CHAMFER dialog: bevel fixture edge 0 of the unit cube
/// by symmetric `distance` and report the real measured topology.
fn run_chamfer(panel: &mut KernelPanel) {
    let distance = match parse_amount(&panel.distance, "distance", DEFAULT_AMOUNT) {
        Ok(v) => v,
        Err(e) => {
            panel.field_error = e;
            return;
        }
    };
    let Some(sys) = panel.sys else {
        return;
    };
    let Some(first_edge) = panel.edges.first().copied() else {
        panel.result = KernelResult::no_geometry("no fixture edge to chamfer");
        return;
    };
    match solid_chamfer(&sys, &[first_edge], distance, tol()) {
        Ok(solid) => {
            let m = solid.measures;
            panel.result = KernelResult {
                geometry: true,
                faces: Some(m.face_count),
                volume: Some(m.volume),
                edges: Some(m.edge_count),
                verts: Some(m.vertex_count),
                message: format!(
                    "chamfer distance {distance} on fixture edge -> {} faces",
                    m.face_count
                ),
            };
        }
        Err(e) => {
            panel.result = KernelResult::no_geometry(format!("chamfer failed: {e}"));
        }
    }
    panel.field_error = (String::new(), String::new());
}

/// Parse a boolean op token into the seam's [`BooleanOp`]; a named refusal on
/// anything else.
fn parse_op(text: &str) -> Result<BooleanOp, (String, String)> {
    let trimmed = text.trim();
    let op = match trimmed {
        "union" => BooleanOp::Union,
        "difference" => BooleanOp::Difference,
        "intersection" => BooleanOp::Intersection,
        _ => {
            return Err((
                "op".to_string(),
                format!(
                    "must be one of {}, got '{trimmed}'",
                    BOOLEAN_OP_TOKENS.join(" | ")
                ),
            ));
        }
    };
    Ok(op)
}

/// Validate and run the BOOLEAN dialog: fold both fixture solids with the
/// chosen operation and report the real measured topology.
fn run_boolean(panel: &mut KernelPanel) {
    let op = match parse_op(&panel.op) {
        Ok(op) => op,
        Err(e) => {
            panel.field_error = e;
            return;
        }
    };
    let Some(sys) = panel.sys else {
        return;
    };
    let operands = match (panel.solid_a.as_ref(), panel.solid_b.as_ref()) {
        (Some(a), Some(b)) => [a.clone(), b.clone()],
        _ => {
            panel.result = KernelResult::no_geometry("no fixture solids to combine");
            return;
        }
    };
    match solid_boolean(op, &sys, &operands, tol()) {
        Ok(solid) => {
            let m = solid.measures;
            panel.result = KernelResult {
                geometry: true,
                faces: Some(m.face_count),
                volume: Some(m.volume),
                edges: Some(m.edge_count),
                verts: Some(m.vertex_count),
                message: format!(
                    "boolean {} -> {} faces (volume {:.4})",
                    match op {
                        BooleanOp::Union => "union",
                        BooleanOp::Difference => "difference",
                        BooleanOp::Intersection => "intersection",
                    },
                    m.face_count,
                    m.volume
                ),
            };
        }
        Err(e) => {
            panel.result = KernelResult::no_geometry(format!("boolean failed: {e}"));
        }
    }
    panel.field_error = (String::new(), String::new());
}

/// Run the SHELL dialog. The seam has NO shell/thicken operation today, so
/// this dialog is the honest unavailable state: it reports the reason and
/// never fabricates geometry (even when a kernel session is open).
fn run_shell(panel: &mut KernelPanel) {
    panel.field_error = (String::new(), String::new());
    panel.result = KernelResult::no_geometry(
        "shell unavailable: no shell operation in the kernel seam (honest no-geometry state)",
    );
}

/// Dispatch one dialog's "Run" button. When the kernel was detected absent,
/// every dialog reports the same honest unavailable reason and no geometry.
fn run_dialog(panel: &mut KernelPanel, dialog: KernelDialog) {
    if dialog == KernelDialog::Shell {
        run_shell(panel);
        return;
    }
    if !panel.available {
        // Honest unavailable: one reason for every dialog when the kernel was
        // detected absent; nothing here ever fabricates a face count.
        let detail = if panel.reason.is_empty() {
            String::new()
        } else {
            format!(": {}", panel.reason)
        };
        panel.result = KernelResult::no_geometry(format!("kernel engine unavailable{detail}"));
        panel.field_error = (String::new(), String::new());
        return;
    }
    match dialog {
        KernelDialog::Fillet => run_fillet(panel),
        KernelDialog::Chamfer => run_chamfer(panel),
        KernelDialog::Boolean => run_boolean(panel),
        KernelDialog::Shell => unreachable!("shell handled above"),
    }
}

/// Deterministic layout constants for the four dialog rows (anchored in the
/// panel region the same way C2/C3 anchor theirs).
const DIALOG_TOP: f32 = 90.0;
const DIALOG_STEP: f32 = 36.0;
const DIALOG_H: f32 = 26.0;
const LABEL_X: f32 = 8.0;
const LABEL_W: f32 = 118.0;
const FIELD_X: f32 = 132.0;
const FIELD_W: f32 = 132.0;
const RUN_X: f32 = 272.0;
const RUN_W: f32 = 96.0;

/// The interactive rects of one dialog row in the given panel region: the
/// parameter text input (absent for shell) and the Run button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogRects {
    /// The parameter text input (`None` for shell, which takes none).
    pub field: Option<egui::Rect>,
    /// The "Run" button.
    pub run: egui::Rect,
}

/// Compute the deterministic widget rects of `dialog`'s row inside `region`.
pub fn dialog_rects(region: egui::Rect, dialog: KernelDialog) -> DialogRects {
    let row = match dialog {
        KernelDialog::Fillet => 0,
        KernelDialog::Chamfer => 1,
        KernelDialog::Boolean => 2,
        KernelDialog::Shell => 3,
    };
    let top = region.min.y + DIALOG_TOP + DIALOG_STEP * row as f32;
    let run = egui::Rect::from_min_size(
        egui::pos2(region.min.x + RUN_X, top),
        egui::vec2(RUN_W, DIALOG_H),
    );
    let field = (dialog != KernelDialog::Shell).then(|| {
        egui::Rect::from_min_size(
            egui::pos2(region.min.x + FIELD_X, top),
            egui::vec2(FIELD_W, DIALOG_H),
        )
    });
    DialogRects { field, run }
}

/// The on-screen lines of the honest availability + result readout (painted
/// below the dialogs at fixed rects; the oracle carries the same numbers).
fn draw_readout(ui: &mut egui::Ui, region: egui::Rect, panel: &KernelPanel) {
    let mode = theme::mode_of(ui);
    let p = oep_ui_theme::Palette::resolve(mode);
    let y_base = region.min.y + DIALOG_TOP + DIALOG_STEP * 4.0 + 10.0;
    let x = region.min.x + LABEL_X;

    let availability = if panel.available {
        "kernel: available (real geometry)"
    } else {
        "kernel: unavailable"
    };
    let availability_color = if panel.available {
        p.text_primary
    } else {
        p.danger
    };
    ui.painter().text(
        egui::pos2(x, y_base),
        egui::Align2::LEFT_TOP,
        availability,
        egui::FontId::proportional(10.0),
        availability_color,
    );
    if !panel.available && !panel.reason.is_empty() {
        ui.painter().text(
            egui::pos2(x, y_base + 16.0),
            egui::Align2::LEFT_TOP,
            format!("reason: {}", panel.reason),
            egui::FontId::proportional(9.0),
            p.text_muted,
        );
    }

    let mut line = 2;
    if panel.available {
        let edges = panel.edges.len();
        ui.painter().text(
            egui::pos2(x, y_base + 16.0 * line as f32),
            egui::Align2::LEFT_TOP,
            format!("fixture: box-reference unit cube, {edges} edge occurrences"),
            egui::FontId::proportional(9.0),
            p.text_muted,
        );
        line += 1;
    }
    if !panel.field_error.1.is_empty() {
        ui.painter().text(
            egui::pos2(x, y_base + 16.0 * line as f32),
            egui::Align2::LEFT_TOP,
            format!("error {}{}", panel.field_error.0, panel.field_error.1),
            egui::FontId::proportional(9.0),
            p.danger,
        );
        line += 1;
    }
    if !panel.result.message.is_empty() {
        ui.painter().text(
            egui::pos2(x, y_base + 16.0 * line as f32),
            egui::Align2::LEFT_TOP,
            &panel.result.message,
            egui::FontId::proportional(9.0),
            p.text_primary,
        );
        line += 1;
    }
    if panel.result.geometry {
        if let Some(f) = panel.result.faces {
            ui.painter().text(
                egui::pos2(x, y_base + 16.0 * line as f32),
                egui::Align2::LEFT_TOP,
                format!("result: {f} faces"),
                egui::FontId::proportional(10.0),
                p.text_primary,
            );
        }
    }
}

/// Build the `String` oracle lines for the current panel state.
fn oracle_lines(panel: &KernelPanel) -> String {
    let mut out = String::new();
    out.push_str(&format!("kernel={}\n", if panel.available { 1 } else { 0 }));
    out.push_str(&format!("reason={}\n", panel.reason));
    out.push_str(&format!(
        "edge_count={}\n",
        if panel.available {
            panel.edges.len()
        } else {
            0
        }
    ));
    out.push_str(&format!(
        "geometry={}\n",
        if panel.result.geometry { 1 } else { 0 }
    ));
    let num = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or_else(|| "-".to_string());
    out.push_str(&format!(
        "faces={}\nvolume={}\nedges={}\nverts={}\n",
        num(panel.result.faces),
        panel
            .result
            .volume
            .map(|v| format!("{v}"))
            .unwrap_or_else(|| "-".to_string()),
        num(panel.result.edges),
        num(panel.result.verts),
    ));
    out.push_str(&format!("last={}\n", panel.result.message));
    out.push_str(&format!("error_field={}\n", panel.field_error.0));
    out.push_str(&format!("error={}\n", panel.field_error.1));
    let runs = |d: KernelDialog| {
        let matches = match d {
            KernelDialog::Fillet => panel.result.message.starts_with("fillet"),
            KernelDialog::Chamfer => panel.result.message.starts_with("chamfer"),
            KernelDialog::Boolean => panel.result.message.starts_with("boolean"),
            KernelDialog::Shell => false,
        };
        if matches {
            1
        } else {
            0
        }
    };
    out.push_str(&format!("run_fillet={}\n", runs(KernelDialog::Fillet)));
    out.push_str(&format!("run_chamfer={}\n", runs(KernelDialog::Chamfer)));
    out.push_str(&format!("run_boolean={}\n", runs(KernelDialog::Boolean)));
    out.push_str(&format!(
        "run_shell={}\n",
        if panel.result.message.starts_with("shell unavailable") {
            1
        } else {
            0
        }
    ));
    out
}

fn write_oracle(ctx: &egui::Context, panel: &KernelPanel) {
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(KERNEL_REPORT_KEY), oracle_lines(panel));
    });
}

/// One frame of the kernel-features panel: detect availability (once), render
/// the four dialogs at their deterministic rects, dispatch Run clicks, and
/// publish the oracle.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    let mode = theme::mode_of(ui);
    ui.label(theme::heading(mode, "Kernel features").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    let ctx = ui.ctx().clone();
    let region = ui.max_rect();

    let mut panel = read_panel(&ctx);
    if !panel.probed {
        probe_kernel(&mut panel);
    }

    // Render each dialog row (labels, the parameter input, the Run button).
    for dialog in KernelDialog::ALL {
        let rects = dialog_rects(region, dialog);
        let x = region.min.x + LABEL_X;
        let top = rects.run.min.y + (DIALOG_H - 12.0) / 2.0;
        let label = dialog.label().to_string();
        let muted = oep_ui_theme::Palette::resolve(mode).text_muted;
        ui.painter().text(
            egui::pos2(x, top),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(10.0),
            muted,
        );

        // The parameter draft lives in the panel so typing persists across
        // frames; the Run button dispatches to the dialog's real runner.
        let mut draft = match dialog {
            KernelDialog::Fillet => panel.radius.clone(),
            KernelDialog::Chamfer => panel.distance.clone(),
            KernelDialog::Boolean => panel.op.clone(),
            KernelDialog::Shell => String::new(),
        };
        if let Some(field_rect) = rects.field {
            let hint = match dialog {
                KernelDialog::Fillet => "0.2",
                KernelDialog::Chamfer => "0.2",
                KernelDialog::Boolean => "union",
                KernelDialog::Shell => "",
            };
            let response = ui.put(
                field_rect,
                egui::TextEdit::singleline(&mut draft)
                    .hint_text(hint)
                    .font(egui::TextStyle::Monospace),
            );
            let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            match dialog {
                KernelDialog::Fillet => panel.radius = draft,
                KernelDialog::Chamfer => panel.distance = draft,
                KernelDialog::Boolean => panel.op = draft,
                KernelDialog::Shell => {}
            }
            let run_clicked = ui.put(
                rects.run,
                egui::Button::new(egui::RichText::new("Run").size(11.0)),
            );
            if run_clicked.clicked() || submitted {
                run_dialog(&mut panel, dialog);
            }
        } else {
            // Shell: no parameter field, just its (always honest) Run button.
            let run_clicked = ui.put(
                rects.run,
                egui::Button::new(egui::RichText::new("Run").size(11.0)),
            );
            if run_clicked.clicked() {
                run_dialog(&mut panel, dialog);
            }
        }
    }

    draw_readout(ui, region, &panel);
    write_panel(&ctx, &panel);
    write_oracle(&ctx, &panel);
}
