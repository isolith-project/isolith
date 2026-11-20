//! The eframe application root for the Isolith editor UI (wave A, card
//! UI-A1).
//!
//! This card only wires the seam: one window, a deterministic title bar, and a
//! headless `--smoke` mode that runs a bounded number of frames inside the
//! real event loop and exits 0. Later wave-A cards build panels and widgets on
//! top of this root.
//!
//! eframe 0.37 calls the per-frame [`eframe::App::ui`] for every repaint of
//! the root viewport, so the smoke frame budget is counted there - never via
//! wall-clock sleeps.

use eframe::egui;

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
}

impl App {
    /// Create the application root.
    pub fn new(smoke: bool) -> Self {
        Self {
            smoke,
            closing: false,
            frames: 0,
        }
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

        egui::CentralPanel::default().show(ui, |_ui| {
            // Wave A: the shell is intentionally empty. UI-A4 owns the panel
            // layout; UI-A5 owns the viewport content.
        });
    }
}
