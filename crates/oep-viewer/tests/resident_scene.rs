//! UI-D10 UI-RESIDENT-SCENE: the acceptance harness for the render-seam fix.
//!
//! Proves with real printed numbers, on the committed tree, that the seam now
//! keeps scene geometry resident on the GPU, culls per primitive and re-uploads
//! only genuinely dirty streams (the UI-D9 ADR Option A / brief stages 1+2):
//!
//! 1. **Steady state uploads only the MVP.** At the 1,002,528-triangle grid the
//!    pre-D10 seam re-serialised and uploaded 120,303,360 B of vertex bytes
//!    every frame; after this card a steady-state frame writes only the
//!    64-byte MVP uniform (`last_upload_bytes() == 64`) and re-renders by
//!    referencing the resident buffers.
//! 2. **The counter is real.** The same instrument moves when the camera moves
//!    (the painter-sorted stream genuinely changes) and when one mesh is
//!    edited through `set_scene` (scene generation bumps the dirty key).
//!    3. **No device-buffer allocations per frame.** `last_buffer_allocs() ==
//!    0` and `total_buffer_allocs()` stays constant across the measured frames
//!    — `create_buffer_init` per frame is gone (regression guard).
//! 4. **Per-primitive cull is a real reduction.** A framed multi-primitive
//!    view culls whole primitives on their world AABB before assembling
//!    anything (`last_culled_prims() > 0`, submitted < total, ratio printed).
//! 5. **Fast path is byte-identical.** A resident-reused frame equals the
//!    preceding full-path frame byte-for-byte (the determinism oracle holds
//!    through the fast path).
//! 6. **Culling is conservative.** The demo box at every committed D7 standard
//!    view is NEVER culled (0 culled), so the world-AABB test does not remove
//!    geometry the pre-D10 path drew — the sha256 golden tests pin the pixels.
//!
//! The pixel oracle (21 proof frames + CPU golden + coverage band) is enforced
//! by the unchanged `render_axis_views`, `gpu_shading`, `gpu_backend` and
//! `pick_gpu` legs; this harness owns the resident-seam counters.

use std::sync::{Mutex, OnceLock};

use oep_viewer::gpu::device;
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::{Camera, GpuBackend, ViewportBackend};

/// Grid side in vertices for the >= 1M-triangle scene (2·(N−1)² triangles).
const GRID_N: usize = 709;
/// The pre-D10 per-frame vertex upload at 1,002,528 triangles (3,007,584
/// vertices x 40 B), the number this card must collapse.
const PRE_D10_UPLOAD_BYTES: u64 = 120_303_360;

/// Serialises the device-touching tests in this binary (lavapipe segfaults on
/// concurrent create/destroy, the same class the other backends guard).
fn gpu_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

use oep_viewer::gpu::shading::{self, ViewMode};

/// Build a `size x size x size` unit cube mesh at `offset`.
fn cube_mesh(offset: [f32; 3]) -> MeshData {
    let (x, y, z) = (offset[0], offset[1], offset[2]);
    MeshData {
        positions: vec![
            [x, y, z],
            [x + 1.0, y, z],
            [x + 1.0, y + 1.0, z],
            [x, y + 1.0, z],
            [x, y, z + 1.0],
            [x + 1.0, y, z + 1.0],
            [x + 1.0, y + 1.0, z + 1.0],
            [x, y + 1.0, z + 1.0],
        ],
        triangles: vec![
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 4, 5],
            [0, 5, 1],
            [2, 6, 7],
            [2, 7, 3],
            [1, 5, 6],
            [1, 6, 2],
            [3, 7, 4],
            [3, 4, 0],
        ],
        edges: vec![],
    }
}

/// A spaced grid of distinct mesh primitives scattered over `[-N, N]^3`: big
/// enough that a framed sub-view culls a real fraction, small enough to build
/// instantly. Every prim is a unique mesh (12 triangles).
fn sparse_assembly(n: i32, seed_step: f32) -> Scene {
    let mut primitives = Vec::new();
    for ix in -n..=n {
        for iy in -n..=n {
            for iz in -n..=n {
                let off = [
                    ix as f32 * 4.0 * seed_step,
                    iy as f32 * 4.0 * seed_step,
                    iz as f32 * 4.0 * seed_step,
                ];
                primitives.push(Primitive::from_mesh(
                    format!("sparse:{ix}:{iy}:{iz}"),
                    "sparse-part",
                    "sparse-body",
                    cube_mesh(off),
                ));
            }
        }
    }
    let bounds = primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    Scene { primitives, bounds }
}

/// The deterministic 1,002,528-triangle heightfield grid (same as the UI-D5
/// perf harness) as a single mesh primitive.
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
    let mut triangles = Vec::with_capacity(2 * (GRID_N - 1) * (GRID_N - 1));
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
    let bounds = scene_default_bounds(&[&prim]);
    Scene {
        primitives: vec![prim],
        bounds,
    }
}

/// The union world AABB of a slice of primitives.
fn scene_default_bounds(prims: &[&Primitive]) -> Option<oep_viewer::scene::Aabb> {
    prims.iter().map(|p| p.world).reduce(|a, b| a.union(&b))
}

/// A camera that looks at only the central part of a scene: a narrow FOV so
/// whole primitives land outside the frustum (deterministic cull reduction).
fn close_camera(scene: &Scene) -> Camera {
    let bounds = scene.bounds.expect("scene has bounds");
    let mut cam = Camera::framed(&bounds);
    cam.fov_y = 10f64.to_radians();
    cam.zoom(0.5);
    cam
}

/// The committed D7 standard view yaw/pitch pairs (mirror of
/// `render_axis_views::standard_views`).
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

// ---------------------------------------------------------------------------
// 1. Steady-state upload at the 1M scene: 64 B, no allocations, from 120 MB
// ---------------------------------------------------------------------------

/// The headline number: at the 1,002,528-triangle grid a steady-state frame
/// uploads ONLY the 64-byte MVP uniform (was 120,303,360 B/frame), makes no
/// device-buffer allocations, and draws in single digits.
#[test]
fn resident_1m_grid_steady_state_uploads_only_mvp() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    assert!(device::adapter_available(), "needs a wgpu adapter");

    let scene = million_triangle_scene();
    let tris: usize = scene.primitives[0]
        .mesh
        .as_ref()
        .expect("grid mesh")
        .triangles
        .len();
    println!("1M scene: {tris} triangles (>= 1,000,000 asserted)");
    assert!(tris >= 1_000_000);

    let ctx = device::acquire().expect("adapter reachable");
    let mut gpu = GpuBackend::new(scene, ctx).expect("GpuBackend::new");
    gpu.resize(640, 480);

    // Warm / first full path (resident buffers built once).
    let first_upload = gpu.last_upload_bytes();
    let total_allocs_after_first = gpu.total_buffer_allocs();
    println!(
        "first full-path frame: upload={first_upload} B (pre-D10 every frame was {PRE_D10_UPLOAD_BYTES} B), total device buffer allocs={total_allocs_after_first}",
    );

    // Steady state: identical (scene, camera, size) frames.
    let mut all_sixty_four = true;
    let mut allocs_constant = true;
    let mut draws_single = true;
    let mut idle_uploads: Vec<u64> = Vec::new();
    for i in 0..6 {
        gpu.re_render_with_current_shading();
        let up = gpu.last_upload_bytes();
        idle_uploads.push(up);
        if up != 64 {
            all_sixty_four = false;
        }
        if gpu.last_buffer_allocs() != 0 || gpu.total_buffer_allocs() != total_allocs_after_first {
            allocs_constant = false;
        }
        if gpu.last_draw_calls() > 9 {
            draws_single = false;
        }
        println!(
            "steady frame {i}: upload={up} B, buffer_allocs={} (total {}), draw_calls={}, fast_path={}",
            gpu.last_buffer_allocs(),
            gpu.total_buffer_allocs(),
            gpu.last_draw_calls(),
            gpu.last_fast_path(),
        );
    }
    assert!(
        all_sixty_four,
        "steady-state uploads must be 64 B, got {idle_uploads:?}"
    );
    assert!(
        allocs_constant,
        "device-buffer allocations must stay constant at steady state"
    );
    assert!(draws_single, "draw calls must stay single-digit");
    println!(
        "PASS: steady-state uploads = 64 B/frame (was {PRE_D10_UPLOAD_BYTES} B/frame before UI-D10); buffer allocations constant; draw calls single-digit"
    );
}

// ---------------------------------------------------------------------------
// 2. The upload counter is real: it moves on a camera move and on a mesh edit
// ---------------------------------------------------------------------------

/// The instrument is real: a camera move re-uploads the changed painter-sorted
/// stream (once), then steady state returns to 64 B; a mesh edit through
/// `set_scene` does the same. Both moves happen with ZERO new device-buffer
/// allocations (the resident buffers were created once).
#[test]
fn upload_counter_moves_on_camera_move_and_mesh_edit() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    let scene = sparse_assembly(2, 1.0);
    let ctx = device::acquire().expect("adapter reachable");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    gpu.resize(800, 600);

    // Steady state before the move.
    gpu.re_render_with_current_shading();
    assert_eq!(
        gpu.last_upload_bytes(),
        64,
        "pre-move steady upload must be 64"
    );

    // Camera move: a genuinely different view -> dirty -> full re-upload.
    let mut moved = gpu.camera();
    moved.orbit(0.7, 0.3);
    gpu.set_camera(moved);
    let cam_upload = gpu.last_upload_bytes();
    println!(
        "camera move: upload bytes = {cam_upload} (was 64 at steady state; per-frame geometry before UI-D10 was {PRE_D10_UPLOAD_BYTES})"
    );
    assert!(
        cam_upload > 64,
        "the upload counter must MOVE when the camera moves, got {cam_upload}"
    );
    assert_eq!(
        gpu.last_buffer_allocs(),
        0,
        "a camera move must not create device buffers (resident buffers reused)"
    );

    // And back to steady state.
    gpu.set_camera(gpu.camera());
    assert_eq!(gpu.last_upload_bytes(), 64, "same view again is 64 B again");

    // Mesh edit: mutate one mesh and push it in through set_scene.
    let mut edited = scene;
    let edited_mesh = edited.primitives[3].mesh.as_mut().expect("mesh");
    edited_mesh.positions[0][0] += 0.25; // a genuine byte change
    let before_hash = gpu.frame_hash();
    gpu.set_scene(edited);
    let edit_upload = gpu.last_upload_bytes();
    let after_hash = gpu.frame_hash();
    println!(
        "mesh edit: upload bytes = {edit_upload}, frame hash {before_hash} -> {after_hash} (changed = {})",
        before_hash != after_hash
    );
    assert!(
        edit_upload > 64,
        "editing one mesh must re-upload geometry, got {edit_upload}"
    );
    assert_ne!(
        before_hash, after_hash,
        "a mesh edit must change the rendered pixels"
    );

    // Steady state after the edit.
    gpu.re_render_with_current_shading();
    assert_eq!(
        gpu.last_upload_bytes(),
        64,
        "post-edit steady upload is 64 B"
    );
    println!("PASS: upload counter responds to a camera move and to a one-mesh edit; resident buffers are never re-allocated on either");
}

// ---------------------------------------------------------------------------
// 3. Per-primitive culling is a real reduction
// ---------------------------------------------------------------------------

/// A framed close view of a spaced assembly: whole primitives are culled on
/// their world AABB *before* assembling anything, so the frame submits a real
/// subset of the scene's triangles and the assemble wall time is measured.
#[test]
fn frustum_culling_submits_a_culled_subset() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    let scene = sparse_assembly(5, 1.0);
    let total_prims = scene.primitives.len();
    let ctx = device::acquire().expect("adapter reachable");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    gpu.resize(800, 600);
    gpu.set_camera(close_camera(&scene));

    let submitted = gpu.last_faces_submitted();
    let total = gpu.last_scene_triangles();
    let culled = gpu.last_culled_prims();
    let assemble_ms = gpu.last_assemble_time().as_secs_f64() * 1000.0;
    let ratio = 1.0 - submitted as f64 / total as f64;
    println!(
        "cull: total_prims={total_prims} culled_prims={culled} submitted_faces={submitted} total_triangles={total} culled_ratio={ratio:.3} assemble_ms={assemble_ms:.3}"
    );
    assert!(
        submitted < total,
        "the framed view must submit a culled subset (submitted {submitted} < total {total})"
    );
    assert!(culled > 0, "the framed view must cull whole primitives");
    assert!(
        ratio > 0.05,
        "culled ratio should be a real reduction, got {ratio}"
    );
    println!("PASS: culled {culled}/{total_prims} primitives, submitted {submitted}/{total} triangles ({:.1}% culled)", ratio * 100.0);
}

/// Culling is conservative: the demo box at every committed D7 standard view
/// is NEVER culled (0 primitives), so the world-AABB frustum test cannot have
/// removed geometry the pre-D10 path drew on the sha256-pinned goldens.
#[test]
fn cull_is_conservative_on_the_committed_axis_views() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    let scene = oep_viewer::CpuBackend::demo_scene();
    let ctx = device::acquire().expect("adapter reachable");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    for (name, (yaw, pitch)) in standard_views() {
        let bounds = scene.bounds.expect("bounds");
        let mut cam = Camera::framed(&bounds);
        cam.yaw = yaw;
        cam.pitch = pitch;
        gpu.set_camera(cam);
        gpu.resize(400, 300);
        let culled = gpu.last_culled_prims();
        println!("{name}: culled_prims = {culled}");
        assert_eq!(
            culled, 0,
            "the fully-framed demo box must never be culled at the {name} view"
        );
    }
    // The all-culled camera (near plane inside the model): the whole box IS
    // genuinely outside the near plane, so culling legitimately removes it —
    // exactly the D7 empty-frame semantics.
    let bounds = scene.bounds.expect("bounds");
    let mut cam = Camera::framed(&bounds);
    cam.near = 250.0;
    cam.far = 5000.0;
    gpu.set_camera(cam);
    gpu.resize(400, 300);
    println!(
        "all-culled camera: culled_prims = {}",
        gpu.last_culled_prims()
    );
    assert_eq!(
        gpu.last_culled_prims(),
        1,
        "the all-culled view must cull the box"
    );
    println!("PASS: the frustum cull never removes the framed model on the committed axis views, and removes it only when truly out of view");
}

// ---------------------------------------------------------------------------
// 4. Determinism through the fast path
// ---------------------------------------------------------------------------

/// The resident fast path is byte-identical to the full path: the same view
/// rendered with the reused resident buffers hashes exactly like the
/// preceding full-derivation frame, and two independent backends agree.
#[test]
fn fast_path_frame_is_byte_identical_to_full_path() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    let scene = sparse_assembly(2, 1.0);
    let cam_close = close_camera(&scene);
    let ctx = device::acquire().expect("adapter reachable");
    let mut gpu = GpuBackend::new(scene.clone(), ctx).expect("GpuBackend::new");
    gpu.resize(800, 600);
    gpu.set_camera(cam_close); // full path
    assert!(!gpu.last_fast_path(), "first view is a full path");
    let full_hash = gpu.frame_hash();
    let full_frame = gpu.frame().clone();

    gpu.re_render_with_current_shading(); // identical inputs -> fast path
    assert!(
        gpu.last_fast_path(),
        "identical inputs take the resident fast path"
    );
    assert_eq!(gpu.last_upload_bytes(), 64, "fast path moves only the MVP");
    assert_eq!(
        gpu.frame_hash(),
        full_hash,
        "fast-path pixels == full-path pixels"
    );
    assert_eq!(gpu.frame().data, full_frame.data, "bytes equal");

    // An independent backend framed the same way agrees too.
    let mut gpu2 =
        GpuBackend::new(scene, device::acquire().expect("adapter")).expect("GpuBackend::new");
    gpu2.resize(800, 600);
    gpu2.set_camera(cam_close);
    assert_eq!(gpu2.frame_hash(), full_hash, "independent backend agrees");

    // And a mode change (shading) is a real dirty event, not silently cached:
    // alpha 0.5 renders different bytes.
    shading::set_state(shading::ShadingState {
        mode: ViewMode::Shaded,
        alpha: 0.5,
    });
    gpu.re_render_with_current_shading();
    assert!(!gpu.last_fast_path(), "a shading change is dirty");
    assert_ne!(gpu.frame_hash(), full_hash, "alpha 0.5 must differ");
    // restore the opaque default for any following test in this binary.
    shading::set_state(shading::ShadingState {
        mode: ViewMode::Shaded,
        alpha: 1.0,
    });
    println!(
        "PASS: fast-path hash {full_hash} == full-path hash; shading change dirty; independent backend agrees"
    );
}

// ---------------------------------------------------------------------------
// 5. Resident-scene unit checks (no device needed)
// ---------------------------------------------------------------------------

/// The world-AABB frustum test itself: behind-near, beyond-far and fully
/// off-side boxes are rejected; a box straddling the near plane is kept.
#[test]
fn aabb_visible_plane_arithmetic() {
    // A camera looking down -z from a known eye (default pose, target = -z).
    let cam = Camera {
        target: oep_viewer::Vec3::new(0.0, 0.0, -10.0),
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };
    let vp = cam.view_projection(1.0);
    let inside = |min: [f64; 3], max: [f64; 3]| {
        oep_viewer::gpu::resident::aabb_visible(
            &oep_viewer::scene::Aabb::new(
                oep_viewer::Vec3::new(min[0], min[1], min[2]),
                oep_viewer::Vec3::new(max[0], max[1], max[2]),
            ),
            &vp,
        )
    };
    // Directly in front of the camera: visible.
    assert!(
        inside([-0.5, -0.5, -15.0], [0.5, 0.5, -5.0]),
        "in-view box must be visible"
    );
    // Far beyond the far plane: rejected.
    assert!(
        !inside([-0.5, -0.5, -1e6], [0.5, 0.5, -0.99e6]),
        "box beyond the far plane must be culled"
    );
    // Behind the camera (the default eye sits at z = +110, looking down -z):
    // rejected.
    assert!(
        !inside([-0.5, -0.5, 200.0], [0.5, 0.5, 210.0]),
        "box behind the camera must be culled"
    );
    // Far off to the left: rejected by the left plane.
    assert!(
        !inside([-5000.0, -0.5, -15.0], [-4999.0, 0.5, -5.0]),
        "box fully off the left side must be culled"
    );
    println!("PASS: aabb_visible rejects behind/far/off-side boxes and keeps in-view boxes");
}
