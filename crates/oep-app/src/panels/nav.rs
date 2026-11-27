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
//! The card is drawn as ONE `egui::Area` fixed at the viewport's bottom-right
//! corner (the wave's other top-right surfaces - the D2 shading card - and
//! the sketch plane, which the default camera projects to the upper-right,
//! own the rest of that quadrant) whose widgets are added through the card's
//! flow layout (the shading.rs idiom). egui auto-sizes the Area to exactly
//! that content, so the Area's click surface is the card alone - never a
//! screen-anchored `fixed_pos(ZERO)` + absolute `ui.put` surface that grows
//! to a huge rect and swallows clicks meant for the neighbouring surfaces
//! (measured: the original layout caught the UI-B8 "Export STEP" click and
//! the D2 shading panel's clicks until the layout was corrected).
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

/// One face button of the gizmo, added to the flow `ui`; `hit` reports the
/// clicked face. The label is one line at a fixed size (an explicit 10px
/// RichText with an Extend wrap mode so it can never wrap to multiple lines),
/// so the button never grows taller than its cell and neighbouring faces'
/// click rects never overlap (the UI-D7-measured gizmo defect: wrapped
/// "Cube Front" labels grew buttons to ~64px in 24px cells, so a kittest
/// click resolved to the wrong face).
fn face_button(ui: &mut egui::Ui, face: CubeFace) -> Option<CubeFace> {
    let btn = egui::Button::new(egui::RichText::new(format!("Cube {}", face.label())).size(10.0))
        .min_size(egui::vec2(FACE_W, FACE_H))
        .wrap_mode(egui::TextWrapMode::Extend);
    let resp = ui.add(btn).on_hover_text(format!(
        "Orient to the {} standard view",
        face.view().label()
    ));
    resp.clicked().then_some(face)
}

/// The miniature orientation cube: six labelled face buttons in the classic
/// front/top/side arrangement, each a normal egui button so every face is in
/// the AccessKit tree by name and reachable by keyboard. Laid out with the
/// card's flow layout (the shading.rs idiom) so the surrounding
/// `egui::Area` auto-sizes to exactly this content and never grows a click
/// surface beyond the card.
fn cube_gizmo(ui: &mut egui::Ui) -> Option<CubeFace> {
    use CubeFace::*;
    let mut hit = None;
    // Centred Top
    ui.horizontal(|ui| {
        ui.add_space(FACE_W + FACE_GAP);
        if let Some(f) = face_button(ui, Top) {
            hit = Some(f);
        }
    });
    // Left | Front | Right
    ui.horizontal(|ui| {
        if let Some(f) = face_button(ui, Left) {
            hit = Some(f);
        }
        if let Some(f) = face_button(ui, Front) {
            hit = Some(f);
        }
        if let Some(f) = face_button(ui, Right) {
            hit = Some(f);
        }
    });
    // Centred Back
    ui.horizontal(|ui| {
        ui.add_space(FACE_W + FACE_GAP);
        if let Some(f) = face_button(ui, Back) {
            hit = Some(f);
        }
    });
    // Centred Bottom
    ui.horizontal(|ui| {
        ui.add_space(FACE_W + FACE_GAP);
        if let Some(f) = face_button(ui, Bottom) {
            hit = Some(f);
        }
    });
    hit
}

/// The six face buttons' cell size: wide enough that the full "Cube <face>"
/// label fits on one line, 20 px tall on a 24 px row pitch, so no two faces
/// overlap (the acceptance probe prints every face's 72x20 click rect).
const FACE_W: f32 = 72.0;
const FACE_H: f32 = 20.0;
const FACE_GAP: f32 = 4.0;

/// The nav card's total width, sized to the widest content row (three 72 px
/// face buttons plus two 4 px gaps) plus the frame's inner margin.
const GW: f32 = 240.0;

/// The nav card is anchored in the viewport's bottom-right corner (free of
/// every other wave surface): the shading card (D2) owns the top-right, the
/// default-camera sketch plane projects to the upper-right (measured:
/// rectangle corners at x 788..912, y 258..333; entry point at 662,407), the
/// finish/export surface is top-left, the measure surface (C5) is bottom-left,
/// and the feature dialog is centre. Placing the card anywhere over the sketch
/// projection swallowed the sketch tool's clicks (`feature_from_sketch`), and
/// over the shading card swallowed D2's clicks (`gpu_shading`) - both measured
/// on this branch. The card's approximate height keeps it above the viewport
/// bottom when anchored in the corner.
const CARD_H: f32 = 220.0;

/// Paint the nav surface (gizmo, quick-view buttons, section slider) over the
/// viewport's bottom-right corner and apply anything the user just clicked.
/// Called exactly once per frame from `viewport::show`, AFTER the backend
/// block, so the camera it reads/writes is this frame's backend.
pub fn update(ui: &egui::Ui, rect: egui::Rect, _scale: f32, _app: &mut App) {
    let ctx = ui.ctx();

    // Read the previous frame's snapshot up front (a read-only context-data
    // borrow, so it never nests inside the write scope below).
    let prev: NavSnapshot = ctx
        .data(|d| d.get_temp(nav_id()))
        .unwrap_or((1.0, 0.0, 0, 0));

    // The nav card, anchored at the viewport's top-right. The `egui::Area`
    // is fixed at the card corner and its widgets are added with the card's
    // flow layout (the shading.rs idiom), so egui auto-sizes the Area to
    // exactly its content and the click surface is the card alone - never a
    // screen-anchored `fixed_pos(ZERO)` + absolute `ui.put` surface that
    // grows to a huge rect and swallows other panels' clicks (measured: the
    // original layout caught the UI-B8 "Export STEP" click at the viewport's
    // top-left, failing `feature_from_sketch::ui_step_export_...`).
    let card_pos = egui::pos2(rect.right() - GW - 10.0, rect.max.y - CARD_H - 10.0);
    let mut pending: Option<StandardView> = None;
    let mut section = prev.0;

    let mode = theme::mode_of(ui);
    let p = oep_ui_theme::Palette::resolve(mode);

    egui::Area::new(egui::Id::unique("ui_d4_nav_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(card_pos)
        .movable(false)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::new()
                .fill(p.surface_raised)
                .corner_radius(egui::CornerRadius::same(4))
                .stroke(egui::Stroke::new(1.0, p.border))
                .inner_margin(egui::Margin::symmetric(8, 8));
            frame.show(ui, |ui| {
                ui.set_width(GW - 16.0);
                ui.label(
                    egui::RichText::new("Orientation")
                        .size(11.0)
                        .color(p.text_primary),
                );
                ui.add_space(4.0);

                // The orientation cube gizmo (six labelled face buttons).
                if let Some(face) = cube_gizmo(ui) {
                    pending = Some(face.view());
                }
                ui.add_space(6.0);

                // Quick standard-view buttons: two 2-button rows.
                let quick_btn = |ui: &mut egui::Ui, text: &str| {
                    ui.add(
                        egui::Button::new(egui::RichText::new(text).size(11.0))
                            .min_size(egui::vec2(52.0, 20.0)),
                    )
                };
                ui.horizontal(|ui| {
                    if quick_btn(ui, "Front").clicked() {
                        pending = Some(StandardView::Front);
                    }
                    if quick_btn(ui, "Top").clicked() {
                        pending = Some(StandardView::Top);
                    }
                });
                ui.horizontal(|ui| {
                    if quick_btn(ui, "Right").clicked() {
                        pending = Some(StandardView::Right);
                    }
                    if quick_btn(ui, "Iso").clicked() {
                        pending = Some(StandardView::Iso);
                    }
                });
                ui.add_space(6.0);

                // Section plane slider (Z axis, 0 = closed to the bottom
                // sheet, 1 = whole box). The AccessKit SetValue action drives
                // the same slider, so the harness can place it exactly and
                // the surface is reachable without a mouse.
                ui.add(
                    egui::Slider::new(&mut section, 0.0..=1.0)
                        .text("Section")
                        .show_value(false),
                );
                ui.label(
                    egui::RichText::new(format!("Triangles: {}", prev.2))
                        .size(10.0)
                        .color(p.text_muted),
                );
            });
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

    // UI-E6 UI-NAV-CUBE: registration site for the 3D orientation cube. Drawn
    // after the nav card (above it, right column), from this frame's camera.
    // The button panel above is untouched: this is a pure appended call.
    super::super::nav_cube::update(ui, rect, _scale, _app);
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
