//! GPU pick panel (editor parity wave D, card UI-D3 UI-PICK).
//!
//! Selection comes from what is actually drawn: this panel resolves the
//! pointer through an [`oep_viewer::gpu::idbuffer::IdPass`] — a second
//! offscreen pass rasterizing the exact faces the viewport draws, read back
//! into a host grid — so the picked identity is the primitive whose face the
//! GPU really rasterized under the cursor, never a hand-rolled projection.
//!
//! Three interactions, each a plain observable:
//!
//! * **Hover pre-highlight** — every frame, when the pointer is over the
//!   viewport, the id grid is queried at the pointer pixel and the covered
//!   primitive enters [`PickState::pre_highlight`] in the SAME frame (one
//!   frame step in the kittest harness, asserted as state, no second frame).
//! * **Shift-click multi-select** — a shift+primary click on a primitive
//!   ACCUMULATES into [`PickState::selection`]: the exact primitive-id list,
//!   appended once each, in scene order. A shift-click into empty space is a
//!   no-op (the set never shrinks by accident).
//! * **Alt-drag box select** — Alt + primary drag over the viewport draws a
//!   rubber band and, on release, commits the enclosed exact set: every
//!   primitive with a drawn id pixel inside the rect
//!   ([`IdPass::pick_rect`]), ordered by scene index. The alt modifier is
//!   free in the input table (viewport.rs excludes it from the orbit lane so
//!   the drag truly belongs to the box).
//!
//! Plus the card's clearing rule: a plain (no-modifier) primary click on
//! EMPTY background clears the selection and picks nothing; a plain click on
//! a primitive single-selects it. Sketch mode owns its own clicks, so while a
//! datum plane is active this panel goes fully idle (the viewport tells it
//! via the `in_sketch` flag) and drives no pass at all.
//!
//! The panel is `#[path]`-included from `viewport.rs` (exactly the
//! sketch/measure/features seam): it is egui + oep-viewer + oep-ui-theme only,
//! never touches a document (UI-B1 mutation guard), draws no raw hex outside
//! the theme tokens (UI-A3 guard), and stays inert on the committed
//! launch-frame goldens (it paints nothing until the pointer actually hovers
//! or a pick gesture runs).
//!
//! Observability: [`PickState`] is stored in the context store (readable by
//! any include copy) and a plain `String` oracle is published under
//! [`PICK_ORACLE_KEY`] every frame — `pre=`, `sel=`, `box=`, `pick=` — the
//! kittest assertions read the oracle, never the panel's internals.

use eframe::egui;

use oep_ui_theme::{Mode, Palette};
use oep_viewer::gpu::idbuffer::IdPass;
use oep_viewer::Camera;

/// Context-data key under which this panel's [`PickState`] lives.
pub const PICK_STATE_KEY: &str = "ui_d3_pick_state";
/// Context-data key under which the panel publishes its `String` oracle
/// (`pre=`, `sel=`, `box=`, `pick=`) every frame.
pub const PICK_ORACLE_KEY: &str = "ui_d3_pick_oracle";
/// Context-data key for an optional [`Scene`](oep_viewer::Scene) override.
/// The runtime defaults to the app's committed demo scene; a harness installs
/// a multi-primitive scene here so identity picks have real substance.
pub const PICK_SCENE_KEY: &str = "ui_d3_pick_scene";
/// Context-data key under which the id pass itself lives (one per session).
const PICK_SURFACE_KEY: &str = "ui_d3_pick_idpass";

/// Minimum box side (pixels) before an alt-drag commits as a box selection;
/// a smaller drag degrades to a single click at the drag centre.
pub const MIN_BOX_PX: u32 = 3;

/// The panel's observable state, persisted in the egui context store.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PickState {
    /// The primitive id under the hovered pointer (pre-highlight), set the
    /// same frame the pointer reaches it; `None` over empty space.
    pub pre_highlight: Option<String>,
    /// The exact multi-select set, as primitive ids in scene order.
    pub selection: Vec<String>,
    /// Active alt-drag box: `(anchor, current)` in logical panel coords while
    /// a box selection is being drawn; `None` otherwise.
    pub box_drag: Option<(egui::Pos2, egui::Pos2)>,
}

/// The per-session id pass, created lazily on the first frame that actually
/// needs it (the host shares the app's single wgpu device via
/// `device::acquire`; GPU-less hosts simply keep `pass == None` and the
/// panel honestly reports no picking in the oracle).
#[derive(Clone, Default)]
struct PickSurface {
    pass: Option<std::sync::Arc<std::sync::Mutex<IdPass>>>,
}

fn state_id() -> egui::Id {
    egui::Id::unique(PICK_STATE_KEY)
}

fn surface_id() -> egui::Id {
    egui::Id::unique(PICK_SURFACE_KEY)
}

/// The scene the id pass rasterizes: the override a harness installs under
/// [`PICK_SCENE_KEY`], else the app's committed demo scene (the same scene
/// the viewport renders by default).
fn scene_of(ctx: &egui::Context) -> oep_viewer::Scene {
    ctx.data(|d| d.get_temp::<oep_viewer::Scene>(egui::Id::unique(PICK_SCENE_KEY)))
        .unwrap_or_else(oep_viewer::CpuBackend::demo_scene)
}

/// The current orbit camera, rebuilt from the scalar snapshot `viewport.rs`
/// publishes every frame (the SAME camera the drawn frame used).
fn camera_of(ctx: &egui::Context) -> Option<Camera> {
    ctx.data(|d| d.get_temp::<super::ViewportSnapshot>(egui::Id::unique(super::VIEWPORT_STATE_KEY)))
        .map(|s| Camera {
            target: oep_viewer::Vec3::new(s.3, s.4, s.5),
            yaw: s.0,
            pitch: s.1,
            distance: s.2,
            fov_y: s.6,
            near: 0.1,
            far: 5000.0,
        })
}

fn read_state(ctx: &egui::Context) -> PickState {
    ctx.data(|d| d.get_temp::<PickState>(state_id()))
        .unwrap_or_default()
}

/// Palette resolved for the currently installed theme mode (same derivation
/// as the sibling panels).
fn palette(ctx: &egui::Context) -> Palette {
    let mode = if ctx.global_style().visuals.dark_mode {
        Mode::Dark
    } else {
        Mode::Light
    };
    Palette::resolve(mode)
}

/// A logical panel point -> pixel coordinates in the id grid (`0,0` =
/// viewport top-left), the SAME mapping the backend target uses (`px =
/// round(rect.width() * scale)`).
fn to_pixel(pos: egui::Pos2, rect: egui::Rect, scale: f32) -> (u32, u32) {
    let x = ((pos.x - rect.min.x) * scale).max(0.0) as u32;
    let y = ((pos.y - rect.min.y) * scale).max(0.0) as u32;
    (x, y)
}

/// Project a world point to panel (logical) coordinates with the ID PASS's and
/// `oep_viewer::pick`'s exact math (the standard perspective divide
/// `nx = clip.x / -view.z`). The pre-highlight and selection outlines drawn
/// here therefore hug exactly the pixels the id buffer reports — what the
/// analytic pick resolves — which coincides with the drawn frame once the
/// colour pass's D1 clip_w quirk is corrected+regoldened (D2's item).
fn panel_of(
    cam: &Camera,
    rect: egui::Rect,
    scale: f32,
    p: &oep_viewer::Vec3,
) -> Option<egui::Pos2> {
    let pixel_w = (rect.width() * scale).round().max(2.0) as f64;
    let pixel_h = (rect.height() * scale).round().max(2.0) as f64;
    let world_to_view = cam.view_matrix();
    let projection = cam.projection(pixel_w / pixel_h);
    let view = world_to_view.transform_point(p);
    let w = -view.z;
    if w.abs() < 1e-9 {
        return None;
    }
    let clip = projection.transform_point(&view);
    let nx = clip.x / w;
    let ny = clip.y / w;
    let nz = clip.z / w;
    if nz < -1.0 || nz > 1.0 {
        return None; // behind / outside the frustum: not drawn
    }
    Some(egui::pos2(
        rect.min.x + ((nx * 0.5 + 0.5) * pixel_w / f64::from(scale)) as f32,
        rect.min.y + ((0.5 - ny * 0.5) * pixel_h / f64::from(scale)) as f32,
    ))
}

/// Project a primitive's world AABB into panel-space edge segments (for the
/// pre-highlight / selection outlines). Edges behind the camera are skipped.
fn aabb_edges(
    ctx: &egui::Context,
    prim: &oep_viewer::Primitive,
    rect: egui::Rect,
    scale: f32,
) -> Vec<[egui::Pos2; 2]> {
    let Some(cam) = camera_of(ctx) else {
        return Vec::new();
    };
    let corners = prim.world.corners();
    const EDGES: [[usize; 2]; 12] = [
        [0, 1],
        [1, 2],
        [2, 3],
        [3, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let mut segs = Vec::with_capacity(12);
    for [i, j] in EDGES {
        if let (Some(pa), Some(pb)) = (
            panel_of(&cam, rect, scale, &corners[i]),
            panel_of(&cam, rect, scale, &corners[j]),
        ) {
            segs.push([pa, pb]);
        }
    }
    segs
}

/// A committed alt-drag resolves to either a box selection, a single pick (a
/// degenerate small drag, or an alt-click) or nothing (box over empty space
/// with no enclosed id) — which also clears, matching the plain-click rule.
enum Boxing {
    /// A real box: the exact enclosed primitive-id set, in scene order.
    Selection(Vec<String>),
    /// A small drag degraded to a single pick.
    Single(String),
    /// A box or click that enclosed nothing.
    BoxedNone,
}

fn commit_box(
    pass: &mut IdPass,
    scene: &oep_viewer::Scene,
    a: egui::Pos2,
    b: egui::Pos2,
    rect: egui::Rect,
    scale: f32,
) -> Boxing {
    let (x0, y0) = to_pixel(a, rect, scale);
    let (x1, y1) = to_pixel(b, rect, scale);
    let (lo_x, hi_x) = (x0.min(x1), x0.max(x1));
    let (lo_y, hi_y) = (y0.min(y1), y0.max(y1));
    if hi_x - lo_x < MIN_BOX_PX || hi_y - lo_y < MIN_BOX_PX {
        // Degenerate drag -> single pick at the centre.
        let (cx, cy) = ((lo_x + hi_x) / 2, (lo_y + hi_y) / 2);
        return match pass
            .pick_at(cx, cy)
            .and_then(|idx| scene.primitives.get(idx as usize))
            .map(|prim| prim.id.clone())
        {
            Some(id) => Boxing::Single(id),
            None => Boxing::BoxedNone,
        };
    }
    let idxs = pass.pick_rect(lo_x, lo_y, hi_x, hi_y);
    let ids: Vec<String> = idxs
        .iter()
        .filter_map(|&i| scene.primitives.get(i as usize).map(|prim| prim.id.clone()))
        .collect();
    if ids.is_empty() {
        Boxing::BoxedNone
    } else {
        Boxing::Selection(ids)
    }
}

/// Drive the GPU pick surface. Called exactly once per frame from
/// `viewport::show`, AFTER the backend block, so the scalar snapshot this
/// frame published holds the current camera; paints its overlays (hover
/// pre-highlight, box rubber band, selection outlines) on top of the frame.
pub fn update(ui: &egui::Ui, rect: egui::Rect, scale: f32, in_sketch: bool) {
    let ctx = ui.ctx();
    let px = (rect.width() * scale).round().max(2.0) as u32;
    let py = (rect.height() * scale).round().max(2.0) as u32;
    let scene = scene_of(ctx);
    // The current camera and click position are read BEFORE the context-data
    // write scope below (calling `ctx.data` / `ctx.input` inside `data_mut`
    // would deadlock the egui lock): they are copied into the scope.
    let cam = camera_of(ctx);
    let click_pos = ctx.input(|i| i.pointer.interact_pos());

    let (hover, primary_clicked, primary_down, shift, alt, pressed, released) = ctx.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_clicked(),
            i.pointer.primary_down(),
            i.modifiers.shift,
            i.modifiers.alt,
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.button_released(egui::PointerButton::Primary),
        )
    });
    let over = hover.is_some_and(|p| rect.contains(p));

    // All interaction lives in ONE context-data scope: the state is read,
    // mutated and written back inside the same borrow, and the id-pass mutex
    // guard is dropped before the block ends (no `&mut` escape, this egui).
    let (pre_highlight, box_visual) = ctx.data_mut(|d| {
        let mut state = d.get_temp::<PickState>(state_id()).unwrap_or_default();
        let surface = d.get_temp_mut_or_insert_with(surface_id(), PickSurface::default);

        let mut pre: Option<String> = None;
        let mut box_visual: Option<[egui::Pos2; 2]> = None;
        let mut last_pick = "none".to_owned();

        if in_sketch {
            // Sketch mode owns its clicks: this panel goes idle (no pass, no
            // state drift), dropping any in-flight box so it never commits.
            state.box_drag = None;
        } else {
            // Lazily create the id pass on the first frame that needs it.
            if surface.pass.is_none() {
                if let (Some(c), Some(gpu_ctx)) = (cam, oep_viewer::gpu::device::acquire()) {
                    surface.pass = IdPass::new(gpu_ctx, scene.clone(), c, px, py)
                        .map(|p| std::sync::Arc::new(std::sync::Mutex::new(p)));
                }
            }

            if let (Some(pass_arc), Some(c)) = (&surface.pass, cam) {
                let mut pass = pass_arc
                    .lock()
                    .expect("pick id-pass lock never held across a data_mut scope");
                let _ = pass.set_view(&scene, c, px, py);

                // Hover pre-highlight: same-frame id lookup at the pointer.
                if over {
                    if let Some(p) = hover {
                        let (ix, iy) = to_pixel(p, rect, scale);
                        pre = pass.pick_at(ix, iy).and_then(|idx| {
                            scene
                                .primitives
                                .get(idx as usize)
                                .map(|prim| prim.id.clone())
                        });
                    }
                }

                // Alt-drag box state machine.
                match state.box_drag {
                    None => {
                        if alt && pressed && over {
                            let anchor = hover.expect("pointer over the panel while pressing");
                            state.box_drag = Some((anchor, anchor));
                            box_visual = Some([anchor, anchor]);
                        }
                    }
                    Some((anchor, _)) => {
                        let current = hover.unwrap_or(anchor);
                        if primary_down && alt && over {
                            state.box_drag = Some((anchor, current));
                            box_visual = Some([anchor, current]);
                        } else if released || !primary_down {
                            // Commit on button release (or a stale drag whose
                            // press was lost). Alt dropping mid-drag does not
                            // abort — the box is simply not extended further
                            // and still commits on release.
                            let outcome =
                                commit_box(&mut pass, &scene, anchor, current, rect, scale);
                            state.box_drag = None;
                            match outcome {
                                Boxing::Selection(ids) => {
                                    state.selection = ids;
                                    last_pick = format!("box {}", state.selection.len());
                                }
                                Boxing::Single(id) => {
                                    state.selection = vec![id.clone()];
                                    last_pick = format!("select {id}");
                                }
                                Boxing::BoxedNone => {
                                    state.selection.clear();
                                    last_pick = "clear".to_owned();
                                }
                            }
                        }
                    }
                }

                // Click handling: shift multi-select, plain select-or-clear.
                if primary_clicked && !alt && over {
                    if let Some(cpos) = click_pos {
                        if rect.contains(cpos) {
                            let (ix, iy) = to_pixel(cpos, rect, scale);
                            let picked = pass.pick_at(ix, iy).and_then(|idx| {
                                scene
                                    .primitives
                                    .get(idx as usize)
                                    .map(|prim| prim.id.clone())
                            });
                            if shift {
                                if let Some(id) = picked {
                                    if !state.selection.contains(&id) {
                                        state.selection.push(id.clone());
                                    }
                                    last_pick = format!("add {id}");
                                }
                            } else {
                                match picked {
                                    Some(id) => {
                                        state.selection = vec![id.clone()];
                                        pre = Some(id.clone());
                                        last_pick = format!("select {id}");
                                    }
                                    None => {
                                        state.selection.clear();
                                        pre = None;
                                        last_pick = "clear".to_owned();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        d.insert_temp(state_id(), state.clone());

        // Publish the oracle last, from the mutated (owned) state.
        let orc = format!(
            "pre={}\nsel={}\nbox={}\npick={}",
            pre.as_deref().unwrap_or("none"),
            if state.selection.is_empty() {
                "0".to_owned()
            } else {
                state.selection.join(",")
            },
            match box_visual {
                Some([a, b]) => format!(
                    "{}x{}",
                    (b.x - a.x).abs().round(),
                    (b.y - a.y).abs().round()
                ),
                None => "none".to_owned(),
            },
            last_pick,
        );
        d.insert_temp(egui::Id::unique(PICK_ORACLE_KEY), orc);

        (pre, box_visual)
    });

    // Paint overlays on top of the frame: hover pre-highlight (accent), the
    // box rubber band (accent fill + stroke), and selection outlines (border
    // ink). Token colours only (UI-A3 guard).
    let p = palette(ctx);
    let painter = ui.painter();
    if let Some(id) = &pre_highlight {
        if let Some(prim) = scene.primitives.iter().find(|prim| &prim.id == id) {
            for [a, b] in aabb_edges(ctx, prim, rect, scale) {
                painter.line_segment([a, b], egui::Stroke::new(1.5, p.accent));
            }
        }
    }
    if let Some([a, b]) = box_visual {
        let r = egui::Rect::from_two_pos(a, b);
        // Translucent accent fill derived from the token (a Color32 METHOD,
        // not a literal constructor — the UI-A3 guard allows it).
        painter.rect_filled(r, 0.0, p.accent.gamma_multiply(0.4));
        painter.rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0, p.accent),
            egui::StrokeKind::Outside,
        );
    }
    let sel = read_state(ctx);
    for id in &sel.selection {
        if let Some(prim) = scene.primitives.iter().find(|prim| &prim.id == id) {
            for [a, b] in aabb_edges(ctx, prim, rect, scale) {
                painter.line_segment([a, b], egui::Stroke::new(1.0, p.border));
            }
        }
    }
}

/// Read the panel's last `String` oracle (cross-copy-safe; empty when no pick
/// surface has run yet). Consumed by the acceptance tests and any status UI;
/// in the binary the oracle is written but nothing reads it back, so dead-code
/// is allowed (same allowance as `viewport::snapshot`).
#[allow(dead_code)]
pub fn oracle(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::unique(PICK_ORACLE_KEY)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_box_constant_is_positive() {
        const {
            assert!(MIN_BOX_PX > 0);
        }
    }

    #[test]
    fn scene_of_defaults_to_the_demo_scene() {
        // A bare context has no override: the panel must fall back to the
        // app's committed demo scene (one part, body b1).
        let ctx = egui::Context::default();
        let scene = scene_of(&ctx);
        assert_eq!(scene.primitives.len(), 1);
        assert_eq!(scene.primitives[0].body_id, "b1");
    }

    #[test]
    fn scene_override_key_is_respected() {
        let ctx = egui::Context::default();
        let scene = oep_viewer::Scene::default();
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique(PICK_SCENE_KEY), scene.clone()));
        assert!(scene_of(&ctx).primitives.is_empty());
    }
}
