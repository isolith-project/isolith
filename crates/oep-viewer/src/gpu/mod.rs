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
//! app registered one ([`device::register_egui_device`]: the shared device
//! with egui; the lock keeps a single wgpu version, checked by
//! `cargo tree -p oep-app -i wgpu`). With no adapter, or `ISOLITH_GPU=off`,
//! the caller falls back to the deterministic CPU backend.
//!
//! # Projection model (the vertex stage owns the transform, UI-D8)
//!
//! The GPU backend now hands the shaded pass WORLD positions (plus view-space
//! normals and the flattened base colour) and the vertex stage owns the
//! transform: an MVP uniform (`mat4x4<f32>`, column-major, packed from
//! `Mat4::to_array16()` with `f32::to_ne_bytes`, no bytemuck) is applied as
//! `clip = mvp · (world, 1)`, then the vertex stage does the perspective
//! divide and outputs `w = 1` so the rasterizer's own divide is a no-op and
//! attribute interpolation stays linear in screen space: byte-identical to
//! the D1/D7 pre-projected behaviour. The CPU still runs the SAME projection
//! (`render::project_ndc`, the crate's single NDC path) purely to cull faces
//! and order the painter; the pixels come from the GPU's vertex stage.
//!
//! The LINE pass (grid, feature edges, the screen-constant triad arms) remains
//! a passthrough for pre-projected NDC: the triad arms are pixel-space by
//! construction (they come from `to_px`/`screen_ndc`, exactly the
//! immediate-mode egui model), so there is no world-space form to transform.
//! The store of the projection formula is `render::project_ndc`: one helper
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
//! not rasterize the axis tip letters X/Y/Z: the arms are drawn, the letters
//! are a wave-D2 (UI-GPU-SHADING) concern.
//!
//! Submodules: [`device`] (adapter/device acquisition, shared with egui),
//! [`target`] (offscreen colour target + readback) and [`idbuffer`] (the id
//! pass: "what is actually drawn" rasterized into a per-pixel primitive id
//! grid, card UI-D3).

pub mod device;
pub mod idbuffer;
pub mod resident;
pub mod shading;
pub mod target;

use std::time::{Duration, Instant};

use oep_geo::math::{Mat4, Vec3};

use crate::backend::frame_hash;
use crate::camera::Camera;
use crate::render::RgbaImage;
use crate::scene::{Primitive, Scene};
use crate::{overlay, theme};

use device::GpuContext;
use resident::{FaceArena, ResidentScene};
use shading::{ViewMode, BOX_EDGES};
use target::{GpuTarget, DEPTH_FORMAT, TARGET_FORMAT};

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
/// is written as the source alpha (`1 * src.a + 0 * dst.a`: REPLACE), so
/// the transparency control is measurable at readback: a 50% body reads back
/// alpha ≈ 128 instead of 255: the number the UI-D2 acceptance prints
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

/// GPU-backend pipeline set. The shaded pipeline renders filled faces through
/// the studio key+fill lighting shader (`shading.wgsl`) whose vertex stage
/// applies the MVP from the bind group (`binding 0` uniform, column-major f32,
/// packed from `Mat4::to_array16()`) and does the perspective divide; the
/// line pipeline draws the grid, feature edges and triad arms (~passthrough
/// colour, pre-projected NDC: the screen-constant arms are pixel-space by
/// construction so the overlay pass stays a passthrough).
struct GpuPipeline {
    /// MVP (view-projection) uniform buffer: 64 bytes = `mat4x4<f32>`
    /// column-major, rewritten every render via `queue.write_buffer`.
    uniform: wgpu::Buffer,
    /// Bind group for the MVP (group 0, binding 0), shared by both pipelines.
    bind_group: wgpu::BindGroup,
    /// Studio-lit triangle pipeline (shading.wgsl, alpha blended, reverse-Z
    /// depth write ON): the opaque pass, drawn in any order because the depth
    /// buffer resolves per-pixel nearest (UI-D11).
    shading: wgpu::RenderPipeline,
    /// The transparent set's pipeline: same shader and blend, depth test ON
    /// but depth write OFF, so it composites source-over after the opaque pass
    /// without perturbing the resolved depth (UI-D11).
    transparent: wgpu::RenderPipeline,
    /// Line pipeline for grid/edges/triad (viewport.wgsl, replace blend,
    /// depth test Always / write off so the overlay stays on top exactly like
    /// the pre-depth pipeline).
    lines: wgpu::RenderPipeline,
    target: GpuTarget,
}

/// UI-D9 per-stage frame cost instrumentation (env-gated, inert when off).
///
/// Active ONLY when `ISOLITH_PERF_STAGES=1` (read once at construction).
/// When unset, `GpuBackend::perf` is `None` and the render path performs zero
/// timing calls; no `Instant::now()` in the seam, so the unset binary is
/// behaviour- and hash-identical to the pre-card tree (the fast smoke proves
/// the GPU frame hash unchanged with and without the variable, UI-D10 re-proves
/// this on its committed tree). When set, `render_frame` records a partition of
/// the driving wall clock into named stages and prints one machine-parseable
/// line per frame to stderr, with each stage's duration against the frame total
/// so the sum can be checked against the measured p95. Consumed by
/// `scripts/ui/perf-profile.sh`. UI-D10 keeps the exact stage names D9
/// committed; a resident fast path reports `assemble=0` and `vertex-prep=0`
/// explicitly so the seam partition stays a true partition of the frame.
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
    /// The resident GPU buffers + per-frame dirty tracking (UI-D10).
    cache: ResidentScene,
    /// UI-D9 env-gated per-stage frame cost instrumentation; `None` unless
    /// `ISOLITH_PERF_STAGES=1` (then the render path records and prints a
    /// per-frame stage partition, inert and zero-timing when unset).
    perf: Option<PerfStages>,
    /// Scene generation counter: bumped on every `set_scene`. The scene is
    /// owned by this backend and immutable between `set_scene` calls, so the
    /// epoch is the exact dirty oracle for every primitive's mesh bytes,
    /// transform, highlighted flag and colour (see `resident::frame_key`).
    session_epoch: u64,
    // ---- reused CPU scratch (Stage 2: nothing allocated per frame) ----
    /// Painter face arena (metadata + flat world/normal scratch).
    arena: FaceArena,
    /// Reused packed face-vertex byte stream.
    face_bytes: Vec<u8>,
    /// Reused line vertex list (feature edges, grid, triad).
    lines: Vec<Vertex>,
    /// Reused packed line-vertex byte stream.
    line_bytes: Vec<u8>,
    /// Reused per-primitive visibility mask from the world-AABB frustum cull.
    visible: Vec<bool>,
    /// Reused f64 mesh position scratch (avoids a fresh `Vec<Vec3>` per mesh
    /// per frame).
    scratch_positions: Vec<Vec3>,
    // ---- seam instrumentation counters exposed to the acceptance harness ----
    /// Upload bytes written in the last render (MVP + geometry + lines):
    /// the pre-D10 per-frame total was 120,303,360 B; steady state is 64.
    last_upload_bytes: u64,
    /// Device-buffer allocations made in the last render.
    last_buffer_allocs: u64,
    /// Draw calls recorded in the last render.
    last_draw_calls: u64,
    /// Wall time of the last render's assemble (0 on a resident fast path).
    last_assemble: Duration,
    /// Faces submitted by the last full-path assemble (0 on a fast path).
    last_faces_submitted: usize,
    /// Opaque byte prefix of the last full-path packed face stream: the
    /// opaque pass draws [0, split), the transparent set [split, face_len).
    /// Set by `assemble`, reused verbatim on a resident fast path (same key =>
    /// same bytes => same split).
    opaque_face_bytes: u64,
    /// Total triangles of the scene the last assemble saw.
    last_scene_triangles: usize,
    /// Primitives the world-AABB cull removed in the last assemble.
    last_culled_prims: usize,
}

impl GpuBackend {
    /// Wrap `scene` with a camera framed to its bounds (or `Camera::default`
    /// for an empty scene) on the given device context, and render once at
    /// the renderer's default size.
    ///
    /// Returns `None` only when the caller should not use the GPU path: the
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
        let cache = ResidentScene::new(ctx.device.clone());
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
            cache,
            perf: PerfStages::enabled(),
            session_epoch: 0,
            arena: FaceArena::default(),
            face_bytes: Vec::new(),
            lines: Vec::new(),
            line_bytes: Vec::new(),
            visible: Vec::new(),
            scratch_positions: Vec::new(),
            last_upload_bytes: 0,
            last_buffer_allocs: 0,
            last_draw_calls: 0,
            last_assemble: Duration::ZERO,
            last_faces_submitted: 0,
            opaque_face_bytes: 0,
            last_scene_triangles: 0,
            last_culled_prims: 0,
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

        // The MVP the vertex stage applies to the WORLD positions: the same
        // view-projection the CPU culling uses (`render::project_ndc`), packed
        // column-major f32: one transform, one place, shared with the CPU.
        let aspect = f64::from(self.width) / f64::from(self.height);
        let vp = self.camera.view_projection(aspect);

        // The per-frame dirty key: (scene generation, camera, size, shading).
        // An unchanged key means the resident streams are byte-identical to
        // what this frame would assemble, so the frame reuses them and moves
        // only the 64-byte MVP uniform: no assemble, no packing, no upload,
        // no buffer allocation, and the SAME single digit draw calls.
        let key = resident::frame_key(
            self.session_epoch,
            &self.camera,
            self.width,
            self.height,
            self.shading.mode as u8,
            self.shading.alpha.to_bits(),
        );

        self.cache.start_frame();
        self.last_buffer_allocs = 0;
        let fast = self.cache.is_resident() && self.cache.resident_key() == Some(key);

        if !fast {
            // Full path: derive the painter-sorted face/line vertex streams
            // for this (scene, camera, shading) into the reused arena, pack
            // them into the reused byte buffers, and re-upload only what
            // actually changed, through the resident buffers (created once).
            // `assemble` reports its own total + sub-split (incl. an exact
            // residual), so the stage partition is self-consistent here.
            let (faces, tris, culled, edge_prims, assemble_t) = self.assemble();
            self.edge_primitives = edge_prims;
            self.last_assemble = assemble_t;
            self.last_faces_submitted = faces;
            self.last_scene_triangles = tris;
            self.last_culled_prims = culled;

            // Vertex preparation: re-upload only the dirty streams through the
            // resident buffers (write_buffer; no device-buffer allocation):
            // the UI-D8 vertex-prep bucket, env-gated to the D9 stage table.
            let prep = timed_push(&mut self.perf, "vertex-prep", || {
                self.cache
                    .upload_faces(&self.ctx.queue, &self.face_bytes, self.opaque_face_bytes);
                self.cache.upload_lines(&self.ctx.queue, &self.line_bytes);
            })
            .unwrap_or_default();
            self.vertex_prep = prep;
            self.cache.mark_resident(key);
        } else {
            // Steady-state / repeat view: nothing in the resident streams
            // changed, so only the MVP uniform is (re)written this frame and
            // the resident ranges are drawn as-is. `edge_primitives` keeps
            // its value from the last full path (same inputs => same count).
            // The D9 stage table records these two seam stages as 0 so the
            // partition stays a true partition of the frame wall clock.
            self.vertex_prep = Duration::ZERO;
            self.last_assemble = Duration::ZERO;
            if let Some(p) = &mut self.perf {
                p.push("assemble", Duration::ZERO);
                p.push("vertex-prep", Duration::ZERO);
            }
        }

        let device = &self.ctx.device;
        let queue = &self.ctx.queue;
        queue.write_buffer(&self.pipeline.uniform, 0, &pack_matrix4(&vp));
        self.cache.record_mvp();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("isolith-gpu-viewport-pass"),
            color_attachments: &[Some(self.pipeline.target.color_attachment(CLEAR))],
            depth_stencil_attachment: Some(self.pipeline.target.depth_attachment()),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.pipeline.bind_group, &[]);

        // 1. Opaque faces: the byte range [0, opaque_len). Depth test on and
        //    depth write on; the reverse-Z buffer resolves nearest per pixel,
        //    so the CPU painter sort is gone and draw order is irrelevant.
        //    (Skip in `Edges` mode, which draws the boundary edge list only.)
        let face_len = self.cache.face_len_bytes();
        let opaque_len = self.cache.face_opaque_bytes().min(face_len);
        if opaque_len > 0 {
            pass.set_pipeline(&self.pipeline.shading);
            pass.set_vertex_buffer(0, self.cache.face_buffer().slice(..));
            self.cache.record_draw();
            pass.draw(
                0..(opaque_len as u32) / resident::SHADED_STRIDE as u32,
                0..1,
            );
        }

        // 2. Transparent set: drawn after the opaque pass, depth test on but
        //    depth write off, so it composites source-over over the resolved
        //    opaque depth without perturbing it.
        let transparent_len = face_len - opaque_len;
        if transparent_len > 0 {
            pass.set_pipeline(&self.pipeline.transparent);
            pass.set_vertex_buffer(0, self.cache.face_buffer().slice(..));
            self.cache.record_draw();
            pass.draw(
                opaque_len as u32 / resident::SHADED_STRIDE as u32
                    ..face_len as u32 / resident::SHADED_STRIDE as u32,
                0..1,
            );
        }

        // 3. Lines (feature edges, grid, then the triad arms). The overlay
        //    keeps depth compare Always + write off, so it stays on top
        //    exactly like the pre-depth pass.
        let line_len = self.cache.line_len_bytes();
        if line_len > 0 {
            pass.set_pipeline(&self.pipeline.lines);
            pass.set_vertex_buffer(0, self.cache.line_buffer().slice(..));
            self.cache.record_draw();
            pass.draw(0..(line_len as u32) / resident::LINE_STRIDE as u32, 0..1);
        }
        drop(pass);

        let s0 = Instant::now();
        queue.submit([encoder.finish()]);
        if let Some(p) = &mut self.perf {
            p.push("submit", s0.elapsed());
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
        self.last_upload_bytes = self.cache.upload_bytes();
        self.last_buffer_allocs = self.cache.buffer_allocs();
        self.last_draw_calls = self.cache.draw_calls();
    }

    /// Project the scene into the reused painter-sorted vertex scratch for one
    /// frame, honouring the current shading state (view mode + alpha).
    ///
    /// Per-primitive world-AABB frustum culling runs first; only survivors are
    /// projected / sorted / packed, into the reused arena and byte buffers
    /// (Stage 2: the CPU touches survivors only, and nothing is heap-allocated
    /// per face). The packed stream is byte-identical to the pre-D10
    /// `pack_shaded` stream: the same projection (`render::project_ndc`), the
    /// same global painter order, the same vertices and colours.
    ///
    /// Returns `(faces_submitted, scene_triangle_total, primitives_culled,
    /// edge_primitives_drawn, wall_time)`. On a resident fast-path frame this
    /// method is not called and the previous totals stand.
    fn assemble(&mut self) -> (usize, usize, usize, usize, Duration) {
        let t0 = Instant::now();
        let scene = &self.scene;
        let camera = self.camera; // `Camera` is Copy; the scene stays borrowed
        let width = self.width;
        let height = self.height;
        let aspect = f64::from(width) / f64::from(height);
        let world_to_view = camera.view_matrix();
        let vp = camera.view_projection(aspect);
        let alpha = self.shading.alpha.clamp(0.0, 1.0);
        let mode = self.shading.mode;

        // UI-D9 exact partition of `assemble`: the total wall time, the timed
        // segments below, and a reported `assemble.other` residual so the
        // stage table is a true partition. The primitive AABB cull and the
        // line/grid/triad assembly land in `other` (as D9's line assembly
        // did); everything else is a named sub-stage with D9's exact names.
        let perf = &mut self.perf;
        let a_total = perf.is_some().then(Instant::now);
        let mut d_pos = Duration::ZERO;
        let mut d_norm = Duration::ZERO;
        let mut d_proj = Duration::ZERO;
        let mut d_sort = Duration::ZERO;
        let mut d_expand = Duration::ZERO;

        // Reuse every scratch allocation from the previous full path.
        self.arena.clear();
        self.face_bytes.clear();
        self.lines.clear();
        self.line_bytes.clear();
        self.visible.clear();
        self.visible.resize(scene.primitives.len(), true);

        // Per-primitive world-AABB frustum cull: the CPU only touches
        // survivors (the OCCT primitive-level BVH-cull essence, ADOPTED in the
        // UI-D9 ADR §4 Option A).
        let mut culled = 0usize;
        let mut tris_total = 0usize;
        for (i, prim) in scene.primitives.iter().enumerate() {
            tris_total += scene_triangle_count(prim);
            let vis = resident::aabb_visible(&prim.world_aabb(), &vp);
            self.visible[i] = vis;
            if !vis {
                culled += 1;
            }
        }

        if mode != ViewMode::Edges {
            for (i, prim) in scene.primitives.iter().enumerate() {
                if !self.visible[i] {
                    continue;
                }
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
                        // Reused f64 position scratch (no fresh Vec per mesh);
                        // `assemble.positions` and `assemble.normals` carry
                        // D9's names so the profile partition is preserved.
                        let mut smooth: Option<Vec<Vec3>> = None;
                        if mode == ViewMode::Smooth || mode == ViewMode::Shaded {
                            d_pos += timed_push(perf, "assemble.positions", || {
                                self.scratch_positions.clear();
                                self.scratch_positions.extend(
                                    mesh.positions
                                        .iter()
                                        .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64)),
                                );
                            })
                            .unwrap_or_default();
                            d_norm += timed_push(perf, "assemble.normals", || {
                                smooth = Some(shading::smooth_normals(
                                    &self.scratch_positions,
                                    &mesh.triangles,
                                ));
                            })
                            .unwrap_or_default();
                        }
                        d_proj += timed_push(perf, "assemble.project+cull", || {
                            for t in &mesh.triangles {
                                let mut poly: [Vec3; 3] = [Vec3::new(0.0, 0.0, 0.0); 3];
                                let mut normals: [Vec3; 3] = [Vec3::new(0.0, 0.0, 1.0); 3];
                                let mut missing = false;
                                for k in 0..3 {
                                    let Some(p) = mesh.positions.get(t[k] as usize) else {
                                        missing = true;
                                        break;
                                    };
                                    poly[k] = Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
                                    normals[k] = match &smooth {
                                        Some(sm) => sm[t[k] as usize],
                                        None => Vec3::new(0.0, 0.0, 1.0),
                                    };
                                }
                                if missing {
                                    continue; // identical to the old skip-partial-face path
                                }
                                if smooth.is_none() {
                                    normals = shading::flat_triangle_normals(&poly);
                                }
                                arena_push(
                                    &mut self.arena,
                                    i,
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
                                let mut normals = [face_normal; 4];
                                if !smooth_corner.is_empty() {
                                    for (k, &i) in idxs.iter().enumerate() {
                                        normals[k] =
                                            smooth_corner.get(i).copied().unwrap_or(face_normal);
                                    }
                                }
                                arena_push(
                                    &mut self.arena,
                                    i,
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
        let faces_submitted = self.arena.faces.len();

        // UI-D11: per-pixel hidden-surface resolution replaces the CPU painter
        // sort. The packed face stream is [opaque set][transparent set]:
        //  - Opaque faces (vertex alpha == 255, i.e. the whole scene when the
        //    global opacity slider is at 1.0) stay in assembly order, ordering
        //    dropped, and the reverse-Z depth buffer resolves the nearest
        //    surface per pixel.
        //  - Transparent faces (alpha < 255, i.e. the whole scene below 1.0)
        //    are packed after the opaque set, sorted back-to-front at ENTITY
        //    level (per primitive), and draw after the opaque pass with depth
        //    test on / write off so the source-over composite is depth-correct.
        // `assemble.sort` now measures the transparent entity sort only (0 for
        // an all-opaque frame, where ordering truly survives nowhere).
        let mut opaque_face_bytes = 0u64;
        d_sort += timed_push(perf, "assemble.sort", || {
            let has_transparent = self.arena.faces.iter().any(|f| f.color[3] != 255);
            if has_transparent {
                // Entity-level depth: the mean view z of each primitive's
                // faces (contiguous in arena order, so a stable per-entity
                // sort keeps one entity's faces together).
                let mut ent_sum = vec![0.0f64; scene.primitives.len()];
                let mut ent_cnt = vec![0usize; scene.primitives.len()];
                for f in &self.arena.faces {
                    ent_sum[f.entity] += f.depth;
                    ent_cnt[f.entity] += 1;
                }
                let ent_depth: Vec<f64> = ent_sum
                    .iter()
                    .zip(&ent_cnt)
                    .map(|(s, c)| if *c > 0 { s / *c as f64 } else { 0.0 })
                    .collect();
                // Stable: opaque first in assembly order, then transparent
                // entities back-to-front (farthest first -> largest depth
                // first, the painter's convention), ties by entity index.
                self.arena.faces.sort_by(|a, b| {
                    let ao = a.color[3] == 255u8;
                    let bo = b.color[3] == 255u8;
                    match (ao, bo) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        (true, true) => std::cmp::Ordering::Equal,
                        (false, false) => ent_depth[b.entity]
                            .partial_cmp(&ent_depth[a.entity])
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then(a.entity.cmp(&b.entity)),
                    }
                });
            }
        })
        .unwrap_or_default();

        // Expand into the byte stream in the exact pre-D10 layout (fan
        // `[0, i, i+1]`, world point + view normal + colour), recording the
        // byte offset where the transparent set begins.
        d_expand += timed_push(perf, "assemble.expand", || {
            for meta in &self.arena.faces {
                let before = self.face_bytes.len();
                let is_opaque = meta.color[3] == 255;
                resident::fan_emit(meta.world_len, |a, b, c| {
                    for k in [a, b, c] {
                        let p = self.arena.world[meta.world_start + k];
                        let n = self.arena.normal[meta.normal_start + k];
                        ShadedVertex::new([p[0], p[1], p[2]], [n[0], n[1], n[2]], meta.color)
                            .pack_into(&mut self.face_bytes);
                    }
                });
                if is_opaque {
                    opaque_face_bytes += (self.face_bytes.len() - before) as u64;
                }
            }
        })
        .unwrap_or_default();
        self.opaque_face_bytes = opaque_face_bytes;

        // The edge pass draws EXACTLY the boundary edge list: a mesh
        // primitive draws its committed `mesh.edges`; a box proxy draws its
        // 12 box edges (the proxy's boundary edge list). The count of line
        // primitives the pass submits is recorded (`edge_primitives`) so the
        // acceptance can compare drawn vs fixture boundary edge count. Edges
        // of culled primitives are skipped (their wire projects fully outside
        // the frustum, so the rasterised pixels are identical either way).
        let mut edge_prims = 0usize;
        if mode == ViewMode::Shaded || mode == ViewMode::Edges {
            for (i, prim) in scene.primitives.iter().enumerate() {
                if !self.visible[i] {
                    continue;
                }
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
                            let before = self.lines.len();
                            push_wire(
                                &mut self.lines,
                                &Vec3::new(pa[0] as f64, pa[1] as f64, pa[2] as f64),
                                &Vec3::new(pb[0] as f64, pb[1] as f64, pb[2] as f64),
                                &vp,
                                color,
                            );
                            if self.lines.len() > before {
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
                            let before = self.lines.len();
                            push_wire(&mut self.lines, pa, pb, &vp, color);
                            if self.lines.len() > before {
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
            push_wire(&mut self.lines, &seg[0], &seg[1], &vp, grid_color());
        }

        // Screen-constant triad arms (the CPU compositor's pixel-space arms,
        // converted to NDC for the passthrough stage).
        append_screen_axis_arms(&mut self.lines, scene, &vp, width, height);

        // Pack the line vertices into the reused byte buffer.
        for v in &self.lines {
            v.pack_into(&mut self.line_bytes);
        }

        // Close the exact partition (same as UI-D9): the whole-assemble wall
        // time (incl. the primitive cull and the line/grid/triad assembly,
        // both unmetered) and the residual the five measured segments do not
        // own, so `assemble == positions + normals + project+cull + sort +
        // expand + other` with no unmetered time left inside the function.
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

        (
            faces_submitted,
            tris_total,
            culled,
            edge_prims,
            t0.elapsed(),
        )
    }

    /// The number of feature-edge line primitives drawn by the LAST render
    /// (the boundary edge list exactly: mesh edges for meshes, the 12 box
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

    /// Replace the scene (a mesh edit, highlight flip or re-tessellation).
    /// Any primitive change funnels through here: the scene is owned by this
    /// backend and immutable between calls, so bumping the scene generation
    /// makes the resident dirty key (UI-D10) re-derive and re-upload the
    /// changed streams on the next render.
    pub fn set_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.session_epoch = self.session_epoch.wrapping_add(1);
        self.render_frame();
    }

    /// Upload bytes written by the seam in the LAST render: the 64-byte MVP
    /// uniform plus any genuinely dirty geometry/line bytes (a fast-path
    /// frame moves exactly 64). Pre-D10 this was ~120,303,360 B every frame.
    pub fn last_upload_bytes(&self) -> u64 {
        self.last_upload_bytes
    }

    /// Device-buffer allocations made by the seam in the LAST render
    /// (steady state: 0; the resident buffers were created once before).
    pub fn last_buffer_allocs(&self) -> u64 {
        self.last_buffer_allocs
    }

    /// Cumulative device-buffer allocations the seam has made since
    /// construction (the regression guard against `create_buffer_init`
    /// coming back; must stay constant across the measured frames).
    pub fn total_buffer_allocs(&self) -> u64 {
        self.cache.allocations_total
    }

    /// Draw calls submitted in the LAST render (single-digit target).
    pub fn last_draw_calls(&self) -> u64 {
        self.last_draw_calls
    }

    /// Wall time of the LAST render's assemble (0 on a resident fast path,
    /// where no assembly ran).
    pub fn last_assemble_time(&self) -> Duration {
        self.last_assemble
    }

    /// Faces submitted by the LAST full-path assemble (0 on a fast path).
    pub fn last_faces_submitted(&self) -> usize {
        self.last_faces_submitted
    }

    /// Total triangles of the scene seen by the LAST full-path assemble.
    pub fn last_scene_triangles(&self) -> usize {
        self.last_scene_triangles
    }

    /// Primitives the world-AABB frustum cull removed in the LAST full-path
    /// assemble (0 on a fast path).
    pub fn last_culled_prims(&self) -> usize {
        self.last_culled_prims
    }

    /// Number of primitives in the current scene.
    pub fn scene_primitive_count(&self) -> usize {
        self.scene.primitives.len()
    }

    /// Whether the last render took the resident fast path (nothing dirty:
    /// only the MVP uniform uploaded, the resident ranges drawn as-is).
    pub fn last_fast_path(&self) -> bool {
        self.last_assemble == Duration::ZERO && self.cache.is_resident()
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

/// Pack a `Mat4` into the 64-byte column-major `mat4x4<f32>` uniform layout
/// the vertex stage consumes: each of the 16 f64 entries of
/// `Mat4::to_array16()` narrowed to f32 in native order (no bytemuck; the
/// UI-D8 no-new-dependency rule). `Mat4` stores column-major
/// `entry (r, c) at c*4 + r`, exactly WGSL's `mat4x4<f32>` memory layout.
fn pack_matrix4(m: &Mat4) -> Vec<u8> {
    m.to_array16()
        .iter()
        .flat_map(|v| (*v as f32).to_ne_bytes())
        .collect()
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

/// Push one world polygon through NDC culling into the reused face arena,
/// storing the per-vertex VIEW-SPACE normals and the base (unlit) colour for
/// the studio fragment shader (the UI-D10 arena form of `push_face`: the
/// old per-face `Vec` pair is replaced by ranges into the arena's flat
/// scratch buffers, so full-path frames allocate no per-face storage). The
/// projection, cull and normal math is byte-identical to the pre-D10 path.
///
/// `normals` must be unit world-space normals (one per polygon vertex); they
/// are rotated into view space here because the light rig is defined in view
/// space (UI-D10 keeps this on the CPU: moving it to the vertex stage changes
/// the per-vertex f32 normal bits: proven, see `resident` module docs).
#[allow(clippy::too_many_arguments)]
fn arena_push(
    arena: &mut resident::FaceArena,
    entity: usize,
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
    let world_start = arena.world.len();
    let normal_start = arena.normal.len();
    for (i, p) in poly.iter().enumerate() {
        let view = world_to_view.transform_point(p);
        // The single projection path: raw clip via transform_point4, divide,
        // near/far and degenerate-w reject (`render::project_ndc`; D7's
        // correction: the combined matrix applied to the body-space point it
        // owns, `w = -view.z`). Used for culling exactly like the CPU path.
        let rst = crate::render::project_ndc(p, vp);
        if rst.is_none() {
            ok = false;
            break;
        }
        depth_sum += view.z;
        arena.world.push([p.x, p.y, p.z]);
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
        arena.normal.push([vn.x, vn.y, vn.z]);
    }
    let world_len = arena.world.len() - world_start;
    if !ok || world_len < 3 {
        arena.world.truncate(world_start);
        arena.normal.truncate(normal_start);
        return;
    }
    // The colour is stored UNLIT: the fragment shader applies the studio
    // key + fill lighting, so the same base colour reaches opaque and
    // transparent renders (the alpha was baked in the vertex colour by the
    // caller).
    arena.faces.push(resident::FaceMeta {
        depth: depth_sum / poly.len() as f64,
        entity,
        color,
        world_start,
        world_len,
        normal_start,
    });
}

/// The number of triangles a primitive contributes: its committed mesh
/// triangles, or the 12 fan triangles of a box proxy's 6 quads.
fn scene_triangle_count(prim: &Primitive) -> usize {
    match &prim.mesh {
        Some(mesh) => mesh.triangles.len(),
        None => 12,
    }
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

/// Fan a convex polygon (any component count: NDC `[f64;2]` for the
/// id-buffer pass, world `[f64;3]` for the shaded pass) into a triangle list.
fn fan_triangles<const N: usize>(pts: &[[f64; N]]) -> Vec<[[f64; N]; 3]> {
    if pts.len() < 3 {
        return Vec::new();
    }
    (1..pts.len() - 1)
        .map(|i| [pts[0], pts[i], pts[i + 1]])
        .collect()
}

/// Project a world-space point to pixel coordinates (top-left origin); `None`
/// when behind the camera or outside the NDC z range. Delegates to the single
/// `render::project_ndc` path (D7: world point; one formula in the crate).
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
/// pipeline layout: the shaded pass consumes it in `shading.wgsl`'s vertex
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
    // `Mat4::to_array16()` as f32 via `f32::to_ne_bytes`: no bytemuck (the
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
                depth: Option<wgpu::DepthStencilState>,
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
            depth_stencil: depth,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };

    // Reverse-Z depth state shared by every pass pipeline (D32Float).
    // Reverse-Z + the shading shader's negated NDC z: near plane -> clip z
    // +1 -> depth 1.0, far plane -> clip z -1 -> depth 0.0. Clear 0.0 (far);
    // `GreaterEqual` keeps the nearer (larger) fragment per pixel, and on an
    // exact-equal tie (coplanar coincident faces) the later-drawn fragment
    // wins exactly as the pre-depth painter let the last draw overwrite,
    // and identically to the id pass (which uses the same GreaterEqual), so
    // the topmost id always equals the topmost colour fragment (I1).
    let depth = |depth_write: bool| wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(depth_write),
        depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
        stencil: Default::default(),
        bias: Default::default(),
    };
    // The line overlay keeps depth write off and a constant-pass compare, so
    // it stays on top of the depth-resolved faces exactly like the pre-depth
    // pipeline (whose geometry had no depth and always replaced).
    let lines_depth = wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: Default::default(),
        bias: Default::default(),
    };

    let shading = make(
        wgpu::PrimitiveTopology::TriangleList,
        &shading_shader,
        "vs_main",
        "fs_main",
        SHADED_VERTEX_LAYOUT,
        Some(STUDIO_BLEND),
        Some(depth(true)),
        "isolith-gpu-viewport-shading-pipeline",
    );
    let transparent = make(
        wgpu::PrimitiveTopology::TriangleList,
        &shading_shader,
        "vs_main",
        "fs_main",
        SHADED_VERTEX_LAYOUT,
        Some(STUDIO_BLEND),
        Some(depth(false)),
        "isolith-gpu-viewport-transparent-pipeline",
    );
    let lines = make(
        wgpu::PrimitiveTopology::LineList,
        &line_shader,
        "vs_main",
        "fs_color",
        VERTEX_LAYOUT,
        Some(wgpu::BlendState::REPLACE),
        Some(lines_depth),
        "isolith-gpu-viewport-lines-pipeline",
    );

    Some(GpuPipeline {
        uniform,
        bind_group,
        shading,
        transparent,
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
