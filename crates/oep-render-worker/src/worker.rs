//! The render worker function: headless `oep-geo` rendering behind the
//! P6-M5-T01 seam, with parameterized resolution and fidelity.
//!
//! This is the "worker function" the P9-M1 job spec names: job input = scene
//! refs (a real [`oep_geo::Document`]), worker = `oep-geo-render`, output =
//! a rendered frame (PNG bytes). The implementation renders entirely
//! offscreen through the [`oep_viewer`] surface, so it is headless by
//! construction: no window, no display, no swapchain is ever involved, which
//! is the ADR's headless contract (P6-M5/renderer-backend-wgpu-adr.md, §3).
//!
//! Fidelity is a supersample factor: we render the scene at `resolution *
//! factor` internally, then box-downsample to the requested output size. Low
//! = 1x (no supersampling), Medium = 2x, High = 4x. Higher fidelity smooths
//! the box-proxy edges, producing different, higher-quality pixels -- a real,
//! testable quality knob, not a flag that changes nothing.

use crate::options::{Fidelity, Resolution};
use oep_geo::Document;
use oep_viewer::camera::Camera;
use oep_viewer::render::{render, RgbaImage};
use oep_viewer::scene::build_scene;

/// The worker output: the rendered image plus the options that produced it,
/// so downstream metadata can record exactly what was rendered.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderResult {
    /// Rendered image.
    pub image: RgbaImage,
    /// Requested output resolution.
    pub resolution: Resolution,
    /// Fidelity level used.
    pub fidelity: Fidelity,
}

/// Render a loaded `oep-geo` [`Document`] headless at `opts`.
///
/// Builds the viewer scene from the real model, frames an orbit camera to
/// fit its bounds, renders at the supersampled internal resolution, and
/// box-downsamples to the requested output size. Deterministic (no random,
/// no threads), so the same document + options always produce the same
/// pixels. Returns [`RenderResult`]; callers encode the image as PNG with
/// [`oep_viewer::render::encode_png`].
pub fn render_document(doc: &Document, opts: crate::RenderOptions) -> RenderResult {
    let scene = build_scene(doc);
    let bounds = scene
        .bounds
        .expect("render worker: document has no visible geometry to render");
    let mut cam = Camera::framed(&bounds);
    cam.orbit(0.6, 0.35);

    let factor = opts.fidelity.factor();
    let ss_w = opts.resolution.width * factor;
    let ss_h = opts.resolution.height * factor;
    let supersampled = render(&scene, &cam, &[], ss_w, ss_h);
    let image = if factor > 1 {
        box_downsample(&supersampled, opts.resolution.width, opts.resolution.height)
    } else {
        supersampled
    };

    RenderResult {
        image,
        resolution: opts.resolution,
        fidelity: opts.fidelity,
    }
}

/// Box-average downsample `src` to `out_w`x`out_h` (integer factor).
///
/// Each output pixel is the exact average of the corresponding `fx  x  fy`
/// block of source pixels, so the downsample is deterministic and matches
/// the supersample intent (area-averaged anti-aliasing). Requires
/// `src.width % out_w == 0 && src.height % out_h == 0` (true by construction
/// when the supersample factor is a whole multiplier).
pub fn box_downsample(src: &RgbaImage, out_w: u32, out_h: u32) -> RgbaImage {
    let fx = src.width / out_w;
    let fy = src.height / out_h;
    assert!(fx >= 1 && fy >= 1, "downsample factors must be >= 1");

    let mut out = RgbaImage {
        width: out_w,
        height: out_h,
        data: vec![0; (out_w * out_h * 4) as usize],
    };
    for y in 0..out_h {
        for x in 0..out_w {
            let mut acc = [0u64; 4];
            for sy in 0..fy {
                for sx in 0..fx {
                    let p = src.pixel(x * fx + sx, y * fy + sy);
                    for c in 0..4 {
                        acc[c] += p[c] as u64;
                    }
                }
            }
            let n = (fx * fy) as u64;
            let px = [
                (acc[0] / n) as u8,
                (acc[1] / n) as u8,
                (acc[2] / n) as u8,
                (acc[3] / n) as u8,
            ];
            out.set_pixel(x, y, px);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Fidelity, RenderOptions, Resolution};
    use oep_geo::{Body, BodyKind, Part};

    fn one_part_doc() -> Document {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p1", "part-one").with_body("b1"));
        doc.push_body(Body::new("b1", "body-one", BodyKind::Solid).with_bbox(
            oep_geo::BoundingBox::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        ));
        doc
    }

    #[test]
    fn renders_at_requested_resolution() {
        let doc = one_part_doc();
        let opts = RenderOptions::new(
            Resolution {
                width: 64,
                height: 48,
            },
            Fidelity::Low,
        );
        let out = render_document(&doc, opts);
        assert_eq!(out.image.width, 64);
        assert_eq!(out.image.height, 48);
        assert_eq!(out.image.data.len(), 64 * 48 * 4);
        // Not blank: something was drawn (background is 28,30,34).
        assert_ne!(out.image.pixel(32, 24), [28, 30, 34, 255]);
    }

    #[test]
    fn fidelity_changes_pixels_supersample() {
        let doc = one_part_doc();
        let res = Resolution {
            width: 64,
            height: 48,
        };
        let low = render_document(&doc, RenderOptions::new(res, Fidelity::Low));
        let high = render_document(&doc, RenderOptions::new(res, Fidelity::High));
        // Same output resolution, but the high-fidelity supersample must have
        // produced different pixels somewhere (anti-aliased edges).
        assert_ne!(low.image.data, high.image.data);
        // Downsample must preserve dimensions.
        assert_eq!(low.image.width, high.image.width);
        assert_eq!(low.image.height, high.image.height);
    }

    #[test]
    fn box_downsample_averages_blocks() {
        let src = RgbaImage {
            width: 4,
            height: 4,
            data: vec![255; 4 * 4 * 4],
        };
        let out = box_downsample(&src, 2, 2);
        assert_eq!(out.data.len(), 2 * 2 * 4);
        assert_eq!(out.pixel(0, 0), [255, 255, 255, 255]);
    }
}
