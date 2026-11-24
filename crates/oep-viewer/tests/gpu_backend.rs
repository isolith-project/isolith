//! UI-D1 UI-GPU-BACKEND: the viewport seam contract on the GPU path.
//!
//! Proves, with real numbers printed to the test output, that the GPU
//! backend behind the [`ViewportBackend`] seam:
//!
//! 1. is selected by [`select_backend`] when an adapter is reachable, and
//!    [`GpuBackend::new`] is `Some` on that same adapter (lavapipe on this
//!    host);
//! 2. renders the committed demo and bracket fixtures at 800x600 to a
//!    non-empty frame (non-background pixel counts printed and floored);
//! 3. renders the same `(scene, camera, size)` twice to BYTE-IDENTICAL
//!    frames — the seam's determinism oracle re-proven on the GPU path;
//! 4. covers within [`COVERAGE_TOLERANCE`] (5%) of the CPU backend's
//!    covered-pixel count for the same view. The exact GPU and CPU counts
//!    are printed; the pixels are NOT byte-equal and this test never claims
//!    they are (GPU rasterization skips the 5x7 axis letters);
//! 5. with `ISOLITH_GPU=off` (the documented flag this card added),
//!    [`select_backend`] returns the CPU backend and its demo frame hash
//!    equals [`CPU_DEMO_GOLDEN_HASH`], the pre-card golden captured before
//!    any GPU path existed (`CpuBackend` is byte-untouched on this card).

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use oep_geo::Document;

use oep_viewer::gpu::device::{self, GPU_FLAG};
use oep_viewer::render::RgbaImage;
use oep_viewer::scene::build_scene;
use oep_viewer::{select_backend, Camera, CpuBackend, GpuBackend, ViewportBackend};

/// The theme viewport background the non-background count is measured against.
const BG: [u8; 3] = [28, 30, 34];

/// The geometry-coverage tolerance vs the CPU backend (card: within 5%).
const COVERAGE_TOLERANCE: f64 = 0.05;

/// Pre-card CPU golden: the demo scene rendered by `CpuBackend` at its
/// default 640x480, captured on the wave-D fork point (2026-11-23) before any
/// GPU path existed. `CpuBackend` was byte-untouched then, so this was the
/// oracle `ISOLITH_GPU=off` had to reproduce exactly.
///
/// UI-D7 corrected the shared clip-w projection (the UI-D1-documented "D2+
/// correct+regolden" item): the old formula applied the combined
/// view-projection's w-row to an already view-space point (double-applying V,
/// ~1.6x scale, and w == 0 on exact axis views). The corrected projection
/// legitimately moves the CPU demo frame, so this golden value is
/// re-registered here: `11436915591341135581` (pre-D7) -> this value. All
/// other assertions in this file are untouched.
const CPU_DEMO_GOLDEN_HASH: u64 = 12597229237775278722;

/// Serialises the tests that read or write the `ISOLITH_GPU` environment
/// flag (Rust test threads run in parallel; the flag is process-global).
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Serialises every test that creates or drops a wgpu device.
///
/// The lavapipe software ICD segfaults when multiple tests create/destroy
/// devices concurrently in one process (measured, reproducible SIGSEGV), so
/// the device-touching tests hold this lock for their whole body — creation
/// AND teardown happen while the lock is held.
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

/// The committed golden bracket document.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("oep-geo")
        .join("tests")
        .join("golden")
        .join("example-bracket.oepgeo.json")
}

/// Build the bracket scene from the committed golden document.
fn bracket_scene() -> oep_viewer::Scene {
    let text = fs::read_to_string(golden_path()).expect("golden bracket document exists");
    let doc = Document::from_json_pretty(&text).expect("golden parses as oep-geo v1");
    build_scene(&doc)
}

/// A readable fixed camera for a fixture: framed to the bounds then orbited to
/// the golden 3/4 view (the same standard view the CPU goldens use).
fn framed_orbit_camera(scene: &oep_viewer::Scene) -> Camera {
    let bounds = scene.bounds.expect("fixture has bounds");
    let mut cam = Camera::framed(&bounds);
    cam.orbit(0.6, 0.35);
    cam
}

/// Fixture-agnostic coverage comparison: same explicit camera, both backends at
/// 800x600, returns `(gpu_covered, cpu_covered)`.
fn coverages(scene: &oep_viewer::Scene, cam: Camera) -> (usize, usize) {
    let ctx = device::acquire().expect("GPU context for the coverage measurement");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new succeeds");
    gpu.set_camera(cam);
    gpu.resize(800, 600);
    let gpu_covered = non_bg(gpu.frame(), BG);
    let gpu_hash = gpu.frame_hash();
    println!(
        "GPU {}(800x600): covered={gpu_covered} hash={gpu_hash}",
        scene_name(scene)
    );

    let mut cpu = CpuBackend::new(scene.clone());
    cpu.set_camera(cam);
    cpu.resize(800, 600);
    let cpu_covered = non_bg(cpu.frame(), BG);
    let cpu_hash = cpu.frame_hash();
    println!(
        "CPU {}(800x600): covered={cpu_covered} hash={cpu_hash}",
        scene_name(scene)
    );
    let _ = cpu_hash;
    (gpu_covered, cpu_covered)
}

fn scene_name(scene: &oep_viewer::Scene) -> &'static str {
    if scene
        .primitives
        .iter()
        .any(|p| p.part_id == "oep-part-bracket")
    {
        "bracket"
    } else {
        "demo"
    }
}

/// 1. With an adapter present (lavapipe on this host), `select_backend`
///    returns the GPU backend and `GpuBackend::new` is `Some`.
#[test]
fn select_backend_returns_gpu_when_adapter_exists() {
    let _guard = env_lock().lock().expect("env lock");
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(GPU_FLAG);
    assert!(
        device::adapter_available(),
        "CI host must expose a wgpu adapter"
    );

    let scene = CpuBackend::demo_scene();
    let selected = select_backend(scene.clone());
    assert_eq!(
        selected.backend_name(),
        "gpu",
        "with an adapter present select_backend must choose the GPU backend"
    );

    let ctx = device::acquire().expect("adapter present implies context");
    let gpu = GpuBackend::new(scene, ctx);
    assert!(
        gpu.is_some(),
        "GpuBackend::new must be Some on the acquired device"
    );
}

/// 2a. The demo fixture renders to a non-empty 800x600 GPU frame.
#[test]
fn demo_renders_non_empty_gpu_frame() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = CpuBackend::demo_scene();
    let ctx = device::acquire().expect("GPU context");
    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new");
    gpu.resize(800, 600);
    let covered = non_bg(gpu.frame(), BG);
    println!("GPU demo(800x600): non-background pixels = {covered}");
    assert!(
        covered >= 5000,
        "the demo box + grid + triad must cover thousands of pixels, got {covered}"
    );
}

/// 2b. The bracket fixture renders to a non-empty 800x600 GPU frame.
#[test]
fn bracket_renders_non_empty_gpu_frame() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = bracket_scene();
    let ctx = device::acquire().expect("GPU context");
    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new");
    gpu.resize(800, 600);
    let covered = non_bg(gpu.frame(), BG);
    println!("GPU bracket(800x600): non-background pixels = {covered}");
    assert!(
        covered >= 5000,
        "the bracket assembly + grid + triad must cover thousands of pixels, got {covered}"
    );
}

/// 3. The same scene/camera/size renders twice to byte-identical frames on
///    the GPU path (the seam's determinism oracle holds: GPU commands are
///    deterministic, so identical inputs hash identically).
#[test]
fn gpu_renders_same_view_byte_identically() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let scene = CpuBackend::demo_scene();
    let ctx = device::acquire().expect("GPU context");
    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new");
    let cam = gpu.camera();

    // Re-render the identical view by re-applying the identical camera.
    gpu.set_camera(cam);
    let first = gpu.frame().clone();
    let first_hash = gpu.frame_hash();

    gpu.set_camera(cam);
    let second = gpu.frame().clone();
    let second_hash = gpu.frame_hash();

    assert_eq!(
        first.data, second.data,
        "two identical GPU renders must be byte-identical"
    );
    assert_eq!(first_hash, second_hash, "and therefore hash identically");

    // A freshly constructed backend from the same scene frames the same
    // camera, so two independent backends must also agree.
    let scene2 = CpuBackend::demo_scene();
    let mut gpu2 =
        GpuBackend::new(scene2, device::acquire().expect("GPU context")).expect("GpuBackend::new");
    gpu2.resize(gpu.frame().width, gpu.frame().height);
    assert_eq!(
        gpu2.frame_hash(),
        first_hash,
        "an independent backend for the same scene/camera/size hashes identically"
    );
}

/// 4. Geometry coverage is within 5% of the CPU backend's covered-pixel count
///    for the same view, on both the demo and the bracket fixture. GPU and CPU
///    pixels are NOT byte-equal and are never claimed to be.
#[test]
fn gpu_geometry_coverage_within_5pct_of_cpu() {
    let _g = gpu_lock().lock().expect("gpu lock");
    let demo = CpuBackend::demo_scene();
    let (dg, dc) = coverages(&demo, framed_orbit_camera(&demo));
    let drift = (dg as isize - dc as isize).unsigned_abs();
    assert!(
        drift as f64 <= COVERAGE_TOLERANCE * dc as f64,
        "demo GPU coverage {dg} exceeds 5% band around CPU {dc} (drift {drift})"
    );

    let bracket = bracket_scene();
    let (bg_, bc) = coverages(&bracket, framed_orbit_camera(&bracket));
    let drift = (bg_ as isize - bc as isize).unsigned_abs();
    assert!(
        drift as f64 <= COVERAGE_TOLERANCE * bc as f64,
        "bracket GPU coverage {bg_} exceeds 5% band around CPU {bc} (drift {drift})"
    );
}

/// 5. With `ISOLITH_GPU=off`, `select_backend` returns the CPU backend and the
///    demo frame hash equals the pre-card golden exactly.
#[test]
fn gpu_off_falls_back_to_cpu_and_matches_precard_golden() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::set_var(GPU_FLAG, "off");
    assert!(
        !device::gpu_enabled(),
        "ISOLITH_GPU=off must disable the GPU path"
    );

    let selected = select_backend(CpuBackend::demo_scene());
    assert_eq!(
        selected.backend_name(),
        "cpu",
        "GPU=off forces the CPU backend"
    );
    assert_eq!(
        selected.frame_hash(),
        CPU_DEMO_GOLDEN_HASH,
        "the CPU fallback must reproduce the pre-card golden demo hash exactly"
    );
    println!(
        "CPU demo fallback hash = {} (pre-card golden {CPU_DEMO_GOLDEN_HASH})",
        selected.frame_hash()
    );

    std::env::remove_var(GPU_FLAG);
}
