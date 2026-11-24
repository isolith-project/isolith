//! Model tree panel: the real document tree (wave C, card UI-C1 UI-FEATURE-TREE).
//!
//! The tree is the document: part, body and feature rows are derived from the
//! live model document every frame (never a cached row list), each row selects
//! into the shared selection id, is renameable, and is deletable with an
//! explicit confirmation and undo.
//!
//! ## The UI-A11 oracle is extended, not duplicated
//!
//! UI-A11 landed the first honest tree panel: it read the live document
//! through the doc panel's plain-`String` oracle ([`DOC_REPORT_KEY`]) and
//! reported the file name plus the body/feature counts that oracle carries.
//! UI-C1 EXTENDS that same panel instead of retiring it: the A11 header lines
//! (file name, "N bodies, M features", the wave-C note) are kept verbatim -
//! they are locked by the `--test tree_empty_state` contract - and the real
//! feature tree is added underneath, sourced from a live,
//! [`doc_io::EditHistory`]-owned model document instead of the count oracle.
//! No second parallel empty-state/tree mechanism is introduced; the header
//! still answers "is a document open", and the tree below it renders only
//! when a document is actually open.
//!
//! ## The live document
//!
//! "Rows come from the live document, never a cached copy": this panel keeps
//! its own authoritative model document (the same committed demo STEP the doc
//! panel auto-opens, imported through the same [`doc_io::import_step`] path; a
//! test may seed any canonical `.oepdoc` JSON under [`TREE_SEED_KEY`] before
//! the first frame, exactly as if a file had been opened). Every frame the
//! visible rows are RE-DERIVED from `document.parts` / `document.bodies` /
//! `document.features` in [`rows_of`]; there is no stored row vector to go
//! stale. A feature whose `kind` is `Sketch` or `VariableSet` renders as a
//! Sketch / Variable Set row, so all five row kinds the card names (part,
//! body, features, sketch, variable set) come from the document whenever it
//! contains them.
//!
//! All edits are ROUTED MUTATIONS through the UI-B2 journal seam
//! ([`doc_io::EditHistory::apply`]): rename writes the entity's `name` and
//! delete removes the row (and cleans its owner references), each recorded as
//! one journal entry with the document hashes before and after. Undo restores
//! the canonical document, so the document hash after an undo is exactly the
//! hash before the delete (the card's exactness contract). [`TREE_REPORT_KEY`]
//! publishes the current hash every frame for the acceptance harness.
//!
//! ## Selection is one shared id
//!
//! Selecting a row publishes a plain `String` (the node's document id) under
//! [`TREE_SELECTION_KEY`] - the one shared id the viewport, the properties
//! panel and the status bar all read (D-UI-5). The status bar is wired on the
//! frame the selection changes via `app.status.selection_count`; the viewport
//! and properties panels read the same key in their cards. The selected row is
//! also painted highlighted in this panel.
//!
//! ## Bounds this panel respects
//!
//! - UI-A3 / UI-A12 guards: heading ink comes from [`crate::theme::heading`],
//!   no raw hex and no `Color32::from_*` here.
//! - UI-B1 mutation guard: no direct write to a document collection outside
//!   [`doc_io`] (the journal seam). The delete helpers here run INSIDE
//!   `EditHistory::apply`, the sanctioned routed seam, so the only collection
//!   touches in this panel are `keep_if`-style predicates and plain `name`
//!   field writes.
//! - UI-B3 sketch node and the UI-A11 header text are preserved verbatim for
//!   the frozen `--test tree_empty_state` / `--test sketch_enter` contracts.

use eframe::egui;

use crate::app::App;
use crate::theme;

use super::doc::DOC_REPORT_KEY;

// The routed document seam (wave B, card UI-B2 UI-HISTORY), included verbatim
// so this panel reads and mutates the live model document through the exact
// journal the wave built. `panels/doc.rs` includes the same file under its own
// module path; the two copies never share a stored typed value (each owner
// keeps its own document and [`doc_io::EditHistory`]), so no cross-copy
// TypeId arises - the same allowance the sketch/feature panels make for the
// journal.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../doc_io.rs"]
mod doc_io;

/// Context-data key under which the panel's typed tree model lives (this copy
/// only; tests read the `String` oracle instead).
pub const TREE_STATE_KEY: &str = "ui_c1_tree_state";
/// Context-data key under which the panel publishes its plain-`String` report
/// every frame (rows, counts, selection, document hash) - the cross-copy-safe
/// oracle the acceptance harness reads back.
pub const TREE_REPORT_KEY: &str = "ui_c1_tree_report";
/// The ONE shared selection id: a plain `String` (the selected document node
/// id, empty when nothing is selected) under this key. The viewport, the
/// properties panel and the status bar all read this same id.
pub const TREE_SELECTION_KEY: &str = "isolith.ui.tree.selection";
/// Context-data key a test (or a future open-file card) may seed with the
/// canonical `.oepdoc` JSON of the document the tree should show before its
/// first frame; like opening a file, the seeded document becomes the live
/// model. Absent, the committed demo STEP is opened instead.
pub const TREE_SEED_KEY: &str = "ui_c1_tree_seed";

/// Journal token this panel records for a rename edit (the UI-B2 journal
/// records the token plus the before/after hashes; the `tree:` family is this
/// panel's own editing language - the registry in `oep-shell` has no
/// rename/delete command, and that crate is not an owned file).
pub const RENAME_TOKEN: &str = "tree:rename";
/// Journal token this panel records for a delete edit.
pub const DELETE_TOKEN: &str = "tree:delete";
/// Journal token this panel records for a suppress/restore edit.
pub const SUPPRESS_TOKEN: &str = "tree:suppress";
/// Journal token this panel records for a roll/unroll edit.
pub const ROLL_TOKEN: &str = "tree:roll";

/// `Feature.parameters` key marking a feature suppressed (UI-C4). Document
/// state: it serializes into the canonical `.oepdoc`, so a saved + reopened
/// document replays the same suppression.
pub const PARAM_SUPPRESSED: &str = "suppressed";
/// `Feature.parameters` key marking the feature the tree is rolled back to
/// (UI-C4). Everything created after it in the document's feature order is
/// hidden (rolled back). Document state, same round-trip as suppression.
pub const PARAM_ROLL: &str = "roll";

/// Context-data key under which the panel publishes the live canonical
/// `.oepdoc` JSON of its model document every frame. Cross-copy-safe plain
/// `String`; the acceptance harness reads this to prove save/reopen (the text
/// `doc_io::write_oepdoc` would write, read as the reopened document).
pub const TREE_DOC_JSON_KEY: &str = "ui_c1_tree_doc_json";

/// The width given to the inline rename text field.
const RENAME_FIELD_WIDTH: f32 = 180.0;
/// Indent, in points, per tree depth level.
const DEPTH_INDENT: f32 = 14.0;

/// Doc-oracle field prefixes (the lines UI-A7's doc panel writes under
/// [`DOC_REPORT_KEY`], which this panel parses for its header).
mod field {
    /// The doc panel's visible display name, e.g. `edpart.step *` (dirty star).
    pub const FILE: &str = "file=";
    /// Body count taken from the current document itself.
    pub const BODIES: &str = "bodies=";
    /// Feature count taken from the current document itself.
    pub const FEATURES: &str = "features=";
}

/// What the tree panel's header currently shows about the open document
/// (UI-A11's truthfulness contract, kept verbatim).
enum TreeState {
    /// No document open: keep the honest "No model open" line.
    Empty,
    /// A document is open: name it, report its real contents, and keep the
    /// wave-C note the frozen tree_empty_state contract locks.
    Open {
        /// The live display name (with dirty star when the doc is dirty).
        file_name: String,
        /// Body count taken from the document itself.
        bodies: usize,
        /// Feature count taken from the document itself.
        features: usize,
    },
}

/// Resolve the header state from the doc panel's live oracle.
///
/// Reads only the egui context store every frame - never a file, never cached
/// state - so the header updates on the same frame the document changes.
fn tree_state(ctx: &egui::Context) -> TreeState {
    let report: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(DOC_REPORT_KEY)));

    let mut file_name = None;
    let mut bodies = None;
    let mut features = None;
    if let Some(report) = report {
        for line in report.lines() {
            if let Some(v) = line.strip_prefix(field::FILE) {
                file_name = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix(field::BODIES) {
                bodies = v.parse().ok();
            } else if let Some(v) = line.strip_prefix(field::FEATURES) {
                features = v.parse().ok();
            }
        }
    }

    // A document is open only when the doc panel reports a real file name and
    // both truthful counts from it. The doc panel names an unopened session
    // "Untitled" (its no-document sentinel, shared with the status bar), so
    // that case stays honestly "No model open" instead of inventing contents.
    match (file_name, bodies, features) {
        (Some(name), Some(bodies), Some(features)) if !name.is_empty() && name != "Untitled" => {
            TreeState::Open {
                file_name: name,
                bodies,
                features,
            }
        }
        _ => TreeState::Empty,
    }
}

/// The active sketch plane's label while sketch mode is on, read from the
/// sketch viewport panel's `String` oracle (UI-B3 UI-SKETCH-ENTER). `None`
/// when not in sketch mode. Kept verbatim for the frozen sketch_enter test.
fn active_sketch_plane(ctx: &egui::Context) -> Option<String> {
    let report: Option<String> = ctx.data(|d| {
        d.get_temp::<String>(egui::Id::unique(super::viewport::sketch::SKETCH_REPORT_KEY))
    });
    let report = report?;
    let mut mode = None;
    let mut label = None;
    for line in report.lines() {
        if let Some(v) = line.strip_prefix("mode=") {
            mode = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("label=") {
            label = Some(v.to_string());
        }
    }
    if mode.as_deref() == Some("sketch") {
        label
    } else {
        None
    }
}

/// The kind of one tree row, used both to derive rows and to decide how a
/// rename/delete applies to the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    /// A `Document.parts` entry.
    Part,
    /// A `Document.bodies` entry.
    Body,
    /// A `Document.features` entry (including Sketch / VariableSet kinds).
    Feature,
}

/// One row in the tree, derived live from the document every frame.
#[derive(Debug, Clone)]
struct Row {
    /// The document-wide id of the entity this row names (the shared
    /// selection id).
    id: String,
    /// Whether this is a part, body or feature row.
    kind: RowKind,
    /// The full accessible label rendered for the row.
    label: String,
    /// Tree depth: parts at 0, owned bodies/features at 1, unowned rows too.
    depth: usize,
}

/// The friendly row-note for a feature kind: sketch and variable-set features
/// become the card's named row kinds; anything else shows its own kind.
fn feature_note(kind: &str) -> String {
    match kind {
        "Sketch" => "Sketch".to_string(),
        "VariableSet" => "Variable Set".to_string(),
        k => k.to_string(),
    }
}

/// The full row label for a feature: name + kind note, plus the honest
/// UI-C4 suppression mark when the feature carries `suppressed` document state
/// (a readable suffix, so screen readers and the AccessKit assertions see the
/// state in plain text).
fn feature_row_label(name: &str, note: &str, suppressed: bool) -> String {
    let base = format!("{name} ({note})");
    if suppressed {
        format!("{base} (suppressed)")
    } else {
        base
    }
}

/// Derive the live tree rows from the document. Pure: every call re-reads the
/// document collections, so an edit made through the journal is reflected on
/// the very next frame without any stored row list ("rows come from the live
/// document, never a cached copy").
fn rows_of(document: &Option<oep_import::Document>) -> Vec<Row> {
    let Some(doc) = document else {
        return Vec::new();
    };
    let mut rows = Vec::new();

    let owned_body = |id: &str, doc: &oep_import::Document| -> bool {
        doc.parts.iter().any(|p| p.body_ids.iter().any(|b| b == id))
    };
    let owned_feature = |id: &str, doc: &oep_import::Document| -> bool {
        doc.parts
            .iter()
            .any(|p| p.feature_ids.iter().any(|f| f == id))
    };

    // 1. Parts and, under each, the bodies and features the part owns.
    for part in &doc.parts {
        rows.push(Row {
            id: part.id.clone(),
            kind: RowKind::Part,
            label: format!("{} (Part)", part.name),
            depth: 0,
        });
        for body_id in &part.body_ids {
            if let Some(body) = doc.bodies.iter().find(|b| &b.id == body_id) {
                rows.push(Row {
                    id: body.id.clone(),
                    kind: RowKind::Body,
                    label: format!("{} (Body)", body.name),
                    depth: 1,
                });
            }
        }
        for feature_id in &part.feature_ids {
            if let Some(feature) = doc.features.iter().find(|f| &f.id == feature_id) {
                // UI-C4: a feature created after the roll point is hidden
                // (rolled back). The tree re-derives rows every frame, so the
                // held-back set is exactly the document's downstream features.
                if is_rolled_hidden(doc, &feature.id) {
                    continue;
                }
                let note = feature_note(&feature.kind);
                let label = feature_row_label(&feature.name, &note, feature_suppressed(feature));
                rows.push(Row {
                    id: feature.id.clone(),
                    kind: RowKind::Feature,
                    label,
                    depth: 1,
                });
            }
        }
    }

    // 2. Bodies and features not owned by any part (a document may hold
    //    top-level entries; the tree tells the truth rather than hiding them).
    for body in &doc.bodies {
        if !owned_body(&body.id, doc) {
            rows.push(Row {
                id: body.id.clone(),
                kind: RowKind::Body,
                label: format!("{} (Body)", body.name),
                depth: 0,
            });
        }
    }
    for feature in &doc.features {
        if !owned_feature(&feature.id, doc) {
            // UI-C4: same downstream hiding for unowned features.
            if is_rolled_hidden(doc, &feature.id) {
                continue;
            }
            let note = feature_note(&feature.kind);
            rows.push(Row {
                id: feature.id.clone(),
                kind: RowKind::Feature,
                label: feature_row_label(&feature.name, &note, feature_suppressed(feature)),
                depth: 0,
            });
        }
    }

    rows
}

/// The typed tree model, stored in the egui context store by this panel's
/// copy only. It owns the live document plus the UI-B2 journal that records
/// and restores every rename/delete.
#[derive(Debug, Clone, Default)]
struct TreeModel {
    /// The live model document (`None` only if opening/import failed).
    document: Option<oep_import::Document>,
    /// The routed, undoable edit history over that document (UI-B2 journal +
    /// bounded canonical snapshot store).
    history: doc_io::EditHistory,
    /// The selected row's document id (the shared selection), empty = none.
    selection: Option<String>,
    /// The row id whose inline rename field is open, if any.
    renaming: Option<String>,
    /// Current draft text of the open rename field.
    rename_draft: String,
    /// The row id awaiting delete confirmation, if any.
    confirm_delete: Option<String>,
    /// Whether the model has been seeded from the source document.
    seeded: bool,
}

fn read_model(ctx: &egui::Context) -> TreeModel {
    ctx.data(|d| d.get_temp(egui::Id::unique(TREE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_model(ctx: &egui::Context, model: &TreeModel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(TREE_STATE_KEY), model.clone()));
}

/// Seed the live document on the first frame: the canonical `.oepdoc` JSON a
/// test placed under [`TREE_SEED_KEY`] if any (equivalent to opening that
/// document), else the committed demo STEP through the app's own import path.
/// A failed open leaves a `None` document and an honest empty tree.
fn seed_model(ctx: &egui::Context, model: &mut TreeModel) {
    let seeded_json: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(TREE_SEED_KEY)));
    if let Some(json) = seeded_json {
        if let Ok(doc) = oep_import::Document::from_json_pretty(&json) {
            model.history.clear();
            model.document = Some(doc);
            model.seeded = true;
            return;
        }
    }
    if let Some(path) = doc_io::demo_step_path() {
        model.document = doc_io::import_step(&path).ok();
    }
    model.seeded = true;
}

/// Find the live row with document id `id` (selection/confirm resolution).
fn label_of<'a>(rows: &'a [Row], id: &str) -> Option<&'a Row> {
    rows.iter().find(|r| r.id == id)
}

// ---------------------------------------------------------------------------
// UI-C4 UI-ROLLBACK: suppression marks and the roll bar (document state)
// ---------------------------------------------------------------------------
//
// Both suppression and roll are DOCUMENT state, carried in the affected
// feature's `parameters` map (`suppressed` / `roll` keys) - the same
// deterministic canonical channels UI-C2's parameter editor and UI-C3's
// variable table use. Because they live in the `.oepdoc` JSON, a saved and
// reopened document replays exactly the same suppression marks and roll
// position (`doc_io::write_oepdoc` -> `read_oepdoc` round-trips them), which
// the acceptance harness proves by re-seeding the panel's own canonical JSON.
//
// - Suppress: `parameters["suppressed"] = true` marks the feature suppressed;
//   the row renders a suppression mark and the document hash moves (the
//   canonical JSON changed). The count feeds the status bar.
// - Roll: `parameters["roll"] = true` on the feature the tree is rolled back
//   to. Everything created after it (higher index in `document.features`,
//   i.e. creation order) is DOWNSTREAM and is hidden: the roll bar shows the
//   position and the hidden count, and the rows re-derive live every frame
//   (the established "rows come from the live document, never a cached copy"
//   contract), so no stale geometry can outlive the edit.

/// True when `feature` carries the suppression mark.
fn feature_suppressed(feature: &oep_import::Feature) -> bool {
    feature
        .parameters
        .get(PARAM_SUPPRESSED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// True when `feature` is the current roll-back position (the roll point).
fn feature_is_roll_point(feature: &oep_import::Feature) -> bool {
    feature
        .parameters
        .get(PARAM_ROLL)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// The roll point, as `(feature_id, creation_index)`: the one feature marked
/// `roll` and its position in `document.features` (creation order).
fn roll_point(document: &oep_import::Document) -> Option<(String, usize)> {
    document
        .features
        .iter()
        .enumerate()
        .find(|(_, f)| feature_is_roll_point(f))
        .map(|(i, f)| (f.id.clone(), i))
}

/// Whether `feature_id` is downstream of the roll point (created after it) and
/// therefore hidden. `None` roll = nothing hidden.
fn is_rolled_hidden(document: &oep_import::Document, feature_id: &str) -> bool {
    let Some((_, roll_index)) = roll_point(document) else {
        return false;
    };
    document
        .features
        .iter()
        .position(|f| f.id == feature_id)
        .map(|i| i > roll_index)
        .unwrap_or(false)
}

/// Count of suppressed features in the document (the status-bar number).
fn suppressed_count(document: &oep_import::Document) -> usize {
    document
        .features
        .iter()
        .filter(|f| feature_suppressed(f))
        .count()
}

/// Count of features hidden because they were created after the roll point.
fn rolled_count(document: &oep_import::Document) -> usize {
    match roll_point(document) {
        Some((_, ri)) => document.features.len().saturating_sub(ri + 1),
        None => 0,
    }
}

/// Mark `row.feature` suppressed or restore it, in the live document. Runs
/// INSIDE [`doc_io::EditHistory::apply`] (the journal seam) like the other
/// helpers, so one suppress/restore is one undoable command.
fn apply_suppress(document: &mut oep_import::Document, row: &Row, suppress: bool) {
    if row.kind != RowKind::Feature {
        return;
    }
    if let Some(feature) = document.features.iter_mut().find(|f| f.id == row.id) {
        if suppress {
            feature
                .parameters
                .insert(PARAM_SUPPRESSED.to_string(), true.into());
        } else {
            feature.parameters.remove(PARAM_SUPPRESSED);
        }
    }
}

/// Roll the tree back to `feature_id`: clear every existing roll mark, then
/// mark that feature as the roll point. Downstream features (created after it)
/// become hidden on the next frame's row derivation.
fn apply_roll_to(document: &mut oep_import::Document, feature_id: &str) {
    for f in &mut document.features {
        f.parameters.remove(PARAM_ROLL);
    }
    if let Some(feature) = document.features.iter_mut().find(|f| f.id == feature_id) {
        feature
            .parameters
            .insert(PARAM_ROLL.to_string(), true.into());
    }
}

/// Clear the roll point entirely: every downstream feature becomes visible
/// again on the next frame.
fn apply_unroll(document: &mut oep_import::Document) {
    for f in &mut document.features {
        f.parameters.remove(PARAM_ROLL);
    }
}

/// Rename the entity named by `row` to `new_name`, in the live document.
///
/// Runs INSIDE the [`doc_io::EditHistory::apply`] closure - the UI-B2 journal
/// seam - so the rename is one journaled command, undoable like the delete.
fn apply_rename(document: &mut oep_import::Document, row: &Row, new_name: &str) {
    match row.kind {
        RowKind::Part => {
            if let Some(part) = document.parts.iter_mut().find(|p| p.id == row.id) {
                part.name = new_name.to_string();
            }
        }
        RowKind::Body => {
            if let Some(body) = document.bodies.iter_mut().find(|b| b.id == row.id) {
                body.name = new_name.to_string();
            }
        }
        RowKind::Feature => {
            if let Some(feature) = document.features.iter_mut().find(|f| f.id == row.id) {
                feature.name = new_name.to_string();
            }
        }
    }
}

/// Delete the entity named by `row` and clean its owner references, in the
/// live document. Runs inside the journal seam like [`apply_rename`].
fn apply_delete(document: &mut oep_import::Document, row: &Row) {
    match row.kind {
        RowKind::Feature => {
            document.features.retain(|f| f.id != row.id);
            for part in &mut document.parts {
                part.feature_ids.retain(|id| id != &row.id);
            }
            for body in &mut document.bodies {
                body.feature_ids.retain(|id| id != &row.id);
            }
        }
        RowKind::Body => {
            document.bodies.retain(|b| b.id != row.id);
            for part in &mut document.parts {
                part.body_ids.retain(|id| id != &row.id);
            }
        }
        RowKind::Part => {
            document.parts.retain(|p| p.id != row.id);
        }
    }
}

/// One UI gesture collected during the frame, applied to the model afterwards
/// (immediate-mode: the panel paints first, then applies one journaled edit).
enum TreeAction {
    None,
    Select,
    BeginRename(String),
    CancelRename,
    CommitRename,
    SetConfirm(String),
    CancelConfirm,
    DoDelete,
    Undo,
    /// Toggle suppression on the selected feature row (true = suppress).
    ToggleSuppress(bool),
    /// Roll the tree back to the selected feature.
    RollTo,
    /// Clear the roll point (everything visible again).
    Unroll,
}

/// Render one selectable row; returns `true` when it was clicked.
fn row_ui(ui: &mut egui::Ui, row: &Row, selected: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(DEPTH_INDENT * row.depth as f32);
        if ui.selectable_label(selected, &row.label).clicked() {
            clicked = true;
        }
    });
    clicked
}

/// Draw the model tree panel content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    ui.label(theme::heading(theme::mode_of(ui), "Model Tree").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    // 1. Header: the UI-A11 truthfulness contract, kept verbatim.
    let header_open = match tree_state(ui.ctx()) {
        TreeState::Empty => {
            ui.label(egui::RichText::new("No model open").weak().italics());
            false
        }
        TreeState::Open {
            file_name,
            bodies,
            features,
        } => {
            ui.label(theme::heading(theme::mode_of(ui), &file_name));
            let body_word = if bodies == 1 { "body" } else { "bodies" };
            let feature_word = if features == 1 { "feature" } else { "features" };
            ui.label(format!("{bodies} {body_word}, {features} {feature_word}"));
            // The wave-C note the frozen tree_empty_state contract locks.
            ui.label(
                egui::RichText::new("Feature tree arrives in wave C (UI-C1)")
                    .weak()
                    .italics(),
            );
            true
        }
    };

    // 2. Tree model state (seeded once from the live document source).
    let mut model = read_model(ui.ctx());
    if !model.seeded {
        seed_model(ui.ctx(), &mut model);
    }

    // 3. Live rows from the document.
    let mut rows = rows_of(&model.document);

    let mut action = TreeAction::None;

    // 4. Rows only render while a document is open (an empty session keeps the
    //    honest header and no fabricated rows).
    if header_open && !rows.is_empty() {
        ui.add_space(4.0);
        let mut clicked: Option<String> = None;
        // UI-C4: the row, if any, the tree is rolled back to (draws the roll
        // bar right under it).
        let roll_mark: Option<String> = model
            .document
            .as_ref()
            .and_then(roll_point)
            .map(|(id, _)| id);
        for row in &rows {
            let selected = model.selection.as_deref() == Some(row.id.as_str());
            if row_ui(ui, row, selected) {
                clicked = Some(row.id.clone());
            }
            if roll_mark.as_deref() == Some(row.id.as_str()) {
                // The roll bar: an honest accessible marker under the roll
                // point, showing the tree is rolled back here; downstream
                // features (created after it) are hidden until Unroll. Painted
                // as real labels so screen readers and the AccessKit
                // assertions see the state, not just pixels.
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "roll bar · {} downstream hidden",
                        model.document.as_ref().map(rolled_count).unwrap_or(0)
                    ))
                    .weak()
                    .italics(),
                );
            }
        }
        if let Some(id) = clicked {
            // A fresh click closes a pending delete confirmation; selecting the
            // row that is already being renamed keeps its rename field open.
            model.confirm_delete = None;
            model.selection = Some(id);
            action = TreeAction::Select;
        }
        if model.renaming.is_some() && model.renaming != model.selection {
            model.renaming = None;
            model.rename_draft.clear();
        }

        // 5. Action toolbar: Rename / Delete, the UI-C4 Suppress / Roll
        //    controls (feature rows only), then Undo (only when there is an
        //    edit to step back).
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let selected = model.selection.clone();
            let idle = model.renaming.is_none() && model.confirm_delete.is_none();
            if selected.is_some() && idle {
                let sel = selected.clone().unwrap_or_default();
                let sel_is_feature = label_of(&rows, &sel)
                    .map(|r| r.kind == RowKind::Feature)
                    .unwrap_or(false);
                let sel_suppressed = model
                    .document
                    .as_ref()
                    .and_then(|d| d.features.iter().find(|f| f.id == sel))
                    .map(feature_suppressed)
                    .unwrap_or(false);
                if ui.button("Rename").clicked() {
                    action = TreeAction::BeginRename(selected.clone().unwrap_or_default());
                }
                if ui.button("Delete").clicked() {
                    action = TreeAction::SetConfirm(selected.unwrap_or_default());
                }
                if sel_is_feature {
                    let label = if sel_suppressed {
                        "Restore"
                    } else {
                        "Suppress"
                    };
                    if ui.button(label).clicked() {
                        action = TreeAction::ToggleSuppress(!sel_suppressed);
                    }
                    if ui.button("Roll to here").clicked() {
                        action = TreeAction::RollTo;
                    }
                }
            }
            // Only offer Unroll when a roll point actually exists.
            let has_roll = model.document.as_ref().and_then(roll_point).is_some();
            if has_roll && idle && ui.button("Unroll").clicked() {
                action = TreeAction::Unroll;
            }
            if model.history.can_undo() && ui.button("Undo").clicked() {
                action = TreeAction::Undo;
            }
        });

        // 6. Inline rename field (only for the selected row).
        if model.renaming.is_some() && model.renaming == model.selection {
            let mut enter = false;
            if !model.rename_draft.trim().is_empty() {
                enter = ui
                    .ctx()
                    .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            }
            let mut commit = false;
            ui.horizontal_wrapped(|ui| {
                let edit = egui::TextEdit::singleline(&mut model.rename_draft)
                    .hint_text("new name")
                    .desired_width(RENAME_FIELD_WIDTH)
                    .id(egui::Id::unique("ui_c1_tree_rename_field"));
                ui.add(edit);
                let applyable = !model.rename_draft.trim().is_empty();
                if ui.button("Apply").clicked() && applyable {
                    commit = true;
                }
                if ui.button("Cancel").clicked() {
                    action = TreeAction::CancelRename;
                }
            });
            if enter {
                commit = true;
            }
            if commit {
                action = TreeAction::CommitRename;
            }
        }

        // 7. Delete confirmation (only for the confirmed row, still present).
        if let Some(confirm_id) = model.confirm_delete.clone() {
            if let Some(row) = label_of(&rows, &confirm_id) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("Delete {} ?", row.label)).weak());
                    if ui.button("Confirm").clicked() {
                        action = TreeAction::DoDelete;
                    }
                    if ui.button("Cancel").clicked() {
                        action = TreeAction::CancelConfirm;
                    }
                });
            } else {
                model.confirm_delete = None;
            }
        }
    }

    // 8. Apply the collected gesture. Document edits happen ONLY inside
    //    `EditHistory::apply` (the UI-B2 journal seam), so every rename and
    //    delete is one journaled command and undo restores the exact hash.
    let mut doc_changed = false;
    match action {
        TreeAction::None | TreeAction::Select => {}
        TreeAction::BeginRename(id) => {
            model.rename_draft.clear();
            model.renaming = Some(id);
        }
        TreeAction::CancelRename => {
            model.renaming = None;
            model.rename_draft.clear();
        }
        TreeAction::CommitRename => {
            // The rename target and its live row are re-resolved at commit
            // time; an empty draft or a closed field is an honest no-op.
            if let Some(id) = model.renaming.clone() {
                let new_name = model.rename_draft.trim().to_string();
                if !new_name.is_empty() {
                    let target = rows_of(&model.document);
                    if let Some(row) = label_of(&target, &id) {
                        let row = row.clone();
                        if let Some(document) = model.document.as_mut() {
                            model.history.apply(document, RENAME_TOKEN, None, |d| {
                                apply_rename(d, &row, &new_name);
                            });
                            doc_changed = true;
                        }
                    }
                }
            }
            model.renaming = None;
            model.rename_draft.clear();
        }
        TreeAction::SetConfirm(id) => {
            model.confirm_delete = Some(id);
        }
        TreeAction::CancelConfirm => {
            model.confirm_delete = None;
        }
        TreeAction::DoDelete => {
            if let Some(id) = model.confirm_delete.clone() {
                if let Some(row) = label_of(&rows, &id) {
                    let row = row.clone();
                    if let Some(document) = model.document.as_mut() {
                        model.history.apply(document, DELETE_TOKEN, None, |d| {
                            apply_delete(d, &row);
                        });
                        doc_changed = true;
                    }
                }
            }
            model.confirm_delete = None;
            model.selection = None;
        }
        TreeAction::Undo => {
            if let Some(document) = model.document.as_mut() {
                doc_changed = model.history.undo(document).is_some();
            }
            model.selection = None;
            model.confirm_delete = None;
        }
        TreeAction::ToggleSuppress(suppress) => {
            let id = model.selection.clone();
            // Resolve the live row BEFORE the mutable document borrow (rows
            // derive from a shared read; the journal borrow comes after).
            let target = rows_of(&model.document);
            let row = id.as_deref().and_then(|id| label_of(&target, id)).cloned();
            if let (Some(row), Some(document)) = (row, model.document.as_mut()) {
                model.history.apply(document, SUPPRESS_TOKEN, None, |d| {
                    apply_suppress(d, &row, suppress);
                });
                doc_changed = true;
            }
        }
        TreeAction::RollTo => {
            let id = model.selection.clone();
            if let Some(id) = id {
                if let Some(document) = model.document.as_mut() {
                    model.history.apply(document, ROLL_TOKEN, None, |d| {
                        apply_roll_to(d, &id);
                    });
                    doc_changed = true;
                }
            }
        }
        TreeAction::Unroll => {
            if let Some(document) = model.document.as_mut() {
                model.history.apply(document, ROLL_TOKEN, None, |d| {
                    apply_unroll(d);
                });
                doc_changed = true;
            }
        }
    }

    // Re-derive rows after a document edit so this frame's rendered state and
    // the published report agree. A selection that pointed at a now-hidden
    // (rolled-back) row is honestly cleared rather than left dangling.
    if doc_changed {
        rows = rows_of(&model.document);
        if let Some(sel) = model.selection.clone() {
            if label_of(&rows, &sel).is_none() {
                model.selection = None;
            }
        }
    }

    // 9. Publish the shared selection id (the one id viewport / properties /
    //    status read), the report oracle and the status-bar selection field.
    let sel_id = model.selection.clone().unwrap_or_default();
    ui.ctx()
        .data_mut(|d| d.insert_temp::<String>(egui::Id::unique(TREE_SELECTION_KEY), sel_id));
    app.status.selection_count = if model.selection.is_some() { 1 } else { 0 };
    // UI-C4: the status bar reports the honest suppression and roll-back
    // counts (document-state-derived every frame, never cached).
    app.suppressed = model.document.as_ref().map(suppressed_count).unwrap_or(0);
    app.rolled = model.document.as_ref().map(rolled_count).unwrap_or(0);

    write_model(ui.ctx(), &model);
    publish_report(ui.ctx(), &model, &rows);

    // 10. Sketch node (UI-B3 UI-SKETCH-ENTER): while sketch mode is active the
    //     tree names the active datum plane. Same-frame oracle read, kept
    //     verbatim for the frozen sketch_enter contract.
    if let Some(plane_label) = active_sketch_plane(ui.ctx()) {
        ui.add_space(10.0);
        ui.label(theme::heading(theme::mode_of(ui), "Sketch"));
        ui.label(plane_label);
        ui.label(
            egui::RichText::new("Sketch layer on · Esc exits sketch")
                .weak()
                .italics(),
        );
    }
}

/// Publish the plain-`String` report oracle with the live tree truth.
fn publish_report(ctx: &egui::Context, model: &TreeModel, rows: &[Row]) {
    let doc = model.document.as_ref();
    let (parts, bodies, features) = doc
        .map(|d| (d.parts.len(), d.bodies.len(), d.features.len()))
        .unwrap_or((0, 0, 0));
    let sketch = doc
        .map(|d| d.features.iter().filter(|f| f.kind == "Sketch").count())
        .unwrap_or(0);
    let varset = doc
        .map(|d| {
            d.features
                .iter()
                .filter(|f| f.kind == "VariableSet")
                .count()
        })
        .unwrap_or(0);
    let hash = model
        .document
        .as_ref()
        .map(doc_io::document_hash)
        .unwrap_or(0);
    let sel = model.selection.clone().unwrap_or_else(|| "-".to_string());
    let renaming = model.renaming.clone().unwrap_or_else(|| "-".to_string());
    let confirm = model
        .confirm_delete
        .clone()
        .unwrap_or_else(|| "-".to_string());
    // UI-C4: honest suppression / roll fields (UI-C2/C3 oracle style).
    let suppressed = doc.map(suppressed_count).unwrap_or(0);
    let rolled = doc.map(rolled_count).unwrap_or(0);
    let roll = doc
        .and_then(roll_point)
        .map(|(id, _)| id)
        .unwrap_or_else(|| "-".to_string());
    let report = format!(
        "open={}\nparts={parts}\nbodies={bodies}\nfeatures={features}\nsketch={sketch}\nvarset={varset}\nrows={}\nhash={hash}\nsel={sel}\nrenaming={renaming}\nconfirm={confirm}\nsuppressed={suppressed}\nrolled={rolled}\nroll={roll}\n",
        model.document.is_some() as u8,
        rows.len(),
    );
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(TREE_REPORT_KEY), report));

    // UI-C4 round-trip: publish the exact canonical `.oepdoc` JSON of the live
    // document (what `doc_io::write_oepdoc` would write). A test can re-seed
    // this under [`TREE_SEED_KEY`] (what `read_oepdoc` would hand back after a
    // reopen) and the suppression/roll state must replay identically.
    if let Some(document) = &model.document {
        if let Ok(json) = document.to_json_pretty() {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(TREE_DOC_JSON_KEY), json);
            });
        }
    }
}
