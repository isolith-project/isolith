//! Sketch mode state, the sketch grid layer and the draw tools (wave B,
//! cards UI-B3 UI-SKETCH-ENTER and UI-B4 UI-SKETCH-DRAW).
//!
//! Entering sketch mode is a viewport behaviour (UI-B3 picks a datum plane on
//! click and swings the camera normal to it), but the *state*, the *grid
//! layer* and the *draw tools* live here so the viewport panel, the model
//! tree and the test harness can share them without touching `app.rs`:
//!
//! - [`SketchState`] is stored in the egui context data store, the same
//!   cross-panel seam the viewport backend and the doc panel use. A plain
//!   `String` oracle under [`SKETCH_REPORT_KEY`] is published every frame so
//!   the model tree (`tree.rs`) and the `--test sketch_*` harnesses can read
//!   the mode, the active plane and the draw-tool state through a
//!   cross-`#[path]`-copy-safe type.
//! - The three datum planes ([`datum_planes`]) are the principal world planes
//!   through the scene centre. [`pick_plane`] resolves the clicked ray (built
//!   with the viewport's existing pick seam [`oep_viewer::pick::unproject`])
//!   to the nearest plane under the cursor; no second picker is introduced.
//! - [`normal_orientation`] swings the camera to look exactly along a plane's
//!   normal while keeping the fixed world-up (`oep_viewer`'s look-at uses
//!   up = (0,1,0)) from degenerating.
//! - [`grid_overlay`] projects the active plane's sketch grid onto the panel
//!   so the viewport paints the grid layer on top of the rendered frame.
//!
//! UI-B4 UI-SKETCH-DRAW adds the five draw tools (point, line, polyline,
//! rectangle, circle) as rubber-band drawing with live preview and a typed
//! dimension readout. The interaction is driven from this module's per-frame
//! entry points (viewport.rs already calls [`read_state`] at the top of its
//! frame and [`write_state`] at the end, before and after any other viewport
//! handling), so nothing outside this file needed changing:
//!
//! - [`read_state`] runs the draw interaction: plain keys P / L / N / R / C
//!   select a tool, a primary click inside the viewport places a vertex on
//!   the active plane ([`ray_plane_uv`], the same unproject seam the picker
//!   uses), Enter finishes an open polyline, and Escape cancels **one** step
//!   (a second Escape leaves the tool; with no tool active Escape is left for
//!   the viewport panel, which exits sketch mode). The gesture state machine
//!   itself lives in [`tools`] (`src/sketch/tools.rs`), egui-free.
//! - [`write_state`] persists state + oracle every frame and then paints the
//!   tool preview from **tool state only** ([`draw_preview`]): placed-vertex
//!   markers, the rubber band to the cursor, live dimension text, and a tool
//!   legend. Nothing is committed until the gesture completes.
//! - Document commands: every completed gesture produces an
//!   [`tools::FinishedShape`]; [`commit_shape`] applies each entity through
//!   the UI-B2 journal (`history::Journal`, hashing with the engine's
//!   FNV-1a-64 scheme over the sketch document's canonical text), using only
//!   the `oep_shell::commands`-validated tokens `sk:point` and `sk:line` the
//!   UI-B1 router and the CLI speak. No module writes geometry directly; the
//!   committed sketch document (`tools::SketchDoc`) lives under
//!   [`SKETCH_DOC_KEY`] and is only ever written inside that journaled seam.
//!
//! Escape leaves sketch mode without touching a document: the viewport panel
//! consumes Escape only while a plane is active (and this module already let
//! it pass - i.e. no draw tool is active) and writes [`SketchState`] back
//! with `plane = None`; no document code is reached.

use eframe::egui;

use oep_ui_theme::{Mode, Palette};
use oep_viewer::pick::Ray;
use oep_viewer::{CpuBackend, Scene, Vec3};

// The UI-B4 draw-tool state machine (egui-free; `src/sketch/tools.rs`).
// `duplicate_mod` is allowed because this file (via viewport.rs) is the only
// compile of tools.rs in the bin build and the harness includes the whole
// sketch module a second time by `#[path]` (the same allowance UI-B2/B3 make
// for their `#[path]` seams).
#[path = "../sketch/tools.rs"]
#[allow(clippy::duplicate_mod)]
pub mod tools;

// The UI-B2 command journal, included verbatim so the draw tools commit
// through the exact journal this wave built (doc_io.rs includes the same file
// under its own module path; the two copies never share a stored value, so no
// cross-copy TypeId arises - the journal is stored and read only by this
// module's commit seam). Like doc_io.rs's include, the whole seam is allowed
// dead-code here because the bin build only drives push/len so far (the undo/
// redo affordances get wired by the later editing cards through the same
// journal).
#[path = "../history.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod history;

/// Context-data key under which the sketch [`SketchState`] struct lives
/// (viewport panel and this module only; a custom type is not safe across
/// `#[path]` copies, so the tree and tests read the `String` oracle instead).
pub const SKETCH_STATE_KEY: &str = "ui_b3_sketch_state";
/// Context-data key under which the sketch `String` oracle is published.
/// Read back by `tree.rs` and the `--test sketch_*` targets as a plain
/// `String`.
pub const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";
/// Context-data key under which the committed sketch document
/// ([`tools::SketchDoc`]) lives. Written only through [`commit_shape`]'s
/// journaled seam.
pub const SKETCH_DOC_KEY: &str = "ui_b4_sketch_doc";
/// Context-data key under which the UI-B2 [`history::Journal`] for sketch
/// commits lives.
pub const SKETCH_JOURNAL_KEY: &str = "ui_b4_sketch_journal";
/// The viewport's published scalar snapshot key (must match
/// `viewport::VIEWPORT_STATE_KEY`; a local copy avoids depending on the
/// parent module across `#[path]` compilations - the test targets read it by
/// the same literal).
pub const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";

/// Stable ids of the three principal datum planes.
pub const PLANE_XY: &str = "XY";
pub const PLANE_XZ: &str = "XZ";
pub const PLANE_YZ: &str = "YZ";

/// Spacing (world units) between sketch grid lines on the active plane.
pub const SKETCH_GRID_SPACING: f64 = 10.0;

/// The viewport snapshot tuple published by `panels/viewport.rs`:
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
type ViewportSnapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// A pickable, bounded datum plane in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatumPlane {
    /// Stable id (`XY`, `XZ` or `YZ`).
    pub id: &'static str,
    /// Human label shown in the tree and status bar.
    pub label: &'static str,
    /// Plane centre point (through the scene centre).
    pub point: Vec3,
    /// First in-plane unit axis.
    pub e1: Vec3,
    /// Second in-plane unit axis.
    pub e2: Vec3,
    /// Unit plane normal (`= e1 x e2`).
    pub normal: Vec3,
    /// Half size along `e1`.
    pub half_w: f64,
    /// Half size along `e2`.
    pub half_h: f64,
}

/// The three principal world datum planes through the scene's bounding-box
/// centre, each sized to cover the model's cross-section with a 40% margin so
/// the sheets are easy to click from the framed camera. An empty scene falls
/// back to origin planes at the demo model's footprint so picking still works.
pub fn datum_planes(scene: &Scene) -> Vec<DatumPlane> {
    let (c, mx, my, mz) = match &scene.bounds {
        Some(b) => {
            let ext = b.max.sub(&b.min);
            (b.centroid(), ext.x * 0.7, ext.y * 0.7, ext.z * 0.7)
        }
        None => (Vec3::zero(), 56.0, 28.0, 7.0),
    };
    vec![
        DatumPlane {
            id: PLANE_XY,
            label: "XY (Top)",
            point: c,
            e1: Vec3::new(1.0, 0.0, 0.0),
            e2: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            half_w: mx,
            half_h: my,
        },
        DatumPlane {
            id: PLANE_XZ,
            label: "XZ (Front)",
            point: c,
            e1: Vec3::new(1.0, 0.0, 0.0),
            e2: Vec3::new(0.0, 0.0, -1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            half_w: mx,
            half_h: mz,
        },
        DatumPlane {
            id: PLANE_YZ,
            label: "YZ (Right)",
            point: c,
            e1: Vec3::new(0.0, 1.0, 0.0),
            e2: Vec3::new(0.0, 0.0, 1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            half_w: my,
            half_h: mz,
        },
    ]
}

/// Ray-plane intersection returning the ray parameter `t` when the hit lies
/// inside the plane's rectangle; `None` on a miss, a glancing (parallel) ray
/// or a hit behind the ray origin.
fn plane_ray_hit(plane: &DatumPlane, ray: &Ray) -> Option<f64> {
    let denom = ray.dir.dot(&plane.normal);
    if denom.abs() < 1e-12 {
        return None; // ray parallel to the plane
    }
    let t = plane.point.sub(&ray.origin).dot(&plane.normal) / denom;
    if t < 0.0 {
        return None; // plane behind the ray
    }
    let hit = ray.at(t);
    let d1 = hit.sub(&plane.point).dot(&plane.e1);
    let d2 = hit.sub(&plane.point).dot(&plane.e2);
    if d1.abs() <= plane.half_w && d2.abs() <= plane.half_h {
        Some(t)
    } else {
        None
    }
}

/// The datum plane a ray picks: the nearest hit under the cursor, with an
/// exact-tie tiebreak to the sheet most normal to the view (the one the user
/// is looking at) so a click through the shared scene centre is deterministic.
pub fn pick_plane<'a>(
    planes: &'a [DatumPlane],
    ray: &Ray,
    view_dir: &Vec3,
) -> Option<&'a DatumPlane> {
    let mut best: Option<(&'a DatumPlane, f64, f64)> = None;
    for p in planes {
        let Some(t) = plane_ray_hit(p, ray) else {
            continue;
        };
        let frontal = p.normal.dot(view_dir).abs();
        let take = match best {
            None => true,
            Some((_, bt, bf)) => {
                if (t - bt).abs() > 1e-9 {
                    t < bt
                } else {
                    frontal > bf
                }
            }
        };
        if take {
            best = Some((p, t, frontal));
        }
    }
    best.map(|(p, _, _)| p)
}

/// The in-plane coordinates `(u, v)` of a point on `plane` (world point
/// projected onto the plane's local axes through the plane centre).
fn plane_uv_of(plane: &DatumPlane, world: &Vec3) -> (f64, f64) {
    let d = world.sub(&plane.point);
    (d.dot(&plane.e1), d.dot(&plane.e2))
}

/// The world point on `plane` for in-plane coordinates `(u, v)`.
fn plane_uv_to_world(plane: &DatumPlane, uv: (f64, f64)) -> Vec3 {
    plane
        .point
        .add(&plane.e1.mul(uv.0))
        .add(&plane.e2.mul(uv.1))
}

/// The `(u, v)` plane coordinates where `ray` pierces `plane`, `None` when it
/// misses (outside the sheet, parallel, or behind the ray origin). UI-B4 uses
/// this to resolve a draw click/cursor onto the active plane - the same
/// unproject seam UI-B3's picker uses, so a drawn vertex lands exactly where
/// a plane pick would have.
pub fn ray_plane_uv(plane: &DatumPlane, ray: &Ray) -> Option<(f64, f64)> {
    let hit = ray.at(plane_ray_hit(plane, ray)?);
    Some(plane_uv_of(plane, &hit))
}

/// Yaw/pitch that make the camera look exactly along world `-n` (a plane
/// normal), so the view is normal to the plane. `pitch` is clamped away from
/// exactly +/- 90 deg so `oep_viewer`'s fixed world-up look-at (up = (0,1,0))
/// never degenerates on a vertical plane normal.
pub fn normal_orientation(n: &Vec3) -> (f64, f64) {
    let pitch = n.y.clamp(-1.0, 1.0).asin().clamp(-1.54, 1.54);
    let yaw = n.x.atan2(n.z);
    (yaw, pitch)
}

/// Which datum plane is currently being sketched on.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivePlane {
    /// Plane id (`XY`, `XZ` or `YZ`).
    pub id: String,
    /// Human label.
    pub label: String,
    /// Unit plane normal `(x, y, z)`.
    pub normal: (f64, f64, f64),
}

/// Sketch mode state, stored in the egui context data store.
///
/// UI-B4 adds the draw gesture ([`SketchState::draw`]) and a one-line status
/// ([`SketchState::last`]) to the UI-B3 struct; viewport.rs only ever touches
/// [`SketchState::plane`], so the additions are invisible to it and survive
/// its read/modify/write of the whole struct.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchState {
    /// `Some(plane)` while sketching on a datum plane; `None` in normal
    /// modelling mode. The sketch grid layer is active exactly when this is
    /// `Some`.
    pub plane: Option<ActivePlane>,
    /// The active draw gesture (tool + pending preview vertices + cursor).
    pub draw: tools::Gesture,
    /// One-line status of the last draw action (for the oracle / preview).
    pub last: Option<String>,
}

impl SketchState {
    /// True while a datum plane is active (sketch mode on, grid layer on).
    pub fn in_sketch(&self) -> bool {
        self.plane.is_some()
    }
}

/// Read the sketch state from the egui context data store, running the
/// UI-B4 draw interaction first (this is the first thing viewport.rs calls
/// each frame, so tool keys, Escape, and draw clicks are handled before the
/// viewport panel re-picks a plane or exits on Escape its own way).
pub fn read_state(ctx: &egui::Context) -> SketchState {
    let mut state = ctx
        .data(|d| d.get_temp::<SketchState>(egui::Id::unique(SKETCH_STATE_KEY)))
        .unwrap_or_default();
    update_draw(ctx, &mut state);
    state
}

/// Rebuild the datum planes through the same demo scene the viewport renders.
fn demo_scene() -> Scene {
    CpuBackend::demo_scene()
}

/// The datum plane behind `state.plane`, if any.
fn active_datum_plane(_ctx: &egui::Context, state: &SketchState) -> Option<DatumPlane> {
    let active = state.plane.as_ref()?;
    datum_planes(&demo_scene())
        .into_iter()
        .find(|p| p.id == active.id)
}

/// The current viewport camera, rebuilt from the published scalar snapshot.
fn current_camera(ctx: &egui::Context) -> Option<oep_viewer::Camera> {
    let s = ctx.data(|d| d.get_temp::<ViewportSnapshot>(egui::Id::unique(VIEWPORT_STATE_KEY)))?;
    Some(oep_viewer::Camera {
        target: Vec3::new(s.3, s.4, s.5),
        yaw: s.0,
        pitch: s.1,
        distance: s.2,
        fov_y: s.6,
        near: 0.1,
        far: 5000.0,
    })
}

/// The exact `window` rect the shell regions are laid out inside
/// (`app::App::show_shell` computes it the same way: the central panel's
/// content rect minus the central panel's inner margin). Both come from the
/// same [`egui::Frame`] egui draws, so the rect this module derives for the
/// draw interaction matches the viewport `rect` viewport.rs uses pixel for
/// pixel.
fn shell_window(ctx: &egui::Context) -> egui::Rect {
    let style = ctx.global_style();
    let margin = egui::Frame::central_panel(style.as_ref()).inner_margin;
    let content = ctx.content_rect();
    egui::Rect::from_min_max(
        egui::pos2(
            content.min.x + margin.leftf(),
            content.min.y + margin.topf(),
        ),
        egui::pos2(
            content.max.x - margin.rightf(),
            content.max.y - margin.bottomf(),
        ),
    )
}

/// The viewport panel's rect, recomputed from the loaded shell layout and the
/// real window rect - the same `ShellLayout::regions(window).viewport` the
/// app used to position the panel. Public so the test harness can assert it
/// matches the AccessKit-reported region bounds (and therefore that a click
/// resolved here lands where the harness clicks).
pub fn viewport_rect(ctx: &egui::Context) -> egui::Rect {
    let layout =
        crate::config::ShellLayout::load_from(&crate::config::layout_path()).unwrap_or_default();
    layout.regions(shell_window(ctx)).viewport
}

/// Resolve a screen position onto the active plane as `(u, v)` plane
/// coordinates, through the viewport's pick seam (unproject then
/// ray-plane hit). `None` off the plane or behind the camera.
fn plane_uv_at(
    ctx: &egui::Context,
    state: &SketchState,
    rect: egui::Rect,
    pos: egui::Pos2,
) -> Option<(f64, f64)> {
    let cam = current_camera(ctx)?;
    let plane = active_datum_plane(ctx, state)?;
    let scale = ctx.pixels_per_point();
    let cx = ((pos.x - rect.min.x) * scale).max(0.0) as f64;
    let cy = ((pos.y - rect.min.y) * scale).max(0.0) as f64;
    let px = (rect.width() * scale).round().max(2.0) as f64;
    let py = (rect.height() * scale).round().max(2.0) as f64;
    let ray = oep_viewer::pick::unproject(&cam, cx, cy, px, py);
    ray_plane_uv(&plane, &ray)
}

/// The egui `Key` a tool's plain key-char maps to (P / L / N / R / C).
fn tool_key(tool: tools::Tool) -> egui::Key {
    match tool.key_char() {
        'P' => egui::Key::P,
        'L' => egui::Key::L,
        'N' => egui::Key::N,
        'R' => egui::Key::R,
        'C' => egui::Key::C,
        other => panic!("tool key char {other:?} has no egui Key"),
    }
}

/// The per-frame draw interaction, run from [`read_state`] before the viewport
/// panel handles clicks/Escape. Only acts while a plane is active (sketch
/// mode):
///
/// 1. plain keys select a tool (and drop any partial gesture);
/// 2. Enter finishes an open chain (polyline);
/// 3. Escape cancels **one** step; with nothing left it leaves the tool.
///    With no tool active the key is *not* consumed, so the viewport panel
///    (which runs next) still uses it to exit sketch mode;
/// 4. a primary click inside the viewport places a vertex on the active
///    plane; a completing click returns a [`tools::FinishedShape`] which is
///    committed through the journal ([`commit_shape`]);
/// 5. the hovered cursor position is refreshed for the rubber band.
fn update_draw(ctx: &egui::Context, state: &mut SketchState) {
    if !state.in_sketch() {
        return;
    }
    let rect = viewport_rect(ctx);

    // 1. Tool selection (plain letters; the input table's bindings all
    //    require a command modifier, so these never fight a shortcut).
    for tool in tools::Tool::ALL {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, tool_key(tool))) {
            state.draw.set_tool(tool);
            state.last = Some(format!("{} tool", tool.label()));
            break;
        }
    }

    // 2. Enter finishes an open chain (polyline).
    if state.draw.is_active()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
    {
        if let Some(shape) = state.draw.finish() {
            commit_shape(ctx, state, &shape);
        }
    }

    // 3. Escape: the card semantics live in the gesture machine - one
    //    step cancelled, then the tool is left. Read first (non-consuming)
    //    so a press with no tool active is left for the viewport panel to
    //    exit sketch.
    if state.draw.is_active() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        });
        state.last = Some(match state.draw.escape() {
            tools::EscapeOutcome::CancelledOneStep => "cancelled one step".to_string(),
            tools::EscapeOutcome::LeftTool => "left draw tool".to_string(),
        });
    }

    // 4. A fresh primary click inside the viewport places a vertex (the
    //    gesture machine decides when that completes into a commit).
    let (latest, interact, clicked) = ctx.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.interact_pos(),
            i.pointer.primary_clicked(),
        )
    });
    if state.draw.is_active() {
        if clicked {
            if let Some(pos) = interact {
                if rect.contains(pos) {
                    if let Some(uv) = plane_uv_at(ctx, state, rect, pos) {
                        match state.draw.step(uv) {
                            tools::StepOutcome::Continuing => {}
                            tools::StepOutcome::Completed(shape) => {
                                commit_shape(ctx, state, &shape);
                            }
                        }
                    }
                }
            }
        }
        // 5. Rubber-band cursor: the hovered plane coordinate (or none).
        state.draw.cursor = latest.and_then(|pos| {
            if rect.contains(pos) {
                plane_uv_at(ctx, state, rect, pos)
            } else {
                None
            }
        });
    } else {
        state.draw.cursor = None;
    }
}

/// Commit one completed shape into the sketch document through the UI-B2
/// journal. Each entity is one discrete command (`sk:point` per point,
/// `sk:line` per line, `sk:point` for a circle's centre) whose token is one
/// `oep_shell::commands` validates - the same registry the UI-B1 router and
/// the CLI speak - and each is journaled as hash-before -> mutate -> hash-after
/// with the B2 [`history::Journal`]. This is the ONLY writer of the sketch
/// document; nothing writes geometry directly.
fn commit_shape(ctx: &egui::Context, state: &mut SketchState, shape: &tools::FinishedShape) {
    let mut doc: tools::SketchDoc = ctx
        .data(|d| d.get_temp(egui::Id::unique(SKETCH_DOC_KEY)))
        .unwrap_or_default();
    let mut journal: history::Journal = ctx
        .data(|d| d.get_temp(egui::Id::unique(SKETCH_JOURNAL_KEY)))
        .unwrap_or_default();

    debug_assert!(
        tools::command_is_registered(tools::POINT_TOKEN)
            && tools::command_is_registered(tools::LINE_TOKEN),
        "the tool's command lines must stay registered (family validates, name resolves)"
    );

    // Points first so line commands can name the assigned ids (p0, p1, ...).
    let mut point_ids: Vec<String> = Vec::with_capacity(shape.points.len());
    for &(x, y) in &shape.points {
        let before = doc.hash();
        let id = doc.add_point(x, y);
        let after = doc.hash();
        journal.push(tools::POINT_TOKEN, None, before, after);
        point_ids.push(id);
    }
    for &(a, b) in &shape.lines {
        let before = doc.hash();
        doc.add_line(&point_ids[a], &point_ids[b]);
        let after = doc.hash();
        journal.push(tools::LINE_TOKEN, None, before, after);
    }
    for &(c, r) in &shape.circles {
        let before = doc.hash();
        doc.add_circle(&point_ids[c], r);
        let after = doc.hash();
        journal.push(tools::POINT_TOKEN, None, before, after);
    }

    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::unique(SKETCH_DOC_KEY), doc);
        d.insert_temp(egui::Id::unique(SKETCH_JOURNAL_KEY), journal);
    });

    let tool = state.draw.tool.map(|t| t.label()).unwrap_or("shape");
    state.last = Some(format!(
        "{tool}: {} points, {} lines, {} circles",
        shape.points.len(),
        shape.lines.len(),
        shape.circles.len()
    ));
}

/// The `String` oracle lines that describe the draw-tool state (appended to
/// the UI-B3 base lines by [`write_state`]): the active tool, the pending
/// preview steps and the live dimension readout.
fn draw_report_lines(state: &SketchState) -> String {
    let d = &state.draw;
    let mut out = String::new();
    out.push_str(&format!(
        "tool={}\n",
        d.tool
            .map(|t| t.id().to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!("steps={}\n", d.pending_points()));
    out.push_str(&format!("preview_points={}\n", d.pending_points()));
    out.push_str(&format!("preview_lines={}\n", d.pending_lines()));

    // Live "typed dimension" readout of the preview: the bounding width and
    // height from the first placed anchor to the cursor (or last placed
    // vertex), in plane units.
    let dim = match (
        d.vertices.first().copied(),
        d.vertices.last().copied().or(d.cursor),
    ) {
        (Some(a), Some(b)) => Some(((b.0 - a.0).abs(), (b.1 - a.1).abs())),
        _ => None,
    };
    out.push_str(&format!(
        "dimx={}\n",
        dim.map(|(w, _)| format!("{w:.3}"))
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "dimy={}\n",
        dim.map(|(_, h)| format!("{h:.3}"))
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!("last={}\n", state.last.as_deref().unwrap_or("-")));
    out
}

/// The `String` oracle's committed-document block (appended by [`write_state`]
/// from the sketch document and the journal in the context store): the sketch
/// document's point/line/circle counts and the journal entry count.
fn doc_report_lines(doc: &tools::SketchDoc, journal: &history::Journal) -> String {
    let mut out = String::new();
    out.push_str(&format!("docpoints={}\n", doc.points.len()));
    out.push_str(&format!("doclines={}\n", doc.lines.len()));
    out.push_str(&format!("circles={}\n", doc.circles.len()));
    out.push_str(&format!("journal={}\n", journal.len()));
    // The committed entities themselves (plane coords / endpoint ids), so the
    // acceptance can assert what actually landed, not just counts.
    for p in &doc.points {
        out.push_str(&format!("point={:.3},{:.3},{}\n", p.x, p.y, p.id));
    }
    for l in &doc.lines {
        out.push_str(&format!("lineseg={}:{}->{}\n", l.id, l.start, l.end));
    }
    out
}

/// The full oracle text for the current sketch state (mode / plane / grid,
/// the draw-tool state, and the committed sketch-document counts).
pub fn report(state: &SketchState, doc: &tools::SketchDoc, journal: &history::Journal) -> String {
    let mut out = match &state.plane {
        None => "mode=idle\nplane=-\ngrid=off\n".to_string(),
        Some(p) => format!(
            "mode=sketch\nplane={}\nlabel={}\nnormal={},{},{}\ngrid=on\n",
            p.id, p.label, p.normal.0, p.normal.1, p.normal.2
        ),
    };
    out.push_str(&draw_report_lines(state));
    out.push_str(&doc_report_lines(doc, journal));
    out
}

/// Persist the sketch state: the struct (for the viewport panel) plus the
/// plain `String` oracle (for the tree and the test harness). Runs at the end
/// of the viewport's frame, when the camera snapshot is current, and then
/// paints the draw-tool preview from tool state.
pub fn write_state(ctx: &egui::Context, state: &SketchState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(SKETCH_STATE_KEY), state.clone()));

    let doc: tools::SketchDoc = ctx
        .data(|d| d.get_temp(egui::Id::unique(SKETCH_DOC_KEY)))
        .unwrap_or_default();
    let journal: history::Journal = ctx
        .data(|d| d.get_temp(egui::Id::unique(SKETCH_JOURNAL_KEY)))
        .unwrap_or_default();
    ctx.data_mut(|d| {
        d.insert_temp::<String>(
            egui::Id::unique(SKETCH_REPORT_KEY),
            report(state, &doc, &journal),
        );
    });

    draw_preview(ctx, state);
}

/// Project a world point onto the viewport panel in logical points (top-left
/// origin), mirroring the picker's unproject divide. `None` behind the near
/// plane (w <= 0). Public so the test harness can turn a world point on a
/// datum plane into the exact click position the panel would resolve.
pub fn world_to_panel(
    cam: &oep_viewer::Camera,
    panel: egui::Rect,
    scale: f32,
    p: &Vec3,
) -> Option<egui::Pos2> {
    let px = (panel.width() * scale).round().max(2.0) as f64;
    let py = (panel.height() * scale).round().max(2.0) as f64;
    let vp = cam.view_projection(px / py);
    let a = vp.to_array16();
    let cw = a[3] * p.x + a[7] * p.y + a[11] * p.z + a[15];
    if cw <= 0.0 {
        return None;
    }
    let iw = 1.0 / cw;
    let cx = (a[0] * p.x + a[4] * p.y + a[8] * p.z + a[12]) * iw;
    let cy = (a[1] * p.x + a[5] * p.y + a[9] * p.z + a[13]) * iw;
    let nx = cx * 0.5 + 0.5;
    let ny = 1.0 - (cy * 0.5 + 0.5); // screen y grows downward
    Some(egui::pos2(
        panel.min.x + (nx * px / f64::from(scale)) as f32,
        panel.min.y + (ny * py / f64::from(scale)) as f32,
    ))
}

/// The sketch grid layer for the active plane, pre-projected to panel points.
#[derive(Debug, Clone, Default)]
pub struct GridOverlay {
    /// The four projected plane corners (the highlight outline).
    pub corners: Vec<egui::Pos2>,
    /// Projected grid segments inside the plane rectangle.
    pub segments: Vec<(egui::Pos2, egui::Pos2)>,
    /// The projected plane centre (pick-marker anchor).
    pub centre: egui::Pos2,
}

/// Compute the sketch grid + highlight geometry for a plane over the panel.
/// `None` when the plane would be drawn behind the camera.
pub fn grid_overlay(
    plane: &DatumPlane,
    cam: &oep_viewer::Camera,
    panel: egui::Rect,
    scale: f32,
) -> Option<GridOverlay> {
    let corner = |u: f64, v: f64| {
        world_to_panel(
            cam,
            panel,
            scale,
            &plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(v)),
        )
    };
    let (w, h) = (plane.half_w, plane.half_h);
    let corners: Vec<egui::Pos2> = [(-w, -h), (w, -h), (w, h), (-w, h)]
        .iter()
        .map(|&(u, v)| corner(u, v))
        .collect::<Option<_>>()?;
    let centre = corner(0.0, 0.0)?;

    let mut segments = Vec::new();
    let mut add_line = |a: Vec3, b: Vec3| {
        if let (Some(pa), Some(pb)) = (
            world_to_panel(cam, panel, scale, &a),
            world_to_panel(cam, panel, scale, &b),
        ) {
            segments.push((pa, pb));
        }
    };
    let n1 = (plane.half_w / SKETCH_GRID_SPACING).floor() as i64;
    let n2 = (plane.half_h / SKETCH_GRID_SPACING).floor() as i64;
    for i in -n1..=n1 {
        let u = i as f64 * SKETCH_GRID_SPACING;
        let a = plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(-h));
        let b = plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(h));
        add_line(a, b);
    }
    for j in -n2..=n2 {
        let v = j as f64 * SKETCH_GRID_SPACING;
        let a = plane.point.add(&plane.e2.mul(v)).add(&plane.e1.mul(-w));
        let b = plane.point.add(&plane.e2.mul(v)).add(&plane.e1.mul(w));
        add_line(a, b);
    }

    Some(GridOverlay {
        corners,
        segments,
        centre,
    })
}

/// (Re)build an `oep_ui_theme` palette for the mode currently installed on
/// `ctx` (the same derivation `app.rs`'s region painter uses).
fn palette(ctx: &egui::Context) -> Palette {
    let mode = if ctx.global_style().visuals.dark_mode {
        Mode::Dark
    } else {
        Mode::Light
    };
    Palette::resolve(mode)
}

/// Paint the draw-tool preview from **tool state only** - placed-vertex
/// markers, the chain/rubber band, the circle bound, a live dimension
/// readout and a tool legend - in a non-interactive foreground layer over the
/// viewport. Runs at the end of the viewport frame ([`write_state`]); nothing
/// here reads or writes the document.
fn draw_preview(ctx: &egui::Context, state: &SketchState) {
    if !state.in_sketch() {
        return;
    }
    let draw = &state.draw;
    let anything = draw.is_active()
        || !draw.vertices.is_empty()
        || draw.cursor.is_some()
        || state.last.is_some();
    if !anything {
        return;
    }
    let Some(plane) = active_datum_plane(ctx, state) else {
        return;
    };
    let Some(cam) = current_camera(ctx) else {
        return;
    };
    let rect = viewport_rect(ctx);
    let scale = ctx.pixels_per_point();
    let p = palette(ctx);

    let anchor = |uv: (f64, f64)| -> Option<egui::Pos2> {
        let world = plane_uv_to_world(&plane, uv);
        world_to_panel(&cam, rect, scale, &world).filter(|pos| rect.contains(*pos))
    };

    egui::Area::new(egui::Id::unique("ui_b4_draw_preview"))
        .order(egui::Order::Foreground)
        .interactable(false)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Legend: the active tool (or a hint when nothing is selected)
            // plus what is pending.
            let legend = match draw.tool {
                Some(tool) => format!(
                    "{} · {} step(s) placed",
                    tool.label(),
                    draw.pending_points()
                ),
                None => "No draw tool · P L N R C".to_string(),
            };
            painter.text(
                egui::pos2(rect.min.x + 8.0, rect.min.y + 8.0),
                egui::Align2::LEFT_TOP,
                legend,
                egui::FontId::proportional(12.0),
                p.text_muted,
            );

            // Placed-vertex markers (the committed shape's anchors).
            for &uv in &draw.vertices {
                if let Some(pos) = anchor(uv) {
                    cross(painter, pos, 4.0, p.accent);
                }
            }

            // Chain among placed vertices (Line / Polyline / Rectangle).
            let placed: Vec<egui::Pos2> =
                draw.vertices.iter().filter_map(|&uv| anchor(uv)).collect();
            for w in placed.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, p.accent));
            }

            // Rubber band: last placed vertex to the cursor.
            if let (Some(&last), Some(cur)) = (draw.vertices.last(), draw.cursor) {
                if let (Some(a), Some(b)) = (anchor(last), anchor(cur)) {
                    painter.line_segment([a, b], egui::Stroke::new(1.5, p.border));
                }
            }

            // Circle bound: centre -> cursor radius while placing a circle.
            if draw.tool == Some(tools::Tool::Circle) {
                if let (Some(&centre), Some(cur)) = (draw.vertices.first(), draw.cursor) {
                    if let (Some(cen_pos), Some(cur_pos)) = (anchor(centre), anchor(cur)) {
                        let r = cen_pos.distance(cur_pos);
                        painter.circle_stroke(cen_pos, r, egui::Stroke::new(1.5, p.border));
                    }
                }
            }

            // Live dimension readout near the cursor (the "typed dimensions"
            // surface: numeric width/height/length/radius as the band grows).
            let label_pos = draw
                .cursor
                .and_then(|uv| anchor(uv).map(|pos| egui::pos2(pos.x + 10.0, pos.y - 18.0)));
            let text = preview_dimensions(state)
                .or_else(|| state.last.clone())
                .unwrap_or_default();
            if !text.is_empty() {
                if let Some(pos) = label_pos.or_else(|| rect_inside(rect.center(), rect)) {
                    painter.text(
                        pos,
                        egui::Align2::LEFT_BOTTOM,
                        text,
                        egui::FontId::monospace(12.0),
                        p.text_muted,
                    );
                }
            }
        });
}

/// Clamp `pos` into `rect` (so labels never draw off-panel).
fn rect_inside(pos: egui::Pos2, rect: egui::Rect) -> Option<egui::Pos2> {
    let x = pos.x.clamp(rect.min.x + 2.0, rect.max.x - 2.0);
    let y = pos.y.clamp(rect.min.y + 2.0, rect.max.y - 2.0);
    Some(egui::pos2(x, y))
}

/// The live dimension text of the rubber band, from tool state only.
fn preview_dimensions(state: &SketchState) -> Option<String> {
    let d = &state.draw;
    let tool = d.tool?;
    let text = match tool {
        tools::Tool::Point => return None,
        tools::Tool::Line | tools::Tool::Polyline => {
            let a = d.vertices.first().copied()?;
            let b = d.vertices.last().copied().or(d.cursor)?;
            format!("len {:.2}", tools::distance(a, b))
        }
        tools::Tool::Rectangle => {
            let a = d.vertices.first().copied()?;
            let b = d.vertices.last().copied().or(d.cursor)?;
            format!("{:.2} x {:.2}", (b.0 - a.0).abs(), (b.1 - a.1).abs())
        }
        tools::Tool::Circle => {
            let a = d.vertices.first().copied()?;
            let b = d.cursor?;
            format!("r {:.2}", tools::distance(a, b))
        }
    };
    Some(text)
}

/// Draw a small cross marker at `pos` (rubber-band anchor / cursor).
fn cross(painter: &egui::Painter, pos: egui::Pos2, r: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.5, color);
    painter.line_segment(
        [egui::pos2(pos.x - r, pos.y), egui::pos2(pos.x + r, pos.y)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(pos.x, pos.y - r), egui::pos2(pos.x, pos.y + r)],
        stroke,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_viewer::pick::unproject;
    use oep_viewer::Camera;

    /// Each plane's normal is the cross of its in-plane axes (the invariant
    /// `grid_overlay` and the camera swing rely on).
    #[test]
    fn every_plane_has_consistent_axes() {
        let scene = oep_viewer::CpuBackend::demo_scene();
        for p in datum_planes(&scene) {
            let n = p.e1.cross(&p.e2);
            assert!(
                n.sub(&p.normal).norm() < 1e-12,
                "{} normal must equal e1 x e2",
                p.id
            );
            assert!((p.e1.norm() - 1.0).abs() < 1e-12);
            assert!((p.e2.norm() - 1.0).abs() < 1e-12);
        }
    }

    /// A centre-ish click (the default framed camera looks at the scene
    /// centre, where all three sheets cross) picks the sheet most normal to
    /// the view, and the chosen plane is deterministic.
    #[test]
    fn nearest_and_most_frontal_plane_wins() {
        let scene = oep_viewer::CpuBackend::demo_scene();
        let planes = datum_planes(&scene);
        let cam = Camera::default();
        let f = cam.target.sub(&cam.eye()).normalize().expect("camera");
        let ray = unproject(&cam, 400.0, 300.0, 800.0, 600.0);
        let picked = pick_plane(&planes, &ray, &f).expect("centre click picks a plane");
        let most_frontal = planes
            .iter()
            .max_by(|a, b| {
                a.normal
                    .dot(&f)
                    .abs()
                    .partial_cmp(&b.normal.dot(&f).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("planes are non-empty");
        assert_eq!(picked.id, most_frontal.id);
        assert_eq!(
            picked.id, PLANE_XY,
            "from the default view the top sheet faces the camera"
        );
    }

    /// normal_orientation turns the camera to look exactly along the normal:
    /// for the safe (non-vertical) normals the forward vector is parallel to
    /// the plane normal within tight tolerance.
    #[test]
    fn orientation_looks_along_the_normal() {
        for n in [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(-1.0, 0.0, 0.0),
        ] {
            let (yaw, pitch) = normal_orientation(&n);
            let cam = Camera {
                yaw,
                pitch,
                ..Camera::default()
            };
            let f = cam
                .target
                .sub(&cam.eye())
                .normalize()
                .expect("non-degenerate camera");
            let cross_mag = f.cross(&n).norm();
            assert!(
                cross_mag < 1e-6,
                "camera forward must be normal to the plane (cross={cross_mag:e})"
            );
        }
    }

    /// The vertical plane normal stays safe: orientation never points the
    /// fixed-up camera straight along the world Y axis (which would make the
    /// look-at degenerate); it clamps within the camera's own safe band.
    #[test]
    fn vertical_normal_never_degenerates() {
        let (yaw, pitch) = normal_orientation(&Vec3::new(0.0, 1.0, 0.0));
        assert!(pitch.abs() < 1.55, "pitch clamped off vertical");
        let cam = Camera {
            yaw,
            pitch,
            ..Camera::default()
        };
        let f = cam
            .target
            .sub(&cam.eye())
            .normalize()
            .expect("still not degenerate");
        let _ = cam.view_matrix(); // must not panic
        assert!(
            f.cross(&Vec3::new(0.0, 1.0, 0.0)).norm() > 1e-3,
            "forward must keep a safe angle away from the world up axis"
        );
    }

    /// A draw click round-trips exactly: resolving a pixel produced by
    /// `world_to_panel` for a point ON the plane back through the same
    /// unproject seam returns the same plane coordinates (relative to the
    /// plane centre, which for the demo scene sits at the part's centroid).
    #[test]
    fn plane_uv_round_trips_a_world_point_on_the_plane() {
        let scene = oep_viewer::CpuBackend::demo_scene();
        let plane = datum_planes(&scene)
            .into_iter()
            .find(|p| p.id == PLANE_XY)
            .expect("XY plane");
        let cam = Camera::default();
        let panel = egui::Rect::from_min_max(egui::pos2(240.0, 32.0), egui::pos2(980.0, 774.0));
        let scale = 1.0f32;

        // A point ON the XY sheet (z = plane centre z), inside its bounds.
        // uv is measured from the plane centre.
        let uv_want = (12.0, -6.0);
        let world = plane_uv_to_world(&plane, uv_want);
        let pos = world_to_panel(&cam, panel, scale, &world).expect("in front of camera");
        let cx = ((pos.x - panel.min.x) * scale).max(0.0) as f64;
        let cy = ((pos.y - panel.min.y) * scale).max(0.0) as f64;
        let px = (panel.width() * scale).round().max(2.0) as f64;
        let py = (panel.height() * scale).round().max(2.0) as f64;
        let ray = unproject(&cam, cx, cy, px, py);
        let uv = ray_plane_uv(&plane, &ray).expect("click on the plane resolves");
        assert!(
            (uv.0 - uv_want.0).abs() < 1e-6 && (uv.1 - uv_want.1).abs() < 1e-6,
            "click uv must round-trip to the world point on the plane: {uv:?} want {uv_want:?}"
        );
    }

    /// The canonical document text/hash are deterministic and the report
    /// oracle carries the UI-B3 fields unchanged (the tree and sketch_enter
    /// depend on those exact prefixes).
    #[test]
    fn report_keeps_uib3_fields_and_carries_doc_counts() {
        let mut doc = tools::SketchDoc::default();
        let p0 = doc.add_point(10.0, 10.0);
        let p1 = doc.add_point(60.0, 10.0);
        doc.add_line(&p0, &p1);
        let mut journal = history::Journal::new();
        journal.push(tools::POINT_TOKEN, None, 1, 2);

        let mut state = SketchState {
            plane: Some(ActivePlane {
                id: PLANE_XY.to_string(),
                label: "XY (Top)".to_string(),
                normal: (0.0, 0.0, 1.0),
            }),
            ..Default::default()
        };
        state.last = Some("Line: 2 points, 1 lines, 0 circles".to_string());

        let text = report(&state, &doc, &journal);
        // UI-B3 fields byte-identical prefixes the tree/sketch_enter parse.
        assert!(text.starts_with("mode=sketch\nplane=XY\nlabel=XY (Top)\n"));
        assert!(text.contains("grid=on\n"));
        assert!(text.contains("normal=0,0,1\n"));
        // UI-B4 additions.
        assert!(text.contains("tool=-\n"));
        assert!(text.contains("steps=0\n"));
        assert!(text.contains("docpoints=2\n"));
        assert!(text.contains("doclines=1\n"));
        assert!(text.contains("journal=1\n"));
        assert!(text.contains("last=Line: 2 points, 1 lines, 0 circles\n"));

        // Idle branch keeps the UI-B3 idle prefixes too.
        let idle = report(
            &SketchState::default(),
            &tools::SketchDoc::default(),
            &history::Journal::new(),
        );
        assert!(idle.starts_with("mode=idle\nplane=-\ngrid=off\n"));
    }
}
