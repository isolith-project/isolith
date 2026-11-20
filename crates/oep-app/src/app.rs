//! The eframe application root for the Isolith editor UI (wave A).
//!
//! The main window is a stable shell of five named regions - model tree,
//! viewport, properties, status bar and command palette host - laid out from a
//! persisted [`config::ShellLayout`] so the sizes survive a restart. Each
//! region is rendered by one module in `panels/`; palette.rs and doc.rs are
//! compiling stubs this card leaves as the seam for UI-A6 and UI-A7, so those
//! cards own one file each without touching this file.
//!
//! eframe 0.37 calls the per-frame [`eframe::App::ui`] for every repaint of
//! the root viewport, so the smoke frame budget is counted there - never via
//! wall-clock sleeps.
//!
//! The shell renders inside a single [`egui::CentralPanel`]: each native egui
//! panel registers as an AccessKit `Pane`, and UI-A2's harness asserts exactly
//! one Pane, so the five regions are labelled child Uis with an explicit
//! [`AccessKit`] `GenericContainer` role and explicit bounds rather than five
//! native panels.

// `mod panels;` names `src/panels/mod.rs` (relative to this file's directory,
// per the wave file map), making `panels` a child module of `app`; the panel
// files address `App` as `crate::app::App`.
#[path = "panels/mod.rs"]
mod panels;

use eframe::egui;
use egui::accesskit::Role;

use crate::config;

use std::io::Write;

/// Root application state.
pub struct App {
    /// True when launched with `--smoke`: run a bounded number of frames then
    /// request close.
    smoke: bool,
    /// True once the close request has been issued, so the smoke line is
    /// printed exactly once.
    closing: bool,
    /// Real frames counted inside [`eframe::App::ui`].
    frames: u32,
    /// Persisted five-region shell geometry (model tree / viewport /
    /// properties / status bar / command palette host).
    layout: config::ShellLayout,
    /// Status bar fields (document name, selection count, mode), stubbed to
    /// defaults now and wired to live state by later cards.
    status: panels::status::Status,
}

impl App {
    /// Create the application root, loading persisted shell geometry.
    pub fn new(smoke: bool) -> Self {
        let layout = config::ShellLayout::load_from(&config::layout_path()).unwrap_or_default();
        Self {
            smoke,
            closing: false,
            frames: 0,
            layout,
            status: Default::default(),
        }
    }

    /// Persist the current shell geometry so a restart comes back to the same
    /// sizes. Best-effort: a missing config directory is not created by the
    /// app on its own; whoever owns bootstrapping the config dir calls this
    /// after a size change.
    pub fn persist_layout(&self) {
        let path = config::layout_path();
        if let Some(parent) = path.parent() {
            if parent.exists() {
                let _ = self.layout.save_to(&path);
            }
        }
    }

    /// Draw the five-region shell inside the already-open central panel.
    fn show_shell(&mut self, ui: &mut egui::Ui) {
        let window = ui.available_rect_before_wrap();
        let region = self.layout.regions(window);

        // Top strip: command palette host. The stub `palette::show` is the
        // seam UI-A6 fills.
        self.region(
            ui,
            "command_palette_host",
            "command palette host",
            region.palette_host,
            |ui, app| {
                ui.label(
                    egui::RichText::new("Command Palette Host")
                        .strong()
                        .size(13.0),
                );
                panels::palette::show(ui.ctx(), app);
            },
        );

        // Left column: model tree.
        self.region(
            ui,
            "model_tree",
            "model tree",
            region.model_tree,
            panels::tree::show,
        );

        // Centre column: viewport.
        self.region(
            ui,
            "viewport",
            "viewport",
            region.viewport,
            panels::viewport::show,
        );

        // Right column: properties.
        self.region(
            ui,
            "properties",
            "properties",
            region.properties,
            panels::properties::show,
        );

        // Bottom strip: status bar.
        self.region(
            ui,
            "status_bar",
            "status bar",
            region.status_bar,
            |ui, app| panels::status::show(ui, &app.status),
        );
    }

    /// Lay out one shell region: a labelled child Ui at exactly `rect`.
    ///
    /// Keeping each region a child Ui (not a native egui panel) preserves the
    /// single-Pane AccessKit contract UI-A2's harness asserts. The node gets a
    /// stable accessible name and, because egui only bounds focusable widget
    /// nodes automatically, an explicit bounds set from the region rect so a
    /// test can assert the geometry (no overlap, no clipping) at any window
    /// size.
    fn region<R>(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
        accessible_name: &str,
        rect: egui::Rect,
        add_contents: impl FnOnce(&mut egui::Ui, &mut App) -> R,
    ) {
        let inner = ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(id_salt)
                .max_rect(rect)
                .accessibility_label(accessible_name)
                .accessibility_role(Role::GenericContainer),
            |ui| add_contents(ui, self),
        );
        // Explicit node bounds: `fill_accesskit_node_common` only runs for
        // focusable widgets, so without this the region node has no geometry
        // for tests (or screen readers) to inspect.
        ui.ctx().accesskit_node_builder(inner.response.id, |node| {
            node.set_bounds(egui::accesskit::Rect {
                x0: rect.min.x.into(),
                y0: rect.min.y.into(),
                x1: rect.max.x.into(),
                y1: rect.max.y.into(),
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.smoke && !self.closing {
            // Count frames in the per-frame callback; no sleep-based timing.
            // Keep requesting repaints so the idle winit event loop issues a
            // real frame each pass instead of waiting for input; the counter
            // then advances deterministically to SMOKE_FRAMES.
            self.frames += 1;
            if self.frames >= config::SMOKE_FRAMES {
                self.closing = true;
                // Flush so the smoke line survives a captured (piped) stdout.
                println!("frames={} smoke ok", self.frames);
                let _ = std::io::stdout().flush();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ui.ctx().request_repaint();
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            self.show_shell(ui);
        });

        // The doc lifecycle seam (UI-A7) hangs off the ctx/app shape app.rs
        // already calls; it draws nothing until that card fills it.
        panels::doc::show(ui.ctx(), self);
    }

    /// Called once on shutdown (real native run): persist the shell sizes so a
    /// restart comes back to the same layout. Best-effort and only when the
    /// config directory exists, so tests (which run under kittest and never
    /// reach a real shutdown) cannot scatter files across the host.
    /// This is the non-glow `eframe::App::on_exit` variant (oep-app builds
    /// eframe with its default backends; `glow` is not a Cargo feature here).
    fn on_exit(&mut self) {
        if let Some(parent) = config::layout_path().parent() {
            if parent.exists() {
                self.persist_layout();
            }
        }
    }
}
