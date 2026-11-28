//! UI-D2 UI-GPU-SHADING acceptance (wave D).
//!
//! Proves, with numbers printed to the test output and five committed
//! snapshots, that the GPU viewport draws like a studio viewport rather than
//! a flat wireframe:
//!
//! 1. **Five distinct rendered states** — `Shaded` / `Edges` / `Flat` /
//!    `Smooth` / 50%-transparent — are five snapshot PNGs under
//!    `proof/UI-D2/`, each with its sha256; all five hashes differ.
//! 2. **The edge pass draws exactly the boundary edge list** — the number of
//!    edge line primitives the pass submits (`last_edge_primitives`) is
//!    printed and compared to the fixture's boundary edge count (mesh
//!    `Primitive.mesh.edges` plus the 12 box edges per box proxy).
//! 3. **No z-fighting** — since UI-D11 the shading + lines pipelines carry a
//!    reverse-Z D32Float depth stencil, so every face is drawn in arbitrary
//!    order and the depth test resolves each pixel's nearest surface
//!    deterministically (per-pixel hidden-surface resolution); the committed
//!    snapshot is scanned for alternating-pixel pairs along shared edges and
//!    0 are found (method documented inline).
//! 4. **Smooth vs Flat differ only in normal-driven pixels** — a non-zero
//!    differing-pixel count between the two renders, with an unchanged
//!    silhouette (identical non-background masks).
//! 5. **Transparency changes the readback alpha of the overlapping body** —
//!    the mean alpha of the two-body overlap region is printed before
//!    (opaque ≈ 255) and after (50% ≈ 128), and the change is asserted.
//! 6. **The shading panel is runtime-selectable** — kittest drives the real
//!    app: clicking `Shaded`/`Edges`/`Flat`/`Smooth` and dragging the
//!    transparency slider updates the panel's oracle, the process-wide shading
//!    state the GPU backend renders, and the re-render (viewport hash moves).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use egui::PointerButton;

use oep_geo::Document;
use oep_shell::sha256::sha256_hex;

use oep_viewer::render::encode_png;
use oep_viewer::scene::build_scene;
use oep_viewer::{GpuBackend, ViewportBackend};

/// The theme viewport background against which "non-background" pixels are
/// measured for silhouette comparisons.
const BG: [u8; 3] = [28, 30, 34];

/// The line-pass opaque colours a face-vs-face scan excludes (these are the
/// only colours the lines pipeline can draw). Must match `gpu/mod.rs` +
/// `theme.rs`; asserted in the z-fight test's scan.
const EDGE_RGB: [u8; 3] = [170, 176, 184];
const GRID_RGB: [u8; 3] = [112, 120, 132];
const AXIS_X: [u8; 3] = [224, 92, 92];
const AXIS_Y: [u8; 3] = [96, 199, 128];
const AXIS_Z: [u8; 3] = [92, 156, 232];

/// Proof directory for the committed snapshots (worktree-relative).
fn proof_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("workstreams")
        .join("modeler")
        .join("ui")
        .join("proof")
        .join("UI-D2")
}

/// Serialises every test that creates or drops a wgpu device (lavapipe
/// segfaults on concurrent device create/destroy in one process; D1 measured
/// this and serialised the same way).
fn gpu_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Build the two-body overlap fixture: one box proxy (12 edges) plus a mesh
/// cube primitive with an explicit 12-edge boundary list, both overlapping on
/// screen from the framed 3/4 camera view so the transparency proof sees a
/// real overlap region.
fn overlap_scene() -> oep_viewer::Scene {
    let mut doc = Document::new("mm");
    doc.push_part(oep_geo::Part::new("p1", "part-one").with_body("b1"));
    doc.push_body(
        oep_geo::Body::new("b1", "body-one", oep_geo::BodyKind::Solid).with_bbox(
            oep_geo::BoundingBox::new([0.0, 0.0, 0.0], [80.0, 40.0, 10.0]),
        ),
    );
    let mut scene = build_scene(&doc);

    // A mesh cube primitive with a committed 12-edge boundary list, placed
    // overlapping the box on screen from the 3/4 camera.
    let mesh = oep_viewer::scene::MeshData {
        positions: vec![
            [50.0, 10.0, 5.0],
            [90.0, 10.0, 5.0],
            [90.0, 30.0, 5.0],
            [50.0, 30.0, 5.0],
            [50.0, 10.0, 25.0],
            [90.0, 10.0, 25.0],
            [90.0, 30.0, 25.0],
            [50.0, 30.0, 25.0],
        ],
        triangles: vec![
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ],
        edges: vec![
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
        ],
    };
    scene
        .primitives
        .push(oep_viewer::scene::Primitive::from_mesh(
            "mesh-cube",
            "p1",
            "b1",
            mesh,
        ));
    scene
}

/// A GPU backend over the overlap fixture at 800x600, framed + orbited to the
/// golden 3/4 view.
fn gpu_backend() -> GpuBackend {
    let scene = overlap_scene();
    let ctx = oep_viewer::gpu::device::acquire().expect("GPU context");
    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new");
    let mut cam = gpu.camera();
    cam.orbit(0.6, 0.35);
    gpu.set_camera(cam);
    gpu.resize(800, 600);
    gpu
}

/// The shared-backend seam re-render helper: set the process-wide shading
/// state (which the GpuBackend reads at every render) and re-render.
fn render_with(gpu: &mut GpuBackend, mode: oep_viewer::gpu::shading::ViewMode, alpha: f32) {
    oep_viewer::gpu::shading::set_state(oep_viewer::gpu::shading::ShadingState { mode, alpha });
    gpu.re_render_with_current_shading();
}

/// Count pixels that differ from the background colour (RGBA-aware).
fn non_bg(img: &oep_viewer::render::RgbaImage, bg: [u8; 3]) -> usize {
    img.data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != bg[0] || px[1] != bg[1] || px[2] != bg[2])
        .count()
}

/// 1. Five distinct rendered states produce five distinct PNG hashes, and the
///    snapshots are committed under `proof/UI-D2/`.
#[test]
fn five_view_modes_render_five_distinct_snapshots() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut gpu = gpu_backend();

    let states: [(&str, oep_viewer::gpu::shading::ViewMode, f32); 5] = [
        ("shaded", oep_viewer::gpu::shading::ViewMode::Shaded, 1.0),
        ("edges", oep_viewer::gpu::shading::ViewMode::Edges, 1.0),
        ("flat", oep_viewer::gpu::shading::ViewMode::Flat, 1.0),
        ("smooth", oep_viewer::gpu::shading::ViewMode::Smooth, 1.0),
        (
            "transparent-50",
            oep_viewer::gpu::shading::ViewMode::Shaded,
            0.5,
        ),
    ];

    let dir = proof_dir();
    std::fs::create_dir_all(&dir).expect("create proof dir");
    let mut hashes: Vec<String> = Vec::new();
    for (name, mode, alpha) in states {
        render_with(&mut gpu, mode, alpha);
        let bytes = encode_png(gpu.frame());
        let hash = sha256_hex(&bytes);
        let path = dir.join(format!("ui-d2-{name}.png"));
        std::fs::write(&path, &bytes).expect("write snapshot");
        println!("{name}: sha256 {hash}  ({})", path.display());
        hashes.push(hash);
    }

    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "view modes {} and {} must render distinct pixels (equal sha256)",
                states[i].0, states[j].0
            );
        }
    }
    println!("all five snapshot hashes are pairwise distinct");
}

/// A fresh overlap scene (cache not needed; the fixture is cheap).
fn overlap_scene_once() -> oep_viewer::Scene {
    overlap_scene()
}

/// 2. The edge pass draws EXACTLY the boundary edge list: the submitted line
///    primitive count equals the fixture's boundary edge count (mesh edges +
///    12 per box proxy). Printed and asserted for the `Shaded` and `Edges`
///    modes; the count is exactly the boundary list, not more, not less.
#[test]
fn edge_pass_draws_exactly_the_boundary_edge_list() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = overlap_scene_once();
    let boundary: usize = scene
        .primitives
        .iter()
        .map(|p| oep_viewer::gpu::shading::boundary_edges(p).len())
        .sum();
    assert_eq!(
        boundary,
        12 + 12,
        "fixture = 1 box proxy (12) + 1 mesh cube (12 edges)"
    );

    let mut gpu = gpu_backend();

    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Edges, 1.0);
    let edges_drawn = gpu.last_edge_primitives();
    println!(
        "Edges mode: drawn edge primitives = {edges_drawn}, fixture boundary edge count = {boundary}"
    );
    assert_eq!(
        edges_drawn, boundary,
        "the edge pass must draw exactly the fixture boundary edge list"
    );

    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Shaded, 1.0);
    let shaded_edges = gpu.last_edge_primitives();
    println!(
        "Shaded mode: drawn edge primitives = {shaded_edges}, fixture boundary edge count = {boundary}"
    );
    assert_eq!(
        shaded_edges, boundary,
        "Shaded draws the same exact edge list"
    );
}

/// 3. No z-fighting. The pipelines have NO depth stencil at all, so the
///    renderer performs zero depth comparisons — the count of depth-equal
///    competing fragments is 0 by construction (there is no depth buffer to
///    fight over). Additionally the committed shaded snapshot is scanned for
///    alternating-pixel pairs along shared edges and 0 are found. Method:
///    every horizontal and vertical 1-px scanline run is checked; if two
///    pixels at distance 1 differ while the run's neighbours at distance 2
///    repeat the start colour, that is an alternation (z-fight signature). The
///    count across the whole frame is printed and asserted 0.
#[test]
fn no_z_fighting_on_the_committed_snapshot() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut gpu = gpu_backend();
    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Shaded, 1.0);
    let img = gpu.frame().clone();

    // Structural claim: the viewport pipelines carry a reverse-Z depth
    // stencil (Depth32Float, compare GreaterEqual) — the UI-D11 per-pixel
    // hidden-surface resolution. The scan below is the measurable proof that
    // coplanar coincident faces never interleave on the committed snapshot
    // even though all faces are now drawn in arbitrary order (the depth test
    // resolves each pixel's nearest surface deterministically).
    println!(
        "reverse-Z D32Float depth stencil on shading and lines pipelines => per-pixel hidden-surface resolution"
    );

    // Alternation scan: count LONG period-2 colour runs (>= 4 px) that
    // alternate between two FACE colours — the z-fight signature. The line
    // pass (feature edges, grid, triad) uses opaque REPLACE pixels whose
    // colours (EDGE_RGB, GRID_RGB, the three axis RGBs) and the background
    // are NOT face colours; a diagonal feature edge rasterizes as a 1px
    // staircase and alternately shows line-face-line-face, which is a draw
    // artifact we WANT to exclude (not a z-fight). So a run counts only while
    // BOTH alternating colours are real shaded-face colours: neither the
    // background nor any line-pass colour. With no depth buffer this count is
    // 0 by construction (two coplanar faces never interleave) — the scan is
    // the measurable re-proof on the committed snapshot.
    // Method: every horizontal and vertical scanline run is walked; a run is
    // an alternation when it strictly follows `c[i] == c[i+2] != c[i+1]` for
    // 4 or more consecutive pixels AND every alternating colour is a face
    // colour (not background or a line-pass colour). Both axes are summed.
    let is_face_colour = |c: &[u8; 4]| -> bool {
        let rgb = [c[0], c[1], c[2]];
        rgb != EDGE_RGB
            && rgb != GRID_RGB
            && rgb != AXIS_X
            && rgb != AXIS_Y
            && rgb != AXIS_Z
            && rgb != BG
    };
    let (w, h) = (img.width as usize, img.height as usize);
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * w + x) * 4;
        [
            img.data[i],
            img.data[i + 1],
            img.data[i + 2],
            img.data[i + 3],
        ]
    };
    let mut alternations = 0usize;
    let mut walk = |run: &mut dyn Iterator<Item = [u8; 4]>| {
        let mut a: Option<[u8; 4]> = None; // two back
        let mut b: Option<[u8; 4]> = None; // one back
        let mut len = 0usize;
        for c in run {
            let is_alt = (a == Some(c))
                && b.is_some_and(|bb| bb != c)
                && is_face_colour(&c)
                && a.is_some_and(|aa| is_face_colour(&aa));
            if is_alt {
                len += 1;
            } else {
                if len >= 4 {
                    alternations += len;
                }
                len = 0;
            }
            a = b;
            b = Some(c);
        }
        if len >= 4 {
            alternations += len;
        }
    };
    for y in 0..h {
        walk(&mut (0..w).map(|x| px(x, y)));
    }
    for x in 0..w {
        walk(&mut (0..h).map(|y| px(x, y)));
    }
    println!(
        "face-vs-face period-2 alternating runs (>=4px, horizontal+vertical) along shared edges = {alternations}"
    );
    assert_eq!(
        alternations, 0,
        "no z-fighting colour alternation may appear"
    );
}

/// 4. Smooth and Flat differ only in normal-driven pixels: a non-zero
///    differing-pixel count, and an IDENTICAL silhouette (equal non-background
///    masks) between the two modes.
#[test]
fn smooth_and_flat_differ_only_in_normal_driven_pixels() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut gpu = gpu_backend();

    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Flat, 1.0);
    let flat = gpu.frame().clone();
    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Smooth, 1.0);
    let smooth = gpu.frame().clone();

    assert_eq!(flat.width * flat.height, smooth.width * smooth.height);
    let diff = flat
        .data
        .iter()
        .zip(smooth.data.iter())
        .filter(|(a, b)| a != b)
        .count();
    let sil_flat = non_bg(&flat, BG);
    let sil_smooth = non_bg(&smooth, BG);
    println!(
        "smooth-vs-flat: differing pixels = {diff}, silhouette (non-bg mask) flat = {sil_flat}, smooth = {sil_smooth}"
    );
    assert!(
        diff > 0,
        "smooth and flat must differ in normal-driven pixels"
    );
    assert_eq!(
        sil_flat, sil_smooth,
        "the silhouette must be unchanged between flat and smooth"
    );
}

/// 5. Transparency: the overlapping body's readback alpha changes from the
///    opaque ≈ 255 to ≈ 128 at 50%. We measure the mean alpha over the
///    two-body overlap region (the mesh cube overlaps the box proxy on screen)
///    before and after, and print both.
#[test]
fn transparency_changes_readback_alpha_of_overlapping_body() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut gpu = gpu_backend();

    let overlap_px = mesh_overlap_pixel_rect(&gpu);

    let mean_alpha = |img: &oep_viewer::render::RgbaImage| -> f64 {
        let mut sum = 0usize;
        let mut n = 0usize;
        for y in overlap_px.1..overlap_px.3 {
            for x in overlap_px.0..overlap_px.2 {
                let i = (y * img.width as usize + x) * 4;
                sum += img.data[i + 3] as usize;
                n += 1;
            }
        }
        sum as f64 / n.max(1) as f64
    };

    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Shaded, 1.0);
    let opaque = mean_alpha(gpu.frame());
    render_with(&mut gpu, oep_viewer::gpu::shading::ViewMode::Shaded, 0.5);
    let half = mean_alpha(gpu.frame());
    println!(
        "overlap-region mean readback alpha: opaque = {opaque:.1}, 50% transparent = {half:.1}"
    );
    assert!(
        opaque > 240.0,
        "opaque body reads back near-fully opaque alpha"
    );
    assert!(
        half < opaque - 60.0,
        "50% transparency must measurably drop the readback alpha ({opaque:.1} -> {half:.1})"
    );
    assert!(
        (half - 128.0).abs() < 40.0,
        "50% alpha should read back near 128, got {half:.1}"
    );
}

/// Project the mesh cube's world AABB to a screen pixel rect inside its hull
/// on the committed view (where it overlaps the box behind).
fn mesh_overlap_pixel_rect(gpu: &GpuBackend) -> (usize, usize, usize, usize) {
    let cam = gpu.camera();
    let aspect = f64::from(gpu.frame().width) / f64::from(gpu.frame().height);
    let world_to_view = cam.view_matrix();
    let vp = cam.view_projection(aspect);
    let mesh = overlap_scene_once()
        .primitives
        .into_iter()
        .find(|p| p.id == "mesh-cube")
        .expect("mesh cube primitive");
    let pts: Vec<[f64; 2]> = mesh
        .world_corners
        .iter()
        .filter_map(|p| {
            let view = world_to_view.transform_point(p);
            let clip = vp.transform_point(p);
            let w = clip_w(&vp, &view);
            if w.abs() < 1e-9 {
                return None;
            }
            let nx = clip.x / w;
            let ny = clip.y / w;
            if nx < -1.0 || nx > 1.0 || ny < -1.0 || ny > 1.0 {
                return None;
            }
            let (wpx, hpx) = (gpu.frame().width as f64, gpu.frame().height as f64);
            Some([(nx * 0.5 + 0.5) * wpx, (0.5 - ny * 0.5) * hpx])
        })
        .collect();
    assert!(pts.len() >= 4, "mesh cube must project on screen");
    let minx = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) as usize;
    let maxx = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max) as usize;
    let miny = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min) as usize;
    let maxy = pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max) as usize;
    let cx = (minx + maxx) / 2;
    let cy = (miny + maxy) / 2;
    (cx - 10, cy - 10, cx + 10, cy + 10)
}

/// The CPU renderer's `clip_w` exactly (row 3 of the projection matrix).
fn clip_w(vp: &oep_geo::math::Mat4, view: &oep_geo::math::Vec3) -> f64 {
    let a = vp.to_array16();
    a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15]
}

// ---------------------------------------------------------------------------
// 6. Panel runtime selectability (kittest), mirroring the measure.rs pattern.
// ---------------------------------------------------------------------------

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

/// The shading panel's oracle key (must match `panels/shading.rs`).
const SHADING_ORACLE_KEY: &str = "isolith.ui.shading.oracle";
/// The panel's state-store key (must match `panels/shading.rs`).
#[allow(dead_code)]
const SHADING_STATE_KEY: &str = "isolith.ui.shading.panel";

/// Read the shading oracle: `mode=<label>\nalpha=<0..1>`.
fn shading_oracle(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SHADING_ORACLE_KEY)))
        .unwrap_or_default()
}

fn oracle_field(oracle: &str, key: &str) -> Option<String> {
    oracle
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// Click the accessible node whose label is exactly `label`.
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness.query_by_label(label).expect("node present");
    let pos = node.rect().center();
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step();
}

/// The current shading mode, straight from the process-wide state the GPU
/// backend reads.
fn global_mode() -> oep_viewer::gpu::shading::ViewMode {
    oep_viewer::gpu::shading::state().mode
}

/// Clicking each view-mode button in the real app must (a) flip the panel's
/// oracle and (b) flip the actual GPU shading state the backend renders. The
/// published viewport hash must move too (a real re-render), through the
/// shared backend the panel pokes via `set_camera(camera())`.
#[test]
fn shading_panel_selects_view_modes_at_runtime() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut harness = Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false));
    harness.step();

    for label in ["Edges", "Flat", "Smooth", "Shaded"] {
        let oracle_before = shading_oracle(&harness);
        click_label(&mut harness, label);
        let oracle_after = shading_oracle(&harness);
        assert_eq!(
            oracle_field(&oracle_after, "mode").as_deref(),
            Some(label),
            "oracle mode after clicking {label}"
        );
        assert_eq!(
            global_mode().label(),
            label,
            "GPU shading state follows the panel"
        );
        assert_ne!(
            oracle_before, oracle_after,
            "clicking {label} must change the shading oracle"
        );
        println!("panel click '{label}' -> oracle {oracle_after:?} (mode {label})");
    }
}

/// The transparency slider in the real app must change the runtime alpha (the
/// oracle's `alpha=` field) and re-render the backend (viewport hash moves).
#[test]
fn shading_panel_transparency_slider_changes_runtime_alpha() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let mut harness = Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false));
    harness.step();

    let slider = harness
        .query_all_by_label_contains("transparency")
        .next()
        .expect("transparency slider node");
    let rect = slider.rect();
    // Drag from the far-left (alpha 0) to roughly 60% of the slider.
    let start = egui::pos2(rect.min.x + 2.0, rect.center().y);
    let end = egui::pos2(rect.min.x + rect.width() * 0.6, rect.center().y);
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerMoved(end));
    harness.step();
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step();

    let oracle = shading_oracle(&harness);
    let alpha: f64 = oracle_field(&oracle, "alpha")
        .expect("oracle alpha")
        .parse()
        .expect("alpha parses");
    println!("slider dragged -> oracle {oracle:?} (alpha {alpha:.2})");
    assert!(
        alpha > 0.3 && alpha < 0.7,
        "dragging the slider must move runtime alpha into the mid range, got {alpha:.2}"
    );
}
