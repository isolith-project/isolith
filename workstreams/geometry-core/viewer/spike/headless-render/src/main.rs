// Isolith P6-M5-T01 renderer backend decision, real-material spike.
//
// Proves the adopted backend (wgpu + WGSL) can render HEADLESS: no display
// session, no window, no on-screen swapchain. A WGSL shader draws a triangle
// into an offscreen texture; the pixels are copied back to the CPU and saved
// as proof.png. This is the mechanism the P6 preview viewer (P6-M5-T02), the
// P9 headless render worker (P9-M3-T01), and any offline render job rely on.
//
// Verified on U1: NVIDIA GB10, Vulkan backend, driver 580.178.04.
// Run:  cargo run --release  (behind a display or not, it makes no surface)

const SHADER: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4f {
    var pos = array<vec2f, 3>(
        vec2f(0.0, 0.5),
        vec2f(-0.5, -0.5),
        vec2f(0.5, -0.5),
    );
    return vec4f(pos[idx], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    // Color by fragment position so the orientation is verifiable.
    let u = pos.x + 0.5;
    let v = pos.y + 0.5;
    return vec4f(u, v, 0.5, 1.0);
}
"#;

fn main() {
    // This instance never creates a surface or swapchain: the headless
    // contract is that the adapter + device + an offscreen target are enough.
    // MESA/llvmpipe may print probe warnings for missing /dev/dri nodes; the
    // adopted adapter is the NVIDIA Vulkan one (see adapter line printed).
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

    println!("adapter: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("headless-spike"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("device request failed");

    let width: u32 = 256;
    let height: u32 = 256;

    // Offscreen render target (no surface involved).
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("triangle"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
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

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("enc"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rpass"),
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
        rpass.set_pipeline(&pipeline);
        rpass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    // Copy back to CPU.
    let bytes_per_pixel = 4;
    let row_bytes = width * bytes_per_pixel;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("map"),
        size: (row_bytes * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder2 = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder2.copy_texture_to_buffer(
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
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder2.finish()));

    device.poll(wgpu::Maintain::Wait);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().expect("map failed");

    let data = slice.get_mapped_range();
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let off = (y * row_bytes + x * 4) as usize;
            rgb.push(data[off + 2]); // r
            rgb.push(data[off + 1]); // g
            rgb.push(data[off]);     // b
        }
    }
    drop(data);
    buffer.unmap();

    let img = image::RgbImage::from_raw(width, height, rgb).expect("image");
    img.save("proof.png").expect("save");

    // Verify pixels: image center must be lit (triangle covers it), and the
    // top-left corner must be clear (triangle does not cover it).
    let cx = (width / 2) as usize;
    let cy = (height / 2) as usize;
    let coff = (cy * width as usize + cx) * 3;
    let (pr, pg, pb) = (img.as_raw()[coff], img.as_raw()[coff + 1], img.as_raw()[coff + 2]);
    assert!(pr + pg + pb > 30, "center black: nothing rendered ({pr},{pg},{pb})");
    let (cr, cg, cb) = (img.as_raw()[0], img.as_raw()[1], img.as_raw()[2]);
    assert!(cr + cg + cb < 10, "corner not clear: wrong coverage ({cr},{cg},{cb})");

    println!(
        "RENDER OK: center=({pr},{pg},{pb}) corner=({cr},{cg},{cb}) size={width}x{height} -> proof.png"
    );
}
