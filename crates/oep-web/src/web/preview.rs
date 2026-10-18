//! Viewer embed: render a stored `oep-geo` document to a PNG through the P6
//! preview viewer ([`oep_viewer`], reference CPU rasterizer). This is what
//! makes the acceptance "the UI renders real oep-geo models via the viewer
//! embed" true over plain HTTP.

use oep_geo::Document;
use oep_viewer::camera::Camera;
use oep_viewer::render::{encode_png, render};
use oep_viewer::scene::build_scene;

/// Render `doc` at `width` x `height` and return `(png_bytes, drawn_pixels)`.
///
/// `drawn_pixels` counts non-background pixels in the rendered image, useful
/// for asserting the model actually drew something (and for the
/// `X-Oep-Drawn-Pixels` response header). Uses a deterministic orbit camera
/// framed to the model bounds.
pub fn render_document_png(
    doc: &Document,
    width: u32,
    height: u32,
) -> Result<(Vec<u8>, usize), String> {
    let scene = build_scene(doc);
    let bounds = scene
        .bounds
        .ok_or_else(|| "empty scene (no geometry to render)".to_string())?;
    let mut cam = Camera::framed(&bounds);
    cam.orbit(0.6, 0.35);
    let img = render(&scene, &cam, &[], width, height);
    let drawn = drawn_pixel_count_inner(&img);
    let png = encode_png(&img);
    Ok((png, drawn))
}

/// Count non-background pixels in a rendered image (the viewer's background is
/// the fixed dark `(28,30,34,255)`).
pub fn drawn_pixel_count(img: &oep_viewer::render::RgbaImage) -> usize {
    drawn_pixel_count_inner(img)
}

fn drawn_pixel_count_inner(img: &oep_viewer::render::RgbaImage) -> usize {
    let mut n = 0usize;
    for i in (0..img.data.len()).step_by(4) {
        let bg = img.data[i] == 28 && img.data[i + 1] == 30 && img.data[i + 2] == 34;
        if !bg {
            n += 1;
        }
    }
    n
}
