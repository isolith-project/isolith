//! Reference software renderer behind the P6-M5-T01 render seam, plus a
//! minimal dependency-free PNG encoder for the proof artifacts.
//!
//! The P6-M5-T01 ADR ("Clean interface") defines the renderer contract as:
//! *callers see a scene (geometry + materials + camera), a render target, and
//! a `render()` that returns pixels;* wgpu/WGSL concepts stay behind the
//! seam. This crate ships the deterministic CPU rasterizer (the ADR's
//! documented escape hatch, section 9) so previews are reproducible on any
//! machine, GPU or not, and the interactive/p2p WGSL backend can later slot in
//! behind the same seam without changing callers.
//!
//! Rendering strategy: painter's algorithm. Each primitive (box proxy) is
//! drawn as its 6 faces; faces are split against every enabled clip plane with
//! [`ClipPlane::clip_polygon`], transformed to NDC, and rasterized
//! back-to-front by view depth. Colors are stable per part id (a simple hash),
//! and a small directional shading term gives depth cues without hurting
//! determinism.

use crate::camera::Camera;
use crate::clip::ClipPlane;
use crate::scene::{Primitive, Scene};
use crate::theme;
use oep_geo::math::{Mat4, Vec3};

/// Width of the rendered frame.
pub const DEFAULT_WIDTH: u32 = 640;
/// Height of the rendered frame.
pub const DEFAULT_HEIGHT: u32 = 480;

/// An RGBA8 raster image.
#[derive(Debug, Clone, PartialEq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width*height*4` bytes, row-major RGBA.
    pub data: Vec<u8>,
}

impl RgbaImage {
    /// Get the RGBA tuple at a pixel.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    /// Set the RGBA tuple at a pixel.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&rgba);
    }
}

/// A projected, clipped face: normalized device coordinates + depth.
struct FaceNdc {
    /// NDC triangle(s) to fill, plus per-vertex z (unused for painter, kept for
    /// future z-buffer).
    _pts: Vec<[f64; 3]>,
    /// Average view-space depth for painter sorting (negative = near).
    depth: f64,
    /// Face color.
    color: [u8; 4],
    /// Deterministic Lambert shading factor in [0,1].
    shade: f64,
}

/// Render `scene` with `cam` at `width`x`height`, applying `clips`.
///
/// Deterministic: no random, no threads, f64 everywhere, painter ordering by
/// face view-depth. Returns an RGBA8 image with a dark background, part-stable
/// colors and simple directional shading.
pub fn render(
    scene: &Scene,
    cam: &Camera,
    clips: &[ClipPlane],
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut img = RgbaImage {
        width,
        height,
        data: vec![0; (width * height * 4) as usize],
    };
    // Background: dark neutral from the theme single source.
    for i in (0..img.data.len()).step_by(4) {
        img.data[i] = theme::BACKGROUND_RGB[0];
        img.data[i + 1] = theme::BACKGROUND_RGB[1];
        img.data[i + 2] = theme::BACKGROUND_RGB[2];
        img.data[i + 3] = 255;
    }
    if scene.primitives.is_empty() {
        return img;
    }

    let aspect = width as f64 / height as f64;
    let vp = cam.view_projection(aspect);
    let world_to_view = cam.view_matrix();
    // Fixed deterministic light direction for Lambert shading.
    let light = Vec3::new(-0.4, 0.8, 0.45).normalize().expect("light dir");

    // Collect faces, frustum/clip culled, transformed to NDC with view depth.
    let mut faces: Vec<FaceNdc> = Vec::new();
    for prim in &scene.primitives {
        let color = theme::part_color(&prim.part_id);
        for face in box_faces(prim) {
            // Clip against every enabled clip plane.
            let mut clipped: Vec<Vec3> = face.to_vec();
            for pl in clips {
                clipped = pl.clip_polygon(&clipped);
                if clipped.len() < 3 {
                    break;
                }
            }
            if clipped.len() < 3 {
                continue;
            }
            // Deterministic world-space face normal (Newell's method over the
            // clipped polygon) -> Lambert shade factor.
            let normal = newell_normal(&clipped);
            let shade = normal.dot(&light).max(0.0) * 0.55 + 0.45;
            // View + NDC transform and view depth.
            let mut ok = true;
            let mut pts: Vec<[f64; 3]> = Vec::with_capacity(clipped.len());
            let mut view_depth_sum = 0.0f64;
            for p in &clipped {
                let view = world_to_view.transform_point(p);
                let clip = vp.transform_point(p);
                let w = clip_w(&vp, &view);
                if w.abs() < 1e-9 {
                    ok = false;
                    break;
                }
                let ndc_x = clip.x / w;
                let ndc_y = clip.y / w;
                let ndc_z = clip.z / w;
                // Near/far frustum cull (NDC z in [-1,1]).
                if ndc_z < -1.0 || ndc_z > 1.0 {
                    ok = false;
                    break;
                }
                view_depth_sum += view.z;
                pts.push([ndc_x, ndc_y, ndc_z]);
            }
            if !ok || pts.len() < 3 {
                continue;
            }
            faces.push(FaceNdc {
                _pts: pts,
                depth: view_depth_sum / clipped.len() as f64,
                color,
                shade,
            });
        }
    }

    // Painter: farthest (largest view z, since view looks down -Z) first.
    faces.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Rasterize: for each face, fill its convex polygon (fan triangulation)
    // with its Lambert-shaded flat color. Opaque convex boxes sorted
    // back-to-front need no z-buffer for a correct fill.
    for face in faces {
        for tri in fan_triangles(&face._pts) {
            fill_triangle(&mut img, tri, shade_color(face.color, face.shade));
        }
    }
    img
}

/// Newell's method: robust area-weighted face normal of a planar polygon.
fn newell_normal(poly: &[Vec3]) -> Vec3 {
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..poly.len() {
        let cur = poly[i];
        let nxt = poly[(i + 1) % poly.len()];
        nx += (cur.y - nxt.y) * (cur.z + nxt.z);
        ny += (cur.z - nxt.z) * (cur.x + nxt.x);
        nz += (cur.x - nxt.x) * (cur.y + nxt.y);
    }
    let n = Vec3::new(nx, ny, nz);
    n.normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// The `w` clip coordinate produced by the projection matrix.
fn clip_w(vp: &Mat4, view: &Vec3) -> f64 {
    // Row 3 (clip.w) = row3 · (view, 1). Column-major: element (3,c) at c*4+3.
    let a = vp.to_array16();
    a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15]
}

/// Shade an opaque color by factor `k` in [0,1].
fn shade_color(c: [u8; 4], k: f64) -> [u8; 4] {
    let f = k.clamp(0.35, 1.0);
    let m = |x: u8| ((x as f64) * f).round().clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2]), c[3]]
}

/// The 6 faces of a box proxy as world-space quads.
fn box_faces(prim: &Primitive) -> [Vec<Vec3>; 6] {
    let p = &prim.world_corners;
    [
        vec![p[0], p[1], p[2], p[3]], // bottom z-
        vec![p[4], p[5], p[6], p[7]], // top z+
        vec![p[0], p[1], p[5], p[4]], // front y-
        vec![p[3], p[2], p[6], p[7]], // back y+
        vec![p[0], p[3], p[7], p[4]], // left x-
        vec![p[1], p[2], p[6], p[5]], // right x+
    ]
}

/// Fan a convex polygon (NDC) into a triangle list for filling.
fn fan_triangles(pts: &[[f64; 3]]) -> Vec<[[f64; 3]; 3]> {
    if pts.len() < 3 {
        return Vec::new();
    }
    (1..pts.len() - 1)
        .map(|i| [pts[0], pts[i], pts[i + 1]])
        .collect()
}

/// Fill one NDC triangle into the image using a scanline rasterizer.
fn fill_triangle(img: &mut RgbaImage, tri: [[f64; 3]; 3], color: [u8; 4]) {
    let w = img.width as f64;
    let h = img.height as f64;
    // NDC -> pixel (y flipped, top-left origin).
    let to_px = |p: &[f64; 3]| [(p[0] * 0.5 + 0.5) * w, (0.5 - p[1] * 0.5) * h];
    let [a, b, c] = tri;
    let pa = to_px(&a);
    let pb = to_px(&b);
    let pc = to_px(&c);
    let xmin = (pa[0].min(pb[0]).min(pc[0])).floor().max(0.0);
    let xmax = (pa[0].max(pb[0]).max(pc[0])).ceil().min(w - 1.0);
    let ymin = (pa[1].min(pb[1]).min(pc[1])).floor().max(0.0);
    let ymax = (pa[1].max(pb[1]).max(pc[1])).ceil().min(h - 1.0);
    let (ax, ay, bx, by, cx, cy) = (pa[0], pa[1], pb[0], pb[1], pc[0], pc[1]);
    let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
    if area.abs() < 1e-9 {
        return;
    }
    let mut y = ymin;
    while y <= ymax {
        let mut x = xmin;
        while x <= xmax {
            // Barycentric (sign-aware for either winding).
            let w0 = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
            let w1 = (cx - bx) * (y - by) - (cy - by) * (x - bx);
            let w2 = (ax - cx) * (y - cy) - (ay - cy) * (x - cx);
            let same =
                (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
            // Barycentric weights for shading later would use the real ones;
            // here we only need a point-in-triangle test, so scale to t values.
            let t0 = w0 / area;
            let t1 = w1 / area;
            let t2 = w2 / area;
            if same && t0 >= -0.001 && t1 >= -0.001 && t2 >= -0.001 {
                img.set_pixel(x as u32, y as u32, color);
            }
            x += 1.0;
        }
        y += 1.0;
    }
}

// ---------------------------------------------------------------------------
// Minimal PNG encoder (std-only). Deterministic, no external dependencies.
// Writes an 8-bit RGBA PNG with a single IDAT (zlib stored/deflate).
// ---------------------------------------------------------------------------

/// Encode an RGBA8 image as PNG bytes.
pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&img.width.to_be_bytes());
    ihdr.extend_from_slice(&img.height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT: raw scanlines with filter byte 0 per row, wrapped in a zlib
    // stream using a single stored (uncompressed) deflate block.
    let raw = raw_scanlines(img);
    let zlib = zlib_stored(&raw);
    write_chunk(&mut out, b"IDAT", &zlib);

    // IEND
    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// Prefix each scanline with a 0 filter byte.
fn raw_scanlines(img: &RgbaImage) -> Vec<u8> {
    let stride = (img.width * 4) as usize;
    let mut raw = Vec::with_capacity((stride + 1) * img.height as usize);
    for y in 0..img.height as usize {
        raw.push(0);
        raw.extend_from_slice(&img.data[y * stride..(y + 1) * stride]);
    }
    raw
}

/// zlib wrapper: 2-byte header + stored deflate block(s) + adler32.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 64);
    out.push(0x78); // CMF: deflate, 32K window
    out.push(0x01); // FLG
    const MAX: usize = 65535;
    let mut pos = 0;
    while pos < data.len() {
        let end = (pos + MAX).min(data.len());
        let len = end - pos;
        let chunk = &data[pos..end];
        let last = end == data.len();
        let bfinal = if last { 1u8 } else { 0u8 };
        out.push(bfinal); // BTYPE=00 stored
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&(!(len as u16)).to_le_bytes());
        out.extend_from_slice(chunk);
        pos = end;
        if last {
            break; // adler is computed over the whole stream regardless
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Adler-32 checksum (RFC 1950).
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

/// CRC-32 (IEEE, as used by PNG).
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// Write one PNG chunk: length, type, data, crc.
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_magic_and_chunks() {
        let img = RgbaImage {
            width: 8,
            height: 8,
            data: vec![0xde; 8 * 8 * 4],
        };
        let png = encode_png(&img);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert!(png.len() > 40);
        // Contains the IHDR and IEND chunk type markers.
        let text = String::from_utf8_lossy(&png);
        assert!(text.contains("IHDR"));
        assert!(text.contains("IEND"));
    }

    #[test]
    fn encode_png_is_deterministic() {
        let img = RgbaImage {
            width: 16,
            height: 16,
            data: (0..16 * 16 * 4).map(|i| (i % 255) as u8).collect(),
        };
        let a = encode_png(&img);
        let b = encode_png(&img);
        assert_eq!(a, b);
    }

    #[test]
    fn render_empty_scene_yields_background() {
        let scene = Scene::default();
        let cam = Camera::default();
        let img = render(&scene, &cam, &[], 32, 32);
        assert_eq!(img.data.len(), 32 * 32 * 4);
        assert_eq!(
            img.pixel(0, 0),
            [
                theme::BACKGROUND_RGB[0],
                theme::BACKGROUND_RGB[1],
                theme::BACKGROUND_RGB[2],
                255
            ]
        );
    }
}
