//! Viewport panel (wave A, card UI-A5; wave B, card UI-B3 UI-SKETCH-ENTER): the
//! live frame behind the backend seam.
//!
//! Replaces the UI-A4 stub: the panel now shows the committed demo scene as a
//! deterministic CPU-rendered frame, navigable with the mouse (orbit = left
//! drag, pan = middle drag or shift+drag, zoom = wheel), all through the
//! [`oep_viewer::ViewportBackend`] trait so Wave D swaps in a GPU backend
//! without touching this file's structure.
//!
//! UI-B3 adds datum-plane picking to the same panel: a clean primary click
//! (no drag, no modifiers) is resolved through the viewport's existing pick
//! seam ([`oep_viewer::pick::unproject`]) into the nearest [`sketch::DatumPlane`]
//! under the cursor. Picking a plane highlights it (accent outline + centre
//! marker), swings the camera to look normal to it within 1e-6, and turns on
//! the sketch grid layer (an egui overlay projected onto the active plane).
//! Escape leaves sketch mode without touching a document: it is consumed here
//! (the viewport panel runs before the doc panel in the frame) only while a
//! plane is active, then the sketch state is written back idle.
//!
//! Backend ownership: `App` is off-limits on these cards, so the [`CpuBackend`]
//! lives in the egui context's per-widget data, keyed by a stable id that the
//! kittest harnesses (`--test viewport`, `--test sketch_enter`) read back with
//! the same id. All work happens inside one [`egui::Context::data_mut`] scope
//! because this egui version cannot return `&mut` out of that closure; the
//! frame hands out owned values (texture image, snapshot, status text, the
//! pre-projected sketch grid layer).

use eframe::egui;

use oep_viewer::{Camera, ViewportBackend};

use crate::app::App;
use crate::theme;

#[path = "sketch.rs"]
pub mod sketch;

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

/// The demo scene this wave's viewport always renders (the backend is
/// constructed with `CpuBackend::demo`), whose bounding box defines where the
/// datum planes sit.
fn demo_scene() -> oep_viewer::Scene {
    oep_viewer::CpuBackend::demo_scene()
}

/// Draw the viewport panel: the live navigable frame plus (in sketch mode) the
/// active datum plane's highlight and sketch grid layer.
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    let state_id = egui::Id::unique(VIEWPORT_STATE_KEY);
    let rect = ui.available_rect_before_wrap();
    let scale = ui.ctx().pixels_per_point();
    let px = (rect.width() * scale).round().max(2.0) as u32;
    let py = (rect.height() * scale).round().max(2.0) as u32;

    let (primary, middle, shift, delta, scroll, zoom, hovered, response) = {
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

    // Sketch mode: read the current state up front (a read-only context-data
    // borrow, so it never nests inside `data_mut` below).
    let mut sketch_state = sketch::read_state(ui.ctx());

    // Escape leaves sketch mode without touching the document. This panel runs
    // first in the frame, so it consumes the key ONLY while a plane is active
    // and never steals the Escape the doc panel / palette need otherwise.
    if sketch_state.in_sketch() {
        let escape = ui
            .ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape {
            sketch_state.plane = None;
        }
    }

    // A clean primary click (press + release, no drag, no modifiers) picks a
    // datum plane. `response.interact_pointer_pos` is the clicked point; it
    // falls back to the live pointer for robustness.
    let click_px: Option<(f64, f64)> = if response.clicked() && !shift && !middle {
        response
            .interact_pointer_pos()
            .or_else(|| ui.ctx().input(|i| i.pointer.latest_pos()))
            .map(|pos| {
                let cx = ((pos.x - rect.min.x) * scale).max(0.0) as f64;
                let cy = ((pos.y - rect.min.y) * scale).max(0.0) as f64;
                (cx, cy)
            })
    } else {
        None
    };

    // All panel mutation lives inside one context-data scope (no `&mut` escape,
    // this egui version); owned outputs are handed out afterwards.
    //
    // Navigation drives off the RAW button state + the per-frame pointer delta,
    // not egui's click-vs-drag flag: that flag lands one frame after the drag
    // decision, which would blur the exact numeric mapping a kittest drag needs
    // to assert. A press with no movement is a no-op (delta == 0).
    let (image, ms_bucket, action, overlay) = ui.ctx().data_mut(|d| {
        let backend: &mut oep_viewer::CpuBackend =
            d.get_temp_mut_or_insert_with(state_id, oep_viewer::CpuBackend::demo);

        // Size the render target to the panel's pixel rect.
        let (cw, ch) = (backend.frame().width, backend.frame().height);
        if (cw, ch) != (px, py) {
            backend.resize(px, py);
        }

        let mut cam = backend.camera();
        let mut action = "Select".to_owned();

        // The datum planes only need building on the frames that use them (a
        // click to pick, or any frame while the sketch layer is on).
        let scene_planes = (click_px.is_some() || sketch_state.in_sketch()).then(|| {
            let scene = demo_scene();
            sketch::datum_planes(&scene)
        });

        // Plane pick on click, through the existing pick seam (unproject).
        if let (Some((cx, cy)), Some(planes)) = (click_px, scene_planes.as_ref()) {
            let ray = oep_viewer::pick::unproject(&cam, cx, cy, f64::from(px), f64::from(py));
            let f = cam
                .target
                .sub(&cam.eye())
                .normalize()
                .expect("viewpoint camera is never degenerate");
            if let Some(plane) = sketch::pick_plane(planes, &ray, &f) {
                let (yaw, pitch) = sketch::normal_orientation(&plane.normal);
                cam.yaw = yaw;
                cam.pitch = pitch;
                sketch_state.plane = Some(sketch::ActivePlane {
                    id: plane.id.to_string(),
                    label: plane.label.to_string(),
                    normal: (plane.normal.x, plane.normal.y, plane.normal.z),
                });
                action = format!("Sketch {}", plane.label);
            }
        }

        // Pan: middle drag, or shift+drag (over the panel).
        if (middle || (shift && primary)) && hovered {
            let wpp = pan_world_per_pixel(&cam, f64::from(py));
            cam.pan(delta.x as f64 * wpp, delta.y as f64 * wpp);
            action = "Pan".to_owned();
        }
        // Orbit: plain left drag.
        else if primary && hovered && !shift {
            cam.orbit(
                delta.x as f64 * ORBIT_RAD_PER_PX,
                delta.y as f64 * ORBIT_RAD_PER_PX,
            );
            action = "Orbit".to_owned();
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
            action = "Zoom".to_owned();
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

        // Sketch grid layer: project the active plane onto this panel (the
        // pre-projected geometry the painter draws after the backend borrow).
        let overlay = match (&sketch_state.plane, scene_planes.as_ref()) {
            (Some(active), Some(planes)) => planes
                .iter()
                .find(|p| p.id == active.id)
                .and_then(|p| sketch::grid_overlay(p, &cam, rect, scale)),
            _ => None,
        };

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

        (image, ms_bucket, action, overlay)
    });

    // Persist the sketch state (struct + String oracle) after the backend
    // borrow: a second, sequential context-data scope.
    sketch::write_state(ui.ctx(), &sketch_state);

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

    // Sketch grid layer on top of the frame: the active plane's triangle-free
    // outline (accent), the grid lines (border), and the centre pick marker.
    if let Some(overlay) = &overlay {
        let p = oep_ui_theme::Palette::resolve(theme::mode_of(ui));
        let painter = ui.painter();
        painter.add(egui::Shape::convex_polygon(
            overlay.corners.clone(),
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(2.0, p.accent),
        ));
        for (a, b) in &overlay.segments {
            painter.line_segment([*a, *b], egui::Stroke::new(1.0, p.border));
        }
        let c = overlay.centre;
        let r = 5.0;
        painter.line_segment(
            [egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)],
            egui::Stroke::new(2.0, p.accent),
        );
        painter.line_segment(
            [egui::pos2(c.x, c.y - r), egui::pos2(c.x, c.y + r)],
            egui::Stroke::new(2.0, p.accent),
        );
    }

    // Report the frame time (and, in sketch mode, the active plane) in the
    // status-bar mode field UI-A4 created.
    app.status.mode = match &sketch_state.plane {
        Some(active) => format!("{} · {ms_bucket} ms", active.label),
        None => format!("{action} · {ms_bucket} ms"),
    };
}
