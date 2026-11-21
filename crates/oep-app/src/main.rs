//! Isolith editor UI entry point (editor parity program, wave A, card UI-A1).
//!
//! Behaviour of this card: the `isolith` binary opens one 1280x800 window
//! whose title bar names the product and version, and exits cleanly when the
//! window is closed. `--smoke` runs the real event loop for a bounded number
//! of frames on a headless display, then requests close and returns exit
//! code 0.
//!
//! No dependency beyond eframe/egui is introduced here: argument parsing is
//! plain `std::env` scanning for the single known flag.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod theme;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let smoke = std::env::args().any(|arg| arg == "--smoke");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(config::WINDOW_SIZE),
        ..Default::default()
    };

    let app_name = config::title();
    eframe::run_native(
        &app_name,
        options,
        Box::new(move |cc| {
            // Install the semantic tokens as the egui style/visuals at
            // startup (wave A, card UI-A3). Light is the default mode.
            theme::install(&cc.egui_ctx, oep_ui_theme::Mode::default());
            Ok(Box::new(app::App::new(smoke)))
        }),
    )
}
