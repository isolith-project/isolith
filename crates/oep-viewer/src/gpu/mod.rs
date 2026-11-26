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
//! # Projection model (the vertex stage owns the transform — UI-D8)
//!
//! The GPU backend now hands the shaded pass WORLD positions (plus view-space
//! normals and the flattened base colour) and the vertex stage owns the
//! transform: an MVP uniform (`mat4x4<f32>`, column-major, packed from
//! `Mat4::to_array16()` with `f32::to_ne_bytes`, no bytemuck) is applied as
//! `clip = mvp · (world, 1)`, then the vertex stage does the perspective
//! divide and outputs `w = 1` so the rasterizer's own divide is a no-op and
//! attribute interpolation stays linear in screen space — byte-identical to
//! the D1/D7 pre-projected behaviour. The CPU still runs the SAME projection
//! (`render::project_ndc`, the crate's single NDC path) purely to cull faces
//! and order the painter; the pixels come from the GPU's vertex stage.
//!
//! The LINE pass (grid, feature edges, the screen-constant triad arms) remains
//! a passthrough for pre-projected NDC: the triad arms are pixel-space by
//! construction (they come from `to_px`/`screen_ndc`, exactly the
//! immediate-mode egui model), so there is no world-space form to transform.
//! The store of the projection formula is `render::project_ndc` — one helper
//! in `render.rs`, no duplicated `clip_w` anywhere in this module (UI-D8).
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

/// One interleaved GPU vertex: screen-space NDC position + flat colour.
///
/// `pos` carries already-projected NDC (x/y in [-1,1], top-left origin, z
/// unused). `color` is the literal final byte colour (already shaded for
/// faces) scaled to 0..1; the `Rgba8Unorm` target round-trips it back to the
/// same byte.
///
/// This is the LINE (overlay) vertex: grid, feature edges and the
/// screen-constant triad arms are projected on the CPU through the single
/// `render::project_ndc` helper and arrive pre-divided, exactly the
/// immediate-mode egui model the D1 module doc describes (the triad arms are
/// pixel-space by construction and have no world-space home, so the line pass
/// stays a passthrough; the shaded FACE pass owns the MVP+divide).
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

/// One shaded-face vertex: WORLD position, view-space normal (unit, for the
/// studio key/fill fragment lighting) and base RGBA (with the transparency
/// alpha applied).
///
/// The vertex stage (`shading.wgsl`) applies the MVP from the uniform buffer
/// to the WORLD position and does the perspective divide (outputting w=1, so
/// the rasterizer's own divide is a no-op and attribute interpolation stays
/// linear in screen space exactly as the D1/D7 pre-projected path). The
/// fragment stage computes `key + fill` diffuse lighting from the
/// interpolated normal and multiplies the base colour, carrying the alpha
/// straight through (opaque REPLACE-equivalent at alpha 1.0, real
/// transparency below that).
#[derive(Debug, Clone, Copy)]
struct ShadedVertex {
    /// WORLD position (x/y/z in scene units; the transform lives in the
    /// vertex stage now).
    pos: [f32; 3],
    /// View-space unit normal (flat: shared per face; smooth: per-vertex).
    normal: [f32; 3],
    /// Base RGBA in 0..1 (alpha = shading transparency).
    color: [f32; 4],
}

impl ShadedVertex {
    fn new(pos: [f64; 3], normal: [f64; 3], rgba: [u8; 4]) -> Self {
        Self {
            pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
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

/// One filled face ready for GPU submission: its world-space polygon, the
/// per-vertex view-space normals (flat: same normal per triangle; smooth:
/// per-vertex averaged), the flattened shade colour, and the view-space depth
/// used for the painter's sort.
struct GpuFace {
    depth: f64,
    color: [u8; 4],
    /// WORLD-space vertices, transformed to clip by the vertex stage's MVP
    /// (the GPU owns the transform now). In the same order as `normal`.
    world: Vec<[f64; 3]>,
    /// View-space (camera-space) unit normal per world vertex, in the same
    /// order as `world`. Flat mode stores the face normal on every vertex;
    /// smooth mode stores the per-vertex averaged normal.
    normal: Vec<[f64; 3]>,
}

/// GPU-backend pipeline set. The shaded pipeline renders filled faces through
/// the studio key+fill lighting shader (`shading.wgsl`) whose vertex stage
/// applies the MVP from the bind group (`binding 0` uniform, column-major f32,
/// packed from `Mat4::to_array16()`) and does the perspective divide; the
/// line pipeline draws the grid, feature edges and triad arms (~passthrough
/// colour, pre-projected NDC — the screen-constant arms are pixel-space by
/// construction so the overlay pass stays a passthrough).
struct GpuPipeline {
    /// MVP (view-projection) uniform buffer: 64 bytes = `mat4x4<f32>`
    /// column-major, rewritten every render via `queue.write_buffer`.
    uniform: wgpu::Buffer,
    /// Bind group for the MVP (group 0, binding 0), shared by both pipelines.
    bind_group: wgpu::BindGroup,
    /// Studio-lit triangle pipeline (shading.wgsl, alpha blended).
    shading: wgpu::RenderPipeline,
    /// Line pipeline for grid/edges/triad (viewport.wgsl, replace blend).
    lines: wgpu::RenderPipeline,
    target: GpuTarget,
}

/// UI-D9 per-stage frame cost instrumentation (env-gated, inert when off).
///
/// Active ONLY when `ISOLITH_PERF_STAGES=1` (read once at construction).
/// When unset, `GpuBackend::perf` is `None` and the render path performs zero
/// timing calls; no `Instant::now()` in the seam, so the unset binary is
/// behaviour- and hash-identical to the pre-card tree (the fast smoke proves
/// the GPU frame hash unchanged with and without the variable). When set,
/// `render_frame` records a partition of the driving wall clock into named
/// stages and prints one machine-parseable line per frame to stderr, with each
/// stage's duration against the frame total so the sum can be checked against
/// the measured p95. Consumed by `scripts/ui/perf-profile.sh`.
struct PerfStages {
    frame: usize,
    samples: Vec<(&'static str, Duration)>,
}

impl PerfStages {
    /// Returns `Some` only when `ISOLITH_PERF_STAGES=1`; the unset path (and
    /// any other value) returns `None`, keeping the seam inert.
    fn enabled() -> Option<Self> {
        match std::env::var_os("ISOLITH_PERF_STAGES") {
            Some(v) if v == "1" => Some(Self {
                frame: 0,
                samples: Vec::new(),
            }),
            _ => None,
        }
    }

    /// Start a new frame: the stage table is per-frame and resets here.
    fn begin_frame(&mut self) {
        self.frame += 1;
        self.samples.clear();
    }

    /// Record one (named, wall-time) segment for the current frame.
    fn push(&mut self, name: &'static str, d: Duration) {
        self.samples.push((name, d));
    }

    /// Merge same-name segments into a deterministic stage table and print it
    /// with the frame total (stderr, `ISOLITH_PERF_STAGES` prefix) so a
    /// profile script can set the per-stage sum against `frame_time`.
    fn print_frame(&self, total: Duration) {
        let mut acc: Vec<(&'static str, Duration)> = Vec::new();
        for (name, d) in &self.samples {
            if let Some(entry) = acc.iter_mut().find(|(n, _)| *n == *name) {
                entry.1 += *d;
            } else {
                acc.push((*name, *d));
            }
        }
        let parts: Vec<String> = acc
            .iter()
            .map(|(n, d)| format!("{n}={:.3}ms", d.as_secs_f64() * 1e3))
            .collect();
        eprintln!(
            "ISOLITH_PERF_STAGES frame={} total={:.3}ms {}",
            self.frame,
            total.as_secs_f64() * 1e3,
            parts.join(" ")
        );
    }
}

/// Run `f` once, and when the stage instrumentation is live (`perf` is
/// `Some`) record the wall time as a named per-stage frame segment and return
/// it (so callers can sum segments and attribute a residual). When `perf` is
/// `None` this is a plain call with no timing work and returns `None`; the
/// seam is inert unless `ISOLITH_PERF_STAGES=1`.
fn timed_push(
    perf: &mut Option<PerfStages>,
    name: &'static str,
    mut f: impl FnMut(),
) -> Option<Duration> {
    if let Some(p) = perf.as_mut() {
        let t0 = Instant::now();
        f();
        let d = t0.elapsed();
        p.push(name, d);
        Some(d)
    } else {
        f();
        None
    }
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
    /// The wall time of the last render's GPU vertex preparation: packing
    /// the face/line vertex bytes and creating the device buffers that carry
    /// them (UI-D8 records this before/after so the transform move's upload
    /// cost is measured, not guessed).
    vertex_prep: Duration,
    /// The shading state applied to the last render (mode + alpha).
    shading: shading::ShadingState,
    /// UI-D9 env-gated per-stage frame cost instrumentation; `None` unless
    /// `ISOLITH_PERF_STAGES=1` (then the render path records and prints a
    /// per-frame stage partition, inert and zero-timing when unset).
    perf: Option<PerfStages>,
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
            vertex_prep: Duration::ZERO,
            shading: shading::state(),
            perf: PerfStages::enabled(),
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
        if let Some(p) = &mut self.perf {
            p.begin_frame();
        }
        self.shading = shading::state();
        // The assemble call is made through a receiver-less associated fn
        // (UI-D9's stage timers need `&mut perf`; a `&mut self` receiver would
        // fight the `&self.scene` argument borrow, so the inputs and the perf
        // handle are passed as disjoint parameters). `assemble` itself reports
        // its own total + sub-split (incl. an exact residual), so the per-stage
        // partition is self-consistent here.
        let scene = &self.scene;
        let camera = self.camera; // Copy
        let shading_state = self.shading; // Copy
        let (faces, lines, edge_prims) = Self::assemble(
            &mut self.perf,
            scene,
            &camera,
            self.width,
            self.height,
            shading_state,
        );
        self.edge_primitives = edge_prims;

        let device = &self.ctx.device;
        let queue = &self.ctx.queue;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // Vertex preparation: pack the face/line bytes and create the device
        // buffers (the per-frame cost the transform move is meant to shift /
        // measure; UI-D8 records it before and after). The MVP uniform is
        // rewritten here too — the vertex stage owns the transform.
        let prep_start = Instant::now();

        // The MVP the vertex stage applies to the WORLD positions: the same
        // view-projection the CPU culling uses (`render::project_ndc`), packed
        // column-major f32 — one transform, one place, shared with the CPU.
        let aspect = f64::from(self.width) / f64::from(self.height);
        let vp = self.camera.view_projection(aspect);
        queue.write_buffer(&self.pipeline.uniform, 0, &pack_matrix4(&vp));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("isolith-gpu-viewport-pass"),
            color_attachments: &[Some(self.pipeline.target.color_attachment(CLEAR))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.pipeline.bind_group, &[]);

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
        self.vertex_prep = prep_start.elapsed();
        if let Some(p) = &mut self.perf {
            // Shaded/lines vertex pack + device-buffer create + draw-record
            // (`prep_start.elapsed()` is exactly the UI-D8 vertex_prep bucket).
            p.push("vertex-prep", prep_start.elapsed());
        }

        let sub0 = Instant::now();
        queue.submit([encoder.finish()]);
        if let Some(p) = &mut self.perf {
            p.push("submit", sub0.elapsed());
        }

        let rb0 = Instant::now();
        self.frame = self.pipeline.target.read_back(device, queue);
        if let Some(p) = &mut self.perf {
            // read_back asserts the copy + map + device.poll; the poll is the
            // synchronous wait that makes the GPU raster time part of this
            // frame's wall clock (the seam is synchronous by design).
            p.push("readback+w", rb0.elapsed());
        }
        self.frame_time = start.elapsed();
        let h0 = Instant::now();
        self.hash = frame_hash(&self.frame);
        if let Some(p) = &mut self.perf {
            p.push("hash", h0.elapsed());
            p.print_frame(self.frame_time);
        }
    }

    /// Project the scene into NDC vertex lists for one frame, honouring the
    /// current shading state (view mode + alpha).
    ///
    /// Returns `(faces, lines)` where `faces` are the (mode-dependent)
    /// shaded-triangle vertices and `lines` hold the feature edges (in the
    /// `Edges`/`Shaded` modes only), the grid and the screen-constant triad
    /// arms in draw order. Every NDC position uses the CPU renderer's exact
    /// projection formula (module docs).
    ///
    /// Receiver-less so `render_frame` can pass the disjoint `perf` handle
    /// (UI-D9 stage instrumentation) alongside the scene/camera inputs.
    fn assemble(
        perf: &mut Option<PerfStages>,
        scene: &Scene,
        camera: &Camera,
        width: u32,
        height: u32,
        shading: shading::ShadingState,
    ) -> (Vec<ShadedVertex>, Vec<Vertex>, usize) {
        let aspect = f64::from(width) / f64::from(height);
        let world_to_view = camera.view_matrix();
        let vp = camera.view_projection(aspect);
        let alpha = shading.alpha.clamp(0.0, 1.0);
        let mode = shading.mode;

        // UI-D9 exact partition of `assemble`: the total wall time, the timed
        // segments below, and a reported `assemble.other` residual so no
        // unmetered time hides between the measured closures (the stage table
        // is a true partition of the function).
        let a_total = perf.is_some().then(Instant::now);
        let mut d_pos = Duration::ZERO;
        let mut d_norm = Duration::ZERO;
        let mut d_proj = Duration::ZERO;
        let mut d_sort = Duration::ZERO;
        let mut d_expand = Duration::ZERO;

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
                        let mut positions: Vec<Vec3> = Vec::new();
                        d_pos += timed_push(perf, "assemble.positions", || {
                            positions = mesh
                                .positions
                                .iter()
                                .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
                                .collect();
                        })
                        .unwrap_or_default();
                        let mut smooth: Option<Vec<Vec3>> = None;
                        if mode == ViewMode::Smooth || mode == ViewMode::Shaded {
                            d_norm += timed_push(perf, "assemble.normals", || {
                                smooth = Some(shading::smooth_normals(&positions, &mesh.triangles));
                            })
                            .unwrap_or_default();
                        }
                        d_proj += timed_push(perf, "assemble.project+cull", || {
                            for t in &mesh.triangles {
                                let poly = tri_positions(&mesh.positions, t);
                                let normals = match &smooth {
                                    Some(sm) => {
                                        [sm[t[0] as usize], sm[t[1] as usize], sm[t[2] as usize]]
                                    }
                                    None => shading::flat_triangle_normals(&poly),
                                };
                                push_face(
                                    &mut face_ndc,
                                    &poly,
                                    &normals,
                                    color,
                                    &world_to_view,
                                    &vp,
                                );
                            }
                        })
                        .unwrap_or_default();
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
                        let mut smooth_corner: Vec<Vec3> = Vec::new();
                        if mode == ViewMode::Smooth || mode == ViewMode::Shaded {
                            d_norm += timed_push(perf, "assemble.normals", || {
                                let tris: Vec<[u32; 3]> =
                                    crate::gpu::shading::box_fan_triangles().to_vec();
                                smooth_corner =
                                    crate::gpu::shading::smooth_normals(corners.as_ref(), &tris);
                            })
                            .unwrap_or_default();
                        }
                        d_proj += timed_push(perf, "assemble.project+cull", || {
                            for (face, idxs) in quads.iter().zip(BOX_FACE_CORNERS.iter()) {
                                let face_normal = crate::gpu::shading::newell(face);
                                let normals: Vec<Vec3> = if smooth_corner.is_empty() {
                                    vec![face_normal; face.len()]
                                } else {
                                    idxs.iter()
                                        .map(|&i| {
                                            smooth_corner.get(i).copied().unwrap_or(face_normal)
                                        })
                                        .collect()
                                };
                                push_face(
                                    &mut face_ndc,
                                    face,
                                    &normals,
                                    color,
                                    &world_to_view,
                                    &vp,
                                );
                            }
                        })
                        .unwrap_or_default();
                    }
                }
            }
        }
        // Painter: farthest (largest view z) first, exactly like the CPU path.
        d_sort += timed_push(perf, "assemble.sort", || {
            face_ndc.sort_by(|a, b| {
                b.depth
                    .partial_cmp(&a.depth)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        })
        .unwrap_or_default();

        let mut faces: Vec<ShadedVertex> = Vec::with_capacity(face_ndc.len() * 3);
        d_expand += timed_push(perf, "assemble.expand", || {
            for face in &face_ndc {
                let idxs = fan_indices(face.world.len());
                for (tri, idx) in fan_triangles(&face.world).iter().zip(idxs.iter()) {
                    for (k, p) in tri.iter().enumerate() {
                        faces.push(ShadedVertex::new(*p, face.normal[idx[k]], face.color));
                    }
                }
            }
        })
        .unwrap_or_default();

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

        // Drop of the per-face intermediate list is real per-frame transient
        // garbage (1M `GpuFace`s, each with two heap-backed `Vec`s): moving
        // the deallocation here (which is what returning would trigger)
        // closes the assemble clock AFTER the cleanup, so the stage honestly
        // owns build + teardown (it shows up in `assemble.other`).
        drop(face_ndc);

        // Screen-constant triad arms (the CPU compositor's pixel-space arms,
        // converted to NDC for the passthrough stage).
        append_screen_axis_arms(&mut lines, scene, &vp, width, height);

        // Close the exact partition: the whole-assemble wall (incl. the
        // dropped intermediate) and the residual the five measured segments do
        // not own (so `assemble` == positions + normals + project+cull + sort
        // + expand + other, with no unmetered time left inside the function).
        if let Some(a0) = a_total {
            let total = a0.elapsed();
            let sub = d_pos + d_norm + d_proj + d_sort + d_expand;
            let other = if total > sub {
                total - sub
            } else {
                Duration::ZERO
            };
            if let Some(p) = perf.as_mut() {
                p.push("assemble", total);
                p.push("assemble.other", other);
            }
        }

        (faces, lines, edge_prims)
    }

    /// The number of feature-edge line primitives drawn by the LAST render
    /// (the boundary edge list exactly — mesh edges for meshes, the 12 box
    /// edges for box proxies). Only `Shaded` and `Edges` modes run the pass,
    /// so other modes report 0. Zeroed at the start of every render.
    pub fn last_edge_primitives(&self) -> usize {
        self.edge_primitives
    }

    /// The GPU path's vertex-preparation wall time for the LAST render:
    /// packing the face/line vertex bytes and creating the device buffers
    /// that carry them to the GPU (UI-D8 records this before/after).
    pub fn vertex_prep_time(&self) -> Duration {
        self.vertex_prep
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

/// Pack a `Mat4` into the 64-byte column-major `mat4x4<f32>` uniform layout
/// the vertex stage consumes: each of the 16 f64 entries of
/// `Mat4::to_array16()` narrowed to f32 in native order (no bytemuck — the
/// UI-D8 no-new-dependency rule). `Mat4` stores column-major
/// `entry (r, c) at c*4 + r`, exactly WGSL's `mat4x4<f32>` memory layout.
fn pack_matrix4(m: &Mat4) -> Vec<u8> {
    m.to_array16()
        .iter()
        .flat_map(|v| (*v as f32).to_ne_bytes())
        .collect()
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
    let mut world = Vec::with_capacity(poly.len());
    let mut normal = Vec::with_capacity(poly.len());
    for (i, p) in poly.iter().enumerate() {
        let view = world_to_view.transform_point(p);
        // The single projection path: raw clip via transform_point4, divide,
        // near/far and degenerate-w reject (`render::project_ndc`; D7's
        // correction — the combined matrix applied to the body-space point it
        // owns, `w = -view.z`). Used for culling exactly like the CPU path.
        let rst = crate::render::project_ndc(p, vp);
        if rst.is_none() {
            ok = false;
            break;
        }
        depth_sum += view.z;
        world.push([p.x, p.y, p.z]);
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
    if !ok || world.len() < 3 {
        return;
    }
    // The colour is stored UNLIT: the fragment shader applies the studio
    // key + fill lighting, so the same base colour reaches opaque and
    // transparent renders (the alpha was baked in the vertex colour by the
    // caller).
    faces.push(GpuFace {
        depth: depth_sum / poly.len() as f64,
        color,
        world,
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

/// Project one world point to `(ndc_x, ndc_y, ndc_z)` with the single
/// `transform_point4` formula (the crate's one clip-space path); `None` only
/// on a degenerate clip w (`w.abs() < 1e-9`, the D7 wire semantics). NDC-z is
/// NOT range-rejected here: wire endpoints are projected raw and the segment
/// is clipped to the visible cube by [`clip_ndc_line`], so wires crossing the
/// near/far plane keep their visible portion exactly as the D7 goldens.
fn to_ndc(p: &Vec3, vp: &Mat4) -> Option<[f64; 3]> {
    let h = vp.transform_point4(p);
    if h.w.abs() < 1e-9 {
        return None;
    }
    Some([h.x / h.w, h.y / h.w, h.z / h.w])
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

/// Fan a convex polygon (any component count — NDC `[f64;2]` for the
/// id-buffer pass, world `[f64;3]` for the shaded pass) into a triangle list.
fn fan_triangles<const N: usize>(pts: &[[f64; N]]) -> Vec<[[f64; N]; 3]> {
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

/// Project a world-space point to pixel coordinates (top-left origin); `None`
/// when behind the camera or outside the NDC z range. Delegates to the single
/// `render::project_ndc` path (D7 — world point; one formula in the crate).
fn to_px(world: Vec3, vp: &Mat4, width: u32, height: u32) -> Option<(f64, f64)> {
    let [nx, ny, _] = crate::render::project_ndc(&world, vp)?;
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
/// One bind group layout carries the MVP (view-projection) uniform for the
/// vertex stage (group 0, binding 0) and both pipelines share the resulting
/// pipeline layout — the shaded pass consumes it in `shading.wgsl`'s vertex
/// stage (multiply + divide), the line pass ignores it (screen-space
/// passthrough). The uniform buffer is created once and rewritten every
/// render via `queue.write_buffer`.
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

    // MVP uniform: 4x4 column-major f32 = 64 bytes, visible to the vertex
    // stage only (the transform lives there). Packed from
    // `Mat4::to_array16()` as f32 via `f32::to_ne_bytes` — no bytemuck (the
    // card's explicit no-new-dependency rule).
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("isolith-gpu-viewport-mvp-uniform"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("isolith-gpu-viewport-mvp-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(64),
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("isolith-gpu-viewport-mvp-bg"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform,
                offset: 0,
                size: wgpu::BufferSize::new(64),
            }),
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolith-gpu-viewport-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
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
        uniform,
        bind_group,
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
