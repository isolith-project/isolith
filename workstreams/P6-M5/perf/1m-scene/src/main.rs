// Keelworks P6-M5-T03 performance benchmark: 1M-triangle scene interactive on
// U1-class hardware, measured budget.
//
// Real material, not mocks: this builds a genuine indexed mesh of >= 1,000,000
// triangles (a deterministic heightfield grid, 709x709 vertices -> 1,002,528
// triangles), uploads it to the GPU, and renders frame after frame headless
// with wgpu (Vulkan, no display, no surface, no swapchain), measuring the
// per-frame wall-clock cadence of submit + GPU completion. It writes a
// measured report (report-1m-triangles.json) and a proof frame
// (frame-proof.png) that shows the mesh actually reached the framebuffer.
//
// Budget (recorded in workstreams/P6-M5/viewer-perf-1m-budget.md): interactive = mean
// frame time <= 33.33 ms (>= 30 FPS floor) with p95 <= 50 ms. The binary exits
// nonzero if the measured mean or p95 misses the budget, so a regression on
// the reference host fails loudly.
//
// Run:  cargo run --release   (headless; a display is never required)
// Verified on U1: NVIDIA GB10, Vulkan, driver 580.178.04 (see report JSON).

use std::time::Instant;

use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct VsIn {
    @location(0) pos: vec3f,
    @location(1) color: vec3f,
};
struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) color: vec3f,
    @location(1) vpos: vec3f,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    // Grid is generated in [-1,1]^2 screen space (z lifts to a heightfield).
    out.pos = vec4f(in.pos.xy, in.pos.z, 1.0);
    out.color = in.color;
    out.vpos = in.pos;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    // Height-graded color so the proof image shows real surface structure.
    let h = in.vpos.z;
    let g = 0.25 + 0.5 * clamp(h / 0.5, 0.0, 1.0);
    let r = clamp(0.2 + h, 0.0, 1.0);
    let b = clamp(0.6 - h, 0.0, 1.0);
    return vec4f(r, g, b, 1.0);
}
"#;

const GRID_N: usize = 709; // vertices per side -> 2*(N-1)^2 = 1,002,528 triangles
#[allow(dead_code)]
const MIN_TRIANGLE_TARGET: usize = 1_000_000;

fn triangle_count() -> usize {
    2 * (GRID_N - 1) * (GRID_N - 1)
}

fn build_grid() -> (Vec<f32>, Vec<u32>) {
    // Indexed heightfield grid in screen space [-1,1]^2, deterministic.
    // Vertex: 6 floats (pos xyz, color rgb). Index: triangles, CCW.
    let mut verts = Vec::with_capacity(GRID_N * GRID_N * 6);
    let mut indices = Vec::with_capacity(triangle_count() * 3);
    let step = 2.0 / (GRID_N as f32 - 1.0);
    for j in 0..GRID_N {
        for i in 0..GRID_N {
            let x = -1.0 + i as f32 * step;
            let y = -1.0 + j as f32 * step;
            // Deterministic heightfield (no RNG): a couple of sine bumps.
            let z = 0.2 * (x * 3.1).sin() * (y * 2.7).cos();
            verts.push(x);
            verts.push(y);
            verts.push(z);
            verts.push((x + 1.0) * 0.5); // color r
            verts.push((y + 1.0) * 0.5); // color g
            verts.push((z + 0.3) * 0.8 + 0.3); // color b
        }
    }
    for j in 0..(GRID_N - 1) {
        for i in 0..(GRID_N - 1) {
            let a = (j * GRID_N + i) as u32;
            let b = (j * GRID_N + i + 1) as u32;
            let c = ((j + 1) * GRID_N + i) as u32;
            let d = ((j + 1) * GRID_N + i + 1) as u32;
            // Two triangles per cell (a,b,d) and (a,d,c).
            indices.push(a); indices.push(b); indices.push(d);
            indices.push(a); indices.push(d); indices.push(c);
        }
    }
    (verts, indices)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (p / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// Minimal bytemuck-equivalent: reinterpreting our f32/u32 slices as bytes.
// SAFETY: the input borrows live as long as the returned slice (same lifetime
// via elision here), alignment is 4 on all supported targets, and both calls
// only ever construct read-only byte views of plain old data.
fn bytemuck_cast_float_slice(v: &[f32]) -> &[u8] {
    // SAFETY: see function doc comment.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}
fn bytemuck_cast_u32_slice(v: &[u32]) -> &[u8] {
    // SAFETY: see function doc comment.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}

fn main() {
    println!("P6-M5-T03 viewer perf benchmark: {} triangles", triangle_count());
    assert!(triangle_count() >= MIN_TRIANGLE_TARGET);

    let (verts, indices) = build_grid();
    println!("grid: {} vertices, {} indices ({} triangles)", 
             verts.len() / 6, indices.len(), indices.len() / 3);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no adapter found (Vulkan)");

    let info = adapter.get_info();
    println!("adapter: {:?}", info);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("1m-scene-perf"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("device request failed");

    const W: u32 = 1280;
    const H: u32 = 720;

    // Offscreen render target + matching proof target (no surface involved).
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Vertex / index buffers for the 1M-triangle mesh.
    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("verts"),
        contents: bytemuck_cast_float_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("indices"),
        contents: bytemuck_cast_u32_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("grid"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 24, // pos(12) + color(12)
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Warmup the pipeline + driver state so the first timed frames are steady.
    let warmup = 10;
    for _ in 0..warmup {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warmup"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            rp.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
        queue.submit(Some(enc.finish()));
    }

    // Measured loop: per-frame wall clock from submit until GPU completion.
    let measured_frames = 120;
    let mut samples: Vec<f64> = Vec::with_capacity(measured_frames);
    for _ in 0..measured_frames {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("measured"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            rp.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
        let t0 = Instant::now();
        queue.submit(Some(enc.finish()));
        device.poll(wgpu::Maintain::Wait); // GPU completion = frame cadence
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
    let min_ms = sorted[0];
    let max_ms = *sorted.last().unwrap();
    let p95_ms = percentile(&sorted, 95.0);
    let fps_mean = 1000.0 / mean_ms;

    println!("frame time ms: min={min_ms:.3} mean={mean_ms:.3} p95={p95_ms:.3} max={max_ms:.3}");
    println!("fps (mean): {fps_mean:.1}  over {measured_frames} measured frames");

    // Write the final frame proof (readback of a rendered frame).
    let bytes_per_pixel = 4;
    let row_bytes = W * bytes_per_pixel;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("map"),
        size: (row_bytes * H) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc2 = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc2.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc2.finish()));
    device.poll(wgpu::Maintain::Wait);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().expect("map failed");
    let data = slice.get_mapped_range();
    let mut rgb = Vec::with_capacity((W * H * 3) as usize);
    let mut lit = 0u64;
    for y in 0..H {
        for x in 0..W {
            let off = (y * row_bytes + x * 4) as usize;
            let (b, g, r) = (data[off] as u32, data[off + 1] as u32, data[off + 2] as u32);
            if r + g + b > 12 {
                lit += 1;
            }
            rgb.push(r as u8);
            rgb.push(g as u8);
            rgb.push(b as u8);
        }
    }
    drop(data);
    buffer.unmap();
    let img = image::RgbImage::from_raw(W, H, rgb).expect("image");
    img.save("frame-proof.png").expect("save proof");

    let lit_frac = lit as f64 / (W as f64 * H as f64);
    println!("proof frame: {lit} lit pixels ({:.1}% of {W}x{H}) -> frame-proof.png", lit_frac * 100.0);
    assert!(lit_frac > 0.05, "proof frame nearly all black; nothing rendered");

    // Report (real measured numbers from this host).
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "u1".to_string());
    let mut report = String::new();
    report.push_str("{\n");
    report.push_str("  \"benchmark\": \"oep-1m-scene-perf\",\n");
    report.push_str("  \"card\": \"P6-M5-T03\",\n");
    report.push_str(&format!("  \"host\": \"{}\",\n", host));
    report.push_str(&format!("  \"adapter\": \"{}\",\n", info.name));
    report.push_str(&format!("  \"driver\": \"{}\",\n", info.driver));
    report.push_str(&format!("  \"driver_info\": \"{}\",\n", info.driver_info));
    report.push_str(&format!("  \"backend\": \"{:?}\",\n", info.backend));
    report.push_str(&format!("  \"triangles\": {},\n", triangle_count()));
    report.push_str(&format!("  \"vertices\": {},\n", verts.len() / 6));
    report.push_str(&format!("  \"indices\": {},\n", indices.len()));
    report.push_str(&format!("  \"width\": {},\n", W));
    report.push_str(&format!("  \"height\": {},\n", H));
    report.push_str(&format!("  \"warmup_frames\": {},\n", warmup));
    report.push_str(&format!("  \"measured_frames\": {},\n", measured_frames));
    report.push_str(&format!("  \"min_frame_ms\": {:.6},\n", min_ms));
    report.push_str(&format!("  \"mean_frame_ms\": {:.6},\n", mean_ms));
    report.push_str(&format!("  \"p95_frame_ms\": {:.6},\n", p95_ms));
    report.push_str(&format!("  \"max_frame_ms\": {:.6},\n", max_ms));
    report.push_str(&format!("  \"fps_mean\": {:.3},\n", fps_mean));
    report.push_str("  \"budget_mean_frame_ms\": 33.33,\n");
    report.push_str("  \"budget_p95_frame_ms\": 50.0\n");
    report.push_str("}\n");
    std::fs::write("report-1m-triangles.json", report).expect("write report");

    // Budget gate: fail loudly if the measured cadence misses the budget.
    let ok = mean_ms <= 33.33 && p95_ms <= 50.0;
    println!("RESULT: {} (mean={mean_ms:.3}ms, p95={p95_ms:.3}ms, {fps_mean:.1} fps)",
             if ok { "BUDGET MET" } else { "BUDGET MISSED" });
    if !ok {
        std::process::exit(1);
    }
}
