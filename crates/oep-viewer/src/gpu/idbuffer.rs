//! Id-buffer picking (editor parity wave D, card UI-D3 UI-PICK).
//!
//! Selection comes from what is actually drawn: this module rasterizes the
//! SAME faces the viewport colour pass draws (same projection, same painter
//! order) into a second offscreen target in which every pixel carries the
//! index of the primitive whose face covers it (`0` = background / nothing),
//! reads that target back into a host grid once per view change, and answers
//! two queries from the grid: [`IdPass::pick_at`] (the primitive under a
//! pixel, the hover/click pick) and [`IdPass::pick_rect`] (the exact set of
//! primitives enclosed by a pixel rect, the box-select query).
//!
//! # What "matches the drawn frame" means
//!
//! The id pass reproduces the colour pass's face SET exactly: it walks the
//! same `scene.primitives` and pushes the same polygons (`box_faces` for box
//! proxies, `mesh.triangles` for meshed primitives), in the same painter
//! order. The one deliberate exception is the PROJECTION: this pass projects
//! with the standard perspective divide (`nx = clip.x / -view.z`, the
//! convention `oep_viewer::pick` unprojects), rather than the colour pass's
//! pre-existing D1 `clip_w = row4(vp)·view` quirk. The card's acceptance
//! contract is that the id-buffer pick EQUALS the analytic ray pick
//! `oep_viewer::pick` for the same camera, and only the standard divide
//! satisfies that; the D1 quirk (which makes the colour pixels drift from the
//! analytic projection off the optical axis) is a documented, D2-owned
//! "correct + regolden" item, so this id surface is the analytic-truth
//! picking surface and coincides exactly with the drawn frame once D2 fixes
//! the colour projection. The grid on the host is therefore "the id the
//! analytic pick reports at each pixel".
//!
//! # The readback cost
//!
//! The grid is re-read from the GPU only when the view (camera / size / scene)
//! changes; a hover sweep re-queries the grid at the same resolution for free.
//! The host buffer is one `R32Uint` texel per pixel (padded rows to the 256
//! byte wgpu alignment), mirrored after `target::GpuTarget`.
//!
//! All ids are primitive indices (`+1` in the grid so `0` stays background);
//! mapping an index to the primitive's stable string id is
//! `scene.primitives[i].id`, done by the caller so the buffer has no
//! schemas of its own.

use oep_geo::math::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::scene::Scene;

use super::device::GpuContext;
use super::{box_faces, fan_triangles, tri_positions};

/// Texture format of the id target: 32-bit unsigned, one encoded primitive
/// index per pixel (`0` = background).
pub const ID_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// One id-pass face: which primitive it belongs to, its painter depth and its
/// NDC polygon (projected with the standard perspective divide, matching
/// `oep_viewer::pick`, per the module docs). Each vertex carries its NDC z so
/// the id pass can resolve per-pixel nearest with the reverse-Z depth buffer.
struct IdFace {
    /// Index into `Scene::primitives`.
    prim: u32,
    /// Projected NDC polygon, as `[x, y, z]` per vertex (z = NDC z, in
    /// reverse-Z convention the shader negates exactly like the colour pass).
    ndc: Vec<[f64; 3]>,
}

/// Project one world polygon into visible NDC with the standard perspective
/// divide (`nx = clip.x / -view.z`, the `oep_viewer::pick` convention):
/// vertices behind the camera or outside the NDC z cube are culled. Mirrors
/// the colour pass's face SET, with the corrected divide (module docs).
fn push_id_polygon(
    out: &mut Vec<IdFace>,
    prim: u32,
    poly: &[Vec3],
    world_to_view: &Mat4,
    projection: &Mat4,
) {
    if poly.len() < 3 {
        return;
    }
    let mut ok = true;
    let mut ndc = Vec::with_capacity(poly.len());
    for p in poly {
        let view = world_to_view.transform_point(p);
        let w = -view.z;
        if w.abs() < 1e-9 {
            ok = false;
            break;
        }
        let clip = projection.transform_point(&view);
        let nx = clip.x / w;
        let ny = clip.y / w;
        let nz = clip.z / w;
        if nz < -1.0 || nz > 1.0 {
            ok = false;
            break;
        }
        ndc.push([nx, ny, nz]);
    }
    if !ok || ndc.len() < 3 {
        return;
    }
    out.push(IdFace { prim, ndc });
}

/// Walk the scene and build the id-pass face list (the same face polygon set
/// as the colour pass, projected with the standard divide). The face ORDER is
/// irrelevant for topmost (the reverse-Z depth buffer resolves per-pixel
/// nearest exactly like the colour pass), so no CPU painter sort survives.
fn assemble_id_faces(scene: &Scene, world_to_view: &Mat4, projection: &Mat4) -> Vec<IdFace> {
    let mut faces: Vec<IdFace> = Vec::new();
    for (i, prim) in scene.primitives.iter().enumerate() {
        match &prim.mesh {
            Some(mesh) => {
                for t in &mesh.triangles {
                    let poly = tri_positions(&mesh.positions, t);
                    push_id_polygon(&mut faces, i as u32, &poly, world_to_view, projection);
                }
            }
            None => {
                for face in box_faces(prim) {
                    push_id_polygon(&mut faces, i as u32, &face, world_to_view, projection);
                }
            }
        }
    }
    faces
}

/// The ordered list of primitive ids of a scene, used as the cheap change
/// fingerprint for re-render decisions (Scene itself is not `PartialEq` and
/// `scene.rs` is outside this card's owned files).
fn scene_id_keys(scene: &Scene) -> Vec<String> {
    scene.primitives.iter().map(|p| p.id.clone()).collect()
}

/// A 32-bit unsigned id target (render attachment + copy source) with host
/// readback, mirrored after [`super::target::GpuTarget`] for the id format.
///
/// UI-D11: the target also owns a reverse-Z D32Float depth attachment with the
/// same format/clear/compare/write semantics as the viewport colour target, so
/// the id pass resolves the per-pixel NEAREST fragment exactly like the colour
/// pass (the topmost id equals the topmost colour fragment). The id vertices
/// carry their NDC z, so identical reverse-Z depth values reach both passes.
struct IdTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Reverse-Z depth texture (D32Float): the same format and semantics as
    /// [`super::target::GpuTarget`]'s depth attachment, so the id pass and the
    /// colour pass agree on the per-pixel nearest face.
    ///
    /// Stored (never read on the host) to keep the texture alive for the
    /// [`IdTarget::depth_view`] the pass attaches.
    #[allow(dead_code)]
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// Bytes per row the copy uses (padded to the 256-byte alignment).
    padded_bytes_per_row: u32,
}

/// Round `value` up to the next multiple of 256 (the wgpu copy alignment for
/// `bytes_per_row`), mirroring `target::align_up`.
const fn align_256(value: u32) -> u32 {
    value.div_ceil(256) * 256
}

impl IdTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("isolith-gpu-id-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ID_TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Reverse-Z depth attachment, identical to the colour target's: the id
        // pass must resolve per-pixel nearest with the same D32Float reverse-Z
        // semantics as the colour pass so topmost id == topmost colour.
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("isolith-gpu-id-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: super::target::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            padded_bytes_per_row: align_256(width * std::mem::size_of::<u32>() as u32),
            texture,
            view,
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    /// The render-pass colour attachment for this target: clear to `0`
    /// (background) and store, so every texel the pass does not cover reads
    /// back as `0` ("nothing here").
    fn attachment(&self) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::default()),
                store: wgpu::StoreOp::Store,
            },
        }
    }

    /// The reverse-Z depth attachment, cleared to far (`DEPTH_CLEAR`) exactly
    /// like the colour target, so the id pass starts "as far as possible" and
    /// writes the nearest fragment's id just like the colour pass.
    fn depth_attachment(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(super::target::DEPTH_CLEAR),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }
    }

    /// Copy the target into a readback buffer sized for the padded rows, then
    /// map it synchronously and strip the row padding into a flat `u32` grid
    /// (`width * height`, row-major, top-left origin).
    fn read_back_u32(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u32> {
        let buf_size = (self.padded_bytes_per_row as u64) * u64::from(self.height);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("isolith-gpu-id-readback"),
            size: buf_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        if let Ok(Err(e)) = rx.recv() {
            panic!("GPU id-buffer readback mapping failed: {e:?}");
        }

        let count = (self.width as usize) * (self.height as usize);
        let stride = self.width as usize * std::mem::size_of::<u32>();
        let padded = self.padded_bytes_per_row as usize;
        let mut data = vec![0u32; count];
        {
            let mapped = readback
                .slice(..)
                .get_mapped_range()
                .expect("mapped id readback range");
            for row in 0..self.height as usize {
                let src = &mapped[row * padded..row * padded + stride];
                for (i, chunk) in src.as_chunks::<4>().0.iter().enumerate() {
                    data[row * self.width as usize + i] = u32::from_le_bytes(*chunk);
                }
            }
        }
        readback.unmap();
        data
    }
}

/// One interleaved id vertex: NDC position (x/y in [-1,1], top-left origin,
/// exactly as the colour pass hands the GPU) plus the encoded id
/// (`primitive index + 1`, so `0` remains background).
#[derive(Debug, Clone, Copy)]
struct IdVertex {
    pos: [f32; 3],
    id: u32,
}

impl IdVertex {
    fn new(pos: [f64; 3], id: u32) -> Self {
        Self {
            pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
            id,
        }
    }

    fn pack_into(&self, bytes: &mut Vec<u8>) {
        for c in self.pos {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        bytes.extend_from_slice(&self.id.to_le_bytes());
    }
}

/// The offscreen id pass: rasterizes the drawn faces into the id target and
/// answers the pick queries from the read-back grid.
///
/// Owns its [`GpuContext`] (the engine's shared device when the app registered
/// one, an autonomous device otherwise), the pipeline, the target and the
/// host grid. [`IdPass::set_view`] re-renders only when the view changed and
/// returns whether it did.
pub struct IdPass {
    ctx: GpuContext,
    pipeline: wgpu::RenderPipeline,
    target: IdTarget,
    scene: Scene,
    camera: Camera,
    width: u32,
    height: u32,
    /// The read-back grid (`width * height`, row-major): encoded id per pixel.
    grid: Vec<u32>,
}

impl IdPass {
    /// Build an id pass for `scene`/`camera`/`size` on `ctx` and render it
    /// once. `None` only when a resource could not be created (the caller
    /// treats that as "no GPU picking").
    pub fn new(
        ctx: GpuContext,
        scene: Scene,
        camera: Camera,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let width = width.max(1);
        let height = height.max(1);
        let pipeline = build_pipeline(&ctx)?;
        let target = IdTarget::new(&ctx.device, width, height);
        let mut pass = Self {
            ctx,
            pipeline,
            target,
            scene,
            camera,
            width,
            height,
            grid: Vec::new(),
        };
        pass.render();
        Some(pass)
    }

    /// Recreate the id target at `width`x`height` and render it.
    fn resize_target(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.target = IdTarget::new(&self.ctx.device, self.width, self.height);
        self.render();
    }

    /// Bring the pass to the given view and re-rasterize the id grid when the
    /// view actually changed. Returns `true` when a re-render happened.
    ///
    /// The scene comparison uses the primitive-id list (a cheap fingerprint);
    /// camera and size compare by value. An unchanged view keeps the previous
    /// grid, so a hover sweep never touches the GPU again.
    pub fn set_view(&mut self, scene: &Scene, camera: Camera, width: u32, height: u32) -> bool {
        let size_changed = self.width != width.max(1) || self.height != height.max(1);
        let camera_changed = camera != self.camera;
        let scene_changed = scene_id_keys(scene) != scene_id_keys(&self.scene);
        self.camera = camera;
        self.scene = scene.clone();
        if size_changed {
            self.resize_target(width, height);
            true
        } else if camera_changed || scene_changed {
            self.render();
            true
        } else {
            false
        }
    }

    /// The pixel size the pass currently covers.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The primitive index (into the bound `Scene::primitives`) whose drawn
    /// face covers pixel `(x, y)`, or `None` for background.
    ///
    /// Coordinates are top-left origin, in pixels; out-of-range requests
    /// return `None`.
    pub fn pick_at(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let v = self.grid[(y * self.width + x) as usize];
        if v == 0 {
            None
        } else {
            Some(v - 1)
        }
    }

    /// The exact enclosed set of a pixel rect: every primitive index with at
    /// least one drawn face pixel inside the rect, ascending, each once.
    /// The rect is normalized (either drag direction) and clamped to the grid.
    pub fn pick_rect(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> Vec<u32> {
        let (lo_x, hi_x) = (x0.min(x1).min(self.width), x0.max(x1).min(self.width));
        let (lo_y, hi_y) = (y0.min(y1).min(self.height), y0.max(y1).min(self.height));
        let mut seen = std::collections::HashSet::new();
        for y in lo_y..hi_y {
            let row = &self.grid[(y * self.width) as usize..((y + 1) * self.width) as usize];
            for &v in &row[lo_x as usize..hi_x as usize] {
                if v != 0 {
                    seen.insert(v - 1);
                }
            }
        }
        let mut ids: Vec<u32> = seen.into_iter().collect();
        ids.sort_unstable();
        ids
    }

    /// Re-rasterize the id grid from the current scene/camera. Idempotent and
    /// deterministic: the same view always produces the same grid.
    pub fn render(&mut self) {
        let aspect = f64::from(self.width) / f64::from(self.height);
        let world_to_view = self.camera.view_matrix();
        let projection = self.camera.projection(aspect);
        let faces = assemble_id_faces(&self.scene, &world_to_view, &projection);

        let mut bytes = Vec::with_capacity(faces.len() * 3 * 16);
        let mut count = 0u32;
        for face in &faces {
            for tri in fan_triangles(&face.ndc) {
                for p in tri {
                    let v = IdVertex::new(p, face.prim + 1);
                    v.pack_into(&mut bytes);
                    count += 1;
                }
            }
        }

        let device = &self.ctx.device;
        let queue = &self.ctx.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("isolith-gpu-id-pass"),
            color_attachments: &[Some(self.target.attachment())],
            depth_stencil_attachment: Some(self.target.depth_attachment()),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        if count > 0 {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("isolith-gpu-id-vertices"),
                contents: &bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..count, 0..1);
        }
        drop(pass);

        queue.submit([encoder.finish()]);
        self.grid = self.target.read_back_u32(device, queue);
    }
}

/// Build the id-pass pipeline (flat u32 colour, blend REPLACE) mirroring the
/// colour pass's construction on the same device.
fn build_pipeline(ctx: &GpuContext) -> Option<wgpu::RenderPipeline> {
    let device = &ctx.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolith-gpu-id-shader"),
        source: wgpu::ShaderSource::Wgsl(ID_SHADER_SRC.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolith-gpu-id-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    Some(
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("isolith-gpu-id-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_id"),
                compilation_options: Default::default(),
                buffers: &[Some(VERTEX_LAYOUT)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_id"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ID_TARGET_FORMAT,
                    // Integer formats are not blendable: no blend state at all.
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            // Reverse-Z D32Float aligned with the colour pass: with the id
            // vertices carrying their NDC z (negated in the vertex stage), the
            // id pass keeps only the nearest fragment per pixel — the same
            // face the viewport colour pass resolved — so the topmost id
            // equals the topmost colour fragment (I1).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::target::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        }),
    )
}

/// Interleaved vertex buffer layout for the id pass: `[pos f32x3 @0][id u32 @1]`.
#[rustfmt::skip]
const ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 12, shader_location: 1 },
];

#[rustfmt::skip]
const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'_> = wgpu::VertexBufferLayout {
    array_stride: 16,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &ATTRS,
};

/// The WGSL for the id pass: passthrough position (the CPU hands already-divided
/// NDC, exactly like the colour pass) plus a flat u32 primitive id written
/// verbatim into the `R32Uint` target. Inline here because the card's owned
/// file list does not include a new shader file.
const ID_SHADER_SRC: &str = r#"
struct IdIn {
    @location(0) pos: vec3<f32>,
    @location(1) id: u32,
};

struct IdOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) id: u32,
};

@vertex
fn vs_id(in: IdIn) -> IdOut {
    var out: IdOut;
    // Reverse-Z, exactly like the colour pass shading.wgsl: map the NDC z so
    // near (nz=-1) -> depth 1.0 and far (nz=+1) -> depth 0.0, matching the
    // viewport target's depth attachment. x/y are passed through untouched.
    out.clip = vec4<f32>(in.pos.x, in.pos.y, 0.5 - 0.5 * in.pos.z, 1.0);
    out.id = in.id;
    return out;
}

@fragment
fn fs_id(in: IdOut) -> @location(0) u32 {
    return in.id;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_256_rounds_up() {
        assert_eq!(align_256(1), 256);
        assert_eq!(align_256(256), 256);
        assert_eq!(align_256(257), 512);
        assert_eq!(align_256(3200), 3328);
    }

    #[test]
    fn empty_scene_reads_back_all_background() {
        // A pass over an empty scene must read back an all-zero grid (nothing
        // drawn -> no id), and every query must report background / none.
        let Some(ctx) = super::super::device::acquire() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let Some(pass) = IdPass::new(ctx, Scene::default(), Camera::default(), 64, 48) else {
            eprintln!("no id pass resources; skipping");
            return;
        };
        assert_eq!(pass.size(), (64, 48));
        assert!(
            pass.grid.iter().all(|&v| v == 0),
            "empty scene -> all background"
        );
        assert_eq!(pass.pick_at(0, 0), None);
        assert_eq!(pass.pick_at(63, 47), None);
        assert!(pass.pick_rect(0, 0, 64, 48).is_empty());
    }
}
