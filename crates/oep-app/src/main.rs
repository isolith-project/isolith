//! Isolith editor UI entry point (editor parity program, wave A, card UI-A1;
//! wave D, card UI-D1 adds the renderer selection).
//!
//! Behaviour: the `isolith` binary opens one 1280x800 window whose title bar
//! names the product and version, and exits cleanly when the window is closed.
//! `--smoke` runs the real event loop for a bounded number of frames on a
//! headless display, then requests close and returns exit code 0.
//!
//! Renderer selection (UI-D1): eframe renders egui with its wgpu backend when
//! a GPU adapter is reachable (and `ISOLITH_GPU` is not `off`), and with the
//! glow backend otherwise - an honest fallback, never a hard error. When wgpu
//! is active, the viewport's `GpuBackend` SHARES the exact device eframe
//! created for egui (see `oep_viewer::gpu::device::register_egui_device`), so
//! the editor and its viewport are one GPU runtime, not two.
//!
//! No dependency beyond eframe/egui is introduced here: argument parsing is
//! plain `std::env` scanning for the single known flag.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod theme;

use eframe::egui;
use oep_viewer::gpu::device;

fn main() -> eframe::Result<()> {
    let smoke = std::env::args().any(|arg| arg == "--smoke");

    // UI-D1: pick the eframe renderer from device availability (the same
    // predicate the viewport's backend selector uses, so the label below and
    // the actual viewport backend agree by construction).
    let renderer_is_wgpu = device::adapter_available();
    let renderer = if renderer_is_wgpu {
        eframe::Renderer::Wgpu
    } else {
        eframe::Renderer::Glow
    };

    // The smoke line must name the backend used (card acceptance): the eframe
    // renderer plus the viewport backend the seam is about to select.
    if smoke {
        let backend = if renderer_is_wgpu { "gpu" } else { "cpu" };
        println!(
            "isolith smoke: renderer={} viewport-backend={} (set ISOLITH_GPU=off to force cpu)",
            renderer, backend
        );
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(config::WINDOW_SIZE),
        renderer,
        ..Default::default()
    };

    let app_name = config::title();
    eframe::run_native(
        &app_name,
        options,
        Box::new(move |cc| {
            // UI-D1: when eframe runs on wgpu, hand its device to the viewport
            // backend so the GPU viewport shares egui's exact device/queue.
            if let Some(render_state) = cc.wgpu_render_state.as_ref() {
                device::register_egui_device(
                    render_state.device.clone(),
                    render_state.queue.clone(),
                );
            }
            // Install the semantic tokens as the egui style/visuals at
            // startup (wave A, card UI-A3). Light is the default mode.
            theme::install(&cc.egui_ctx, oep_ui_theme::Mode::default());
            Ok(Box::new(app::App::new(smoke)))
        }),
    )
}
