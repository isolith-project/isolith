//! UI-D10 UI-RESIDENT-SCENE: the resident geometry cache, per-frame dirty
//! key and reused CPU scratch for the render seam (stages 1 and 2 of the
//! UI-D9 decision record).
//!
//! ## Why the cache is one merged stream, not per-mesh ranges
//!
//! The card's pixel oracle is the strongest gate: the 21 committed GPU
//! proof frames sha256-identical, so the GPU must receive EXACTLY the
//! byte-identical vertex stream the pre-card path drew: the renderer's one
//! triangle list in global painter-sorted order (every face sorted by mean
//! view depth, `assemble`). That order is camera-dependent, so the byte
//! stream changes whenever the sort changes. A per-primitive buffer layout
//! would fix each mesh's internal face order and change that sorted stream,
//! moving pixels on any scene whose faces interleave. Probe (committed as
//! `proof/UI-D10/probe-normal-bits.txt`): rotating world normals in the GPU
//! vertex stage instead of the CPU's per-frame view-space normals produces
//! 292/292 per-vertex f32 mismatches across the golden fixtures: the
//! `normals computed once at upload` transport the ADR sketches is therefore
//! REFUTED for this renderer by the byte-pixel requirement, and the proof
//! says so (I1/I5 outrank transport in the ADR's own invariant contract).
//!
//! So this cache keeps ONE device-resident face buffer and ONE line buffer
//! holding the byte-identical merged painter-sorted streams, created once and
//! written with `queue.write_buffer` only when the frame's packed bytes
//! actually change. Dirty tracking keys the whole frame on (scene generation,
//! camera pose, size, shading state): the scene is owned by `GpuBackend` and
//! immutable between `set_scene` calls, so a monotone scene generation is an
//! exact, allocation-free oracle for every `(prim.id, version)` dirty key the
//! card names (mesh bytes via `MeshData: PartialEq`, transform, highlighted,
//! base colour). Nothing re-uploads in a steady-state frame: only the 64-byte
//! MVP uniform moves and the resident ranges are referenced (drawn) as-is.

use wgpu::Queue;

use crate::camera::Camera;
use crate::scene::Aabb;
use oep_geo::math::{Mat4, Vec3};

/// Shaded-face vertex stride (world pos + view-space normal + RGBA = 40 B),
/// byte-for-byte the D8/D2 interleaved layout the shader consumes.
pub const SHADED_STRIDE: u64 = 40;
/// Line vertex stride (pre-projected NDC + colour = 28 B).
pub const LINE_STRIDE: u64 = 28;
/// The 64-byte MVP uniform the vertex stage owns (UI-D8).
pub const MVP_UNIFORM_BYTES: u64 = 64;

// ---------------------------------------------------------------------------
// The reused face arena (Stage 2: no per-frame heap churn)
// ---------------------------------------------------------------------------

/// One painter-sorted face in the arena. The world/normal data lives in the
/// flat scratch buffers; a face references its ranges so the expand step fans
/// exactly the same polygon the pre-card path fanned.
#[derive(Clone, Copy)]
pub struct FaceMeta {
    /// Mean view-space depth (`depth_sum / len`), the painter sort key.
    pub depth: f64,
    /// Flat base colour (alpha baked), byte-identical to the old per-face
    /// `GpuFace::color`.
    pub color: [u8; 4],
    /// Range into [`FaceArena::world`].
    pub world_start: usize,
    /// Number of polygon points in [`FaceArena::world`] starting at
    /// `world_start` (equal to the face's vertex count).
    pub world_len: usize,
    /// Range into [`FaceArena::normal`].
    pub normal_start: usize,
}

/// The reused CPU face arena: one allocation lifetime across full-path
/// frames. `clear()` keeps the backing store, so a camera orbit never
/// re-allocates the ~1M face descriptors nor the ~1M per-face vertex Veccies
/// the pre-D10 path freed-and-allocated every frame.
#[derive(Default)]
pub struct FaceArena {
    /// Painter metadata (sort key + colour + ranges).
    pub faces: Vec<FaceMeta>,
    /// Flattened world-space polygon points (f64 `[x, y, z]`).
    pub world: Vec<[f64; 3]>,
    /// Flattened view-space unit normals (f64), parallel to `world`.
    pub normal: Vec<[f64; 3]>,
}

impl FaceArena {
    /// Drop all faces but keep every allocation for the next frame.
    pub fn clear(&mut self) {
        self.faces.clear();
        self.world.clear();
        self.normal.clear();
    }
}

// ---------------------------------------------------------------------------
// The resident buffers + dirty key
// ---------------------------------------------------------------------------

/// The seam's GPU-resident buffers and frame dirty tracking.
pub struct ResidentScene {
    device: wgpu::Device,
    face: Option<wgpu::Buffer>,
    face_cap: u64,
    face_len: u64,
    line: Option<wgpu::Buffer>,
    line_cap: u64,
    line_len: u64,
    /// Signature of the packed streams currently resident; `None` until the
    /// first upload. When it equals the current frame's
    /// [`frame_key`], nothing in the resident buffers changed and the frame
    /// reuses them (uploads only the MVP uniform).
    resident_key: Option<u64>,
    /// Cumulative device-buffer allocations made by THIS seam since
    /// construction (the regression guard against `create_buffer_init`
    /// coming back per frame).
    pub allocations_total: u64,
    /// Upload bytes written by this seam in the last frame (MVP + faces +
    /// lines), reset by [`Self::start_frame`].
    upload_bytes: u64,
    /// Device-buffer allocations made in the last frame.
    buffer_allocs: u64,
    /// Draw calls recorded in the last frame.
    draw_calls: u64,
}

impl ResidentScene {
    /// A fresh cache over `device` (no buffers yet; created on first upload).
    pub fn new(device: wgpu::Device) -> Self {
        Self {
            device,
            face: None,
            face_cap: 0,
            face_len: 0,
            line: None,
            line_cap: 0,
            line_len: 0,
            resident_key: None,
            allocations_total: 0,
            upload_bytes: 0,
            buffer_allocs: 0,
            draw_calls: 0,
        }
    }

    /// Whether geometry was uploaded at least once (fast path precondition).
    pub fn is_resident(&self) -> bool {
        self.resident_key.is_some()
    }

    /// The signature of the streams currently resident.
    pub fn resident_key(&self) -> Option<u64> {
        self.resident_key
    }

    /// Record that the resident streams now match `key` (call after upload).
    pub fn mark_resident(&mut self, key: u64) {
        self.resident_key = Some(key);
    }

    /// Valid face bytes in the resident face buffer (0 draws nothing).
    pub fn face_len_bytes(&self) -> u64 {
        self.face_len
    }

    /// Valid line bytes in the resident line buffer.
    pub fn line_len_bytes(&self) -> u64 {
        self.line_len
    }

    /// Per-frame counter reset; call once at the top of `render_frame`.
    pub fn start_frame(&mut self) {
        self.upload_bytes = 0;
        self.buffer_allocs = 0;
        self.draw_calls = 0;
    }

    /// Upload bytes the seam wrote last frame (MVP + faces + lines).
    pub fn upload_bytes(&self) -> u64 {
        self.upload_bytes
    }

    /// Device-buffer allocations the seam made last frame.
    pub fn buffer_allocs(&self) -> u64 {
        self.buffer_allocs
    }

    /// Draw calls the seam recorded last frame.
    pub fn draw_calls(&self) -> u64 {
        self.draw_calls
    }

    /// Record the per-frame MVP uniform write (always 64 bytes).
    pub fn record_mvp(&mut self) {
        self.upload_bytes += MVP_UNIFORM_BYTES;
    }

    /// Record one draw call (face or line pass).
    pub fn record_draw(&mut self) {
        self.draw_calls += 1;
    }

    /// The resident face buffer (for `set_vertex_buffer`; `None` until the
    /// first face upload).
    pub fn face_buffer(&self) -> &wgpu::Buffer {
        self.face.as_ref().expect("resident face buffer exists")
    }

    /// The resident line buffer.
    pub fn line_buffer(&self) -> &wgpu::Buffer {
        self.line.as_ref().expect("resident line buffer exists")
    }

    /// Write the packed face stream into the resident face buffer, creating
    /// or growing it when needed. A device buffer is allocated only when no
    /// buffer exists or the stream outgrew it (both tracked by the counters,
    /// so a steady-state frame makes zero allocations).
    pub fn upload_faces(&mut self, queue: &Queue, bytes: &[u8]) {
        self.face_len = 0;
        if bytes.is_empty() {
            return;
        }
        let need = bytes.len() as u64;
        if self.face.as_ref().is_none() || self.face_cap < need {
            let size = need.max(self.face_cap.max(1));
            self.face = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("isolith-resident-faces"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.allocations_total += 1;
            self.buffer_allocs += 1;
            self.face_cap = size;
        }
        queue.write_buffer(self.face.as_ref().unwrap(), 0, bytes);
        self.face_len = need;
        self.upload_bytes += need;
    }

    /// Write the packed line stream into the resident line buffer (same
    /// create-once / grow-only discipline as the faces).
    pub fn upload_lines(&mut self, queue: &Queue, bytes: &[u8]) {
        self.line_len = 0;
        if bytes.is_empty() {
            return;
        }
        let need = bytes.len() as u64;
        if self.line.as_ref().is_none() || self.line_cap < need {
            let size = need.max(self.line_cap.max(1));
            self.line = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("isolith-resident-lines"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.allocations_total += 1;
            self.buffer_allocs += 1;
            self.line_cap = size;
        }
        queue.write_buffer(self.line.as_ref().unwrap(), 0, bytes);
        self.line_len = need;
        self.upload_bytes += need;
    }

    /// A debug-style label for counters in transcripts.
    pub fn state(&self) -> (u64, u64, u64, u64) {
        (
            self.upload_bytes,
            self.buffer_allocs,
            self.draw_calls,
            self.face_len,
        )
    }
}

// ---------------------------------------------------------------------------
// The per-frame dirty key
// ---------------------------------------------------------------------------

/// FNV-1a 64 over the frame's determinism inputs that would change the packed
/// vertex streams: the scene generation (a monotone counter `GpuBackend`
/// bumps on every `set_scene`; the scene is owned privately and immutable in
/// between, so the epoch is an exact oracle for every per-primitive
/// `(prim.id, version)` dirty key: mesh bytes via `MeshData: PartialEq`,
/// transform, highlighted and base colour), the camera pose (through both the
/// view and the view-projection matrices, so rotation AND projection changes
/// dirty the stream), the target size, and the shading state.
///
/// Equal key => the resident streams are byte-identical to what this frame
/// would pack, so the frame reuses them (MVP only). Any change => dirty,
/// full re-derive + `write_buffer`.
pub fn frame_key(
    scene_epoch: u64,
    camera: &Camera,
    width: u32,
    height: u32,
    mode: u8,
    alpha_bits: u32,
) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&scene_epoch.to_be_bytes());
    // The camera: every field that feeds view/projection.
    for v in [
        camera.target.x,
        camera.target.y,
        camera.target.z,
        camera.yaw,
        camera.pitch,
        camera.distance,
        camera.fov_y,
        camera.near,
        camera.far,
    ] {
        feed(&v.to_bits().to_be_bytes());
    }
    feed(&width.to_be_bytes());
    feed(&height.to_be_bytes());
    feed(&mode.to_be_bytes());
    feed(&alpha_bits.to_be_bytes());
    h
}

// ---------------------------------------------------------------------------
// Per-primitive world-AABB frustum cull (Stage 2)
// ---------------------------------------------------------------------------

/// Extract the six frustum half-space planes `a*x + b*y + c*z + d >= 0`
/// (inside) from a GL-style column-major view-projection matrix (the
/// Gribb–Hartmann extraction, `p_clip = M · p_world`, clip volume
/// `-w <= x,y,z <= w`).
///
/// `Mat4::to_array16` stores `entry(r, c)` at `c*4 + r`; row `r` is
/// `[m[r], m[4+r], m[8+r], m[12+r]]`.
fn frustum_planes(vp: &Mat4) -> [[f64; 4]; 6] {
    let m = vp.to_array16();
    let row = |r: usize| [m[r], m[4 + r], m[8 + r], m[12 + r]];
    let add = |a: [f64; 4], b: [f64; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
    let sub = |a: [f64; 4], b: [f64; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
    // left, right, bottom, top, near, far
    [
        add(row(3), row(0)),
        sub(row(3), row(0)),
        add(row(3), row(1)),
        sub(row(3), row(1)),
        add(row(3), row(2)),
        sub(row(3), row(2)),
    ]
}

/// Conservative frustum test for a world AABB: the prim may be visible when
/// its AABB intersects (or touches) the frustum volume: rejected ONLY when
/// the whole box lies strictly outside at least one plane (its most positive
/// corner along the plane normal is still outside). This is the OCCT
/// primitive-level BVH-cull essence; it never removes geometry that
/// `render::project_ndc` would have kept in the old path, because a face with
/// any surviving vertex has an AABB that intersects the frustum.
pub fn aabb_visible(world: &Aabb, vp: &Mat4) -> bool {
    let planes = frustum_planes(vp);
    let (mn, mx) = (world.min, world.max);
    for [a, b, c, d] in planes {
        // The corner of the box that maximises n·p.
        let px = if a >= 0.0 { mx.x } else { mn.x };
        let py = if b >= 0.0 { mx.y } else { mn.y };
        let pz = if c >= 0.0 { mx.z } else { mn.z };
        if a * px + b * py + c * pz + d <= 0.0 {
            return false; // whole box strictly outside this plane
        }
    }
    true
}

/// World-space triangle fan index pattern for a convex `n`-gon: one
/// `[0, i, i+1]` per fan triangle (identical order to the pre-D10
/// `fan_triangles`/`fan_indices` pair, inlined here without per-face
/// allocations).
#[inline]
pub fn fan_emit<F: FnMut(usize, usize, usize)>(n: usize, mut emit: F) {
    for i in 1..n.saturating_sub(1) {
        emit(0, i, i + 1);
    }
}

/// Convenience: a `Vec3` from a stored `[f64; 3]`.
#[inline]
pub fn v3(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}
