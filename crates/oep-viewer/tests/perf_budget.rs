//! UI-D5 UI-PERF: interactive frame budget on U1, recorded numbers.
//!
//! The single repeatable perf harness of the editor-parity wave D: it renders
//! a >= 1,000,000-triangle scene at 1600x1000 through the SAME GPU viewport
//! backend the app drives ([`GpuBackend`] behind the [`ViewportBackend`]
//! seam, with the same `device::acquire()` context selection), and records the
//! frame-time distribution: so "interactive" is a measured claim with a
//! committed p95 number, not a hope.
//!
//! Two tests live here:
//!
//! * `perf_budget_fast_smoke`: ALWAYS runs (never `#[ignore]`). Builds the
//!   1M-triangle scene (cheap: index generation), prints the scene stats and
//!   the committed CPU determinism oracle (the demo `CpuBackend` frame_hash
//!   must still equal the committed golden, so unchanged CPU determinism is a
//!   printed fact), and, when an adapter is reachable, renders the SMALL demo
//!   scene once through the GPU backend and prints the adapter name and the
//!   GPU determinism echo. Fast: no repeated rendering, no threshold gate.
//! * `perf_budget_1m_gpu_heavy`: `#[ignore]`d by default (perf is a manual
//!   measurement, not a CI assertion). Builds the 1M scene, renders it on the
//!   GPU backend at 1600x1000, warms up 20 frames, times 120 frames, reports
//!   p50 / p95 / p99 / max and min / mean fps plus the adapter name and driver
//!   version, and writes the machine report JSON. The gate is the COMMITTED
//!   threshold p95 <= 16.6 ms: if the measured p95 misses it, the report is
//!   still written and the miss stated plainly, and the test fails loudly
//!   (the card is then a legitimate `capability` block on this device: the
//!   bar is never lowered).
//!
//! The scene is the same deterministic heightfield grid the program's P6-M5-T03
//! 1M budget used (a 709x709 grid -> 1,002,528 triangles, no RNG), attached as
//! a real indexed mesh ([`MeshData`]) so it exercises the actual mesh fill
//! path of the backend, exactly what a tessellated CAD preview presents.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use oep_viewer::gpu::device::{self, GPU_FLAG};
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::{CpuBackend, GpuBackend, ViewportBackend};

/// Grid side in vertices: 2*(N-1)^2 triangles (1,002,528 >= 1,000,000).
const GRID_N: usize = 709;
/// The card's required target surface size (pixels).
const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1000;
/// Warm-up renders before the measured run (pipeline/driver steady state).
const WARMUP_FRAMES: usize = 20;
/// Frames in the measured distribution (>= 120 per the card).
const MEASURED_FRAMES: usize = 120;
/// The committed acceptance threshold: p95 <= 16.6 ms (60 FPS p95).
const BUDGET_P95_MS: f64 = 16.6;

/// The committed CPU golden: the demo scene rendered by `CpuBackend` hashed
/// with the seam's `frame_hash` oracle. Re-registered by UI-D7 after the
/// clip-w projection correction; the perf card prints that this value is
/// STILL reproduced, proving CPU determinism is unchanged on this card.
/// (Value identical to `gpu_backend.rs::CPU_DEMO_GOLDEN_HASH`.)
const CPU_DEMO_GOLDEN_HASH: u64 = 12597229237775278722;

/// Serialises device-touching tests in this binary: lavapipe segfaults when
/// multiple tests create/destroy devices concurrently in one process (the
/// same measured class gpu_backend.rs guards).
fn gpu_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 2 * (GRID_N - 1)^2 triangles.
fn triangle_count() -> usize {
    2 * (GRID_N - 1) * (GRID_N - 1)
}

/// Build the deterministic >= 1M-triangle scene: a heightfield grid in
/// [-1,1]^2 with two sine bumps for the third coordinate, indexed as a real
/// `MeshData`. No random numbers, so the scene is byte-reproducible. The mesh
/// carries no boundary-edge list (a pure fill measurement; the grid + triad
/// overlay still renders exactly as the app viewport draws it).
fn million_triangle_scene() -> Scene {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(GRID_N * GRID_N);
    let step = 2.0 / (GRID_N as f32 - 1.0);
    for j in 0..GRID_N {
        for i in 0..GRID_N {
            let x = -1.0 + i as f32 * step;
            let y = -1.0 + j as f32 * step;
            let z = 0.2 * (x * 3.1).sin() * (y * 2.7).cos();
            positions.push([x, y, z]);
        }
    }
    let mut triangles: Vec<[u32; 3]> = Vec::with_capacity(triangle_count());
    for j in 0..(GRID_N - 1) {
        for i in 0..(GRID_N - 1) {
            let a = (j * GRID_N + i) as u32;
            let b = (j * GRID_N + i + 1) as u32;
            let c = ((j + 1) * GRID_N + i) as u32;
            let d = ((j + 1) * GRID_N + i + 1) as u32;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }
    let mesh = MeshData {
        positions,
        triangles,
        edges: Vec::new(),
    };
    let prim = Primitive::from_mesh("perf:1m", "perf-part", "perf-body", mesh);
    let mut scene = Scene::default();
    scene.primitives.push(prim);
    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    scene
}

/// The `frame_hash` of a freshly built `CpuBackend` demo: the committed CPU
/// determinism oracle value (printed by both tests).
fn cpu_demo_frame_hash() -> u64 {
    CpuBackend::demo().frame_hash()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (p / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// The output path for the machine report. Defaults to `./perf.json`; the
/// runner (`scripts/ui/perf.sh`) sets `UI_D5_PERF_JSON` to the proof path.
fn report_path() -> std::path::PathBuf {
    std::env::var_os("UI_D5_PERF_JSON")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("perf.json"))
}

/// FAST SMOKE: always runs (`cargo test -p oep-viewer --test perf_budget`).
///
/// No threshold is asserted here; this leg proves the harness composes: the
/// 1M scene really contains >= 1,000,000 triangles, the committed CPU
/// determinism oracle is unchanged (the printed hash == committed golden), and
/// the GPU backend renders and echo-hashes deterministically when an adapter
/// is reachable.
#[test]
fn perf_budget_fast_smoke() {
    // 1. Scene composition: a real >= 1M triangle indexed mesh.
    let scene = million_triangle_scene();
    assert_eq!(scene.primitives.len(), 1);
    let tris = triangle_count();
    let mesh = scene.primitives[0].mesh.as_ref().expect("mesh attached");
    println!(
        "perf scene: {} triangles (target >= 1,000,000), {} vertices, bounds {:?}",
        tris,
        mesh.positions.len(),
        scene.bounds.map(|b| (b.min, b.max))
    );
    assert!(
        tris >= 1_000_000,
        "harness scene must carry >= 1,000,000 triangles, got {tris}"
    );
    assert_eq!(mesh.triangles.len(), tris);

    // 2. CPU determinism unchanged: the committed golden is STILL reproduced.
    let cpu_hash = cpu_demo_frame_hash();
    println!(
        "cpu determinism: demo frame_hash = {cpu_hash}  (committed golden {CPU_DEMO_GOLDEN_HASH}, equal = {})",
        cpu_hash == CPU_DEMO_GOLDEN_HASH
    );
    assert_eq!(
        cpu_hash, CPU_DEMO_GOLDEN_HASH,
        "CPU determinism moved on the perf card: demo frame_hash {cpu_hash} != committed golden {CPU_DEMO_GOLDEN_HASH}"
    );

    // 3. GPU leg (guarded): render the SMALL demo once and echo-hash it, and
    //    print the adapter the runtime would use. Guarded so the always-green
    //    leg still passes on a GPU-less host.
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(GPU_FLAG);
    if !device::adapter_available() {
        println!("gpu leg skipped: no adapter reachable (cpu determinism leg still green)");
        return;
    }
    let ctx = device::acquire().expect("adapter reachable implies context");
    let info = ctx.device.adapter_info();
    println!(
        "gpu adapter: name={} driver={} driver_info={} backend={:?} device_type={:?}",
        info.name, info.driver, info.driver_info, info.backend, info.device_type
    );
    let mut gpu = GpuBackend::new(CpuBackend::demo_scene(), ctx).expect("GpuBackend::new succeeds");
    gpu.set_camera(gpu.camera()); // deterministic re-render of the same view
    let h1 = gpu.frame_hash();
    gpu.set_camera(gpu.camera());
    let h2 = gpu.frame_hash();
    println!(
        "gpu structural echo: frame_hash {h1} -> {h2}, identical = {}",
        h1 == h2
    );
    assert_eq!(
        h1, h2,
        "GPU determinism oracle must hold on the harness leg"
    );
    assert!(
        gpu.frame().data.iter().any(|&b| b != 0),
        "demo GPU frame must not be blank"
    );
}

/// HEAVY: `#[ignore]`d by default. Runs the full 1M-triangle 1600x1000
/// interactive measurement on the GPU backend and writes the machine report.
///
/// Gate: p95 <= 16.6 ms (the COMMITTED bar). On a miss the report is still
/// written with the real numbers and the miss is stated plainly, then the test
/// fails so the outcome is loud: the card must then be blocked `capability`
/// with the measured p95; the bar is never lowered.
#[test]
#[ignore]
fn perf_budget_1m_gpu_heavy() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(GPU_FLAG);

    let scene = million_triangle_scene();
    let tris = triangle_count();
    let ctx = device::acquire().expect("perf requires a reachable GPU adapter");
    let info = ctx.device.adapter_info();

    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new succeeds");
    gpu.resize(WIDTH, HEIGHT);

    // Warm-up: stable pipeline/driver state before the measured run.
    for _ in 0..WARMUP_FRAMES {
        gpu.re_render_with_current_shading();
    }

    // Measured run: TIMED frames, each a full render_frame on the seam path
    // (CPU projection + submit + readback + GPU completion: the real
    // interactive cost the app pays per frame at this size).
    const {
        assert!(
            MEASURED_FRAMES >= 120,
            "the card requires at least 120 measured frames"
        );
    }
    let mut samples: Vec<f64> = Vec::with_capacity(MEASURED_FRAMES);
    for _ in 0..MEASURED_FRAMES {
        let t0 = Instant::now();
        gpu.re_render_with_current_shading();
        samples.push(t0.elapsed().as_secs_f64() * 1000.0); // ms
    }

    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
    let min_ms = sorted[0];
    let max_ms = *sorted.last().unwrap();
    let p50_ms = percentile(&sorted, 50.0);
    let p95_ms = percentile(&sorted, 95.0);
    let p99_ms = percentile(&sorted, 99.0);
    let fps_min = 1000.0 / max_ms;
    let fps_mean = 1000.0 / mean_ms;

    let cpu_hash = cpu_demo_frame_hash();
    let cpu_hashes_match = cpu_hash == CPU_DEMO_GOLDEN_HASH;
    let budget_met = p95_ms <= BUDGET_P95_MS;

    // Machine report (written BEFORE the gate so a miss still records reality).
    let json = serde_json::json!({
        "benchmark": "ui-d5-interactive-frame-budget",
        "card": "UI-D5",
        "host": std::env::var("HOSTNAME").unwrap_or_else(|_| "u1".into()),
        "adapter": { "name": info.name, "driver": info.driver, "driver_info": info.driver_info,
                     "backend": format!("{:?}", info.backend), "device_type": format!("{:?}", info.device_type) },
        "scene_triangles": tris,
        "width": WIDTH,
        "height": HEIGHT,
        "warmup_frames": WARMUP_FRAMES,
        "measured_frames": MEASURED_FRAMES,
        "threshold_p95_ms": BUDGET_P95_MS,
        "min_frame_ms": min_ms,
        "mean_frame_ms": mean_ms,
        "p50_frame_ms": p50_ms,
        "p95_frame_ms": p95_ms,
        "p99_frame_ms": p99_ms,
        "max_frame_ms": max_ms,
        "fps_min": fps_min,
        "fps_mean": fps_mean,
        "budget_met": budget_met,
        "cpu_demo_frame_hash": cpu_hash,
        "cpu_demo_golden_hash": CPU_DEMO_GOLDEN_HASH,
        "cpu_determinism_unchanged": cpu_hashes_match,
    });
    let pretty = serde_json::to_string_pretty(&json).expect("report serializes");
    let path = report_path();
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")))
        .expect("report dir exists");
    std::fs::write(&path, &pretty).expect("report written");
    println!("report written: {}", path.display());

    // Full loud, machine-parsable summary.
    println!(
        "UI-D5 perf: {tris} triangles @ {WIDTH}x{HEIGHT} (real wgpu adapter {})",
        info.name
    );
    println!(
        "adapter: name={} driver={} driver_info={} backend={:?} device_type={:?}",
        info.name, info.driver, info.driver_info, info.backend, info.device_type
    );
    println!(
        "frame ms: min={min_ms:.3} mean={mean_ms:.3} p50={p50_ms:.3} p95={p95_ms:.3} p99={p99_ms:.3} max={max_ms:.3}"
    );
    println!(
        "fps: min={fps_min:.1} mean={fps_mean:.1} over {MEASURED_FRAMES} measured frames (after {WARMUP_FRAMES} warm-up)"
    );
    println!(
        "cpu determinism: demo frame_hash = {cpu_hash}, committed golden = {CPU_DEMO_GOLDEN_HASH}, match = {cpu_hashes_match}"
    );
    assert!(
        cpu_hashes_match,
        "CPU determinism changed on the perf card ({cpu_hash} != {CPU_DEMO_GOLDEN_HASH})"
    );
    let verdict = if budget_met {
        "THRESHOLD MET"
    } else {
        "THRESHOLD MISSED"
    };
    println!("BUDGET: p95={p95_ms:.3} ms vs committed threshold {BUDGET_P95_MS} ms -> {verdict}");
    assert!(
        budget_met,
        "UI-D5 interactive budget NOT met on this device: p95 = {p95_ms:.3} ms > {BUDGET_P95_MS} ms at {tris} triangles / {WIDTH}x{HEIGHT} on {} ({}) - recorded in {}, bar NOT lowered",
        info.name,
        info.driver,
        path.display()
    );
}
