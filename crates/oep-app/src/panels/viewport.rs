//! Viewport panel (wave A, card UI-A5): the live frame behind the backend seam.
//!
//! Replaces the UI-A4 stub: the panel now shows the committed demo scene as a
//! deterministic CPU-rendered frame, navigable with the mouse (orbit = left
//! drag, pan = middle drag or shift+drag, zoom = wheel), all through the
//! [`oep_viewer::ViewportBackend`] trait so Wave D swaps in a GPU backend
//! without touching this file's structure.
//!
//! Backend ownership: `App` is off-limits on this card, so the [`CpuBackend`]
//! lives in the egui context's per-widget data, keyed by a stable id that the
//! kittest harness (`--test viewport`) reads back with the same id. All work
//! happens inside one [`egui::Context::data_mut`] scope because this egui
//! version cannot return `&mut` out of that closure; the frame hands out owned
//! values (texture image, snapshot, status text).

use eframe::egui;

use oep_viewer::{Camera, ViewportBackend};

use crate::app::App;

/// Context-data key under which the backend lives (and the scalar snapshot is
/// published) in the egui per-context store. Shared with
/// `crates/oep-app/tests/viewport.rs`, which derives the same id value.
pub const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";

/// Orbit sensitivity: radians of yaw per point (logical pixel) of left-drag.
pub const ORBIT_RAD_PER_PX: f64 = 0.005;

/// Zoom sensitivity: distance scales by `1 + scroll_delta * this`.
pub const ZOOM_FACTOR_PER_SCROLL_POINT: f64 = 0.001;

/// World units per pixel for a pan at the given camera and panel pixel height.
pub fn pan_world_per_pixel(cam: &Camera, height_px: f64) -> f64 {
    let half_fov = cam.fov_y * 0.5;
    (2.0 * cam.distance * half_fov.tan()) / height_px.max(1.0)
}

/// A plain-scalar snapshot of the viewport state, published by [`show`] so the
/// harness can assert camera deltas and the frame-hash oracle.
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
pub type ViewportSnapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// Read the last published [`ViewportSnapshot`], if any.
///
/// Consumed only by `crates/oep-app/tests/viewport.rs` (which includes this
/// module by `#[path]`); the binary build never calls it, so it is allowed to
/// be dead code in that compile alone.
#[allow(dead_code)]
pub fn snapshot(ctx: &egui::Context) -> Option<ViewportSnapshot> {
    ctx.data(|d| d.get_temp::<ViewportSnapshot>(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

/// Draw the viewport panel: the live navigable frame.
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    let state_id = egui::Id::unique(VIEWPORT_STATE_KEY);
    let rect = ui.available_rect_before_wrap();
    let scale = ui.ctx().pixels_per_point();
    let px = (rect.width() * scale).round().max(2.0) as u32;
    let py = (rect.height() * scale).round().max(2.0) as u32;

    let (primary, middle, shift, delta, scroll, zoom, hovered, _response) = {
        let (primary, middle, shift, delta, scroll, zoom) = ui.ctx().input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.modifiers.shift,
                i.pointer.delta(),
                i.smooth_scroll_delta.y,
                i.zoom_delta(),
            )
        });
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        (
            primary,
            middle,
            shift,
            delta,
            scroll,
            zoom,
            ui.rect_contains_pointer(rect),
            response,
        )
    };

    // All panel mutation lives inside one context-data scope (no `&mut` escape,
    // this egui version); owned outputs are handed out afterwards.
    //
    // Navigation drives off the RAW button state + the per-frame pointer delta,
    // not egui's click-vs-drag flag: that flag lands one frame after the drag
    // decision, which would blur the exact numeric mapping a kittest drag needs
    // to assert. A press with no movement is a no-op (delta == 0).
    let (image, ms_bucket, action) = ui.ctx().data_mut(|d| {
        let backend: &mut oep_viewer::CpuBackend =
            d.get_temp_mut_or_insert_with(state_id, oep_viewer::CpuBackend::demo);

        // Size the render target to the panel's pixel rect.
        let (cw, ch) = (backend.frame().width, backend.frame().height);
        if (cw, ch) != (px, py) {
            backend.resize(px, py);
        }

        let mut cam = backend.camera();
        let mut action = "Select";

        // Pan: middle drag, or shift+drag (over the panel).
        if (middle || (shift && primary)) && hovered {
            let wpp = pan_world_per_pixel(&cam, py as f64);
            cam.pan(delta.x as f64 * wpp, delta.y as f64 * wpp);
            action = "Pan";
        }
        // Orbit: plain left drag.
        else if primary && hovered && !shift {
            cam.orbit(
                delta.x as f64 * ORBIT_RAD_PER_PX,
                delta.y as f64 * ORBIT_RAD_PER_PX,
            );
            action = "Orbit";
        }
        // Zoom: pinch/zoom event (zoom_delta > 1 zooms in => distance shrinks)
        // or wheel over the panel.
        if hovered && ((zoom - 1.0).abs() > 1e-6 || scroll != 0.0) {
            let factor = if (zoom - 1.0).abs() > 1e-6 {
                1.0 / zoom as f64
            } else {
                1.0 / (1.0 + scroll as f64 * ZOOM_FACTOR_PER_SCROLL_POINT).clamp(0.2, 5.0)
            };
            cam.zoom(factor);
            action = "Zoom";
        }

        if cam != backend.camera() {
            backend.set_camera(cam);
        }

        let cam = backend.camera();
        let frame_hash = backend.frame_hash();
        let frame = backend.frame();
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.data,
        );
        let ms_bucket = ((backend.frame_time_secs() * 1000.0) as u64 / 10) * 10;

        // Publish the scalar snapshot for the harness after the backend borrow
        // has ended (this egui IdTypeMap cannot be borrowed mutably twice).
        let snapshot = (
            cam.yaw,
            cam.pitch,
            cam.distance,
            cam.target.x,
            cam.target.y,
            cam.target.z,
            cam.fov_y,
            frame_hash,
        );
        d.insert_temp::<ViewportSnapshot>(state_id, snapshot);

        (image, ms_bucket, action)
    });

    // Draw the frame into the panel via an egui texture.
    let texture_id = ui.scope_id().with("viewport_frame_texture");
    let texture = ui.ctx().load_texture_cached(
        "isolith_viewport_frame",
        texture_id,
        image,
        egui::TextureOptions::NEAREST,
    );
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter()
        .image(texture.id(), rect, uv, egui::Color32::WHITE);

    // Report the frame time in the status-bar mode field UI-A4 created.
    app.status.mode = format!("{action} · {ms_bucket} ms");
}
