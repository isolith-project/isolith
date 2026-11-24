//! GPU viewport backend (wave D, card UI-D1).
//!
//! [`GpuBackend`] is a second implementation of the [`ViewportBackend`] seam
//! that renders the SAME `scene`/`camera`/`size` on the GPU with wgpu,
//! instead of the CPU rasterizer. It renders into an offscreen colour target
//! and reads the pixels back into the seam's [`RgbaImage`], so nothing above
//! the trait changes: the panel still draws a plain image texture and the
//! costly sync happens once per frame, offscreen.
//!
//! It is selected at runtime ([`crate::backend::select_backend`]) when a wgpu
//! adapter is reachable, using THE SAME device the egui renderer uses when the
//! app registered one ([`device::register_egui_device`] — the shared device
//! with egui; the lock keeps a single wgpu version, checked by
//! `cargo tree -p oep-app -i wgpu`). With no adapter, or `ISOLITH_GPU=off`,
//! the caller falls back to the deterministic CPU backend.
//!
//! # Projection model (why the CPU computes the NDC)
//!
//! The CPU backend's committed goldens are produced by `render::render_present`,
//! whose clip-space convention projects a point with `w = row4(mvp)·(view·p)`
//! — the renderer's documented `clip_w`. The mathematically-raw
//! `row4(mvp)·p` does NOT reproduce it: measured on the committed demo view,
//! the raw form projects the same 80x40x10 box onto screen ~1.6x larger
//! (corner bbox 340..456px vs 325..511px), so a GPU that projects with the
//! textbook formula shows a genuinely different view than the CPU goldens.
//! To keep "same scene, same view" numerically identical across backends —
//! the wave-D parity line — [`GpuBackend`] projects every vertex with the
//! exact same CPU math and hands the GPU pre-divided NDC positions. The GPU
//! owns the rasterization; the CPU owns the matrix pipeline. This is the same
//! immediate-mode model egui itself uses (screen-space quads through a
//! passthrough vertex stage), and it is what makes the 5% coverage contract
//! well-posed. Wave D2+ may move the transform into the vertex shader once
//! the CPU path's projection is corrected and the goldens re-captured.
//!
//! So the visual-fidelity contract with the CPU path is geometric: the same
//! polygons, in the same painter order, at the same pixels within
//! rasterization noise. GPU and CPU pixels are NOT byte-equal (different
//! rasterizers and edge rules, and the GPU skips the 5x7 axis LETTERS),
//! which is why the seam's determinism oracle is re-proven on the GPU path
//! (two identical renders hash identically) and the coverage comparison
//! allows a documented 5% band rather than byte equality.
//!
//! The one intentional visual difference from `CpuBackend`: the GPU path does
//! not rasterize the axis tip letters X/Y/Z — the arms are drawn, the letters
//! are a wave-D2 (UI-GPU-SHADING) concern.
//!
//! Submodules: [`device`] (adapter/device acquisition, shared with egui),
//! [`target`] (offscreen colour target + readback) and [`idbuffer`] (the id
//! pass: "what is actually drawn" rasterized into a per-pixel primitive id
//! grid, card UI-D3).

pub mod device;
pub mod idbuffer;
pub mod target;

use std::time::{Duration, Instant};

use oep_geo::math::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::backend::frame_hash;
use crate::camera::Camera;
use crate::render::RgbaImage;
use crate::scene::{Primitive, Scene};
use crate::{overlay, theme};

use device::GpuContext;
use target::{GpuTarget, TARGET_FORMAT};

/// The background colour the target is cleared to (the theme's viewport
/// background, in 0..1 floats, byte-identical to the CPU background after the
/// `Rgba8Unorm` round-trip).
const CLEAR: [f32; 4] = [
    theme::BACKGROUND_RGB[0] as f32 / 255.0,
    theme::BACKGROUND_RGB[1] as f32 / 255.0,
    theme::BACKGROUND_RGB[2] as f32 / 255.0,
    1.0,
];

/// Overlay wire colour for solid boundary edges (same neutral the CPU
/// renderer uses, `render::EDGE_RGB`).
const EDGE_RGB: [u8; 4] = [170, 176, 184, 255];

/// Default width/height at backend construction, matching `CpuBackend`.
const DEFAULT_WIDTH: u32 = crate::render::DEFAULT_WIDTH;
const DEFAULT_HEIGHT: u32 = crate::render::DEFAULT_HEIGHT;

/// One interleaved GPU vertex: NDC position + flat colour.
///
/// `pos` carries already-projected NDC (x/y in [-1,1], top-left origin, z
/// unused). `color` is the literal final byte colour (already shaded for
/// faces) scaled to 0..1; the `Rgba8Unorm` target round-trips it back to the
/// same byte.
#[derive(Debug, Clone, Copy)]
struct Vertex {
    /// NDC position (x/y in [-1,1]; z unused).
    pos: [f32; 3],
    /// Flat colour in 0..1.
    color: [f32; 4],
}

impl Vertex {
    fn new(pos: [f64; 2], rgba: [u8; 4]) -> Self {
        Self {
            pos: [pos[0] as f32, pos[1] as f32, 0.0],
            color: [
                f32::from(rgba[0]) / 255.0,
                f32::from(rgba[1]) / 255.0,
                f32::from(rgba[2]) / 255.0,
                f32::from(rgba[3]) / 255.0,
            ],
        }
    }

    /// Pack this vertex into `bytes` as `[pos f32x3][color f32x4]`.
    fn pack_into(&self, bytes: &mut Vec<u8>) {
        for c in self.pos {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        for c in self.color {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
}

/// One filled face ready for GPU submission: its NDC polygon, the flattened
/// shade colour, and the view-space depth used for the painter's sort.
struct GpuFace {
    depth: f64,
    color: [u8; 4],
    ndc: Vec<[f64; 2]>,
}

/// GPU-backend pipeline set: one triangle pipeline for filled faces, one line
/// pipeline for the grid, mesh edges and triad arms. No bind groups or
/// uniform buffers — every vertex arrives pre-projected (NDC).
struct GpuPipeline {
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    target: GpuTarget,
}

/// The GPU viewport backend: same scene, GPU path, shared device with egui.
///
/// Owns the device context, the pipelines and the offscreen target, and
/// mirrors the CPU frame composition as described in the module docs.
pub struct GpuBackend {
    scene: Scene,
    camera: Camera,
    width: u32,
    height: u32,
    ctx: GpuContext,
    pipeline: GpuPipeline,
    frame: RgbaImage,
    hash: u64,
    frame_time: Duration,
}

impl GpuBackend {
    /// Wrap `scene` with a camera framed to its bounds (or `Camera::default`
    /// for an empty scene) on the given device context, and render once at
    /// the renderer's default size.
    ///
    /// Returns `None` only when the caller should not use the GPU path — the
    /// module's contract is that `Some` is returned for any context whose
    /// device is usable (the seam test asserts exactly this when an adapter
    /// exists).
    pub fn new(scene: Scene, ctx: GpuContext) -> Option<Self> {
        let camera = if let Some(bounds) = &scene.bounds {
            Camera::framed(bounds)
        } else {
            Camera::default()
        };
        let width = DEFAULT_WIDTH;
        let height = DEFAULT_HEIGHT;
        let pipeline = init_pipeline(&ctx, width, height)?;
        let mut backend = Self {
            scene,
            camera,
            width,
            height,
            ctx,
            pipeline,
            frame: RgbaImage {
                width: 0,
                height: 0,
                data: Vec::new(),
            },
            hash: 0,
            frame_time: Duration::ZERO,
        };
        backend.render_frame();
        Some(backend)
    }

    /// Render the current `(scene, camera, size)` into the offscreen target
    /// and update `frame`/`hash`/`frame_time`.
    ///
    /// The same inputs always produce the same command sequence and therefore
    /// byte-identical pixels (the seam's determinism oracle on the GPU path).
    fn render_frame(&mut self) {
        let start = Instant::now();
        let (faces, lines) = self.assemble(&self.scene, &self.camera, self.width, self.height);

        let device = &self.ctx.device;
        let queue = &self.ctx.queue;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("isolith-gpu-viewport-pass"),
            color_attachments: &[Some(self.pipeline.target.color_attachment(CLEAR))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // 1. Filled faces, painter-sorted back-to-front (deepest first) so
        //    later draws overwrite nearer ones exactly like the CPU raster.
        let face_bytes = pack(&faces);
        let face_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("isolith-gpu-viewport-faces"),
            contents: &face_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.pipeline.triangles);
        pass.set_vertex_buffer(0, face_buf.slice(..));
        pass.draw(0..face_bytes.len() as u32 / 28, 0..1);

        // 2. Lines (mesh boundary edges, grid, then the triad arms).
        if !lines.is_empty() {
            let line_bytes = pack(&lines);
            let line_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("isolith-gpu-viewport-lines"),
                contents: &line_bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            pass.set_pipeline(&self.pipeline.lines);
            pass.set_vertex_buffer(0, line_buf.slice(..));
            pass.draw(0..line_bytes.len() as u32 / 28, 0..1);
        }
        drop(pass);

        queue.submit([encoder.finish()]);

        self.frame = self.pipeline.target.read_back(device, queue);
        self.frame_time = start.elapsed();
        self.hash = frame_hash(&self.frame);
    }

    /// Project the scene into NDC vertex lists for one frame, mirroring
    /// `render::render` + `render::render_present`.
    ///
    /// Returns `(faces, lines)` where `lines` holds the mesh boundary edges,
    /// the grid and the screen-constant triad arms in draw order. Every NDC
    /// position uses the CPU renderer's exact projection formula (module
    /// docs).
    fn assemble(
        &self,
        scene: &Scene,
        camera: &Camera,
        width: u32,
        height: u32,
    ) -> (Vec<Vertex>, Vec<Vertex>) {
        let aspect = f64::from(width) / f64::from(height);
        let world_to_view = camera.view_matrix();
        let vp = camera.view_projection(aspect);
        let light = Vec3::new(-0.4, 0.8, 0.45).normalize().expect("light dir");

        let mut face_ndc: Vec<GpuFace> = Vec::new();
        for prim in &scene.primitives {
            let color = if prim.highlighted {
                [
                    theme::HIGHLIGHT_RGB[0],
                    theme::HIGHLIGHT_RGB[1],
                    theme::HIGHLIGHT_RGB[2],
                    255,
                ]
            } else {
                theme::part_color(&prim.part_id)
            };
            match &prim.mesh {
                Some(mesh) => {
                    for t in &mesh.triangles {
                        let poly = tri_positions(&mesh.positions, t);
                        push_polygon(&mut face_ndc, &poly, color, &world_to_view, &vp, &light);
                    }
                }
                None => {
                    for face in box_faces(prim) {
                        push_polygon(&mut face_ndc, &face, color, &world_to_view, &vp, &light);
                    }
                }
            }
        }
        // Painter: farthest (largest view z) first, exactly like the CPU path.
        face_ndc.sort_by(|a, b| {
            b.depth
                .partial_cmp(&a.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut faces = Vec::with_capacity(face_ndc.len() * 3);
        for face in &face_ndc {
            for tri in fan_triangles(&face.ndc) {
                for p in tri {
                    faces.push(Vertex::new(p, face.color));
                }
            }
        }

        let mut lines: Vec<Vertex> = Vec::new();

        // Mesh boundary edges, projected and clipped to the NDC cube like the
        // CPU wire path.
        for prim in &scene.primitives {
            let Some(mesh) = &prim.mesh else {
                continue;
            };
            if mesh.edges.is_empty() {
                continue;
            }
            let color = if prim.highlighted {
                prim_color(prim)
            } else {
                EDGE_RGB
            };
            for [ia, ib] in &mesh.edges {
                let (Some(pa), Some(pb)) = (mesh.positions.get(*ia), mesh.positions.get(*ib))
                else {
                    continue;
                };
                push_wire(
                    &mut lines,
                    &Vec3::new(pa[0] as f64, pa[1] as f64, pa[2] as f64),
                    &Vec3::new(pb[0] as f64, pb[1] as f64, pb[2] as f64),
                    &world_to_view,
                    &vp,
                    color,
                );
            }
        }

        // World grid on the sketch plane, same 1/2/5 spacing derivation.
        let raw = scene
            .bounds
            .map(|b| {
                [
                    b.min.x.abs().max(b.max.x.abs()),
                    b.min.y.abs().max(b.max.y.abs()),
                ]
            })
            .unwrap_or([100.0, 100.0]);
        let extent = raw.into_iter().fold(1.0f64, f64::max);
        let spacing = overlay::nice_spacing(extent / 4.0);
        for seg in overlay::grid_segments(extent * 1.3, spacing) {
            push_wire(
                &mut lines,
                &seg[0],
                &seg[1],
                &world_to_view,
                &vp,
                grid_color(),
            );
        }

        // Screen-constant triad arms (the CPU compositor's pixel-space arms,
        // converted to NDC for the passthrough stage).
        append_screen_axis_arms(&mut lines, scene, &world_to_view, &vp, width, height);

        (faces, lines)
    }
}

fn prim_color(prim: &Primitive) -> [u8; 4] {
    if prim.highlighted {
        [
            theme::HIGHLIGHT_RGB[0],
            theme::HIGHLIGHT_RGB[1],
            theme::HIGHLIGHT_RGB[2],
            255,
        ]
    } else {
        theme::part_color(&prim.part_id)
    }
}

fn grid_color() -> [u8; 4] {
    [
        theme::GRID_RGB[0],
        theme::GRID_RGB[1],
        theme::GRID_RGB[2],
        255,
    ]
}

/// Pack a list of vertices into interleaved LE bytes.
fn pack(verts: &[Vertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(verts.len() * 28);
    for v in verts {
        v.pack_into(&mut bytes);
    }
    bytes
}

/// The world-space polygon of a mesh triangle.
fn tri_positions(positions: &[[f32; 3]], t: &[u32; 3]) -> Vec<Vec3> {
    let mut v = Vec::with_capacity(3);
    for i in t {
        if let Some(p) = positions.get(*i as usize) {
            v.push(Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        }
    }
    v
}

/// The 6 faces of a box proxy as world-space quads (mirror of
/// `render::box_faces`).
fn box_faces(prim: &Primitive) -> [Vec<Vec3>; 6] {
    let p = &prim.world_corners;
    [
        vec![p[0], p[1], p[2], p[3]],
        vec![p[4], p[5], p[6], p[7]],
        vec![p[0], p[1], p[5], p[4]],
        vec![p[3], p[2], p[6], p[7]],
        vec![p[0], p[3], p[7], p[4]],
        vec![p[1], p[2], p[6], p[5]],
    ]
}

/// Push one world polygon through NDC culling, Lambert shading and depth
/// sorting into the face list (mirror of `render::push_polygon`, clip-plane
/// free because this backend passes no clip planes). The stored points are the
/// projected NDC (x/y) so the GPU rasterizes exactly the CPU's polygon.
fn push_polygon(
    faces: &mut Vec<GpuFace>,
    poly: &[Vec3],
    color: [u8; 4],
    world_to_view: &Mat4,
    vp: &Mat4,
    light: &Vec3,
) {
    if poly.len() < 3 {
        return;
    }
    let normal = newell_normal(poly);
    let shade = normal.dot(light).max(0.0) * 0.55 + 0.45;
    let fcolor = shade_color(color, shade);
    let mut ok = true;
    let mut depth_sum = 0.0f64;
    let mut ndc = Vec::with_capacity(poly.len());
    for p in poly {
        let view = world_to_view.transform_point(p);
        let clip = vp.transform_point(p);
        let w = clip_w(vp, &view);
        if w.abs() < 1e-9 {
            ok = false;
            break;
        }
        let nx = clip.x / w;
        let ny = clip.y / w;
        let nz = clip.z / w;
        // Near/far frustum cull (NDC z in [-1,1]), exactly like the CPU.
        if nz < -1.0 || nz > 1.0 {
            ok = false;
            break;
        }
        depth_sum += view.z;
        ndc.push([nx, ny]);
    }
    if !ok || ndc.len() < 3 {
        return;
    }
    faces.push(GpuFace {
        depth: depth_sum / poly.len() as f64,
        color: fcolor,
        ndc,
    });
}

/// Project a single edge segment into visible NDC, clip it to the NDC cube
/// with the CPU wire path's Liang-Barsky step, and append the two surviving
/// endpoints (each a 2D NDC position, as a pair for the line pipeline).
fn push_wire(
    out: &mut Vec<Vertex>,
    a: &Vec3,
    b: &Vec3,
    world_to_view: &Mat4,
    vp: &Mat4,
    color: [u8; 4],
) {
    let Some(ndc_a) = to_ndc(a, world_to_view, vp) else {
        return;
    };
    let Some(ndc_b) = to_ndc(b, world_to_view, vp) else {
        return;
    };
    let Some((t0, t1)) = clip_ndc_line(ndc_a, ndc_b) else {
        return;
    };
    out.push(Vertex::new(lerp_ndc_xy(ndc_a, ndc_b, t0), color));
    out.push(Vertex::new(lerp_ndc_xy(ndc_a, ndc_b, t1), color));
}

/// Project one world point to `(ndc_x, ndc_y, ndc_z)` with the CPU's exact
/// projection formula; `None` when the clip w is degenerate.
fn to_ndc(p: &Vec3, world_to_view: &Mat4, vp: &Mat4) -> Option<[f64; 3]> {
    let view = world_to_view.transform_point(p);
    let clip = vp.transform_point(p);
    let w = clip_w(vp, &view);
    if w.abs() < 1e-9 {
        return None;
    }
    Some([clip.x / w, clip.y / w, clip.z / w])
}

/// Liang-Barsky clip of a 3D NDC segment to the visible cube `[-1,1]^3`
/// (mirror of `render::clip_ndc_line`); returns the surviving `t` interval.
fn clip_ndc_line(a: [f64; 3], b: [f64; 3]) -> Option<(f64, f64)> {
    let mut t0 = 0.0f64;
    let mut t1 = 1.0f64;
    for axis in 0..3 {
        let d0 = -1.0 - a[axis];
        let d1 = 1.0 - a[axis];
        let dd = b[axis] - a[axis];
        if dd.abs() < 1e-12 {
            if d0 < 0.0 || d1 < 0.0 {
                return None;
            }
            continue;
        }
        let ta = d0 / dd;
        let tb = d1 / dd;
        let (lo, hi) = if ta < tb { (ta, tb) } else { (tb, ta) };
        t0 = t0.max(lo);
        t1 = t1.min(hi);
        if t0 > t1 {
            return None;
        }
    }
    if t1 < 0.0 || t0 > 1.0 {
        return None;
    }
    Some((t0.max(0.0), t1.min(1.0)))
}

/// The x/y of the NDC segment at parameter `t`.
fn lerp_ndc_xy(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Newell's method: robust area-weighted face normal (mirror of
/// `render::newell_normal`).
fn newell_normal(poly: &[Vec3]) -> Vec3 {
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..poly.len() {
        let cur = poly[i];
        let nxt = poly[(i + 1) % poly.len()];
        nx += (cur.y - nxt.y) * (cur.z + nxt.z);
        ny += (cur.z - nxt.z) * (cur.x + nxt.x);
        nz += (cur.x - nxt.x) * (cur.y + nxt.y);
    }
    Vec3::new(nx, ny, nz)
        .normalize()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// Shade an opaque colour by a factor, clamped exactly like the CPU raster
/// (mirror of `render::shade_color`).
fn shade_color(c: [u8; 4], k: f64) -> [u8; 4] {
    let f = k.clamp(0.35, 1.0);
    let m = |x: u8| ((f64::from(x)) * f).round().clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2]), c[3]]
}

/// Fan a convex NDC polygon into a triangle list (mirror of
/// `render::fan_triangles`).
fn fan_triangles(pts: &[[f64; 2]]) -> Vec<[[f64; 2]; 3]> {
    if pts.len() < 3 {
        return Vec::new();
    }
    (1..pts.len() - 1)
        .map(|i| [pts[0], pts[i], pts[i + 1]])
        .collect()
}

/// The `w` clip coordinate produced by the CPU renderer (mirror of
/// `render::clip_w` — the shipped goldens encode this exact formula).
fn clip_w(vp: &Mat4, view: &Vec3) -> f64 {
    let a = vp.to_array16();
    a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15]
}

/// Project a view-space point to pixel coordinates (top-left origin); `None`
/// when behind the camera or outside the NDC z range (mirror of
/// `render::to_px`).
fn to_px(view: Vec3, vp: &Mat4, width: u32, height: u32) -> Option<(f64, f64)> {
    let clip = vp.transform_point(&view);
    let w = clip_w(vp, &view);
    if w.abs() < 1e-9 {
        return None;
    }
    let nx = clip.x / w;
    let ny = clip.y / w;
    let nz = clip.z / w;
    if nz < -1.0 || nz > 1.0 {
        return None;
    }
    Some((
        (nx * 0.5 + 0.5) * f64::from(width),
        (0.5 - ny * 0.5) * f64::from(height),
    ))
}

/// The screen-constant XY triad arms, as NDC line vertices (mirror of the
/// axes block in `render::render_present`, without the 5x7 tip letters).
fn append_screen_axis_arms(
    verts: &mut Vec<Vertex>,
    scene: &Scene,
    world_to_view: &Mat4,
    vp: &Mat4,
    width: u32,
    height: u32,
) {
    let origin = scene
        .bounds
        .map(|b| Vec3::new(0.0, 0.0, b.min.z.min(0.0)))
        .unwrap_or_else(Vec3::zero);
    let Some(proj_origin) = to_px(world_to_view.transform_point(&origin), vp, width, height) else {
        return;
    };
    let (ax, ay) = (
        proj_origin.0.clamp(8.0, width.saturating_sub(8) as f64),
        proj_origin.1.clamp(8.0, height.saturating_sub(8) as f64),
    );
    let arm_px = (f64::from(width.min(height)) * 0.07).max(24.0);
    let arm_world = scene
        .bounds
        .map(|b| b.sphere_radius())
        .unwrap_or(10.0)
        .max(1e-3);
    let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let colors = [
        [
            theme::AXIS_X_RGB[0],
            theme::AXIS_X_RGB[1],
            theme::AXIS_X_RGB[2],
            255,
        ],
        [
            theme::AXIS_Y_RGB[0],
            theme::AXIS_Y_RGB[1],
            theme::AXIS_Y_RGB[2],
            255,
        ],
        [
            theme::AXIS_Z_RGB[0],
            theme::AXIS_Z_RGB[1],
            theme::AXIS_Z_RGB[2],
            255,
        ],
    ];
    for (i, axis) in axes.iter().enumerate() {
        let p = origin.add(&Vec3::new(axis[0], axis[1], axis[2]).mul(arm_world));
        let Some(pp) = to_px(world_to_view.transform_point(&p), vp, width, height) else {
            continue;
        };
        let (dx, dy) = (pp.0 - proj_origin.0, pp.1 - proj_origin.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }
        let (ux, uy) = (dx / len, dy / len);
        let (ex, ey) = (ax + ux * arm_px, ay + uy * arm_px);
        let color = colors[i];
        verts.push(Vertex::new(screen_ndc(ax, ay, width, height), color));
        verts.push(Vertex::new(screen_ndc(ex, ey, width, height), color));
    }
}

/// Pixel -> NDC for the top-left-origin passthrough stage (matches the CPU
/// `to_px` inverse and egui's own screen convention).
fn screen_ndc(x: f64, y: f64, width: u32, height: u32) -> [f64; 2] {
    [
        x / f64::from(width) * 2.0 - 1.0,
        1.0 - y / f64::from(height) * 2.0,
    ]
}

/// Build the pipelines and target for a device context.
fn init_pipeline(ctx: &GpuContext, width: u32, height: u32) -> Option<GpuPipeline> {
    let device = &ctx.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolith-gpu-viewport-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/viewport.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolith-gpu-viewport-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let target = GpuTarget::new(device, width, height);

    let make = |topology: wgpu::PrimitiveTopology| -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("isolith-gpu-viewport-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(VERTEX_LAYOUT)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_color"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };

    Some(GpuPipeline {
        triangles: make(wgpu::PrimitiveTopology::TriangleList),
        lines: make(wgpu::PrimitiveTopology::LineList),
        target,
    })
}

/// The interleaved vertex buffer layout: `[pos f32x3 @0][color f32x4 @1]`.
const ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 12,
        shader_location: 1,
    },
];

/// The single vertex buffer layout used by every pipeline.
const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'_> = wgpu::VertexBufferLayout {
    array_stride: 28,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &ATTRS,
};

impl crate::backend::ViewportBackend for GpuBackend {
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.pipeline.target = GpuTarget::new(&self.ctx.device, self.width, self.height);
        self.render_frame();
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.render_frame();
    }

    fn frame(&self) -> &RgbaImage {
        &self.frame
    }

    fn frame_hash(&self) -> u64 {
        self.hash
    }

    fn frame_time_secs(&self) -> f64 {
        self.frame_time.as_secs_f64()
    }

    fn backend_name(&self) -> &'static str {
        "gpu"
    }
}
