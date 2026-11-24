//! UI-D7 UI-RENDER-FIX: the shared clip-w projection and empty-face safety.
//!
//! Acceptance harness for the renderer correction. Proves, with real numbers
//! printed to the test output:
//!
//! 1. **Every exact standard view renders the model on BOTH backends.** The
//!    committed 80x40x10 demo solid is rendered at 400x300 from each of the
//!    six cube-face views (and the isometric view). The covered-pixel counts
//!    are printed in a CPU-vs-GPU table; every view > 0 on both backends
//!    (today Back = 0 on the CPU path and a panic on the GPU path), and the
//!    two backends agree within 10% for the same view.
//! 2. **The correction itself.** States the exact formula change and prints
//!    the near (z=0) corner `w` values at the Back view before (the old
//!    `row4(vp)·(view,1)` double-application) and after (`-view.z`, the true
//!    clip w). Today they are exactly 0; after the correction they are
//!    non-zero and the Back-view frame covers pixels.
//! 3. **An explicitly constructed all-culled scene renders an empty frame
//!    with no panic in both backends.** A camera whose frustum excludes the
//!    whole model yields zero faces; the CPU path writes the background and
//!    the GPU path (which used to panic on an empty vertex buffer) instead
//!    renders a clean background frame. The empty frame is asserted and no
//!    panic happens (the test itself completes).

use std::sync::{Mutex, OnceLock};

use oep_geo::math::{Mat4, Vec3};

use oep_viewer::camera::Camera;
use oep_viewer::gpu::device;
use oep_viewer::render::RgbaImage;
use oep_viewer::{CpuBackend, GpuBackend, ViewportBackend};

/// The theme viewport background the covered count is measured against.
const BG: [u8; 3] = [28, 30, 34];

/// Serialises every test that creates or drops a wgpu device (lavapipe
/// segfaults on concurrent device create/destroy in one process).
fn gpu_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Count pixels that differ from the background colour.
fn non_bg(img: &RgbaImage, bg: [u8; 3]) -> usize {
    img.data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != bg[0] || px[1] != bg[1] || px[2] != bg[2])
        .count()
}

/// The committed standard views, mirrored from `oep_viewer::views::StandardView`
/// (which UI-D4 lands; the yaw/pitch pairs are committed literals).
fn standard_views() -> Vec<(&'static str, (f64, f64))> {
    vec![
        ("Front", (0.0, 0.0)),
        ("Back", (std::f64::consts::PI, 0.0)),
        ("Left", (-std::f64::consts::FRAC_PI_2, 0.0)),
        ("Right", (std::f64::consts::FRAC_PI_2, 0.0)),
        ("Top", (0.0, 1.54)),
        ("Bottom", (0.0, -1.54)),
        (
            "Iso",
            (std::f64::consts::FRAC_PI_4, (1.0 / 3.0_f64.sqrt()).asin()),
        ),
    ]
}

/// A camera framed to the demo solid's bounds, then set to the committed
/// standard-view yaw/pitch (target, distance and fov untouched).
fn standard_camera(scene: &oep_viewer::Scene, yaw: f64, pitch: f64) -> Camera {
    let bounds = scene.bounds.expect("demo scene has bounds");
    let mut cam = Camera::framed(&bounds);
    cam.yaw = yaw;
    cam.pitch = pitch;
    cam
}

/// The `w` a point gets under the OLD (pre-D7) formula: the combined
/// view-projection's 4th row applied to the VIEW-SPACE point — the view
/// transform applied twice.
fn old_clip_w(vp: &Mat4, world: &Vec3, world_to_view: &Mat4) -> f64 {
    let view = world_to_view.transform_point(world);
    let a = vp.to_array16();
    a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15]
}

/// The `w` a point gets under the D7 formula: `row4(vp)·(world,1)`, which for
/// this right-handed camera equals `-view.z` (the true clip w).
fn d7_clip_w(vp: &Mat4, world: &Vec3) -> f64 {
    let a = vp.to_array16();
    a[3] * world.x + a[7] * world.y + a[11] * world.z + a[15]
}

/// Cover the same `(scene, camera, size)` on both backends; returns
/// `(gpu_covered, cpu_covered)`.
fn coverages(scene: &oep_viewer::Scene, cam: Camera) -> (usize, usize) {
    let ctx = device::acquire().expect("GPU context");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    gpu.set_camera(cam);
    gpu.resize(400, 300);
    let gpu_covered = non_bg(gpu.frame(), BG);

    let mut cpu = CpuBackend::new(scene.clone());
    cpu.set_camera(cam);
    cpu.resize(400, 300);
    let cpu_covered = non_bg(cpu.frame(), BG);

    (gpu_covered, cpu_covered)
}

/// All six standard views + iso render non-empty frames on BOTH backends,
/// every view > 0 (today Back = 0 on CPU and panics on GPU), and the CPU and
/// GPU covered-pixel counts agree within 10% per view. Prints the table.
#[test]
fn all_standard_views_render_on_both_backends() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = CpuBackend::demo_scene();

    println!(
        "{:<7} {:>10} {:>10} {:>8}",
        "view", "gpu_px", "cpu_px", "drift%"
    );
    for (name, (yaw, pitch)) in standard_views() {
        let cam = standard_camera(&scene, yaw, pitch);
        let (gpu, cpu) = coverages(&scene, cam);
        assert!(
            cpu > 0,
            "{name}: CPU covered-pixels must be > 0 after the clip-w fix (was 0 at Back), got {cpu}"
        );
        assert!(
            gpu > 0,
            "{name}: GPU covered-pixels must be > 0 after the clip-w fix (Back panicked before), got {gpu}"
        );
        let max = cpu.max(gpu) as f64;
        let drift = (gpu as isize - cpu as isize).unsigned_abs() as f64 / max;
        assert!(
            drift <= 0.10,
            "{name}: CPU {cpu} vs GPU {gpu} drift {drift:.5} exceeds 10%"
        );
        println!("{name:<7} {gpu:>10} {cpu:>10} {drift:>8.4}");
    }
}

/// The correction: the exact formula change, with the near (z=0) corner `w`
/// values at the Back view printed BEFORE (old double-applied formula) and
/// AFTER (`-view.z`). Today they are exactly 0 at these corners; after the
/// fix they are non-zero and > the degenerate threshold by orders of
/// magnitude.
#[test]
fn back_view_near_corner_w_before_and_after() {
    let scene = CpuBackend::demo_scene();
    let cam = standard_camera(&scene, std::f64::consts::PI, 0.0); // Back
    let aspect = 400.0 / 300.0;
    let world_to_view = cam.view_matrix();
    let vp = cam.view_projection(aspect);

    // The demo solid's 4 near (z=0) corners (the face the old formula
    // dropped a whole frame of).
    let near_corners = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(80.0, 0.0, 0.0),
        Vec3::new(80.0, 40.0, 0.0),
        Vec3::new(0.0, 40.0, 0.0),
    ];

    println!("clip-w formula change: row4(vp)·(view,1)  ->  row4(vp)·(world,1) = -view.z");
    println!("{:<8} {:>16} {:>16}", "corner", "old_w", "d7_w");
    for c in &near_corners {
        let old = old_clip_w(&vp, c, &world_to_view);
        let new = d7_clip_w(&vp, c);
        println!("{:>8} {:>16.9} {:>16.9}", "", old, new);
        assert!(
            old.abs() < 1e-9,
            "old w must be exactly degenerate at Back's near corners, got {old}"
        );
        assert!(new.abs() > 1e-9, "D7 w must be non-degenerate, got {new}");
    }

    // Sanity: the same fix makes the Back frame cover real pixels (CPU).
    let cpu_covered = {
        let mut cpu = CpuBackend::new(scene.clone());
        cpu.set_camera(cam);
        cpu.resize(400, 300);
        non_bg(cpu.frame(), BG)
    };
    assert!(
        cpu_covered > 0,
        "Back view must cover pixels, got {cpu_covered}"
    );
    println!("Back view CPU covered-pixels after fix: {cpu_covered}");
}

/// An explicitly constructed all-culled scene: a camera whose frustum
/// excludes the whole model (near plane pulled in past it). Neither backend
/// panics, and neither draws ANY model pixels — the all-culled frame is
/// byte-count-identical to the overlays-only baseline (the same scene with
/// no primitives), which is what "renders an empty frame" means here. The
/// old GPU path panicked on the empty face buffer (`buffer slice can not be
/// empty`) before the guard.
#[test]
fn all_culled_scene_renders_empty_frame_without_panic_on_both_backends() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = CpuBackend::demo_scene();

    // The overlays-only baseline: same bounds (=> same grid extent / triad),
    // no primitives (=> no model faces can be drawn).
    let mut empty_scene = scene.clone();
    empty_scene.primitives.clear();

    // An explicitly all-culled setup: the normal framed 3/4 camera, but with
    // the near plane pulled in past the whole model so every face fails the
    // NDC-z cull on both backends (no degenerate w involved - pure face-list
    // emptiness). All 8 demo corners sit at view depth <= 190 (framed
    // distance ~141 + half-diagonal ~45); near = 250 keeps them inside.
    let bounds = scene.bounds.expect("demo scene has bounds");
    let mut cam = Camera::framed(&bounds);
    cam.near = 250.0;
    cam.far = 5000.0;

    // CPU: every face culled -> the frame covers exactly the overlays (grid +
    // axes triad), which is the empty model frame. No panic.
    let mut cpu_faces = CpuBackend::new(scene.clone());
    cpu_faces.set_camera(cam);
    cpu_faces.resize(400, 300);
    let cpu_px = non_bg(cpu_faces.frame(), BG);
    let mut cpu_baseline = CpuBackend::new(empty_scene.clone());
    cpu_baseline.set_camera(cam);
    cpu_baseline.resize(400, 300);
    let cpu_base_px = non_bg(cpu_baseline.frame(), BG);
    assert_eq!(
        cpu_px, cpu_base_px,
        "all-culled CPU frame must equal the overlay-only baseline ({cpu_base_px}), got {cpu_px} model pixels"
    );

    // GPU: empty face list -> the face branch must be guarded, the frame
    // covers exactly the same overlays, and it must NOT panic.
    let ctx = device::acquire().expect("GPU context");
    let mut gpu_faces = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    gpu_faces.set_camera(cam);
    gpu_faces.resize(400, 300);
    let gpu_px = non_bg(gpu_faces.frame(), BG);
    let mut gpu_baseline =
        GpuBackend::new(empty_scene.clone(), device::acquire().expect("GPU context"))
            .expect("GpuBackend::new");
    gpu_baseline.set_camera(cam);
    gpu_baseline.resize(400, 300);
    let gpu_base_px = non_bg(gpu_baseline.frame(), BG);
    assert_eq!(
        gpu_px, gpu_base_px,
        "all-culled GPU frame must equal the overlay-only baseline ({gpu_base_px}), got {gpu_px} model pixels"
    );

    println!(
        "all-culled scene: CPU faces={cpu_px} baseline={cpu_base_px}; GPU faces={gpu_px} baseline={gpu_base_px}; no panic"
    );
}
