//! Model tree panel (wave A, card UI-A11 UI-TREE-EMPTY-STATE).
//!
//! Renders the left "model tree" region of the editor shell. The real feature
//! tree is UI-C1's card; until then this panel must tell the truth about the
//! document instead of the UI-A4 hardcoded stub's "No model open":
//!
//! - No document open: keep the honest "No model open" line.
//! - Document open: name the document (the same live file name the status bar
//!   carries), report what the document actually contains (body / feature
//!   counts taken from the document itself), and visibly label the placeholder
//!   with a weak italic "Feature tree arrives in wave C (UI-C1)" line rather
//!   than pretending to be a tree. It never fabricates tree rows.
//!
//! Reading the document: this panel does not open files itself and caches no
//! document state. UI-A7 owns the document (`doc_io` behind its panel) and,
//! every frame, publishes a plain `String` oracle under
//! [`super::doc::DOC_REPORT_KEY`] in the egui context store carrying the live
//! display name and the body/feature counts of the current document. Because
//! the oracle is a `String` (a std type identically shared across `#[path]`
//! copies), this panel reads it through the same handle the `--test doc_io`
//! oracle uses and stays on the current frame's truth — open a document and
//! the tree names it with no restart.
//!
//! UI-B3 UI-SKETCH-ENTER: while sketch mode is active the tree also shows a
//! sketch node (the active datum plane). The sketch viewport panel publishes a
//! `String` oracle under [`super::viewport::sketch::SKETCH_REPORT_KEY`] exactly
//! as the doc panel does, so this panel reads the mode and the active plane
//! through the same cross-`#[path]`-copy-safe channel and the node appears on
//! the same frame sketch mode turns on.
//!
//! No raw hex and no `Color32::from_*` here (UI-A3 guard): only the token
//! styles egui already applies (weak / italic), never a literal colour.

use eframe::egui;

use crate::app::App;
use crate::theme;

use super::doc::DOC_REPORT_KEY;

/// Doc-oracle field prefixes (the lines UI-A7's doc panel writes under
/// [`DOC_REPORT_KEY`], which this panel parses).
mod field {
    /// The doc panel's visible display name, e.g. `edpart.step *` (dirty star).
    pub const FILE: &str = "file=";
    /// Body count taken from the current document itself.
    pub const BODIES: &str = "bodies=";
    /// Feature count taken from the current document itself.
    pub const FEATURES: &str = "features=";
}

/// What the tree panel currently shows about the open document.
enum TreeState {
    /// No document open: keep the honest "No model open" line.
    Empty,
    /// A document is open: name it, report its real contents, and label the
    /// feature-tree placeholder as arriving in wave C.
    Open {
        /// The live display name (with dirty star when the doc is dirty).
        file_name: String,
        /// Body count taken from the document itself.
        bodies: usize,
        /// Feature count taken from the document itself.
        features: usize,
    },
}

/// Resolve the current tree-panel state from the doc panel's live oracle.
///
/// Reads only the egui context store every frame — never a file, never cached
/// state — so the panel updates on the same frame the document changes.
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
/// when not in sketch mode.
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

/// Draw the model tree panel content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    ui.label(theme::heading(theme::mode_of(ui), "Model Tree").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    match tree_state(ui.ctx()) {
        TreeState::Empty => {
            ui.label(egui::RichText::new("No model open").weak().italics());
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
            // The real tree is UI-C1's card; label the placeholder rather than
            // hiding it or pretending this is a tree.
            ui.label(
                egui::RichText::new("Feature tree arrives in wave C (UI-C1)")
                    .weak()
                    .italics(),
            );
        }
    }

    // Sketch node (UI-B3 UI-SKETCH-ENTER): while sketch mode is active the
    // tree names the active datum plane. The same-frame oracle read keeps it
    // honest — the node appears exactly when the viewport entered sketch mode
    // and disappears when Escape exits it.
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
