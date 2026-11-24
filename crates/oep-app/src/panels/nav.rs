//! UI-D4 UI-NAV: view cube, standard views, section planes (wave D).
//!
//! The navigation surface this card owns:
//!
//! - an **orientation cube gizmo** - six labelled face buttons laid out as a
//!   miniature cube - plus four quick-view buttons (Front / Top / Right /
//!   Iso). Clicking any face or quick button lands the orbit camera on the
//!   exact committed standard orientation (`oep_viewer::views::StandardView`
//!   numeric yaw/pitch), keeping the orbit target, distance and fov;
//! - a **Section slider** that maps a `[0,1]` fraction onto the committed
//!   world clip-plane preset (`views::section_plane` on the Z axis) and
//!   reports the visible-triangle count the clipped demo solid leaves
//!   (`views::section_triangle_count` / `views::visible_triangle_count`) -
//!   the "clips the model predictably" number, published with the plane.
//!
//! Like `sketch.rs` / `features.rs` / `measure.rs` / `pick_gpu.rs` it is
//! `#[path]`-included from `viewport.rs` (one `nav::update(ui, rect, scale,
//! app)` call at the end of `viewport::show`) and holds its state in the egui
//! context store. It reads and writes the SAME runtime backend
//! `Arc<Mutex<Box<dyn ViewportBackend>>>` the viewport panel guards
//! (`super::VIEWPORT_STATE_KEY`), so applying an orientation actually moves
//! the camera the next rasterize, and it publishes a plain-scalar
//! [`NavSnapshot`] tuple (the section fraction, the plane offset and the two
//! visible-triangle counts) under [`NAV_STATE_KEY`] for the acceptance
//! harness.
//!
//! The card is drawn as ONE `egui::Area` at screen origin whose interactive
//! widgets are each `put` at an explicit screen rect (the UI-B8 feature-panel
//! idiom). That keeps the Area's click surface exactly the card - never a
//! layout-flow surface that expands and swallows clicks meant for the
//! neighbouring properties panel.
//!
//! ## Bounds this panel respects
//!
//! - UI-A3 / UI-A12 guards: no raw hex and no bold-text ink construction
//!   outside `theme.rs`; all painted ink comes from `oep_ui_theme` tokens.
//! - UI-B1 mutation guard: this panel never writes a document collection; it
//!   only moves the viewport camera and publishes its own nav state.
//! - Default frame unchanged: the surface draws only in the viewport's
//!   top-right corner and the committed launch-frame goldens are untouched.
//! - One `Pane`: the gizmo is an [`egui::Area`] overlay (no native panel), so
//!   the single-Pane AccessKit contract AI-A2's harness asserts is preserved.

use eframe::egui;

use oep_viewer::views::{self, CubeFace, StandardView};
use oep_viewer::Aabb;
use std::sync::{Arc, Mutex};

use crate::app::App;
use crate::theme;

/// Context-data key under which the plain-scalar [`NavSnapshot`] is published
/// every frame (mirror of the viewport snapshot pattern; the acceptance
/// harness reads it back with the same key).
pub const NAV_STATE_KEY: &str = "isolith.ui.viewport.nav";

/// The committed Z-axis section slider positions whose visible-triangle
/// counts the acceptance pins, in slider order: `1.0` = whole box at the
/// right end of the rail, `0.5` = the half cut, `0.0` = the box closed to
/// its bottom sheet. Consumed by the acceptance harness; the binary build
/// never reads them, so they are allowed dead code in that compile alone.
#[allow(dead_code)]
pub const SECTION_FRACTIONS: [f64; 3] = [1.0, 0.5, 0.0];

/// The analytic visible-triangle counts for [`SECTION_FRACTIONS`] over the
/// demo solid (80 x 40 x 10 box, Z-axis section, kept below the plane): the
/// full box is six quads = 12; a half cut leaves the lower sheet of every
/// side face (5 quads) = 10; closed to the bottom leaves only the bottom
/// quad = 2. These are the numbers the acceptance prints and asserts;
/// consumed by the harness, so allowed dead code in the binary build alone.
#[allow(dead_code)]
pub const SECTION_ANALYTIC_COUNTS: [usize; 3] = [12, 10, 2];

/// A plain-scalar snapshot of the nav surface, published every frame into the
/// egui context store (mirror of the viewport [`super::ViewportSnapshot`]
/// pattern) so the acceptance harness can assert the exact numbers the
/// surface produced without reaching app-private types:
/// `(section, plane_offset, visible_triangles, analytic_triangles)`.
pub type NavSnapshot = (f64, f64, usize, usize);

/// The demo solid's bounding box (the committed 80 x 40 x 10 scene the
/// viewport renders), computed once.
fn demo_bounds() -> Aabb {
    static BOUNDS: std::sync::OnceLock<Aabb> = std::sync::OnceLock::new();
    *BOUNDS.get_or_init(|| {
        oep_viewer::CpuBackend::demo_scene()
            .bounds
            .expect("the committed demo scene always has bounds")
    })
}

/// Read the published nav snapshot, if any.
///
/// Consumed only by `crates/oep-app/tests/nav.rs` (which includes this
/// module by `#[path]`); the binary build never calls it, so it is allowed to
/// be dead code in that compile alone.
#[allow(dead_code)]
pub fn nav_state(ctx: &egui::Context) -> Option<NavSnapshot> {
    ctx.data(|d| d.get_temp(egui::Id::unique(NAV_STATE_KEY)))
}

/// The context-data id under which [`NavSnapshot`] lives.
fn nav_id() -> egui::Id {
    egui::Id::unique(NAV_STATE_KEY)
}

/// The context-data id under which the runtime backend Arc lives (the
/// viewport panel's own `VIEWPORT_STATE_KEY`, shared verbatim so this panel
/// moves the real camera).
fn backend_id() -> egui::Id {
    egui::Id::unique(super::VIEWPORT_STATE_KEY)
}

/// Apply a standard orientation to the real viewport camera: keep the orbit
/// target, distance and fov, set exactly the committed yaw/pitch, and
/// re-rasterize through the seam.
fn apply_view(ctx: &egui::Context, view: StandardView) {
    let (yaw, pitch) = view.yaw_pitch();
    let arc: Option<Arc<Mutex<Box<dyn oep_viewer::ViewportBackend>>>> =
        ctx.data(|d| d.get_temp(backend_id()));
    if let Some(arc) = arc {
        let mut guard = arc
            .lock()
            .expect("viewport backend lock never held across a nav frame");
        let mut cam = guard.camera();
        cam.yaw = yaw;
        cam.pitch = pitch;
        guard.set_camera(cam);
    }
}

/// The committed triangle counts for a section fraction, over the demo solid.
fn section_numbers(bounds: &Aabb, fraction: f64) -> (f64, usize, usize) {
    let plane = views::section_plane(bounds, views::SectionAxis::Z, fraction);
    let offset = bounds.min.z + fraction * (bounds.max.z - bounds.min.z);
    let measured = views::visible_triangle_count(bounds, &plane);
    let analytic = views::section_triangle_count(bounds, &plane);
    (offset, measured, analytic)
}

/// One face button of the gizmo, placed at an explicit screen rect; `hit`
/// records the clicked face.
fn face_button(ui: &mut egui::Ui, rect: egui::Rect, face: CubeFace, hit: &mut Option<CubeFace>) {
    let btn = egui::Button::new(format!("Cube {}", face.label())).min_size(rect.size());
    let resp = ui.put(rect, btn).on_hover_text(format!(
        "Orient to the {} standard view",
        face.view().label()
    ));
    if resp.clicked() {
        *hit = Some(face);
    }
}

/// The miniature orientation cube: six labelled face buttons in the classic
/// front/top/side arrangement, each a normal egui button so every face is in
/// the AccessKit tree by name and reachable by keyboard. Widgets are `put` at
/// explicit screen rects, so the surrounding `egui::Area` never grows a click
/// surface beyond the nav card.
fn cube_gizmo(ui: &mut egui::Ui, px: f32, py: f32) -> Option<CubeFace> {
    const FW: f32 = 40.0;
    const FH: f32 = 20.0;
    const GAP: f32 = 4.0;
    let col = |i: usize| px + i as f32 * (FW + GAP);
    let row = |i: usize| py + i as f32 * (FH + GAP);
    let cell = |c: usize, r: usize| {
        egui::Rect::from_min_size(egui::pos2(col(c), row(r)), egui::vec2(FW, FH))
    };

    let mut hit = None;
    face_button(ui, cell(1, 0), CubeFace::Top, &mut hit);
    face_button(ui, cell(0, 1), CubeFace::Left, &mut hit);
    face_button(ui, cell(1, 1), CubeFace::Front, &mut hit);
    face_button(ui, cell(2, 1), CubeFace::Right, &mut hit);
    face_button(ui, cell(1, 2), CubeFace::Back, &mut hit);
    face_button(ui, cell(1, 3), CubeFace::Bottom, &mut hit);
    hit
}

/// Paint the nav surface (gizmo, quick-view buttons, section slider) over the
/// viewport's top-right corner and apply anything the user just clicked.
/// Called exactly once per frame from `viewport::show`, AFTER the backend
/// block, so the camera it reads/writes is this frame's backend.
pub fn update(ui: &egui::Ui, rect: egui::Rect, _scale: f32, _app: &mut App) {
    let ctx = ui.ctx();

    // Read the previous frame's snapshot up front (a read-only context-data
    // borrow, so it never nests inside the write scope below).
    let prev: NavSnapshot = ctx
        .data(|d| d.get_temp(nav_id()))
        .unwrap_or((1.0, 0.0, 0, 0));

    // The nav card's explicit geometry, anchored to the viewport's top-right.
    // One `egui::Area` at screen origin whose interactive widgets are each
    // `put` at an exact rect (the UI-B8 feature-panel idiom), so the Area's
    // click surface stays the card and never expands into the neighbouring
    // properties panel.
    let gw = 152.0_f32;
    let gh = 236.0_f32;
    let card = egui::Rect::from_min_size(
        egui::pos2(rect.right() - gw - 10.0, rect.top() + 8.0),
        egui::vec2(gw, gh),
    );
    let (px, py) = (card.min.x + 6.0, card.min.y + 6.0);
    let mut pending: Option<StandardView> = None;
    let mut section = prev.0;

    let mode = theme::mode_of(ui);
    let p = oep_ui_theme::Palette::resolve(mode);

    egui::Area::new(egui::Id::unique("ui_d4_nav_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ui.ctx(), |ui| {
            // Card background + hairline + heading (painted first in a scoped
            // painter, before the interactive `put`s below borrow `ui`).
            {
                let painter = ui.painter();
                painter.rect_filled(card, 4.0, p.surface_raised);
                painter.rect_stroke(
                    card,
                    4.0,
                    egui::Stroke::new(1.0, p.border),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    egui::pos2(px, py),
                    egui::Align2::LEFT_TOP,
                    "Orientation",
                    egui::FontId::proportional(11.0),
                    p.text_primary,
                );
            }

            // The orientation cube gizmo (six labelled face buttons).
            if let Some(face) = cube_gizmo(ui, px, py + 18.0) {
                pending = Some(face.view());
            }

            // Quick standard-view buttons: a 2x2 cluster under the cube.
            let qy = py + 18.0 + 4.0 * 24.0 + 6.0;
            let quick = |c: usize, r: usize| {
                egui::Rect::from_min_size(
                    egui::pos2(px + c as f32 * 54.0, qy + r as f32 * 21.0),
                    egui::vec2(52.0, 20.0),
                )
            };
            let quick_btn = |ui: &mut egui::Ui, r: egui::Rect, text: &str| {
                ui.put(r, egui::Button::new(text).min_size(r.size()))
            };
            if quick_btn(ui, quick(0, 0), "Front").clicked() {
                pending = Some(StandardView::Front);
            }
            if quick_btn(ui, quick(1, 0), "Top").clicked() {
                pending = Some(StandardView::Top);
            }
            if quick_btn(ui, quick(0, 1), "Right").clicked() {
                pending = Some(StandardView::Right);
            }
            if quick_btn(ui, quick(1, 1), "Iso").clicked() {
                pending = Some(StandardView::Iso);
            }

            // Section plane slider (Z axis, 0 = closed to the bottom sheet,
            // 1 = whole box). The AccessKit SetValue action drives the same
            // slider, so the harness can place it exactly and the surface is
            // reachable without a mouse.
            let slider_rect = egui::Rect::from_min_size(
                egui::pos2(px, qy + 2.0 * 21.0 + 8.0),
                egui::vec2(gw - 12.0, 20.0),
            );
            ui.put(
                slider_rect,
                egui::Slider::new(&mut section, 0.0..=1.0)
                    .text("Section")
                    .show_value(false),
            );
            {
                let painter = ui.painter();
                painter.text(
                    egui::pos2(px, slider_rect.max.y + 4.0),
                    egui::Align2::LEFT_TOP,
                    format!("Triangles: {}", prev.2),
                    egui::FontId::proportional(10.0),
                    p.text_muted,
                );
            }
        });

    // Apply a pending orientation (writes the camera AFTER the interactive
    // block, outside any context-data borrow).
    if let Some(view) = pending {
        apply_view(ctx, view);
    }

    // Always recompute the section numbers for the current slider value
    // (cheap: clipping six small faces), so the published snapshot and the
    // readout stay in lockstep with the live section fraction.
    let (offset, measured, analytic) = section_numbers(&demo_bounds(), section);

    // Publish the plain-scalar snapshot (a second, sequential context-data
    // scope, free of the backend borrow).
    ctx.data_mut(|d| {
        d.insert_temp::<NavSnapshot>(nav_id(), (section, offset, measured, analytic));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed section literals the acceptance prints and asserts.
    #[test]
    fn committed_section_numbers_match_literals() {
        let b = demo_bounds();
        for (frac, expected) in SECTION_FRACTIONS
            .iter()
            .copied()
            .zip(SECTION_ANALYTIC_COUNTS)
        {
            let (_, measured, analytic) = section_numbers(&b, frac);
            assert_eq!(measured, expected, "measured at {frac}");
            assert_eq!(analytic, expected, "analytic at {frac}");
        }
    }
}
