//! UI-B8 UI-FEATURE-FROM-SKETCH: extrude/revolve dialog with preview (wave B).
//!
//! Finishing a sketch into a solid goes through this dialog: opening it builds
//! a **document clone marked preview**, the viewport frame switches to the
//! derived solid (the preview frame), Cancel leaves the document byte-identical
//! (asserted by hash equality), and Apply commits the feature into the command
//! journal. The STEP export produced here is the CLI's own
//! `EditorSession::export_step` lane, so the UI bytes equal the CLI's for the
//! same profile and amount (the export-hash honesty check).
//!
//! Model (following D-UI-4 and the B4/B7 drive-lane precedent):
//!
//! - The committed sketch geometry is read from the sketch `String` oracle
//!   (the same `point=` / `lineseg=` lines B7's dimension engine reads), so
//!   this panel never depends on a typed sketch document across `#[path]`
//!   copies.
//! - The **preview is a document clone marked preview**: opening the dialog
//!   builds a fresh [`oep_shell::editor::EditorSession`] from the committed
//!   geometry, solves it and sweeps a solid ([`FeatureKind::Extrude`] or
//!   [`FeatureKind::Revolve`]). The clone lives only in the open dialog's
//!   [`FeatureDialog::preview`]; nothing is written to the sketch document and
//!   nothing reaches the command journal until Apply.
//! - **Cancel** drops the clone. The feature document digest
//!   ([`document_digest`], FNV-1a-64 over the committed geometry lines plus
//!   the applied-feature records) is byte-identical before and after.
//! - **Apply** journals exactly one entry under the registry-validated token
//!   (`feat:extrude` / `feat:revolve`, the CLI's own editor feature names)
//!   with the before/after digests, records the feature in [`FeatureState`],
//!   and closes the dialog; the solid stays visible in the viewport.
//! - **Export** writes the applied profile/amount through
//!   `EditorSession::export_step` - the exact method `oep-workbench export:step`
//!   dispatches - so UI and CLI produce the same bytes for the same model.
//!
//! All typed panel state is stored under this module's context-data keys and
//! read back only here (the usual cross-`#[path]` seam): the viewport and the
//! test harness read the `String` oracle (`ui_b8_feature_report`) and the
//! published viewport scalar snapshot instead, exactly like the B6/B7 lanes.

use eframe::egui;

use oep_ui_theme::{Mode, Palette};

// The UI-B2 command journal (D-UI-4), included verbatim the way sketch.rs and
// doc_io.rs include it. The feature lane owns its own journal instance;
// preview never writes it, Apply pushes exactly one entry. Allowed dead-code
// here for the same reason the sibling `#[path]` journal copies are: the bin
// build only drives push/len so far and the undo/redo affordances are wired by
// the later editing cards through the same journal.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../history.rs"]
mod history;

/// Context-data key under which the typed feature panel state lives.
const FEATURE_STATE_KEY: &str = "ui_b8_feature_state";
/// Context-data key under which the feature journal lives (typed, this copy).
const FEATURE_JOURNAL_KEY: &str = "ui_b8_feature_journal";
/// Context-data key under which the `String` feature oracle is published.
pub const FEATURE_REPORT_KEY: &str = "ui_b8_feature_report";
/// Context-data key (a plain `String`, so cross-`#[path]`-copy safe) holding
/// the STEP export target path. A test sets it to aim the UI export.
pub const EXPORT_PATH_KEY: &str = "ui_b8_export_path";
/// The sketch panel's `String` report oracle (must match
/// `sketch::SKETCH_REPORT_KEY`; a local literal avoids depending on the parent
/// module across `#[path]` compilations).
const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";

/// The preview frame is rendered by this module with the same CPU raster the
/// backend uses; the shared viewport snapshot the app publishes carries its
/// hash, so no second snapshot key is invented here.
///
/// The plain key that opens the finish-sketch dialog while sketching a closed
/// loop. `F` is free in the B1 table (Frame Model needs Ctrl, the letter tools
/// own P/L/N/R/C) and in every shell region.
pub const FINISH_KEY: egui::Key = egui::Key::F;

/// Default depth of a fresh extrude dialog (mm).
pub const DEFAULT_AMOUNT: f64 = 10.0;
/// Default basename of the UI STEP export file.
pub const DEFAULT_EXPORT_FILE: &str = "isolith-ui-export.step";

/// The two solid features this dialog offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    /// Extrude the closed profile by a depth along +Z.
    Extrude,
    /// Revolve the closed profile about Z by an angle in radians.
    Revolve,
}

impl FeatureKind {
    /// Stable machine id used in the oracle / report.
    pub fn id(self) -> &'static str {
        match self {
            FeatureKind::Extrude => "extrude",
            FeatureKind::Revolve => "revolve",
        }
    }

    /// Human label shown on the dialog buttons.
    pub fn label(self) -> &'static str {
        match self {
            FeatureKind::Extrude => "Extrude",
            FeatureKind::Revolve => "Revolve",
        }
    }

    /// Amount unit shown next to the value readout.
    pub fn unit(self) -> &'static str {
        match self {
            FeatureKind::Extrude => "mm",
            FeatureKind::Revolve => "rad",
        }
    }

    /// The registry-validated command token Apply journals this kind under
    /// (the CLI's own `feat:extrude` / `feat:revolve` editor commands).
    pub fn token(self) -> &'static str {
        match self {
            FeatureKind::Extrude => "feat:extrude",
            FeatureKind::Revolve => "feat:revolve",
        }
    }
}

/// A named, typed failure of the feature dialog. Never a panic, never
/// fabricated geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum FeatureError {
    /// No closed loop to finish (fewer than three points, or every point is
    /// not exactly degree 2).
    NotAFinishedSketch(String),
    /// The editor drive-lane (solve or sweep) refused the sketch.
    Drive(String),
}

impl std::fmt::Display for FeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureError::NotAFinishedSketch(msg) => write!(f, "sketch is not finished: {msg}"),
            FeatureError::Drive(msg) => write!(f, "could not derive the solid: {msg}"),
        }
    }
}

impl std::error::Error for FeatureError {}

/// Ordered committed sketch geometry parsed from the oracle: plane `(x, y)`
/// points (ids `p0..` by position) and line segments as point ids.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommittedSketch {
    /// Ordered plane points.
    pub points: Vec<(f64, f64)>,
    /// Line segments `(start_point_id, end_point_id)`.
    pub lines: Vec<(String, String)>,
}

/// Parse the committed sketch geometry from the sketch oracle (the same
/// `point=` / `lineseg=` lines the dimension engine reads). `None` when the
/// oracle carries no committed geometry.
pub fn committed_geometry(report: &str) -> Option<CommittedSketch> {
    let mut sk = CommittedSketch::default();
    let mut saw_point = false;
    for line in report.lines() {
        if let Some(rest) = line.strip_prefix("point=") {
            // format: <x>,<y>,<id>  (id has no commas, split from the right)
            let (coords, _id) = rest.rsplit_once(',')?;
            let mut c = coords.split(',');
            let x = c.next()?.parse::<f64>().ok()?;
            let y = c.next()?.parse::<f64>().ok()?;
            sk.points.push((x, y));
            saw_point = true;
        } else if let Some(rest) = line.strip_prefix("lineseg=") {
            // format: <id>:<start>-><end>
            let (_id, seg) = rest.split_once(':')?;
            let mut ends = seg.split("->");
            let start = ends.next()?.to_string();
            let end = ends.next()?.to_string();
            sk.lines.push((start, end));
        }
    }
    if saw_point && !sk.points.is_empty() && !sk.lines.is_empty() {
        Some(sk)
    } else {
        None
    }
}

/// True when every committed point is an endpoint of exactly two lines (a
/// closed single loop - what a solid feature can be finished from).
pub fn is_closed_loop(sk: &CommittedSketch) -> bool {
    if sk.points.len() < 3 || sk.lines.is_empty() {
        return false;
    }
    let deg = |id: &str| -> usize {
        let n: Option<usize> = id.strip_prefix('p').and_then(|s| s.parse().ok());
        let n = match n {
            Some(n) if n < sk.points.len() => n,
            _ => return usize::MAX,
        };
        let mut d = 0usize;
        for (a, b) in &sk.lines {
            for end in [a, b] {
                let en: usize = end
                    .strip_prefix('p')
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(usize::MAX);
                if en == n {
                    d += 1;
                }
            }
        }
        d
    };
    (0..sk.points.len()).all(|i| deg(&format!("p{i}")) == 2)
}

/// FNV-1a-64 over the feature document's canonical text: the committed sketch
/// geometry lines plus one record line per applied feature (same constants as
/// the engine's frame hash and `SketchDoc::hash`). Byte-identical while the
/// sketch and the applied set are unchanged (Cancel must preserve it); changes
/// when geometry is committed or a feature is applied.
pub fn document_digest(sk: &CommittedSketch, applied: &[AppliedFeature]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut feed = |s: &str| {
        for &b in s.as_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
    };
    for (i, (x, y)) in sk.points.iter().enumerate() {
        feed(&format!("point=p{i}:{x:.3}:{y:.3}\n"));
    }
    for l in &sk.lines {
        feed(&format!("line={}:{}\n", l.0, l.1));
    }
    for f in applied {
        feed(&f.record_line());
    }
    h
}

/// One committed (applied) feature.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedFeature {
    /// Which solid the sketch was finished into.
    pub kind: FeatureKind,
    /// The amount (extrude depth in mm, or revolve angle in radians).
    pub amount: f64,
    /// The derived solid volume (the document's own mass property).
    pub volume: f64,
    /// Number of mesh faces.
    pub faces: usize,
}

impl AppliedFeature {
    /// One canonical record line the document digest folds in.
    fn record_line(&self) -> String {
        format!(
            "feat:{} amount={} volume={}\n",
            self.kind.id(),
            self.amount,
            self.volume
        )
    }
}

/// The live preview: a document clone marked preview. The [`oep_shell::editor::EditorSession`]
/// here is derived from the committed sketch (a copy of the geometry), solved
/// and swept; it never touches the committed document and its work never
/// enters the journal until Apply promotes it into an [`AppliedFeature`].
#[derive(Debug, Clone)]
pub struct FeaturePreview {
    /// The derived session (the document clone).
    pub session: oep_shell::editor::EditorSession,
    /// The mark that makes this a preview clone.
    pub marked_preview: bool,
    /// The derived solid volume, from the sweep's `FeatureReport`.
    pub volume: f64,
    /// Mesh face count, from the sweep's `FeatureReport`.
    pub faces: usize,
}

/// The open finish-sketch dialog.
#[derive(Debug, Clone)]
pub struct FeatureDialog {
    /// The selected kind (the dialog offers both).
    pub kind: FeatureKind,
    /// The typed amount text (the editable field).
    pub draft: String,
    /// The parsed, current amount (re-derived on any valid edit).
    pub amount: f64,
    /// The live preview clone, rebuilt whenever kind or amount changes.
    pub preview: Option<FeaturePreview>,
    /// The last honest named refusal, if any (never a fabricated success).
    pub error: Option<FeatureError>,
}

/// The feature panel state, stored in the egui context data store (typed, this
/// copy only; the oracle is the cross-copy surface).
#[derive(Debug, Clone, Default)]
pub struct FeatureState {
    /// The open dialog (and its live preview), or `None`.
    pub dialog: Option<FeatureDialog>,
    /// The committed feature set (grows by exactly one on Apply).
    pub applied: Vec<AppliedFeature>,
    /// One-line description of the last action (oracle readout).
    pub last: Option<String>,
}

impl Default for FeatureDialog {
    fn default() -> Self {
        Self {
            kind: FeatureKind::Extrude,
            draft: format!("{DEFAULT_AMOUNT}"),
            amount: DEFAULT_AMOUNT,
            preview: None,
            error: None,
        }
    }
}

fn read_state(ctx: &egui::Context) -> FeatureState {
    ctx.data(|d| d.get_temp(egui::Id::unique(FEATURE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, state: &FeatureState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(FEATURE_STATE_KEY), state.clone()));
}

fn sketch_report(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// Whether the sketch oracle reports an active sketch plane.
fn in_sketch_mode(ctx: &egui::Context) -> bool {
    sketch_report(ctx).lines().any(|l| l == "mode=sketch")
}

/// The STEP export target path from the `String` store, or the deterministic
/// temp-dir default (whose basename matches the CLI's export file so UI and
/// CLI writes share the embedded STEP file name and therefore bytes).
pub fn current_export_path(ctx: &egui::Context) -> std::path::PathBuf {
    let over = ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(EXPORT_PATH_KEY)))
        .unwrap_or_default();
    if over.is_empty() {
        std::env::temp_dir().join(DEFAULT_EXPORT_FILE)
    } else {
        std::path::PathBuf::from(over)
    }
}

fn feature_journal(ctx: &egui::Context) -> history::Journal {
    ctx.data(|d| d.get_temp(egui::Id::unique(FEATURE_JOURNAL_KEY)))
        .unwrap_or_default()
}

fn write_feature_journal(ctx: &egui::Context, j: &history::Journal) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(FEATURE_JOURNAL_KEY), j.clone()));
}

/// Build the `String` oracle lines: dialog state, the preview clone (marked),
/// the derived solid's own volume/faces, the applied set, the feature document
/// digest and the journal count.
fn oracle_lines(ctx: &egui::Context, state: &FeatureState) -> String {
    let sk = committed_geometry(&sketch_report(ctx)).unwrap_or_default();
    let app = &state.applied;
    let mut out = String::new();
    match &state.dialog {
        None => {
            out.push_str("dialog=0\nkind=-\namount=-\n");
        }
        Some(d) => {
            out.push_str(&format!("dialog=1\nkind={}\n", d.kind.id()));
            out.push_str(&format!("amount={:.4}\n", d.amount));
        }
    }
    let preview = state.dialog.as_ref().and_then(|d| d.preview.as_ref());
    match preview {
        None => {
            out.push_str("preview=0\npreview_marked=0\nvolume=-\nfaces=-\n");
        }
        Some(p) => {
            out.push_str(&format!(
                "preview=1\npreview_marked={}\n",
                u8::from(p.marked_preview)
            ));
            out.push_str(&format!("volume={:.4}\n", p.volume));
            out.push_str(&format!("faces={}\n", p.faces));
        }
    }
    match state.dialog.as_ref().and_then(|d| d.error.as_ref()) {
        Some(e) => out.push_str(&format!("preview_error={e}\n")),
        None => out.push_str("preview_error=-\n"),
    }
    out.push_str(&format!("applied={}\n", app.len()));
    for f in app {
        out.push_str(&format!("feat={}::{}\n", f.kind.id(), f.volume));
    }
    out.push_str(&format!("journal={}\n", feature_journal(ctx).len()));
    out.push_str(&format!("fdoc_hash={}\n", document_digest(&sk, app)));
    out.push_str(&format!(
        "export_path={}\n",
        current_export_path(ctx).display()
    ));
    out.push_str(&format!("last={}\n", state.last.as_deref().unwrap_or("-")));
    out
}

/// Publish the `String` oracle into the egui context data store. The oracle
/// text is built BEFORE the write lock (it reads the sketch oracle, so it must
/// not run inside `data_mut`).
fn publish_oracle(ctx: &egui::Context, state: &FeatureState) {
    let text = oracle_lines(ctx, state);
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(FEATURE_REPORT_KEY), text);
    });
}

/// Build the editor session clone for a committed sketch: the CLI's own
/// interactive-editor surface, seeded with the committed geometry, solved and
/// swept. Fails typed when the sketch is not a single closed loop or the
/// drive-lane refuses (never a fabricated solid).
pub fn build_clone(
    sk: &CommittedSketch,
    kind: FeatureKind,
    amount: f64,
) -> Result<oep_shell::editor::EditorSession, FeatureError> {
    if !is_closed_loop(sk) {
        return Err(FeatureError::NotAFinishedSketch(format!(
            "{} points / {} lines do not form a single closed loop",
            sk.points.len(),
            sk.lines.len()
        )));
    }
    let mut s = oep_shell::editor::EditorSession::new();
    for &(x, y) in &sk.points {
        s.point(x, y)
            .map_err(|e| FeatureError::Drive(e.to_string()))?;
    }
    for (a, b) in &sk.lines {
        s.line(a, b)
            .map_err(|e| FeatureError::Drive(e.to_string()))?;
    }
    s.solve().map_err(|e| FeatureError::Drive(e.to_string()))?;
    let dim = oep_sketch::sketch::DimExpr::Literal(amount);
    match kind {
        FeatureKind::Extrude => {
            s.extrude(dim)
                .map_err(|e| FeatureError::Drive(e.to_string()))?;
        }
        FeatureKind::Revolve => {
            s.revolve(dim)
                .map_err(|e| FeatureError::Drive(e.to_string()))?;
        }
    }
    Ok(s)
}

/// Finish one `EditorSession` sweep and read back the derived solid's own mass
/// property and face count (the document's `FeatureReport`/`props` seam, and
/// the session mesh's public triangle count - no oep-geo import needed).
fn preview_from_clone(session: &oep_shell::editor::EditorSession) -> (f64, usize) {
    let volume = session.props().volume.unwrap_or(0.0);
    (volume, session_face_count(session))
}

/// Face count of the session's derived mesh, read through the shared viewer
/// seam (no oep-geo import needed).
fn session_face_count(session: &oep_shell::editor::EditorSession) -> usize {
    session.mesh().map(|m| m.triangles.len()).unwrap_or(0)
}

/// Rebuild the live preview clone for the open dialog (kind + amount). The
/// clone is re-derived whenever either changes, so the preview is live - the
/// viewport frame moves with the amount.
fn rebuild_preview(ctx: &egui::Context, dialog: &mut FeatureDialog) {
    let sk = match committed_geometry(&sketch_report(ctx)) {
        Some(sk) => sk,
        None => {
            dialog.preview = None;
            dialog.error = Some(FeatureError::NotAFinishedSketch(
                "no committed sketch geometry".into(),
            ));
            return;
        }
    };
    let amount = dialog.amount;
    match build_clone(&sk, dialog.kind, amount) {
        Ok(session) => {
            let (volume, faces) = preview_from_clone(&session);
            dialog.preview = Some(FeaturePreview {
                session,
                marked_preview: true,
                volume,
                faces,
            });
            dialog.error = None;
        }
        Err(e) => {
            dialog.preview = None;
            dialog.error = Some(e);
        }
    }
}

/// Open the finish dialog from a committed geometry: seed the default state,
/// build the live preview clone, and (if the sketch cannot be finished) keep
/// the dialog with the honest named error.
fn open_dialog(ctx: &egui::Context, state: &mut FeatureState) {
    let mut dialog = FeatureDialog::default();
    rebuild_preview(ctx, &mut dialog);
    state.last = Some(match &dialog.preview {
        Some(_) => "finish sketch: preview".to_string(),
        None => dialog
            .error
            .as_ref()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "finish sketch".to_string()),
    });
    state.dialog = Some(dialog);
}

// ---------------------------------------------------------------------------
// Deterministic layout (the renderer and the acceptance harness click the same
// rects, the B6 contract)
// ---------------------------------------------------------------------------

/// Rects of the feature surface anchored at the viewport's top-left, below the
/// B4 draw legend (clear of the B6 constraint toolbar at the top-right).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureRects {
    /// "Finish sketch" button (sketch mode, closed loop, nothing applied).
    pub finish: egui::Rect,
    /// "Export STEP" button (a feature is applied).
    pub export_step: egui::Rect,
}

/// Deterministic viewport-anchored surface rects.
pub fn feature_rects(viewport: egui::Rect) -> FeatureRects {
    let x = viewport.min.x + 8.0;
    let y = viewport.min.y + 34.0;
    let size = egui::vec2(120.0, 24.0);
    FeatureRects {
        finish: egui::Rect::from_min_size(egui::pos2(x, y), size),
        export_step: egui::Rect::from_min_size(egui::pos2(x, y), size),
    }
}

/// Rects of the dialog card (kind toggles, amount steppers, Apply / Cancel),
/// anchored at the viewport centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogRects {
    /// "Extrude" kind toggle.
    pub kind_extrude: egui::Rect,
    /// "Revolve" kind toggle.
    pub kind_revolve: egui::Rect,
    /// "−" amount decrement.
    pub minus: egui::Rect,
    /// The amount value field (readable/typed).
    pub amount: egui::Rect,
    /// "+" amount increment.
    pub plus: egui::Rect,
    /// Apply button.
    pub apply: egui::Rect,
    /// Cancel button.
    pub cancel: egui::Rect,
}

/// Deterministic dialog-card rects centred on the viewport.
pub fn dialog_rects(viewport: egui::Rect) -> DialogRects {
    let w = 300.0;
    let h = 170.0;
    let x0 = viewport.min.x + (viewport.width() - w) * 0.5;
    let y0 = viewport.min.y + (viewport.height() - h) * 0.45;
    let row = |x: f32, y: f32, sx: f32, sy: f32| {
        egui::Rect::from_min_size(egui::pos2(x0 + x, y0 + y), egui::vec2(sx, sy))
    };
    DialogRects {
        kind_extrude: row(16.0, 40.0, 96.0, 24.0),
        kind_revolve: row(120.0, 40.0, 96.0, 24.0),
        minus: row(16.0, 76.0, 30.0, 24.0),
        amount: row(54.0, 76.0, 96.0, 24.0),
        plus: row(158.0, 76.0, 30.0, 24.0),
        apply: row(16.0, 128.0, 90.0, 26.0),
        cancel: row(114.0, 128.0, 90.0, 26.0),
    }
}

fn palette(ctx: &egui::Context) -> Palette {
    let mode = if ctx.global_style().visuals.dark_mode {
        Mode::Dark
    } else {
        Mode::Light
    };
    Palette::resolve(mode)
}

/// Step the current amount by one `dir` (-1 / +1) for the given kind (extrude
/// steps millimetres, revolve steps a 30-degree band).
fn stepped_amount(kind: FeatureKind, amount: f64, dir: i32) -> f64 {
    let step = match kind {
        FeatureKind::Extrude => 5.0,
        FeatureKind::Revolve => std::f64::consts::PI / 6.0,
    };
    (amount + step * f64::from(dir)).max(0.0)
}

/// Parse the amount draft field; invalid text keeps the current amount and
/// sets an honest refusal on the dialog.
fn apply_draft(ctx: &egui::Context, dialog: &mut FeatureDialog) {
    let parsed: Option<f64> = dialog.draft.trim().parse().ok();
    match parsed {
        Some(v) if v > 0.0 && v.is_finite() => {
            if (v - dialog.amount).abs() > f64::EPSILON {
                dialog.amount = v;
                rebuild_preview(ctx, dialog);
            }
        }
        Some(v) => {
            dialog.error = Some(FeatureError::NotAFinishedSketch(format!(
                "amount must be a positive number, got {v}"
            )));
        }
        None => {
            dialog.error = Some(FeatureError::NotAFinishedSketch(
                "amount must be a number".into(),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Per-frame interaction (called by the viewport panel, which owns the frame)
// ---------------------------------------------------------------------------

/// Per-frame keyboard interaction, run from `viewport.rs` after the sketch
/// panel's own read:
///
/// - plain `F` while sketching a closed loop (no dialog, nothing applied)
///   opens the finish dialog and builds the live preview;
/// - Escape cancels the open dialog (consumed here so the viewport's
///   exit-sketch Escape does not also fire).
pub fn poll(ctx: &egui::Context) {
    let mut state = read_state(ctx);

    if state.dialog.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        });
        state.last = Some("cancelled feature".to_string());
        state.dialog = None;
    }

    if state.dialog.is_none()
        && state.applied.is_empty()
        && in_sketch_mode(ctx)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, FINISH_KEY))
    {
        open_dialog(ctx, &mut state);
    }

    write_state(ctx, &state);
    publish_oracle(ctx, &state);
}

/// Export the applied feature as STEP through the CLI's own lane
/// (`EditorSession::export_step`, the exact method `oep-workbench export:step`
/// dispatches). The session is re-derived from the committed sketch + applied
/// kind/amount so the bytes come from the document, never a frozen copy.
pub fn export_step(ctx: &egui::Context, path: &std::path::Path) -> Result<String, FeatureError> {
    let state = read_state(ctx);
    let applied = state
        .applied
        .last()
        .cloned()
        .ok_or_else(|| FeatureError::Drive("apply a feature before exporting".into()))?;
    let sk = committed_geometry(&sketch_report(ctx))
        .ok_or_else(|| FeatureError::NotAFinishedSketch("no committed sketch".into()))?;
    let session = build_clone(&sk, applied.kind, applied.amount)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| FeatureError::Drive("export path is not valid UTF-8".into()))?;
    session
        .export_step(path_str)
        .map_err(|e| FeatureError::Drive(e.to_string()))?;
    let bytes = std::fs::read(path).map_err(|e| FeatureError::Drive(e.to_string()))?;
    Ok(oep_shell::sha256::sha256_hex(&bytes))
}

// ---------------------------------------------------------------------------
// The preview frame path (the viewport panel owns rendering; this provides the
// rasterized frame)
// ---------------------------------------------------------------------------

/// Render the current preview frame at `(w, h)`: the derived solid's scene
/// (a document clone's `build_scene`) with a camera framed to its own bounds,
/// rasterized through the existing deterministic CPU renderer. Returns the
/// rendered frame plus its stable FNV-1a-64 hash - the "preview frame" the
/// viewport swaps in, and the hash the acceptance asserts differs from the
/// pre-feature frame.
///
/// `None` when nothing is being previewed (no open dialog, no applied feature)
/// - the viewport then keeps its ordinary frame.
pub fn preview_frame(
    ctx: &egui::Context,
    w: u32,
    h: u32,
) -> Option<(oep_viewer::render::RgbaImage, u64)> {
    let state = read_state(ctx);
    let session = if let Some(d) = state.dialog.as_ref() {
        d.preview.as_ref().map(|p| p.session.clone())
    } else if !state.applied.is_empty() {
        // After Apply the solid stays visible (re-derived from the document).
        let sk = committed_geometry(&sketch_report(ctx))?;
        let applied = state.applied.last()?;
        build_clone(&sk, applied.kind, applied.amount).ok()
    } else {
        None
    }?;
    let scene = session.build_scene().ok()?;
    let cam = scene
        .bounds
        .as_ref()
        .map(oep_viewer::Camera::framed)
        .unwrap_or_default();
    let img = oep_viewer::render::render(&scene, &cam, &[], w.max(1), h.max(1));
    let hash = oep_viewer::backend::frame_hash(&img);
    Some((img, hash))
}

/// Apply one dialog mutation: kind toggle, amount step, Apply or Cancel. Reads
/// the current panel state, mutates in place, rebuilds the preview when the
/// derived input changed, and persists + republishes the oracle. Returns true
/// when the panel changed (the viewport should request a repaint).
fn run_action(ctx: &egui::Context, action: DialogAction) -> bool {
    let mut state = read_state(ctx);
    let mut changed = false;
    match action {
        DialogAction::Open => {
            if state.dialog.is_none() && state.applied.is_empty() {
                open_dialog(ctx, &mut state);
                changed = true;
            }
        }
        DialogAction::Cancel => {
            if state.dialog.is_some() {
                state.last = Some("cancelled feature".to_string());
                state.dialog = None;
                changed = true;
            }
        }
        DialogAction::Kind(kind) => {
            if let Some(d) = state.dialog.as_mut() {
                if d.kind != kind {
                    d.kind = kind;
                    d.draft = format!("{}", d.amount);
                    rebuild_preview(ctx, d);
                    state.last = Some(format!("kind: {}", kind.id()));
                    changed = true;
                }
            }
        }
        DialogAction::Step(dir) => {
            if let Some(d) = state.dialog.as_mut() {
                let next = stepped_amount(d.kind, d.amount, dir);
                // An honest no-op at the floor is still a click; only rebuild
                // when the value moved.
                if next != d.amount {
                    d.amount = next;
                    d.draft = format!("{}", next);
                    rebuild_preview(ctx, d);
                    state.last = Some(format!("{} amount {}", d.kind.id(), next));
                    changed = true;
                }
            }
        }
        DialogAction::Apply => {
            if state.dialog.is_some() {
                if let Some(p) = state.dialog.as_ref().and_then(|d| d.preview.as_ref()) {
                    let dlg = state.dialog.as_ref().unwrap();
                    let feature = AppliedFeature {
                        kind: dlg.kind,
                        amount: dlg.amount,
                        volume: p.volume,
                        faces: p.faces,
                    };
                    let sk = committed_geometry(&sketch_report(ctx)).unwrap_or_default();
                    let before = document_digest(&sk, &state.applied);
                    let mut applied = state.applied.clone();
                    applied.push(feature.clone());
                    let after = document_digest(&sk, &applied);
                    let mut j = feature_journal(ctx);
                    j.push(feature.kind.token(), None, before, after);
                    write_feature_journal(ctx, &j);
                    state.applied = applied;
                    state.last = Some(format!(
                        "applied {}: volume {}",
                        feature.kind.id(),
                        feature.volume
                    ));
                    state.dialog = None;
                    changed = true;
                }
            }
        }
        DialogAction::Export => {
            if !state.applied.is_empty() {
                let path = current_export_path(ctx);
                match export_step(ctx, &path) {
                    Ok(hash) => state.last = Some(format!("STEP {}", &hash[..8.min(hash.len())])),
                    Err(e) => state.last = Some(e.to_string()),
                }
                changed = true;
            }
        }
        DialogAction::Draft(text) => {
            if let Some(d) = state.dialog.as_mut() {
                d.draft = text;
                apply_draft(ctx, d);
                changed = true;
            }
        }
    }
    if changed {
        write_state(ctx, &state);
        publish_oracle(ctx, &state);
    }
    changed
}

/// One deterministic interaction the dialog/surface buttons can trigger.
enum DialogAction {
    Open,
    Cancel,
    Kind(FeatureKind),
    Step(i32),
    Apply,
    Export,
    Draft(String),
}

// ---------------------------------------------------------------------------
// Painting (called by the viewport panel at the end of the frame)
// ---------------------------------------------------------------------------

/// Draw the feature surface over the viewport: the Finish button (sketch mode,
/// closed loop, none applied), the persistent Export button (a feature is
/// applied), or the live dialog card. Uses the same deterministic rects the
/// acceptance harness clicks (the B6 contract).
pub fn draw(ctx: &egui::Context, viewport: egui::Rect) {
    let state = read_state(ctx);
    let p = palette(ctx);
    let rects = feature_rects(viewport);

    egui::Area::new(egui::Id::unique("ui_b8_feature_surface"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            let painter = ui.painter();
            if state.dialog.is_none() && !state.applied.is_empty() {
                painter.text(
                    egui::pos2(rects.export_step.min.x, rects.export_step.min.y - 14.0),
                    egui::Align2::LEFT_TOP,
                    "solid ready",
                    egui::FontId::proportional(9.0),
                    p.text_muted,
                );
                let btn =
                    egui::Button::new(egui::RichText::new("Export STEP").size(11.0)).min_size(
                        egui::vec2(rects.export_step.width(), rects.export_step.height()),
                    );
                let resp = ui.put(rects.export_step, btn);
                if resp.clicked() {
                    run_action(ui.ctx(), DialogAction::Export);
                    // repaint to surface the new oracle/status
                    ui.ctx().request_repaint();
                }
            } else if state.dialog.is_none() && in_sketch_mode(ctx) {
                let btn = egui::Button::new(egui::RichText::new("Finish sketch").size(11.0))
                    .min_size(egui::vec2(rects.finish.width(), rects.finish.height()));
                let resp = ui.put(rects.finish, btn);
                if resp.clicked() {
                    run_action(ui.ctx(), DialogAction::Open);
                    ui.ctx().request_repaint();
                }
            }
        });

    if state.dialog.is_some() {
        let d = dialog_rects(viewport);
        egui::Area::new(egui::Id::unique("ui_b8_feature_dialog"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .interactable(true)
            .show(ctx, |ui| {
                let card = egui::Rect::from_min_size(
                    egui::pos2(d.kind_extrude.min.x - 16.0, d.kind_extrude.min.y - 32.0),
                    egui::vec2(248.0, 178.0),
                );
                // Background + title: painted first in a scoped painter (the
                // widget `put`s below need the mutable ui borrow).
                {
                    let painter = ui.painter();
                    painter.rect_filled(card, 4.0, p.surface_raised);
                    painter.rect_stroke(
                        card,
                        4.0,
                        egui::Stroke::new(1.0, p.border),
                        egui::StrokeKind::Outside,
                    );
                    painter.text(
                        egui::pos2(card.min.x + 8.0, card.min.y + 6.0),
                        egui::Align2::LEFT_TOP,
                        "Finish sketch",
                        egui::FontId::proportional(12.0),
                        p.text_primary,
                    );
                    painter.text(
                        egui::pos2(card.min.x + 8.0, card.min.y + 20.0),
                        egui::Align2::LEFT_TOP,
                        "live preview in viewport",
                        egui::FontId::proportional(9.0),
                        p.text_muted,
                    );
                }

                let kind = state
                    .dialog
                    .as_ref()
                    .map(|d| d.kind)
                    .unwrap_or(FeatureKind::Extrude);

                // Kind toggle buttons (real widgets; clicks route to the same
                // run_action lane).
                let mut kind_btn =
                    |rect: egui::Rect, label: &str, active: bool, target: FeatureKind| {
                        let text = egui::RichText::new(label).size(11.0);
                        let btn = if active {
                            egui::Button::new(text).fill(p.accent)
                        } else {
                            egui::Button::new(text)
                        };
                        let resp = ui.put(rect, btn);
                        if resp.clicked() {
                            run_action(ui.ctx(), DialogAction::Kind(target));
                            ui.ctx().request_repaint();
                        }
                    };
                kind_btn(
                    d.kind_extrude,
                    FeatureKind::Extrude.label(),
                    kind == FeatureKind::Extrude,
                    FeatureKind::Extrude,
                );
                kind_btn(
                    d.kind_revolve,
                    FeatureKind::Revolve.label(),
                    kind == FeatureKind::Revolve,
                    FeatureKind::Revolve,
                );

                // Amount steppers + editable field.
                let minus = ui.put(d.minus, egui::Button::new("−"));
                if minus.clicked() {
                    run_action(ui.ctx(), DialogAction::Step(-1));
                    ui.ctx().request_repaint();
                }
                let unit = kind.unit();
                let draft = state
                    .dialog
                    .as_ref()
                    .map(|d| d.draft.clone())
                    .unwrap_or_default();
                let mut draft = draft;
                let field = ui.put(
                    d.amount,
                    egui::TextEdit::singleline(&mut draft)
                        .desired_width(d.amount.width() - 6.0)
                        .font(egui::TextStyle::Monospace),
                );
                if field.changed() {
                    run_action(ui.ctx(), DialogAction::Draft(draft.clone()));
                }
                let plus = ui.put(d.plus, egui::Button::new("+"));
                if plus.clicked() {
                    run_action(ui.ctx(), DialogAction::Step(1));
                    ui.ctx().request_repaint();
                }

                // Apply / Cancel.
                let apply = egui::Button::new(egui::RichText::new("Apply").size(11.0));
                let apply_resp = ui.put(d.apply, apply);
                if apply_resp.clicked() {
                    run_action(ui.ctx(), DialogAction::Apply);
                    ui.ctx().request_repaint();
                }
                let cancel_resp = ui.put(
                    d.cancel,
                    egui::Button::new(egui::RichText::new("Cancel").size(11.0)),
                );
                if cancel_resp.clicked() {
                    run_action(ui.ctx(), DialogAction::Cancel);
                    ui.ctx().request_repaint();
                }

                // Honest readout + unit label: painted last (after the puts,
                // so no live painter borrow crosses them).
                let readout = match state.dialog.as_ref().and_then(|d| d.preview.as_ref()) {
                    Some(p) => format!("volume {} · {} faces", p.volume, p.faces),
                    None => state
                        .dialog
                        .as_ref()
                        .and_then(|d| d.error.as_ref())
                        .map(|e| e.to_string())
                        .unwrap_or_default(),
                };
                let painter = ui.painter();
                painter.text(
                    egui::pos2(d.amount.max.x + 6.0, d.amount.min.y + 7.0),
                    egui::Align2::LEFT_TOP,
                    unit,
                    egui::FontId::monospace(10.0),
                    p.text_muted,
                );
                painter.text(
                    egui::pos2(d.apply.min.x, d.apply.max.y + 8.0),
                    egui::Align2::LEFT_TOP,
                    readout,
                    egui::FontId::monospace(10.0),
                    p.text_muted,
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed_report() -> String {
        concat!(
            "mode=sketch\nplane=XY\nlabel=XY (Top)\nnormal=0,0,1\ngrid=on\n",
            "tool=-\nsteps=0\npreview_points=0\npreview_lines=0\ndimx=-\ndimy=-\nlast=-\n",
            "snap=on\ninfer=-\nsnapx=-\nsnapy=-\n",
            "docpoints=4\ndoclines=4\ncircles=0\njournal=0\n",
            "point=0.000,0.000,p0\npoint=40.000,0.000,p1\npoint=40.000,24.000,p2\npoint=0.000,24.000,p3\n",
            "lineseg=l0:p0->p1\nlineseg=l1:p1->p2\nlineseg=l2:p2->p3\nlineseg=l3:p3->p0\n",
            "constraints=0\ndof=-\nsolve=unsolved\nconstraint_status=not solved\nconstraint_last=\ncontext_menu=0\n",
        )
        .to_string()
    }

    /// The committed geometry parse is the same oracle seam B7 reads.
    #[test]
    fn geometry_parses_a_closed_loop() {
        let sk = committed_geometry(&closed_report()).expect("closed loop present");
        assert_eq!(sk.points.len(), 4);
        assert_eq!(sk.lines.len(), 4);
        assert!(is_closed_loop(&sk));
        assert_eq!(document_digest(&sk, &[]), document_digest(&sk, &[]));
        assert_ne!(
            document_digest(&sk, &[]),
            document_digest(&CommittedSketch::default(), &[])
        );
    }

    /// An open chain is NOT a closed loop - the dialog must refuse it.
    #[test]
    fn open_chain_is_not_a_closed_loop() {
        let report = closed_report()
            .replacen("point=40.000,24.000,p2\npoint=0.000,24.000,p3\n", "", 1)
            .replacen("docpoints=4", "docpoints=2", 1)
            .replacen("lineseg=l2:p2->p3\nlineseg=l3:p3->p0\n", "", 1);
        let sk = committed_geometry(&report).expect("geometry parsed");
        assert!(!is_closed_loop(&sk));
    }

    /// The closed-loop clone solves and extrudes: the document's own volume
    /// for a 40x24 profile at depth 10 is 40*24*10 = 9600, read back from the
    /// session's mass property (never a test literal).
    #[test]
    fn closed_loop_clone_extrudes_and_reports_volume() {
        let sk = committed_geometry(&closed_report()).unwrap();
        let session = build_clone(&sk, FeatureKind::Extrude, 10.0).expect("extrude ok");
        let (volume, faces) = preview_from_clone(&session);
        assert!((volume - 9600.0).abs() < 1e-6, "volume {volume}");
        assert_eq!(faces, 12, "box: 12 triangles");
    }

    /// A revolve of the closed loop derives a solid too (the dialog offers
    /// both kinds and both preview live).
    #[test]
    fn closed_loop_clone_revolves() {
        let sk = committed_geometry(&closed_report()).unwrap();
        // revolve about Z by a quarter turn
        let session =
            build_clone(&sk, FeatureKind::Revolve, std::f64::consts::PI / 2.0).expect("revolve ok");
        let (volume, faces) = preview_from_clone(&session);
        assert!(volume > 0.0, "revolve derives a solid");
        assert!(faces > 0, "revolve has faces");
    }

    /// Both feature tokens are registered in the CLI's command registry (the
    /// "one command language" the router and CLI share).
    #[test]
    fn feature_tokens_are_registry_validated() {
        assert!(oep_shell::commands::find(FeatureKind::Extrude.token()).is_some());
        assert!(oep_shell::commands::find(FeatureKind::Revolve.token()).is_some());
        for kind in [FeatureKind::Extrude, FeatureKind::Revolve] {
            let token = kind.token().split(':').next().unwrap();
            assert_eq!(oep_shell::commands::validate_token(token), Ok(token));
        }
    }
}
