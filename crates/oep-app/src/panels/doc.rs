//! Document lifecycle panel (wave A, card UI-A7 UI-OPEN-SAVE).
//!
//! Fills the UI-A4 stub. Owns the open/save/recent-files half of the document
//! lifecycle and renders its state onto the shell's existing seams - no new
//! shell region and no new Pane:
//!
//! - The **native title bar** is updated with the file name plus a dirty star
//!   (`egui::ViewportCommand::Title`), which is what "title bar shows the file
//!   name plus a dirty star" means for a real eframe window.
//! - The **status bar document field** (A4's `Status::document_name`, stubbed
//!   "Untitled" until this card) shows the same file name + star, so the
//!   deterministic screenshot records it too.
//! - The **recent list** persists in the config directory `crate::config`
//!   resolves (`recent.conf` next to `layout.conf`), so a restart restores it.
//!   Ctrl+R toggles a compact overlay listing recent `.oepdoc` files;
//!   ArrowUp/Down selects, Enter opens the selected file, Esc cancels
//!   (document unchanged).
//!
//! Open paths, all through [`doc_io`] (never a new parser):
//! - The committed demo STEP is opened automatically once on first frame
//!   (`doc_io::import_step`, which goes through `oep_import`). Ctrl+O
//!   re-imports it. An import is a *new unsaved* in-memory model, so it is
//!   honestly dirty (star on) until saved as `.oepdoc`.
//! - Ctrl+S writes the canonical `.oepdoc` into the config directory and
//!   pushes it to the recent list. Saving makes the document clean (star off).
//! - Opening a recent `.oepdoc` re-parses that file from disk, so the reopened
//!   document hash is byte-identical to the saved one - the card's "reopen to
//!   identical hash" contract - and it is clean (matches the bytes on disk).
//!
//! Dirty is derived, never stored as a hand-set flag: `dirty == saved_hash !=
//! Some(current_hash)`. Any document mutation that changes the document hash
//! (in this wave: import / save / open of a different file; later waves add
//! editor tools through the same hash comparison) flips the star on, and a
//! save flips it off. A failed open leaves the current document and hash
//! untouched and records the typed reason instead - a never-half-loaded
//! contract.
//!
//! State and the spy: like the palette, all per-app state lives in the egui
//! context data store (never in `app.rs`), and the card's oracle is a plain
//! `String` report under [`DOC_REPORT_KEY`] so the `--test doc_io` target can
//! read the app's real per-context state even though this file is compiled
//! twice under `#[path]` (a `String` TypeId is shared across copies; a private
//! struct is not).

use eframe::egui;

use crate::app::App;
use crate::config;

#[path = "../doc_io.rs"]
mod doc_io;

/// Context-data key under which the panel's document state lives.
pub const DOC_STATE_KEY: &str = "ui_a7_doc_state";
/// Context-data key under which the panel writes its `String` report (the
/// UI-A7 oracle the acceptance test reads back through the same `egui::Context`).
pub const DOC_REPORT_KEY: &str = "ui_a7_doc_report";

/// Max height of the recent overlay's scroll area.
const RECENT_OVERLAY_HEIGHT: f32 = 240.0;
/// Fixed width of the recent overlay.
const RECENT_OVERLAY_WIDTH: f32 = 460.0;

/// Per-app document lifecycle state (context-data resident, like the palette).
#[derive(Clone, Default)]
struct DocState {
    /// Current file name shown in title/status (e.g. `edpart.step`).
    file_name: String,
    /// The in-memory canonical model (`None` before the first open).
    document: Option<oep_import::Document>,
    /// Hash of the in-memory document.
    current_hash: u64,
    /// `Some(hash)` after a save or a `.oepdoc` open; `None` for an unsaved
    /// import. `dirty == saved_hash != Some(current_hash)`.
    saved_hash: Option<u64>,
    /// Recent `.oepdoc` paths, newest first (persisted via [`doc_io`]).
    recent: Vec<std::path::PathBuf>,
    /// Last open/save error text (`None` when the last action succeeded).
    last_error: Option<String>,
    /// The recent-files overlay is currently open.
    recent_open: bool,
    /// Index of the selected recent row while the overlay is open.
    selected: usize,
    /// Title last pushed to the native title bar (avoid re-sending).
    last_title: String,
}

impl DocState {
    /// Whether the in-memory document differs from the last saved `.oepdoc`.
    fn dirty(&self) -> bool {
        self.saved_hash != Some(self.current_hash)
    }

    /// The file name plus a dirty star when dirty (the status/title text).
    fn display_name(&self) -> String {
        let base = if self.file_name.is_empty() {
            "Untitled".to_string()
        } else {
            self.file_name.clone()
        };
        if self.dirty() {
            format!("{base} *")
        } else {
            base
        }
    }

    /// The config directory the panel persists recent state into (the same one
    /// `crate::config::layout_path` names, via its parent).
    fn config_dir() -> std::path::PathBuf {
        config::layout_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    }

    /// Import the committed demo STEP into a fresh (unsaved) document. Honest
    /// failure semantics: on error the current document is left untouched and
    /// the reason is recorded.
    fn open_demo_step(&mut self) {
        let Some(path) = doc_io::demo_step_path() else {
            self.last_error = Some("committed demo STEP not found".to_string());
            return;
        };
        match doc_io::import_step(&path) {
            Ok(document) => {
                let hash = doc_io::document_hash(&document);
                self.document = Some(document);
                self.current_hash = hash;
                self.file_name = file_stem_name(&path);
                // A fresh import is not yet a saved .oepdoc: honestly dirty.
                self.saved_hash = None;
                self.last_error = None;
            }
            Err(e) => {
                // A failed open must not half-load: keep the current document.
                self.last_error = Some(e);
            }
        }
    }

    /// Open an existing `.oepdoc` (a recent entry): the reopened document is
    /// clean (matches the bytes on disk) and its hash equals the saved one.
    fn open_oepdoc(&mut self, path: &std::path::Path) {
        match doc_io::read_oepdoc(path) {
            Ok(document) => {
                let hash = doc_io::document_hash(&document);
                self.document = Some(document);
                self.current_hash = hash;
                self.file_name = file_stem_name(path);
                self.saved_hash = Some(hash);
                let dir = Self::config_dir();
                self.recent = doc_io::push_recent(&dir, path);
                self.recent_open = false;
                self.last_error = None;
            }
            Err(e) => {
                // A failed open must not half-load: keep the current document.
                self.last_error = Some(e);
            }
        }
    }

    /// Write the current document as `.oepdoc` into the config directory and
    /// record it in the recent list.
    fn save_oepdoc(&mut self) {
        let Some(document) = &self.document else {
            self.last_error = Some("no document to save".to_string());
            return;
        };
        let name = if self.file_name.is_empty() {
            "document".to_string()
        } else {
            self.file_name.clone()
        };
        let stem = std::path::Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or(name);
        let path = Self::config_dir().join(format!("{stem}{}", doc_io::OEPDOC_EXTENSION));
        match doc_io::write_oepdoc(&path, document) {
            Ok(()) => {
                self.saved_hash = Some(self.current_hash);
                let dir = Self::config_dir();
                self.recent = doc_io::push_recent(&dir, &path);
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(e),
        }
    }

    /// One compact, machine-readable report line every frame: visible file
    /// name (with star), dirty flag, hashes, error, and recent list.
    fn report(&self) -> String {
        let mut out = format!(
            "file={}\ndirty={}\nhash={}\nsaved={}\n",
            self.display_name(),
            self.dirty(),
            self.current_hash,
            self.saved_hash.map(|h| h.to_string()).unwrap_or_default(),
        );
        if let Some(err) = &self.last_error {
            out.push_str(&format!("error={err}\n"));
        }
        for path in &self.recent {
            out.push_str(&format!("recent={}\n", path.display()));
        }
        out
    }
}

/// The file name of `path` (its last path component), e.g. `edpart.step`.
fn file_stem_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Consume Ctrl/Cmd+`key`, accepting the platform command modifier and a plain
/// Ctrl (so behaviour and tests are identical on every host).
fn consume_shortcut(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key))
        || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key))
}

/// The keys this panel owns, read once per frame before any context-data
/// borrow (the palette's borrow discipline: never nest `input_mut` inside a
/// `data_mut` closure in this egui version).
struct DocKeys {
    open: bool,
    save: bool,
    toggle_recent: bool,
    enter: bool,
    escape: bool,
    arrow_down: bool,
    arrow_up: bool,
}

impl DocKeys {
    fn read(ctx: &egui::Context) -> Self {
        Self {
            open: consume_shortcut(ctx, egui::Key::O),
            save: consume_shortcut(ctx, egui::Key::S),
            toggle_recent: consume_shortcut(ctx, egui::Key::R),
            enter: ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)),
            escape: ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)),
            arrow_down: consume_shortcut(ctx, egui::Key::ArrowDown),
            arrow_up: consume_shortcut(ctx, egui::Key::ArrowUp),
        }
    }
}

fn read_state(ctx: &egui::Context) -> DocState {
    ctx.data(|d| d.get_temp(egui::Id::unique(DOC_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, state: &DocState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(DOC_STATE_KEY), state.clone()));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(DOC_REPORT_KEY), state.report()));
}

/// Draw the recent-files overlay: one row per persisted `.oepdoc`.
fn recent_overlay(ui: &mut egui::Ui, state: &mut DocState, clicked_row: &mut Option<usize>) {
    if state.recent.is_empty() {
        ui.label(egui::RichText::new("No recent files yet - Ctrl+S to save").weak());
        return;
    }
    let mut clicked = None;
    for (i, path) in state.recent.iter().enumerate() {
        let row = ui.selectable_label(i == state.selected, path.display().to_string());
        if row.clicked() {
            clicked = Some(i);
        }
    }
    if let Some(i) = clicked {
        *clicked_row = Some(i);
    }
}

/// Draw the document lifecycle layer (fills the UI-A4 stub body).
///
/// Runs every frame on the shell. Keyboard surface (Ctrl/Cmd):
/// - `O` (re-)opens the committed demo STEP;
/// - `S` saves `.oepdoc` into the config directory (and recent);
/// - `R` toggles the recent overlay (ArrowUp/Down, Enter opens, Esc cancels).
///
/// The native title bar and the status-bar document field both carry the file
/// name plus the dirty star; the report oracle is written every frame.
pub fn show(ctx: &egui::Context, app: &mut App) {
    // 1. Consume all keyboard up-front so state mutation never re-borrows the
    //    egui context (this egui version cannot do that inside a data_*).
    let keys = DocKeys::read(ctx);

    // 2. Mutate a local copy of the state (palette pattern), applying this
    //    frame's actions and the overlay interaction.
    let mut state = read_state(ctx);

    if state.document.is_none() {
        // First frame of a fresh session: auto-open the committed demo STEP
        // and restore the persisted recent list from the config directory.
        state.recent = doc_io::load_recent(&DocState::config_dir());
        state.open_demo_step();
    }
    if keys.open {
        state.open_demo_step();
    }
    if keys.save {
        state.save_oepdoc();
    }
    if keys.toggle_recent {
        state.recent_open = !state.recent_open;
        state.selected = 0;
    }

    if state.recent_open {
        if keys.enter {
            if let Some(path) = state.recent.get(state.selected).cloned() {
                state.open_oepdoc(&path);
            }
        } else if keys.escape {
            state.recent_open = false; // cancel: document unchanged
        } else if keys.arrow_down && !state.recent.is_empty() {
            state.selected = (state.selected + 1).min(state.recent.len() - 1);
        } else if keys.arrow_up && !state.recent.is_empty() {
            state.selected = state.selected.saturating_sub(1);
        }
    }

    // 3. Native title bar reflects file name + dirty star (only when it
    //    changes); status-bar document field shows the same visible label.
    let title = format!("{} - {}", config::APP_NAME, state.display_name());
    if title != state.last_title {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        state.last_title = title.clone();
    }
    app.status.document_name = state.display_name();

    // 4. Write state + report back after all mutations.
    write_state(ctx, &state);

    // 5. Draw the recent overlay (self-contained Area, no new Pane) and apply
    //    a click on it (select + open).
    if state.recent_open {
        let mut clicked_row = None;
        egui::Area::new(egui::Id::unique("ui_a7_recent_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 64.0))
            .constrain(true)
            .fade_in(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(RECENT_OVERLAY_WIDTH);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.label(egui::RichText::new("Recent Documents (Esc to close)").strong());
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .max_height(RECENT_OVERLAY_HEIGHT)
                            .show(ui, |ui| recent_overlay(ui, &mut state, &mut clicked_row));
                    });
                });
            });
        if let Some(i) = clicked_row {
            if let Some(path) = state.recent.get(i).cloned() {
                state.selected = i;
                state.open_oepdoc(&path);
                let title = format!("{} - {}", config::APP_NAME, state.display_name());
                if title != state.last_title {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
                    state.last_title = title.clone();
                }
                app.status.document_name = state.display_name();
                write_state(ctx, &state);
            }
        }
    }
}
