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
//! Backend ownership (UI-D1): `App` is off-limits on these cards, so the
//! runtime backend lives in the egui context's per-widget data, keyed by a
//! stable id that the kittest harnesses (`--test viewport`, `--test
//! sketch_enter`) read back with the same id. Since wave D the panel
//! constructs it via `oep_viewer::select_backend` - `GpuBackend` when a wgpu
//! adapter exists, `CpuBackend` otherwise - and holds the seam behind an
//! `Arc<Mutex<Box<dyn ViewportBackend>>>` (the egui store needs cloneable,
//! sendable state). All work happens inside one [`egui::Context::data_mut`]
//! scope because this egui version cannot return `&mut` out of that closure;
//! the frame hands out owned values (texture image, snapshot, status text,
//! the pre-projected sketch grid layer).

use eframe::egui;

use oep_viewer::Camera;

use crate::app::App;
use crate::theme;

#[path = "sketch.rs"]
pub mod sketch;

// UI-B8 UI-FEATURE-FROM-SKETCH: the finish-sketch (extrude/revolve) dialog, its
// document-clone preview, the feature journal and the STEP export. This panel
// owns the interactive half (keys, buttons, oracle, painting); `features.rs`
// is its own `#[path]` include exactly like `sketch.rs` above, so the app
// binary (and the test targets that pull `app.rs`) compile the same source.
#[path = "features.rs"]
pub mod features;

// UI-C5 UI-MEASURE: the measure surface (plain `M`), its egui-free geometry
// engine and the on-screen annotation. `measure.rs` is included verbatim like
// `sketch.rs` / `features.rs`, so the app and the test targets that pull
// `app.rs` compile the same source; by default the surface is closed and
// draws nothing, so the committed launch-frame goldens are untouched.
#[path = "measure.rs"]
pub mod measure;

// UI-D3 UI-PICK: the GPU id-buffer pick surface (hover pre-highlight, shift
// click multi-select, alt-drag box select) over the same scene the viewport
// draws. `pick_gpu.rs` is included verbatim like the panels above; it drives
// its own [`crate::gpu idbuffer`] pass from the scalar snapshot and paints
// nothing until the pointer interacts, so the committed goldens stay valid.
// It is driven by the single `pick_gpu::update(...)` call in `show` below.
#[path = "pick_gpu.rs"]
pub mod pick_gpu;

// UI-D4 UI-NAV: the orientation cube gizmo, quick standard-view buttons and
// the section-plane slider. `nav.rs` is included verbatim like the panels
// above and driven by the single `nav::update(...)` call in `show` below -
// after pick_gpu, so its buttons and slider sit on the top-right corner of
// the frame and win pointer interaction there.
#[path = "nav.rs"]
pub mod nav;

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

    let (primary, middle, shift, alt, delta, scroll, zoom, hovered, response) = {
        let (primary, middle, shift, alt, delta, scroll, zoom) = ui.ctx().input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.modifiers.shift,
                i.modifiers.alt,
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
            alt,
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

    // UI-B8 UI-FEATURE-FROM-SKETCH: plain F opens the finish-sketch dialog,
    // Escape cancels it. Runs after the sketch read (so draw-tool keys win)
    // and before this panel's own Exit-sketch Escape, so a cancel inside the
    // dialog is consumed here and never exits the sketch.
    features::poll(ui.ctx());

    // UI-C5 UI-MEASURE: plain M toggles the measure surface. Runs alongside
    // the feature poll so the two plain keys never fight.
    measure::poll(ui.ctx());

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

    // UI-B8 UI-FEATURE-FROM-SKETCH: the preview frame path. Computed once
    // before the backend borrow (it reads the feature panel state from the
    // same context store); when a document clone is being previewed (or a
    // feature is applied) its rasterized solid frame and hash replace the
    // ordinary backend frame - so the published viewport hash moves with the
    // preview, and Cancel returns it to the pre-feature value.
    let preview = features::preview_frame(ui.ctx(), px, py);

    // All panel mutation lives inside one context-data scope (no `&mut` escape,
    // this egui version); owned outputs are handed out afterwards.
    //
    // Navigation drives off the RAW button state + the per-frame pointer delta,
    // not egui's click-vs-drag flag: that flag lands one frame after the drag
    // decision, which would blur the exact numeric mapping a kittest drag needs
    // to assert. A press with no movement is a no-op (delta == 0).
    let (image, ms_bucket, action, overlay) = ui.ctx().data_mut(|d| {
        // UI-D1: the seam now selects the runtime backend - GPU (`GpuBackend`)
        // when an adapter exists, deterministic CPU otherwise (see
        // `oep_viewer::select_backend`). The egui context store requires
        // cloned, `Send + Sync` values, so the `Box<dyn ViewportBackend>` is
        // held in an `Arc<Mutex<...>>`. All backend work lives in one inner
        // block so the `MutexGuard` (a Drop type) releases the store before
        // the snapshot is published into the same store.
        let (snapshot, image, ms_bucket, action, overlay) = {
            let backend_arc = d.get_temp_mut_or_insert_with(state_id, || {
                std::sync::Arc::new(std::sync::Mutex::new(oep_viewer::select_backend(
                    demo_scene(),
                )))
            });
            let mut guard = backend_arc
                .lock()
                .expect("viewport backend lock never held across a data_mut scope");
            let backend: &mut dyn oep_viewer::ViewportBackend = &mut **guard;

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
            // Orbit: plain left drag. The `alt` modifier is deliberately
            // excluded so Alt+left-drag belongs to the UI-D3 box-select
            // gesture (pick_gpu) instead of fighting with navigation.
            else if primary && hovered && !shift && !alt {
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
            // UI-B8 preview frame path: the preview document clone's rasterized
            // solid replaces the ordinary backend frame (and its hash is what the
            // snapshot publishes), so "the preview frame differs from the
            // pre-feature frame" is a real, published value.
            let (image, published_hash) = match &preview {
                Some((pimg, phash)) => (
                    egui::ColorImage::from_rgba_unmultiplied(
                        [pimg.width as usize, pimg.height as usize],
                        &pimg.data,
                    ),
                    *phash,
                ),
                None => (
                    egui::ColorImage::from_rgba_unmultiplied(
                        [frame.width as usize, frame.height as usize],
                        &frame.data,
                    ),
                    frame_hash,
                ),
            };
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

            let snapshot = (
                cam.yaw,
                cam.pitch,
                cam.distance,
                cam.target.x,
                cam.target.y,
                cam.target.z,
                cam.fov_y,
                published_hash,
            );
            (snapshot, image, ms_bucket, action, overlay)
        }; // backend borrow (guard) released here

        // Publish the scalar snapshot for the harness (a second, sequential
        // context-data scope, free of the backend borrow).
        d.insert_temp::<ViewportSnapshot>(state_id, snapshot);

        (image, ms_bucket, action, overlay)
    });

    // Persist the sketch state (struct + String oracle) after the backend
    // borrow: a second, sequential context-data scope.
    sketch::write_state(ui.ctx(), &sketch_state);

    // UI-B8 UI-FEATURE-FROM-SKETCH: paint the feature surface (Finish button,
    // live dialog, Export button) over the viewport, from the same panel state
    // the preview frame reads.
    features::draw(ui.ctx(), rect);

    // UI-C5 UI-MEASURE: paint the measure surface + annotation over the
    // viewport (draws nothing while the surface is closed).
    measure::draw(ui.ctx(), rect);

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

    // UI-D3 UI-PICK: drive the GPU id-buffer pick surface from the frame's
    // fresh scalar snapshot (runs after the backend block so the camera is
    // this frame's), painting the hover pre-highlight, the box rubber band
    // and the multi-select outlines on top of the frame. Sketch mode owns its
    // clicks, so `in_sketch` idles the surface.
    pick_gpu::update(ui, rect, scale, sketch_state.in_sketch());

    // UI-D4 UI-NAV: drive the navigation surface (orientation cube, quick
    // standard-view buttons, section slider) from this frame's camera. Runs
    // after pick_gpu so its buttons and slider sit on the frame's top-right
    // corner and win pointer interaction there; any orientation it applies
    // reaches the backend now and renders at the next frame.
    nav::update(ui, rect, scale, app);

    // Report the frame time (and, in sketch mode, the active plane) in the
    // status-bar mode field UI-A4 created.
    app.status.mode = match &sketch_state.plane {
        Some(active) => format!("{} · {ms_bucket} ms", active.label),
        None => format!("{action} · {ms_bucket} ms"),
    };
}
