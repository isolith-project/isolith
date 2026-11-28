//! UI-D11 UI-DEPTH-SORT, Stage 1: PROVE the painter-sort misorder.
//!
//! This is the gate test of card UI-D11. The shared projection and the face
//! list are byte-trusted (UI-D7/D8/D10 pinned them); what is on trial here is
//! ORDER: the mean-depth painter sort orders each face by its AVERAGE view
//! depth, so it cannot be per-pixel correct where (a) two components
//! interpenetrate and the nearer surface changes inside one face, or (b) one
//! large face's near and far ends fall on opposite sides of another face.
//!
//! The test CONSTRUCTS both failing cases (no RNG, no corpus file, fully
//! deterministic geometry + a fixed set of camera poses) and measures, through
//! the EXISTING render path (the real [`GpuBackend`] + the real [`IdPass`]),
//! how many pixels the current pipeline paints with the wrong surface, against
//! an independent analytic oracle: per-pixel, the nearest triangle along the
//! camera ray.
//!
//! The verdict is quoted in the print output as MISORDER CONFIRMED with the
//! wrong-surface pixel count, or NOT REPRODUCED with what failed to break it.
//!
//! Everything is deterministic: the scenes are built from fixed coordinates,
//! the camera poses are a fixed list (framed, then the committed orbit
//! offsets), and the oracle is pure f64 arithmetic over the same scene.

use std::sync::{Mutex, OnceLock};

use oep_geo::math::Vec3;

use oep_viewer::camera::Camera;
use oep_viewer::gpu::device::{self, GpuContext};
use oep_viewer::gpu::idbuffer::IdPass;
use oep_viewer::render::{encode_png, RgbaImage};
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::{GpuBackend, ViewportBackend};

/// The proof directory for Stage 1 images / numbers (worktree-relative).
fn stage1_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("workstreams")
        .join("modeler")
        .join("ui")
        .join("proof")
        .join("UI-D11")
        .join("stage1")
}

/// Serialises every test that creates or drops a wgpu device (the repo-wide
/// lavapipe segfault guard).
fn gpu_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Pure-Rust SHA-256 (FIPS 180-4), self-contained (same helper as the
/// render_axis_views proof hashing).
fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let ml = (bytes.len() as u64).wrapping_mul(8);
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter()
        .flat_map(|v| v.to_be_bytes())
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ---------------------------------------------------------------------------
// Scene builders (deterministic, fixed coordinates only)
// ---------------------------------------------------------------------------

/// Box mesh (8 vertices, 12 triangles, 12 boundary edges) from 8 explicit
/// corners in the standard AABB ring order (0..3 = -z ring, 4..7 = +z ring).
fn rotated_box_mesh(p: &[[f64; 3]]) -> MeshData {
    let positions: Vec<[f32; 3]> = p
        .iter()
        .map(|v| [v[0] as f32, v[1] as f32, v[2] as f32])
        .collect();
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
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
    MeshData {
        positions,
        triangles,
        edges,
    }
}

/// Box mesh (same indexing as `rotated_box_mesh`) from axis-aligned min/max.
fn box_mesh(min: [f64; 3], max: [f64; 3]) -> MeshData {
    let corners: [[f64; 3]; 8] = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    rotated_box_mesh(&corners)
}

/// Scene 1, case (a): two interpenetrating components. Two thin diagonal
/// walls (planes y=x and y=-x) cross along the Z axis. Each wall is a long
/// flat slab; from an asymmetric camera their front faces overlap across a
/// wide stripe, and inside that stripe the actually-nearer wall alternates
/// left/right of the crossing line. A mean-depth painter cannot express the
/// alternation: it picks one global order, so on one side of the crossing it
/// paints the FARTHER wall where the NEARER wall should be visible.
fn scene_interpenetrating_walls() -> Scene {
    let mut scene = Scene::default();
    let (c, s) = (
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    );

    // Wall A: plane y = x. Axes u=(c,s,0) length, v=(0,0,1) span, n=(-s,c,0).
    let build = |su: f64, sv: f64, sn: f64, u: &Vec3, v: &Vec3, n: &Vec3| -> [f64; 3] {
        let p = u.mul(9.0 * su).add(&v.mul(5.0 * sv)).add(&n.mul(0.35 * sn));
        [p.x, p.y, p.z]
    };
    let ua = Vec3::new(c, s, 0.0);
    let va = Vec3::new(0.0, 0.0, 1.0);
    let na = Vec3::new(-s, c, 0.0);
    let pa = [
        build(1.0, 1.0, 1.0, &ua, &va, &na),
        build(1.0, 1.0, -1.0, &ua, &va, &na),
        build(1.0, -1.0, 1.0, &ua, &va, &na),
        build(1.0, -1.0, -1.0, &ua, &va, &na),
        build(-1.0, 1.0, 1.0, &ua, &va, &na),
        build(-1.0, 1.0, -1.0, &ua, &va, &na),
        build(-1.0, -1.0, 1.0, &ua, &va, &na),
        build(-1.0, -1.0, -1.0, &ua, &va, &na),
    ];
    scene
        .primitives
        .push(Primitive::from_mesh("wall-a", "pa", "ba", rotated_box_mesh(&pa)));

    // Wall B: plane y = -x. Axes u=(-c,s,0), v=(0,0,1), n=(s,c,0).
    let ub = Vec3::new(-c, s, 0.0);
    let nb = Vec3::new(s, c, 0.0);
    let pb = [
        build(1.0, 1.0, 1.0, &ub, &va, &nb),
        build(1.0, 1.0, -1.0, &ub, &va, &nb),
        build(1.0, -1.0, 1.0, &ub, &va, &nb),
        build(1.0, -1.0, -1.0, &ub, &va, &nb),
        build(-1.0, 1.0, 1.0, &ub, &va, &nb),
        build(-1.0, 1.0, -1.0, &ub, &va, &nb),
        build(-1.0, -1.0, 1.0, &ub, &va, &nb),
        build(-1.0, -1.0, -1.0, &ub, &va, &nb),
    ];
    scene
        .primitives
        .push(Primitive::from_mesh("wall-b", "pb", "bb", rotated_box_mesh(&pb)));

    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    scene
}

/// Scene 2, case (b): a concave L face crossing another face. The L face is a
/// single flat L-shaped sheet in the XZ plane (y=0); a vertical wall in the
/// plane x=1.2 crosses its long arm, so parts of the L sit in front of and
/// behind the wall's face simultaneously.
fn scene_concave_l_crossing_wall() -> Scene {
    let mut scene = Scene::default();

    // L-shaped sheet in the XZ plane (y = 0). Outline (x, z):
    //   (-7,-6) -> (7,-6) -> (7,-1.5) -> (0,-1.5) -> (0,6) -> (-7,6)
    // Four triangles as a simple ear fan around vertex 0.
    let xs = [-7.0, 7.0, 7.0, 0.0, 0.0, -7.0];
    let zs = [-6.0, -6.0, -1.5, -1.5, 6.0, 6.0];
    let positions: Vec<[f32; 3]> = (0..6)
        .map(|i| [xs[i] as f32, 0.0, zs[i] as f32])
        .collect();
    let triangles = vec![[0u32, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 5]];
    let mesh_l = MeshData {
        positions,
        triangles,
        edges: Vec::new(),
    };
    scene
        .primitives
        .push(Primitive::from_mesh("l-sheet", "pl", "bl", mesh_l));

    // Crossing vertical wall in the plane x = 1.2, spanning z in [-7,7],
    // y in [-3,3], thin in x (0.3 half-thickness).
    let wall = box_mesh([1.2 - 0.3, -3.0, -7.0], [1.2 + 0.3, 3.0, 7.0]);
    scene
        .primitives
        .push(Primitive::from_mesh("cross-wall", "pw", "bw", wall));

    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    scene
}

/// CONTROL scene: three disjoint boxes with no interpenetration and no
/// concave/self-intersecting face. Here the painter's mean-depth order IS the
/// correct per-pixel order, so the analytic oracle must agree with the current
/// pipeline almost everywhere (the only allowed disagreement is the thin
/// silhouette band where rasterization and the pixel-centre ray can round
/// differently). This validates the oracle: a large disagreement on THIS scene
/// would mean the ray oracle itself is wrong, not the painter.
fn control_disjoint_boxes() -> Scene {
    let mut scene = Scene::default();
    // Three separated boxes at different depths along the camera's default
    // view direction so one fully occludes another portion (no crossing).
    // Box 1: front-left, Box 2: back-centre, Box 3: front-right, disjoint.
    scene
        .primitives
        .push(Primitive::from_mesh(
            "box-1",
            "p1",
            "b1",
            box_mesh([-10.0, -4.0, -2.0], [-2.0, 4.0, 4.0]),
        ));
    scene
        .primitives
        .push(Primitive::from_mesh(
            "box-2",
            "p2",
            "b2",
            box_mesh([-3.0, -4.0, -14.0], [5.0, 4.0, -8.0]),
        ));
    scene
        .primitives
        .push(Primitive::from_mesh(
            "box-3",
            "p3",
            "b3",
            box_mesh([6.0, -4.0, -2.0], [14.0, 4.0, 4.0]),
        ));
    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    scene
}

// ---------------------------------------------------------------------------
// The analytic per-pixel oracle
// ---------------------------------------------------------------------------

/// One world-space triangle, tagged with its primitive index.
struct FaceTri {
    prim: u32,
    a: Vec3,
    b: Vec3,
    c: Vec3,
}

/// Triangle soup of a scene (mesh triangles only; box proxies are not used by
/// the Stage-1 scenes, which are all explicit meshes).
fn scene_triangles(scene: &Scene) -> Vec<FaceTri> {
    let mut out = Vec::new();
    for (pi, prim) in scene.primitives.iter().enumerate() {
        if let Some(mesh) = &prim.mesh {
            for t in &mesh.triangles {
                let get = |i: usize| {
                    let p = &mesh.positions[t[i] as usize];
                    Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
                };
                out.push(FaceTri {
                    prim: pi as u32,
                    a: get(0),
                    b: get(1),
                    c: get(2),
                });
            }
        }
    }
    out
}

/// Möller–Trumbore ray-triangle intersection; nearest forward `t` or `None`.
fn ray_tri(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let e1 = b.sub(&a);
    let e2 = c.sub(&a);
    let pv = dir.cross(&e2);
    let det = e1.dot(&pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = origin.sub(&a);
    let u = tv.dot(&pv) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let qv = tv.cross(&e1);
    let v = dir.dot(&qv) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(&qv) * inv;
    if t < 1e-6 {
        return None;
    }
    Some(t)
}

/// Per-pixel analytic nearest primitive: cast a camera ray (through the pixel
/// CENTER, matching GPU rasterization) and record the primitive index of the
/// nearest triangle hit (-1 = background). Deterministic f64.
fn analytic_nearest_grid(scene: &Scene, cam: &Camera, width: u32, height: u32) -> Vec<i32> {
    let tris = scene_triangles(scene);
    let mut grid = vec![-1i32; (width as usize) * (height as usize)];
    let aspect = f64::from(width) / f64::from(height);
    let eye = cam.eye();
    let vp_inv = cam
        .view_projection(aspect)
        .inverse()
        .expect("view-projection invertible");
    let a = vp_inv.to_array16();
    for y in 0..height {
        for x in 0..width {
            let nx = (f64::from(x) + 0.5) / f64::from(width) * 2.0 - 1.0;
            let ny = 1.0 - (f64::from(y) + 0.5) / f64::from(height) * 2.0;
            let unproj = |z: f64| {
                let cx = a[0] * nx + a[4] * ny + a[8] * z + a[12];
                let cy = a[1] * nx + a[5] * ny + a[9] * z + a[13];
                let cz = a[2] * nx + a[6] * ny + a[10] * z + a[14];
                let cw = a[3] * nx + a[7] * ny + a[11] * z + a[15];
                let iw = 1.0 / cw;
                Vec3::new(cx * iw, cy * iw, cz * iw)
            };
            let p_near = unproj(-1.0);
            let p_far = unproj(1.0);
            let dir = p_far.sub(&p_near).normalize().expect("non-degenerate");
            let mut best_t = f64::INFINITY;
            let mut best_prim: i32 = -1;
            for tri in &tris {
                if let Some(t) = ray_tri(eye, dir, tri.a, tri.b, tri.c) {
                    if t < best_t {
                        best_t = t;
                        best_prim = tri.prim as i32;
                    }
                }
            }
            grid[(y * width + x) as usize] = best_prim;
        }
    }
    grid
}

/// Compare the current pipeline's id buffer (same painter order as the colour
/// pass) to the analytic nearest-surface grid wherever both see a face.
/// Returns (wrong_pixels, compared_pixels, sampled mismatches).
fn wrong_surface_stats(
    pass: &IdPass,
    analytic: &[i32],
    width: u32,
    height: u32,
) -> (usize, usize, Vec<(u32, u32, u32, u32)>) {
    let mut wrong = 0usize;
    let mut compared = 0usize;
    let mut samples = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let an = analytic[(y * width + x) as usize];
            let drawn = pass.pick_at(x, y).map(|i| i as i32).unwrap_or(-1);
            if an >= 0 && drawn >= 0 {
                compared += 1;
                if an != drawn {
                    wrong += 1;
                    if samples.len() < 12 {
                        samples.push((x, y, drawn as u32, an as u32));
                    }
                }
            }
        }
    }
    (wrong, compared, samples)
}

/// Save one Stage-1 PNG and return its sha256.
fn save_stage1_image(name: &str, img: &RgbaImage) -> String {
    let dir = stage1_dir();
    std::fs::create_dir_all(&dir).expect("stage1 dir");
    let bytes = encode_png(img);
    let hash = sha256_hex(&bytes);
    std::fs::write(dir.join(format!("{name}.png")), &bytes).expect("write png");
    std::fs::write(dir.join(format!("{name}.png.sha256")), format!("{hash}\n")).unwrap();
    hash
}

/// The committed camera poses probed per scene: framed to the scene bounds,
/// then orbited to a fixed list of deterministic (yaw, pitch) offsets. The
/// test takes the WORST pose (largest wrong-surface count) for the verdict.
const POSE_OFFSETS: [(f64, f64); 8] = [
    (0.35, 0.25),
    (0.6, 0.3),
    (0.9, 0.45),
    (1.2, 0.4),
    (1.9, 0.55),
    (2.4, 0.35),
    (4.6, 0.5),
    (5.2, 0.3),
];

/// The proof resolution (modest so the analytic oracle stays fast).
const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

struct ProbeResult {
    scene_name: &'static str,
    chosen_pose: (f64, f64),
    wrong: usize,
    compared: usize,
    samples: Vec<(u32, u32, u32, u32)>,
    colour_hash: String,
}

/// Run the whole Stage-1 probe on one scene: render through the real backend
/// + id pass at every pose, count wrong surfaces vs the analytic oracle, keep
/// the worst pose, dump the colour frame + a wrong-surface overlay image.
fn probe_scene(name: &'static str, scene: Scene, ctx: &GpuContext) -> ProbeResult {
    let bounds = scene.bounds.expect("scene bounds");
    let base_cam = Camera::framed(&bounds);

    let mut best: Option<(f64, f64, usize, usize, Vec<(u32, u32, u32, u32)>)> = None;
    for (dyaw, dpitch) in POSE_OFFSETS {
        let mut cam = base_cam;
        cam.orbit(dyaw, dpitch);
        let analytic = analytic_nearest_grid(&scene, &cam, WIDTH, HEIGHT);
        let pass = IdPass::new(ctx.clone(), scene.clone(), cam, WIDTH, HEIGHT)
            .expect("id pass resources");
        let (wrong, compared, samples) = wrong_surface_stats(&pass, &analytic, WIDTH, HEIGHT);
        println!(
            "pose yaw={dyaw:.2} pitch={dpitch:.2} -> compared px={compared} wrong-surface px={wrong} ({:.2}% of compared)",
            if compared == 0 {
                0.0
            } else {
                wrong as f64 / compared as f64 * 100.0
            }
        );
        if best
            .as_ref()
            .map(|b| wrong > b.2)
            .unwrap_or(true)
        {
            best = Some((dyaw, dpitch, wrong, compared, samples));
        }
    }
    let (dyaw, dpitch, wrong, compared, samples) = best.expect("at least one pose");

    // Colour frame + wrong-surface overlay at the chosen pose.
    let mut cam = base_cam;
    cam.orbit(dyaw, dpitch);
    let mut gpu = GpuBackend::new(scene.clone(), ctx.clone()).expect("backend");
    gpu.set_camera(cam);
    gpu.resize(WIDTH, HEIGHT);
    let colour_hash = save_stage1_image(&format!("{name}-colour"), gpu.frame());

    let analytic = analytic_nearest_grid(&scene, &cam, WIDTH, HEIGHT);
    let pass = IdPass::new(ctx.clone(), scene.clone(), cam, WIDTH, HEIGHT).expect("id pass");
    let mut mask = gpu.frame().clone();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let an = analytic[(y * WIDTH + x) as usize];
            let drawn = pass.pick_at(x, y).map(|i| i as i32).unwrap_or(-1);
            if an >= 0 && drawn >= 0 && an != drawn {
                let i = ((y * WIDTH + x) as usize) * 4;
                mask.data[i] = 255;
                mask.data[i + 1] = 30;
                mask.data[i + 2] = 30;
                mask.data[i + 3] = 255;
            }
        }
    }
    save_stage1_image(&format!("{name}-wrongmask"), &mask);

    ProbeResult {
        scene_name: name,
        chosen_pose: (dyaw, dpitch),
        wrong,
        compared,
        samples,
        colour_hash,
    }
}

/// STAGE 1 GATE: prove the mean-depth painter misorders the constructed
/// scenes, with the real current pipeline (GpuBackend + IdPass) against the
/// analytic per-pixel oracle. Verdict quoted in the output and asserted.
#[test]
fn stage1_painter_sort_misorders_constructed_scenes() {
    let _g = gpu_lock().lock().expect("gpu lock");
    std::env::remove_var(device::GPU_FLAG);
    assert!(
        device::adapter_available(),
        "stage 1 needs a wgpu adapter (real render path requirement)"
    );
    let ctx = device::acquire().expect("adapter reachable");

    let walls = probe_scene("interpenetrating-walls", scene_interpenetrating_walls(), &ctx);
    let lcross = probe_scene("concave-l-cross", scene_concave_l_crossing_wall(), &ctx);
    let control = probe_scene("control-disjoint-boxes", control_disjoint_boxes(), &ctx);

    println!("\n=== STAGE 1 VERDICT ===");
    for r in [&walls, &lcross, &control] {
        println!(
            "[{}] chosen pose yaw={:.2} pitch={:.2}: wrong-surface px = {}/{} compared ({:.2}%)",
            r.scene_name,
            r.chosen_pose.0,
            r.chosen_pose.1,
            r.wrong,
            r.compared,
            r.wrong as f64 / r.compared.max(1) as f64 * 100.0
        );
        println!("[{}] colour frame sha256 = {}", r.scene_name, r.colour_hash);
        for (x, y, drawn, expected) in &r.samples {
            println!(
                "[{}] sample px ({x},{y}): current pipeline draws prim {drawn}, analytic expects {expected}",
                r.scene_name
            );
        }
    }

    // Oracle validation: the disjoint-boxes control (no interpenetration, no
    // concave/self-intersection) must show a tiny disagreement rate — the
    // painter is CORRECT there, so only the thin silhouette band can differ.
    // This proves the oracle agrees with the pipeline when ordering is right;
    // the huge disagreement on the two constructed cases is then attributable
    // to the mean-depth misorder, not to the oracle. The bands are separated
    // with a wide margin so a platform rasterizer quirk cannot blur them:
    // control < 2%, each failing case >= 10% and >= 10x the control rate.
    let control_rate = control.wrong as f64 / control.compared.max(1) as f64;
    let walls_rate = walls.wrong as f64 / walls.compared.max(1) as f64;
    let lcross_rate = lcross.wrong as f64 / lcross.compared.max(1) as f64;
    assert!(
        control_rate < 0.02,
        "oracle sanity: disjoint boxes must be ordered correctly by the painter \
         (< 2% of compared pixels may differ from silhouette rounding); got {:.2}% \
         ({}/{}). If this fails the analytic oracle is broken, not the painter.",
        control_rate * 100.0,
        control.wrong,
        control.compared
    );
    for (name, rate, wrong, compared) in [
        ("interpenetrating-walls", walls_rate, walls.wrong, walls.compared),
        ("concave-l-cross", lcross_rate, lcross.wrong, lcross.compared),
    ] {
        assert!(
            rate >= 0.10,
            "constructed case {name} must misorder at least 10% of compared pixels \
             (mean-depth defect), got {:.2}% ({}/{})",
            rate * 100.0,
            wrong,
            compared
        );
        assert!(
            rate >= 10.0 * control_rate,
            "constructed case {name} mismatch rate {:.2}% must dominate the control \
             silhouette band {:.2}% by >= 10x",
            rate * 100.0,
            control_rate * 100.0
        );
    }

    println!(
        "MISORDER CONFIRMED: the mean-depth painter paints the wrong surface at {} and {} pixels \
         on the two constructed scenes (real GpuBackend + IdPass vs analytic ray oracle); \
         the disjoint-box control stays within its {} px silhouette band",
        walls.wrong, lcross.wrong, control.wrong
    );
}
