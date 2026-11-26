//! Offscreen colour target and host readback for the GPU viewport backend
//! (wave D, card UI-D1).
//!
//! The backend renders the scene into a plain [`wgpu::Texture`]
//! (`Rgba8Unorm`, render-attachment + copy-source) with no surface and no
//! window: the frame is then copied to a host-mapped buffer and returned as
//! the seam's [`RgbaImage`], so the viewport panel keeps drawing an ordinary
//! image texture exactly as it does for the CPU backend. The format is
//! intentionally non-sRGB so the bytes written by the fragment shader are
//! read back unchanged (the colour constants and shade factors are already
//! the literal final bytes, mirroring the CPU rasterizer).

use crate::render::RgbaImage;
use wgpu::TextureUsages;

/// Texture format of the offscreen target: 8-bit RGBA, plain unorm so shader
/// output bytes round-trip to the read image exactly.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A colour target the backend renders into each frame, plus the statics for
/// reading it back.
pub struct GpuTarget {
    /// The render target texture (`Rgba8Unorm`, render-attachment + copy-src).
    pub texture: wgpu::Texture,
    /// View the render pass clears and draws into.
    pub view: wgpu::TextureView,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Bytes per row the copy uses (padded up to the 256-byte alignment
    /// `copy_texture_to_buffer` requires), so a 800px image is copied as
    /// (832, N) rows and the padding is stripped in [`Self::read_back`].
    padded_bytes_per_row: u32,
}

impl GpuTarget {
    /// Create a `width`x`height` render target on `device`.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("isolith-gpu-viewport-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let padded_bytes_per_row = align_up(width * 4, 256);
        Self {
            texture,
            view,
            width,
            height,
            padded_bytes_per_row,
        }
    }

    /// The colour attachment for this target (clear to `clear`, write on
    /// `false` so later draws overwrite like the CPU painter's algorithm).
    pub fn color_attachment(&self, clear: [f32; 4]) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: f64::from(clear[0]),
                    g: f64::from(clear[1]),
                    b: f64::from(clear[2]),
                    a: f64::from(clear[3]),
                }),
                store: wgpu::StoreOp::Store,
            },
        }
    }

    /// Copy the target into a readback buffer sized for the padded rows, then
    /// map it synchronously and strip the row padding into an [`RgbaImage`].
    ///
    /// Deterministic: the buffer is filled by one copy from one render pass,
    /// the mapping is waited on with `PollType::wait_indefinitely`, and the
    /// returned image is the byte-for-byte texture contents.
    pub fn read_back(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> RgbaImage {
        let buf_size = (self.padded_bytes_per_row as u64) * u64::from(self.height);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("isolith-gpu-viewport-readback"),
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
        // Wait until the copy + callback have completed.
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        if let Ok(Err(e)) = rx.recv() {
            panic!("GPU frame readback mapping failed: {e:?}");
        }

        let stride = (self.width as usize) * 4;
        let padded = self.padded_bytes_per_row as usize;
        let mut data = vec![0u8; stride * self.height as usize];
        {
            let mapped = readback
                .slice(..)
                .get_mapped_range()
                .expect("mapped readback range");
            for row in 0..self.height as usize {
                let src = &mapped[row * padded..row * padded + stride];
                let dst = &mut data[row * stride..(row + 1) * stride];
                dst.copy_from_slice(src);
            }
        }
        readback.unmap();

        RgbaImage {
            width: self.width,
            height: self.height,
            data,
        }
    }
}

/// Round `value` up to the next multiple of `align`.
const fn align_up(value: u32, align: u32) -> u32 {
    value.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_rounds_up_to_256() {
        assert_eq!(align_up(1, 256), 256);
        assert_eq!(align_up(3200, 256), 3328);
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(257, 256), 512);
        assert_eq!(align_up(128, 256), 256);
    }
}
