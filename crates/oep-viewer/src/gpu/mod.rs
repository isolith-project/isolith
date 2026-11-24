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
//! [`GpuBackend`] projects every vertex with the exact same CPU math and
//! hands the GPU pre-divided NDC positions: `clip = vp · (p, 1)` and
//! `w = row4(vp) · (p, 1)` in BODY (world) space, then NDC =
//! `clip.xyz / w` — the `render::clip_w` formula. The GPU owns the
//! rasterization; the CPU owns the matrix pipeline. This is the same
//! immediate-mode model egui itself uses (screen-space quads through a
//! passthrough vertex stage), and it is what makes the 5% coverage contract
//! well-posed.
//!
//! The pre-D7 code fed the VIEW-space point into the w computation
//! (`w = row4(vp) · (view, 1)`), double-applying the view transform: it
//! scaled every frame ~1.6x and degenerated to `w == 0` at the exact axis
//! views (UI-D4's Back-view blocker). UI-D7 corrected line and GPU both to
//! the world-space form above and re-captured the goldens, so the quirk
//! paragraph is intentionally gone.
//!
//! So the visual-fidelity contract with the CPU path is geometric: the same
//! polygons, in the same painter order, at the same pixels within
//! rasterization noise. GPU and CPU pixels are NOT byte-equal (different
//! rasterizers and edge rules, and the GPU skips the 5x7 axis LETTERS),
//! which is why the seam's determinism oracle is re-proven on the GPU path
//! (two identical renders hash identically) and the coverage comparison
//! allows a documented band rather than byte equality.
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
pub mod shading;
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
use shading::{ViewMode, BOX_EDGES};
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

/// The shaded pass's blend state.
///
/// Colour is straight-alpha source-over (`src.a * src + (1 - src.a) * dst`),
/// so at `alpha = 1.0` it is byte-equivalent to REPLACE (the D1 opaque
/// pass) and below 1.0 an overlapping body shows through. The ALPHA channel
/// is written as the source alpha (`1 * src.a + 0 * dst.a` — REPLACE), so
/// the transparency control is measurable at readback: a 50% body reads back
/// alpha ≈ 128 instead of 255 — the number the UI-D2 acceptance prints
/// before/after. (wgpu's `ALPHA_BLENDING`/OVER alpha equation saturates the
/// alpha channel to 1.0 once the dst below is opaque, which would make the
/// readback proof unmeasurable; the source-alpha write keeps the control
/// inspectable while the rgb stays a true source-over composite.)
const STUDIO_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    },
};

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

/// One shaded-face vertex: NDC position, view-space normal (unit, for the
/// studio key/fill fragment lighting) and base RGBA (with the transparency
/// alpha applied).
///
/// The fragment stage (`shading.wgsl`) computes `key + fill` diffuse lighting
/// from the interpolated normal and multiplies the base colour, and carries
/// the alpha straight through (opaque REPLACE-equivalent at alpha 1.0, real
/// transparency below that).
#[derive(Debug, Clone, Copy)]
struct ShadedVertex {
    /// NDC position (x/y in [-1,1]; z unused).
    pos: [f32; 3],
    /// View-space unit normal (flat: shared per face; smooth: per-vertex).
    normal: [f32; 3],
    /// Base RGBA in 0..1 (alpha = shading transparency).
    color: [f32; 4],
}

impl ShadedVertex {
    fn new(pos: [f64; 2], normal: [f64; 3], rgba: [u8; 4]) -> Self {
        Self {
            pos: [pos[0] as f32, pos[1] as f32, 0.0],
            normal: [normal[0] as f32, normal[1] as f32, normal[2] as f32],
            color: [
                f32::from(rgba[0]) / 255.0,
                f32::from(rgba[1]) / 255.0,
                f32::from(rgba[2]) / 255.0,
                f32::from(rgba[3]) / 255.0,
            ],
        }
    }

    /// Pack into `bytes` as `[pos f32x3][normal f32x3][color f32x4]` (40 B).
    fn pack_into(&self, bytes: &mut Vec<u8>) {
        for c in self.pos {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        for c in self.normal {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        for c in self.color {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
}

/// One filled face ready for GPU submission: its NDC polygon, the per-vertex
/// view-space normals (flat: same normal per triangle; smooth: per-vertex
/// averaged), the flattened shade colour, and the view-space depth used for
/// the painter's sort.
struct GpuFace {
    depth: f64,
    color: [u8; 4],
    ndc: Vec<[f64; 2]>,
    /// View-space (camera-space) unit normal per NDC vertex, in the same
    /// order as `ndc`. Flat mode stores the face normal on every vertex;
    /// smooth mode stores the per-vertex averaged normal.
    normal: Vec<[f64; 3]>,
}

/// GPU-backend pipeline set. The shaded pipeline renders filled faces through
/// the studio key+fill lighting shader (`shading.wgsl`); the line pipeline
/// draws the grid, feature edges and triad arms (~passthrough colour).
/// No bind groups or uniform buffers — vertices arrive pre-projected (NDC),
/// normals in view space.
struct GpuPipeline {
    /// Studio-lit triangle pipeline (shading.wgsl, alpha blended).
    shading: wgpu::RenderPipeline,
    /// Line pipeline for grid/edges/triad (viewport.wgsl, replace blend).
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
    /// Number of feature-edge line primitives drawn in the last render
    /// (the `edges` boundary list exactly). Reset on every render.
    edge_primitives: usize,
    /// The shading state applied to the last render (mode + alpha).
    shading: shading::ShadingState,
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
            edge_primitives: 0,
            shading: shading::state(),
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
        self.shading = shading::state();
        let (faces, lines, edge_prims) =
            self.assemble(&self.scene, &self.camera, self.width, self.height);
        self.edge_primitives = edge_prims;

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
        //    (Skip in `Edges` mode, which draws the boundary edge list only.)
        if !faces.is_empty() {
            let face_bytes = pack_shaded(&faces);
            let face_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("isolith-gpu-viewport-faces"),
                contents: &face_bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            pass.set_pipeline(&self.pipeline.shading);
            pass.set_vertex_buffer(0, face_buf.slice(..));
            pass.draw(0..face_bytes.len() as u32 / 40, 0..1);
        }

        // 2. Lines (feature edges, grid, then the triad arms).
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

    /// Project the scene into NDC vertex lists for one frame, honouring the
    /// current shading state (view mode + alpha).
    ///
    /// Returns `(faces, lines)` where `faces` are the (mode-dependent)
    /// shaded-triangle vertices and `lines` hold the feature edges (in the
    /// `Edges`/`Shaded` modes only), the grid and the screen-constant triad
    /// arms in draw order. Every NDC position uses the CPU renderer's exact
    /// projection formula (module docs).
    fn assemble(
        &self,
        scene: &Scene,
        camera: &Camera,
        width: u32,
        height: u32,
    ) -> (Vec<ShadedVertex>, Vec<Vertex>, usize) {
        let aspect = f64::from(width) / f64::from(height);
        let world_to_view = camera.view_matrix();
        let vp = camera.view_projection(aspect);
        let alpha = self.shading.alpha.clamp(0.0, 1.0);
        let mode = self.shading.mode;

        let mut face_ndc: Vec<GpuFace> = Vec::new();
        if mode != ViewMode::Edges {
            for prim in &scene.primitives {
                let color = if prim.highlighted {
                    [
                        theme::HIGHLIGHT_RGB[0],
                        theme::HIGHLIGHT_RGB[1],
                        theme::HIGHLIGHT_RGB[2],
                        (alpha * 255.0).round() as u8,
                    ]
                } else {
                    let mut c = theme::part_color(&prim.part_id);
                    c[3] = (alpha * 255.0).round() as u8;
                    c
                };
                match &prim.mesh {
                    Some(mesh) => {
                        let positions: Vec<Vec3> = mesh
                            .positions
                            .iter()
                            .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
                            .collect();
                        let smooth = if mode == ViewMode::Smooth || mode == ViewMode::Shaded {
                            Some(shading::smooth_normals(&positions, &mesh.triangles))
                        } else {
                            None
                        };
                        for t in &mesh.triangles {
                            let poly = tri_positions(&mesh.positions, t);
                            let normals = match &smooth {
                                Some(sm) => {
                                    [sm[t[0] as usize], sm[t[1] as usize], sm[t[2] as usize]]
                                }
                                None => shading::flat_triangle_normals(&poly),
                            };
                            push_face(&mut face_ndc, &poly, &normals, color, &world_to_view, &vp);
                        }
                    }
                    None => {
                        let quads = box_faces(prim);
                        // Smooth corner normals: average the incident face
                        // normals per corner (area-weighted by the quad fan).
                        // `quads` vertices are `world_corners` in the fixed
                        // index pattern below (mirrors `render::box_faces`).
                        const BOX_FACE_CORNERS: [[usize; 4]; 6] = [
                            [0, 1, 2, 3],
                            [4, 5, 6, 7],
                            [0, 1, 5, 4],
                            [3, 2, 6, 7],
                            [0, 3, 7, 4],
                            [1, 2, 6, 5],
                        ];
                        let corners = prim.world_corners;
                        let smooth_corner = if mode == ViewMode::Smooth || mode == ViewMode::Shaded
                        {
                            let tris: Vec<[u32; 3]> =
                                crate::gpu::shading::box_fan_triangles().to_vec();
                            crate::gpu::shading::smooth_normals(corners.as_ref(), &tris)
                        } else {
                            Vec::new()
                        };
                        for (face, idxs) in quads.iter().zip(BOX_FACE_CORNERS.iter()) {
                            let face_normal = crate::gpu::shading::newell(face);
                            let normals: Vec<Vec3> = if smooth_corner.is_empty() {
                                vec![face_normal; face.len()]
                            } else {
                                idxs.iter()
                                    .map(|&i| smooth_corner.get(i).copied().unwrap_or(face_normal))
                                    .collect()
                            };
                            push_face(&mut face_ndc, face, &normals, color, &world_to_view, &vp);
                        }
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

        let mut faces: Vec<ShadedVertex> = Vec::with_capacity(face_ndc.len() * 3);
        for face in &face_ndc {
            let idxs = fan_indices(face.ndc.len());
            for (tri, idx) in fan_triangles(&face.ndc).iter().zip(idxs.iter()) {
                for (k, p) in tri.iter().enumerate() {
                    faces.push(ShadedVertex::new(*p, face.normal[idx[k]], face.color));
                }
            }
        }

        let mut lines: Vec<Vertex> = Vec::new();
        // The edge pass draws EXACTLY the boundary edge list: a mesh
        // primitive draws its committed `mesh.edges`; a box proxy draws its
        // 12 box edges (the proxy's boundary edge list). The count of line
        // primitives the pass submits is recorded (`edge_primitives`) so the
        // acceptance can compare drawn vs fixture boundary edge count.
        let mut edge_prims = 0usize;
        if mode == ViewMode::Shaded || mode == ViewMode::Edges {
            for prim in &scene.primitives {
                let color = if prim.highlighted {
                    prim_color(prim)
                } else {
                    EDGE_RGB
                };
                match &prim.mesh {
                    Some(mesh) => {
                        for [ia, ib] in &mesh.edges {
                            let (Some(pa), Some(pb)) =
                                (mesh.positions.get(*ia), mesh.positions.get(*ib))
                            else {
                                continue;
                            };
                            let before = lines.len();
                            push_wire(
                                &mut lines,
                                &Vec3::new(pa[0] as f64, pa[1] as f64, pa[2] as f64),
                                &Vec3::new(pb[0] as f64, pb[1] as f64, pb[2] as f64),
                                &vp,
                                color,
                            );
                            if lines.len() > before {
                                edge_prims += 1;
                            }
                        }
                    }
                    None => {
                        let corners = prim.world_corners;
                        for [ia, ib] in BOX_EDGES {
                            let (Some(pa), Some(pb)) = (corners.get(ia), corners.get(ib)) else {
                                continue;
                            };
                            let before = lines.len();
                            push_wire(&mut lines, pa, pb, &vp, color);
                            if lines.len() > before {
                                edge_prims += 1;
                            }
                        }
                    }
                }
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
            push_wire(&mut lines, &seg[0], &seg[1], &vp, grid_color());
        }

        // Screen-constant triad arms (the CPU compositor's pixel-space arms,
        // converted to NDC for the passthrough stage).
        append_screen_axis_arms(&mut lines, scene, &vp, width, height);

        (faces, lines, edge_prims)
    }

    /// The number of feature-edge line primitives drawn by the LAST render
    /// (the boundary edge list exactly — mesh edges for meshes, the 12 box
    /// edges for box proxies). Only `Shaded` and `Edges` modes run the pass,
    /// so other modes report 0. Zeroed at the start of every render.
    pub fn last_edge_primitives(&self) -> usize {
        self.edge_primitives
    }

    /// The shading state (view mode + alpha) of the last render.
    pub fn shading_state(&self) -> shading::ShadingState {
        self.shading
    }

    /// Re-render the current view, picking up any shading-state change that
    /// [`shading::set_state`] made since the previous render. Used by the
    /// runtime shading panel (and by the gpu_shading acceptance) to apply a
    /// new view mode / alpha without moving the camera.
    pub fn re_render_with_current_shading(&mut self) {
        self.render_frame();
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

/// Pack a list of shaded-face vertices into interleaved LE bytes (40 B each).
fn pack_shaded(verts: &[ShadedVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(verts.len() * 40);
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

/// Push one world polygon through NDC culling into the face list, storing the
/// per-vertex VIEW-SPACE normals and the base (unlit) colour for the studio
/// fragment shader (mirror of `render::push_polygon`, clip-plane free, with
/// the colour flattened instead of Lambert-shaded — the GPU owns the lighting
/// now).
///
/// The stored points are the projected NDC (x/y) so the GPU rasterizes
/// exactly the CPU's polygon. `normals` must be unit world-space normals (one
/// per polygon vertex); they are rotated into view space here because the
/// light rig is defined in view space.
fn push_face(
    faces: &mut Vec<GpuFace>,
    poly: &[Vec3],
    world_normals: &[Vec3],
    color: [u8; 4],
    world_to_view: &Mat4,
    vp: &Mat4,
) {
    if poly.len() < 3 {
        return;
    }
    let mut ok = true;
    let mut depth_sum = 0.0f64;
    let mut ndc = Vec::with_capacity(poly.len());
    let mut normal = Vec::with_capacity(poly.len());
    for (i, p) in poly.iter().enumerate() {
        let view = world_to_view.transform_point(p);
        let clip = vp.transform_point(p);
        // `w` must be the clip w of the SAME world point `p` (D7: feeding
        // the view-space point here double-applied the view transform and
        // zeroed w on exact axis views).
        let w = clip_w(vp, p);
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
        // Rotate the world normal into view space (normalize guards
        // degenerate input, matching the CPU's Newell fallback).
        let wn = world_normals
            .get(i)
            .copied()
            .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        let vn = world_to_view
            .transform_vector(&wn)
            .normalize()
            .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        normal.push([vn.x, vn.y, vn.z]);
    }
    if !ok || ndc.len() < 3 {
        return;
    }
    // The colour is stored UNLIT: the fragment shader applies the studio
    // key + fill lighting, so the same base colour reaches opaque and
    // transparent renders (the alpha was baked in the vertex colour by the
    // caller).
    faces.push(GpuFace {
        depth: depth_sum / poly.len() as f64,
        color,
        ndc,
        normal,
    });
}

/// Project a single edge segment into visible NDC, clip it to the NDC cube
/// with the CPU wire path's Liang-Barsky step, and append the two surviving
/// endpoints (each a 2D NDC position, as a pair for the line pipeline).
fn push_wire(out: &mut Vec<Vertex>, a: &Vec3, b: &Vec3, vp: &Mat4, color: [u8; 4]) {
    let Some(ndc_a) = to_ndc(a, vp) else {
        return;
    };
    let Some(ndc_b) = to_ndc(b, vp) else {
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
fn to_ndc(p: &Vec3, vp: &Mat4) -> Option<[f64; 3]> {
    let clip = vp.transform_point(p);
    let w = clip_w(vp, p);
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

/// The indices of each fan triangle's vertices (parallel to
/// [`fan_triangles`]) — one `[0, i, i+1]` per fan triangle, so a face's
/// per-vertex normals follow the geometry exactly.
fn fan_indices(n: usize) -> Vec<[usize; 3]> {
    if n < 3 {
        return Vec::new();
    }
    (1..n - 1).map(|i| [0, i, i + 1]).collect()
}

/// The `w` clip coordinate for a WORLD point under the combined
/// view-projection matrix (mirror of `render::clip_w`; the D7 correction —
/// feed the world point, never a pre-transformed view point, or the view
/// transform applies twice and `w` degenerates to 0 on exact axis views).
fn clip_w(vp: &Mat4, world: &Vec3) -> f64 {
    let a = vp.to_array16();
    a[3] * world.x + a[7] * world.y + a[11] * world.z + a[15]
}

/// Project a world-space point to pixel coordinates (top-left origin); `None`
/// when behind the camera or outside the NDC z range (mirror of
/// `render::to_px`; D7 — takes the world point, see `clip_w`).
fn to_px(world: Vec3, vp: &Mat4, width: u32, height: u32) -> Option<(f64, f64)> {
    let clip = vp.transform_point(&world);
    let w = clip_w(vp, &world);
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
    vp: &Mat4,
    width: u32,
    height: u32,
) {
    let origin = scene
        .bounds
        .map(|b| Vec3::new(0.0, 0.0, b.min.z.min(0.0)))
        .unwrap_or_else(Vec3::zero);
    let Some(proj_origin) = to_px(origin, vp, width, height) else {
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
        let Some(pp) = to_px(p, vp, width, height) else {
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
///
/// Two pipelines share one layout (no bind groups): the **shading** triangle
/// pass (studio key/fill lighting, alpha-blended so transparency composites)
/// and the **lines** pass (grid, feature edges, triad — opaque replace).
fn init_pipeline(ctx: &GpuContext, width: u32, height: u32) -> Option<GpuPipeline> {
    let device = &ctx.device;
    let shading_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolith-gpu-viewport-shading-shader"),
        source: wgpu::ShaderSource::Wgsl(shading::shading_wgsl().into()),
    });
    let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolith-gpu-viewport-line-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/viewport.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolith-gpu-viewport-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let target = GpuTarget::new(device, width, height);

    let make = |topology: wgpu::PrimitiveTopology,
                shader: &wgpu::ShaderModule,
                vertex_entry: &str,
                fragment_entry: &str,
                layout: wgpu::VertexBufferLayout<'_>,
                blend: Option<wgpu::BlendState>,
                label: &str| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(vertex_entry),
                compilation_options: Default::default(),
                buffers: &[Some(layout)],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment_entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend,
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

    let shading = make(
        wgpu::PrimitiveTopology::TriangleList,
        &shading_shader,
        "vs_main",
        "fs_main",
        SHADED_VERTEX_LAYOUT,
        Some(STUDIO_BLEND),
        "isolith-gpu-viewport-shading-pipeline",
    );
    let lines = make(
        wgpu::PrimitiveTopology::LineList,
        &line_shader,
        "vs_main",
        "fs_color",
        VERTEX_LAYOUT,
        Some(wgpu::BlendState::REPLACE),
        "isolith-gpu-viewport-lines-pipeline",
    );

    Some(GpuPipeline {
        shading,
        lines,
        target,
    })
}

/// The interleaved vertex buffer layout for line vertices:
/// `[pos f32x3 @0][color f32x4 @1]` (28 B).
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

/// The line vertex buffer layout (28 B).
const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'_> = wgpu::VertexBufferLayout {
    array_stride: 28,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &ATTRS,
};

/// The interleaved shaded-face vertex layout:
/// `[pos f32x3 @0][normal f32x3 @1][color f32x4 @2]` (40 B).
const SHADED_ATTRS: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 12,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 24,
        shader_location: 2,
    },
];

/// The shaded-face vertex buffer layout (40 B).
const SHADED_VERTEX_LAYOUT: wgpu::VertexBufferLayout<'_> = wgpu::VertexBufferLayout {
    array_stride: 40,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &SHADED_ATTRS,
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
