//! UI-D3 UI-PICK: id-buffer picking, hover pre-highlight, box select (wave D).
//!
//! Acceptance harness. Two layers:
//!
//! 1. **Id-buffer vs analytic oracle (numbers).** A three-cube scene is
//!    rasterized into an [`oep_viewer::gpu::idbuffer::IdPass`]; 20 sampled
//!    screen points that are genuinely ON a drawn face have their id-buffer
//!    identity compared to the analytic ray pick (`oep_viewer::pick`) for the
//!    SAME camera, printed one per line, all equal.
//!
//! 2. **Real-app kittest interactions.** The real `app::App` runs with the
//!    viewport backend AND the pick surface pointed at the same three-cube
//!    scene (so selection comes from what is actually drawn, literally):
//!    hover sets the pre-highlight within ONE frame (and no second frame was
//!    needed); shift-click accumulates the exact multi-select id list; an
//!    alt-drag box over a committed viewport rect returns exactly the
//!    enclosed set (analytic grid expectation printed against the returned
//!    ids, must match); a plain click on empty background clears the
//!    selection and picks nothing.
//!
//! The scene has three separated cubes (`cube-a` / `cube-b` / `cube-c`) so a
//! box can enclose exactly two of them with clean background gaps.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

use egui::PointerButton;
use oep_viewer::Vec3;

/// The viewport panel's context-data key (must match
/// `viewport::VIEWPORT_STATE_KEY`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";
/// The pick panel's scene-override key (must match
/// `pick_gpu::PICK_SCENE_KEY`).
const PICK_SCENE_KEY: &str = "ui_d3_pick_scene";
/// The pick panel's oracle key (must match `pick_gpu::PICK_ORACLE_KEY`).
const PICK_ORACLE_KEY: &str = "ui_d3_pick_oracle";

/// The panel's scalar snapshot tuple.
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

// ---------------------------------------------------------------------------
// Fixture: three separated cubes
// ---------------------------------------------------------------------------

/// An axis-aligned cube mesh (8 vertices, 12 triangles, 12 boundary edges)
/// centred at `center` with half side `half`.
fn cube_mesh(center: (f64, f64, f64), half: f64) -> oep_viewer::MeshData {
    let (cx, cy, cz) = center;
    let h = half;
    let positions: Vec<[f32; 3]> = [
        [cx - h, cy - h, cz - h],
        [cx + h, cy - h, cz - h],
        [cx + h, cy + h, cz - h],
        [cx - h, cy + h, cz - h],
        [cx - h, cy - h, cz + h],
        [cx + h, cy - h, cz + h],
        [cx + h, cy + h, cz + h],
        [cx - h, cy + h, cz + h],
    ]
    .into_iter()
    .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
    .collect();
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2], // -z
        [4, 5, 6],
        [4, 6, 7], // +z
        [0, 1, 5],
        [0, 5, 4], // -y
        [2, 3, 7],
        [2, 7, 6], // +y
        [0, 4, 7],
        [0, 7, 3], // -x
        [1, 2, 6],
        [1, 6, 5], // +x
    ];
    let edges: Vec<[usize; 2]> = vec![
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
    oep_viewer::MeshData {
        positions,
        triangles,
        edges,
    }
}

/// The committed three-cube scene: A at x=50, B at x=130, C at x=210, all
/// 20 tall/wide/deep. The 60-unit gaps make the enclosures unambiguous.
fn three_cubes_scene() -> oep_viewer::Scene {
    let a =
        oep_viewer::Primitive::from_mesh("cube-a", "pa", "ba", cube_mesh((50.0, 0.0, 10.0), 10.0));
    let b =
        oep_viewer::Primitive::from_mesh("cube-b", "pb", "bb", cube_mesh((130.0, 0.0, 10.0), 10.0));
    let c =
        oep_viewer::Primitive::from_mesh("cube-c", "pc", "bc", cube_mesh((210.0, 0.0, 10.0), 10.0));
    let mut scene = oep_viewer::Scene::default();
    let prims = vec![a, b, c];
    let bounds = prims.iter().map(|p| p.world).reduce(|a, b| a.union(&b));
    scene.primitives = prims;
    scene.bounds = bounds;
    scene
}

// ---------------------------------------------------------------------------
// Projection helpers (the same math the viewport panels use, kept local)
// ---------------------------------------------------------------------------

/// Rebuild the exact camera the panel holds from the published scalars.
fn camera_from_snapshot(s: &Snapshot) -> oep_viewer::Camera {
    oep_viewer::Camera {
        target: oep_viewer::Vec3::new(s.3, s.4, s.5),
        yaw: s.0,
        pitch: s.1,
        distance: s.2,
        fov_y: s.6,
        near: 0.1,
        far: 5000.0,
    }
}

/// World point -> panel (logical) coordinates, replicating the ID PASS's /
/// `oep_viewer::pick` projection (the standard perspective divide
/// `nx = clip.x / -view.z`). A click computed here lands on the pixel the
/// analytic pick resolves (and, after the D2 correct+regolden, the drawn one).
fn world_to_panel(
    cam: &oep_viewer::Camera,
    panel: egui::Rect,
    scale: f32,
    p: &Vec3,
) -> Option<egui::Pos2> {
    let (sx, sy) = world_to_screen(cam, p, panel.width() * scale, panel.height() * scale)?;
    Some(egui::pos2(
        panel.min.x + (sx as f32 / scale),
        panel.min.y + (sy as f32 / scale),
    ))
}

/// World point -> raw pixel coordinates (top-left origin, pixel units), with
/// the same projection. Used for the analytic grid.
fn world_to_px(cam: &oep_viewer::Camera, p: &Vec3, w: f64, h: f64) -> Option<(f64, f64)> {
    let (sx, sy) = world_to_screen(cam, p, w as f32, h as f32)?;
    Some((sx, sy))
}

/// The ID-PASS-exact screen pixel for a world point (see `world_to_panel`).
fn world_to_screen(cam: &oep_viewer::Camera, p: &Vec3, w: f32, h: f32) -> Option<(f64, f64)> {
    let pixel_w = (w as f64).max(2.0);
    let pixel_h = (h as f64).max(2.0);
    let world_to_view = cam.view_matrix();
    let projection = cam.projection(pixel_w / pixel_h);
    let view = world_to_view.transform_point(p);
    let dw = -view.z;
    if dw.abs() < 1e-9 {
        return None;
    }
    let clip = projection.transform_point(&view);
    let nx = clip.x / dw;
    let ny = clip.y / dw;
    let nz = clip.z / dw;
    if nz < -1.0 || nz > 1.0 {
        return None; // behind / outside the frustum: not drawn
    }
    Some(((nx * 0.5 + 0.5) * pixel_w, (0.5 - ny * 0.5) * pixel_h))
}

// ---------------------------------------------------------------------------
// Harness plumbing
// ---------------------------------------------------------------------------

/// Build the real app harness with the viewport backend AND the pick surface
/// both pointed at `scene`, so the pick resolves exactly what is drawn.
fn harness_with_scene(scene: &oep_viewer::Scene) -> Harness<'static, app::App> {
    let h = Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false));

    // Route the viewport's own backend at the fixture scene (same seam the
    // panel uses at runtime, GPU when an adapter exists).
    let backend: std::sync::Arc<std::sync::Mutex<Box<dyn oep_viewer::ViewportBackend>>> =
        std::sync::Arc::new(std::sync::Mutex::new(oep_viewer::select_backend(
            scene.clone(),
        )));
    h.ctx
        .data_mut(|d| d.insert_temp(egui::Id::unique(VIEWPORT_STATE_KEY), backend));
    // And mirror the scene into the pick surface's override slot.
    h.ctx
        .data_mut(|d| d.insert_temp(egui::Id::unique(PICK_SCENE_KEY), scene.clone()));

    h
}

/// The viewport region's full rect, read from the app's accessibility tree.
fn viewport_rect(harness: &Harness<'_, app::App>) -> egui::Rect {
    let node = harness
        .get_all_by_label("viewport")
        .next()
        .expect("viewport region present");
    let bounds = node
        .accesskit_node()
        .bounding_box()
        .expect("viewport region has explicit bounds");
    egui::Rect::from_min_max(
        egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
        egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
    )
}

fn current_snapshot(harness: &Harness<'_, app::App>) -> Option<Snapshot> {
    harness
        .ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

fn oracle(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(PICK_ORACLE_KEY)))
        .unwrap_or_default()
}

/// One field of the pick oracle, e.g. `("sel", "cube-a,cube-b")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

fn press_at(
    h: &Harness<'_, app::App>,
    pos: egui::Pos2,
    button: PointerButton,
    mods: egui::Modifiers,
) {
    h.event(egui::Event::PointerButton {
        pos,
        button,
        pressed: true,
        modifiers: mods,
    });
}

fn release_at(
    h: &Harness<'_, app::App>,
    pos: egui::Pos2,
    button: PointerButton,
    mods: egui::Modifiers,
) {
    h.event(egui::Event::PointerButton {
        pos,
        button,
        pressed: false,
        modifiers: mods,
    });
}

/// A plain click (press + release, no modifiers) at `pos`, stepped so egui
/// registers the click in its own frame.
fn plain_click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    press_at(h, pos, PointerButton::Primary, egui::Modifiers::NONE);
    h.step();
    release_at(h, pos, PointerButton::Primary, egui::Modifiers::NONE);
    h.step();
}

/// A shift+click at `pos` (the multi-select gesture). The modifier rides on
/// the events AND is pressed via `Event::ModifiersChanged` so the panel's
/// `i.modifiers.shift` is true exactly while the click lands.
fn shift_click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    let mods = egui::Modifiers {
        shift: true,
        ..egui::Modifiers::NONE
    };
    h.event_modifiers(egui::Event::PointerMoved(pos), mods);
    h.step();
    h.event_modifiers(
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        },
        mods,
    );
    h.step();
    h.event_modifiers(
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: mods,
        },
        mods,
    );
    h.step();
}

/// An alt+drag from `from` to `to` (the box-select gesture), stepped through
/// press / mid / end / release with Alt held the whole way. `ModifiersChanged`
/// is pushed directly (not via `event_modifiers`, whose trailing reset would
/// clear Alt before each next frame) so `i.modifiers.alt` stays true for the
/// whole gesture.
fn alt_box_drag(h: &mut Harness<'_, app::App>, from: egui::Pos2, to: egui::Pos2) {
    let mods = egui::Modifiers {
        alt: true,
        ..egui::Modifiers::NONE
    };
    h.event(egui::Event::ModifiersChanged(mods));
    h.step(); // Alt now held for every following frame until the reset below.
    h.event(egui::Event::PointerMoved(from));
    h.step();
    press_at(h, from, PointerButton::Primary, mods);
    h.step();
    let mid = egui::pos2((from.x + to.x) * 0.5, (from.y + to.y) * 0.5);
    h.event(egui::Event::PointerMoved(mid));
    h.step();
    h.event(egui::Event::PointerMoved(to));
    h.step();
    release_at(h, to, PointerButton::Primary, mods);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step(); // release Alt after the gesture.
}

// ---------------------------------------------------------------------------
// 1) 20 sampled points: id-buffer == analytic ray pick (numbers in proof)
// ---------------------------------------------------------------------------

#[test]
fn id_buffer_pick_equals_analytic_ray_pick_for_20_sampled_points() {
    let scene = three_cubes_scene();
    let bounds = scene.bounds.expect("fixture scene has bounds");
    let cam = oep_viewer::Camera::framed(&bounds);
    let (w, h) = (800.0, 600.0);

    let gpu = oep_viewer::gpu::device::acquire()
        .expect("a GPU adapter is required for the id-buffer acceptance");
    let pass = oep_viewer::gpu::idbuffer::IdPass::new(gpu, scene.clone(), cam, 800, 600)
        .expect("id pass resources created");

    // Candidate sample points: triangle centroids projected to pixels. A
    // centroid maps inside its own projected triangle (convexity), so the
    // pixel is on the drawn face; candidates whose pixel the id pass does NOT
    // assign to that primitive are silhouette/occlusion edge cases and are
    // skipped as sample LOCATIONS (the equality claim is tested at the
    // remaining covered pixels).
    let mut candidates: Vec<Vec<(u32, u32)>> = Vec::new();
    for prim in &scene.primitives {
        let mut list = Vec::new();
        let Some(mesh) = &prim.mesh else {
            continue;
        };
        for t in &mesh.triangles {
            let centroid = triangle_centroid(&mesh.positions, t);
            if let Some((px, py)) = world_to_px(&cam, &centroid, w, h) {
                let (xi, yi) = (px.round() as u32, py.round() as u32);
                if xi < 800 && yi < 600 {
                    list.push((xi, yi));
                }
            }
        }
        candidates.push(list);
    }
    // Deduplicate within each primitive.
    for list in &mut candidates {
        list.sort_unstable();
        list.dedup();
    }

    // Round-robin across primitives until exactly 20 sample points.
    let mut samples: Vec<(usize, u32, u32)> = Vec::new();
    let mut cursor = 0usize;
    'collect: loop {
        let mut took_any = false;
        for (prim_idx, list) in candidates.iter().enumerate() {
            if cursor < list.len() {
                took_any = true;
                let (x, y) = list[cursor];
                // Keep only pixels the id pass genuinely draws as this prim.
                if pass.pick_at(x, y) == Some(prim_idx as u32) {
                    samples.push((prim_idx, x, y));
                    if samples.len() >= 20 {
                        break 'collect;
                    }
                }
            }
        }
        if !took_any {
            break;
        }
        cursor += 1;
    }

    assert_eq!(
        samples.len(),
        20,
        "expected exactly 20 sampled comparison points, got {}",
        samples.len()
    );

    let mut compared = 0;
    for (i, (prim_idx, x, y)) in samples.iter().enumerate() {
        let ib = pass
            .pick_at(*x, *y)
            .map(|idx| scene.primitives[idx as usize].id.clone());
        let an =
            oep_viewer::pick(&scene, &cam, f64::from(*x), f64::from(*y), w, h).map(|sel| sel.id);
        let equal = ib == an && *x < 800 && *y < 600;
        println!(
            "sample {} (cube idx {}): px=({}, {}) idbuf={:?} pick={:?} match={}",
            compared, prim_idx, x, y, ib, an, equal
        );
        assert_eq!(
            ib, an,
            "sample {}: id-buffer pick must equal the analytic ray pick",
            i
        );
        compared += 1;
    }
    assert_eq!(compared, 20, "all 20 comparisons printed");
}

/// World-space centroid of a mesh triangle.
fn triangle_centroid(positions: &[[f32; 3]], t: &[u32; 3]) -> Vec3 {
    let p = |i: usize| {
        let v = positions[t[i] as usize];
        Vec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
    };
    let (a, b, c) = (p(0), p(1), p(2));
    Vec3::new(
        (a.x + b.x + c.x) / 3.0,
        (a.y + b.y + c.y) / 3.0,
        (a.z + b.z + c.z) / 3.0,
    )
}

// ---------------------------------------------------------------------------
// 2) Hover sets the pre-highlight within ONE frame
// ---------------------------------------------------------------------------

#[test]
fn hover_sets_pre_highlight_within_one_frame() {
    let scene = three_cubes_scene();
    let mut h = harness_with_scene(&scene);
    h.run();

    let rect = viewport_rect(&h);
    let scale = h.ctx.pixels_per_point();
    let snap = current_snapshot(&h).expect("snapshot published");
    let cam = camera_from_snapshot(&snap);

    // Cube A centre -> panel position (the cube is drawn there).
    let over_a = world_to_panel(&cam, rect, scale, &Vec3::new(50.0, 0.0, 10.0))
        .expect("A projects in front of the camera");
    assert!(
        rect.contains(over_a),
        "A's centre must be inside the viewport"
    );

    // Baseline: pointer not over the viewport -> no pre-highlight.
    assert_eq!(
        field(&oracle(&h), "pre").as_deref(),
        Some("none"),
        "before any hover the pre-highlight must be off"
    );

    // ONE frame with the pointer over cube A.
    h.event(egui::Event::PointerMoved(over_a));
    h.step();

    let after_one = oracle(&h);
    assert_eq!(
        field(&after_one, "pre").as_deref(),
        Some("cube-a"),
        "the pre-highlight must be set within ONE frame (oracle after one step: {after_one:?})"
    );

    // It did NOT need a second frame: the next step keeps the same state (the
    // first step already had it right).
    h.step();
    let after_two = oracle(&h);
    assert_eq!(
        field(&after_two, "pre").as_deref(),
        Some("cube-a"),
        "a second frame must not be required to establish the pre-highlight"
    );
}

// ---------------------------------------------------------------------------
// 3) Shift-click accumulates the exact multi-select set
// ---------------------------------------------------------------------------

#[test]
fn shift_click_accumulates_multi_select_set() {
    let scene = three_cubes_scene();
    let mut h = harness_with_scene(&scene);
    h.run();

    let rect = viewport_rect(&h);
    let scale = h.ctx.pixels_per_point();
    let snap = current_snapshot(&h).expect("snapshot published");
    let cam = camera_from_snapshot(&snap);

    let over_a =
        world_to_panel(&cam, rect, scale, &Vec3::new(50.0, 0.0, 10.0)).expect("A on screen");
    let over_b =
        world_to_panel(&cam, rect, scale, &Vec3::new(130.0, 0.0, 10.0)).expect("B on screen");
    let over_c =
        world_to_panel(&cam, rect, scale, &Vec3::new(210.0, 0.0, 10.0)).expect("C on screen");
    for p in [over_a, over_b, over_c] {
        assert!(
            rect.contains(p),
            "cube projections must lie inside the viewport"
        );
    }

    shift_click(&mut h, over_a);
    assert_eq!(
        field(&oracle(&h), "sel").as_deref(),
        Some("cube-a"),
        "first shift-click selects exactly A"
    );

    shift_click(&mut h, over_b);
    assert_eq!(
        field(&oracle(&h), "sel").as_deref(),
        Some("cube-a,cube-b"),
        "second shift-click accumulates B into the exact set [A, B]"
    );

    shift_click(&mut h, over_c);
    assert_eq!(
        field(&oracle(&h), "sel").as_deref(),
        Some("cube-a,cube-b,cube-c"),
        "third shift-click accumulates C: exact list must be [A, B, C]"
    );
}

// ---------------------------------------------------------------------------
// 4) Box select returns exactly the enclosed set (expected vs returned)
// ---------------------------------------------------------------------------

#[test]
fn box_select_returns_exactly_the_enclosed_set() {
    let scene = three_cubes_scene();
    let mut h = harness_with_scene(&scene);
    h.run();

    let rect = viewport_rect(&h);
    let scale = h.ctx.pixels_per_point();
    let snap = current_snapshot(&h).expect("snapshot published");
    let cam = camera_from_snapshot(&snap);

    // Committed viewport rect: the screen-space bounding box of cubes A and B
    // (their 8 world corners each). It fully contains their drawn pixels and
    // stops ~60 world units short of C, so the enclosed set is exactly A+B.
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [-f64::INFINITY; 2];
    let px_of = |p: &Vec3| -> (f64, f64) {
        let (x, y) = world_to_px(
            &cam,
            p,
            f64::from(rect.width() * scale),
            f64::from(rect.height() * scale),
        )
        .expect("corner projects in front");
        (x, y)
    };
    for prim in &scene.primitives {
        if prim.id == "cube-c" {
            continue;
        }
        for corner in prim.world.corners() {
            let (x, y) = px_of(&corner);
            lo[0] = lo[0].min(x);
            lo[1] = lo[1].min(y);
            hi[0] = hi[0].max(x);
            hi[1] = hi[1].max(y);
        }
    }

    // Analytic expectation: ray-pick a fine grid of points inside the rect
    // with oep_viewer::pick and collect the enclosed ids.
    let width = hi[0] - lo[0];
    let height = hi[1] - lo[1];
    assert!(
        width >= 8.0 && height >= 8.0,
        "committed rect must be a real box"
    );
    let step = 4.0;
    let mut expected: Vec<String> = Vec::new();
    let mut y = lo[1];
    while y < hi[1] {
        let mut x = lo[0];
        while x < hi[0] {
            if let Some(sel) = oep_viewer::pick(
                &scene,
                &cam,
                x,
                y,
                f64::from(rect.width() * scale),
                f64::from(rect.height() * scale),
            ) {
                if !expected.contains(&sel.id) {
                    expected.push(sel.id);
                }
            }
            x += step;
        }
        y += step;
    }
    expected.sort();
    println!(
        "box select expected ids (analytic grid inside px rect [{:.1},{:.1}]..[{:.1},{:.1}]): {:?}",
        lo[0], lo[1], hi[0], hi[1], expected
    );

    // Convert the pixel rect to logical panel coords and alt-drag it.
    let from = egui::pos2(
        rect.min.x + (lo[0] / f64::from(scale)) as f32,
        rect.min.y + (lo[1] / f64::from(scale)) as f32,
    );
    let to = egui::pos2(
        rect.min.x + (hi[0] / f64::from(scale)) as f32,
        rect.min.y + (hi[1] / f64::from(scale)) as f32,
    );
    assert!(
        rect.contains(from) && rect.contains(to),
        "box corners on panel"
    );
    alt_box_drag(&mut h, from, to);

    let mut returned: Vec<String> = field(&oracle(&h), "sel")
        .unwrap_or_else(|| "0".to_owned())
        .split(',')
        .filter(|s| !s.is_empty() && *s != "0")
        .map(|s| s.to_string())
        .collect();
    returned.sort();
    println!("box select returned ids: {:?}", returned);

    assert_eq!(
        returned, expected,
        "the id-buffer box must return exactly the enclosed analytic set"
    );
}

// ---------------------------------------------------------------------------
// 5) Clicking empty background clears the selection and picks nothing
// ---------------------------------------------------------------------------

#[test]
fn click_empty_background_clears_selection_and_picks_nothing() {
    let scene = three_cubes_scene();
    let mut h = harness_with_scene(&scene);
    h.run();

    let rect = viewport_rect(&h);
    let scale = h.ctx.pixels_per_point();
    let snap = current_snapshot(&h).expect("snapshot published");
    let cam = camera_from_snapshot(&snap);

    // Select something first: shift-click cube A.
    let over_a =
        world_to_panel(&cam, rect, scale, &Vec3::new(50.0, 0.0, 10.0)).expect("A on screen");
    shift_click(&mut h, over_a);
    assert_eq!(
        field(&oracle(&h), "sel").as_deref(),
        Some("cube-a"),
        "precondition: A selected"
    );

    // An empty point: a world point below the whole fixture, projected to the
    // panel. It is off every cube and (by assertion below) picks no primitive.
    let empty_world = Vec3::new(130.0, -120.0, 10.0);
    let empty =
        world_to_panel(&cam, rect, scale, &empty_world).expect("empty point projects in front");
    assert!(rect.contains(empty), "empty point inside the viewport");
    // The same analytic seam confirms it really is empty.
    let (ex, ey) = world_to_px(&cam, &empty_world, 800.0, 600.0)
        .expect("empty point projects in front of the camera");
    assert!(
        oep_viewer::pick(&scene, &cam, ex, ey, 800.0, 600.0).is_none(),
        "the chosen empty point must be genuinely empty under the analytic pick"
    );

    plain_click(&mut h, empty);

    let report = oracle(&h);
    assert_eq!(
        field(&report, "sel").as_deref(),
        Some("0"),
        "a click on empty background must clear the selection (oracle: {report:?})"
    );
    assert_eq!(
        field(&report, "pre").as_deref(),
        Some("none"),
        "and it must pick nothing (no pre-highlight on empty): {report:?}"
    );
    assert_eq!(
        field(&report, "pick").as_deref(),
        Some("clear"),
        "the pick oracle must record the clear: {report:?}"
    );
}
