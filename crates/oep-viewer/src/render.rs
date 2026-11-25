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
use crate::overlay;
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

    // Collect faces (box proxies or real meshes), frustum/clip culled,
    // transformed to NDC with view depth; plus overlay wire segments
    // (solid boundary edges and sketch wires) drawn after the fill.
    let mut faces: Vec<FaceNdc> = Vec::new();
    let mut wires: Vec<([Vec3; 2], [u8; 4])> = Vec::new();
    for prim in &scene.primitives {
        let color = if prim.highlighted {
            [
                theme::HIGHLIGHT_RGB[0],
                theme::HIGHLIGHT_RGB[1],
                theme::HIGHLIGHT_RGB[2],
                255,
            ]
        } else {
            theme::part_color(&prim.part_id)
        };
        match &prim.mesh {
            Some(mesh) => {
                for t in &mesh.triangles {
                    let poly = tri_poly(t, &mesh.positions);
                    push_polygon(&mut faces, &poly, color, &world_to_view, &vp, clips, &light);
                }
                if !mesh.edges.is_empty() {
                    let edge_color = if prim.highlighted { color } else { EDGE_RGB };
                    for [ia, ib] in &mesh.edges {
                        let (Some(pa), Some(pb)) =
                            (pos(&mesh.positions, *ia), pos(&mesh.positions, *ib))
                        else {
                            continue;
                        };
                        wires.push(([pa, pb], edge_color));
                    }
                }
            }
            None => {
                for face in box_faces(prim) {
                    push_polygon(&mut faces, &face, color, &world_to_view, &vp, clips, &light);
                }
            }
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

    // Overlay wires on top: solid boundary edges and sketch lines. Drawn
    // last so silhouette and guidance lines read above the fill.
    for (seg, color) in wires {
        draw_wire(&mut img, seg, &vp, color);
    }
    img
}

/// Which overlays a [`render_present`] call draws, and what data feeds them.
#[derive(Debug, Clone, Default)]
pub struct PresentOptions {
    /// Draw the world grid on the sketch plane (Z=0), spaced from the model.
    pub grid: bool,
    /// Draw the XYZ axes triad through the projected model origin.
    pub axes: bool,
    /// Screen-space label lines (dimension values from the variable set and
    /// the current selection id), drawn on a dark chip in the top-left.
    pub labels: Vec<String>,
}

/// The rendered present-mode frame plus the overlay composition counts, so
/// callers can report (and tests can assert) exactly what was drawn.
#[derive(Debug, Clone)]
pub struct PresentFrame {
    /// The composited RGBA raster.
    pub img: RgbaImage,
    /// Number of world grid segments drawn.
    pub grid_lines: usize,
    /// Number of axis arms drawn (0 or 3).
    pub axis_arms: usize,
    /// Number of label lines drawn.
    pub labels: usize,
}

/// Render `scene` at `width`x`height` and composite the present-mode viewport
/// overlays (grid, axes triad, labels) into the SAME raster.
///
/// The base render is byte-identical to [`render`] with the same inputs; the
/// overlays are composited afterward. Grid spacing is derived from the model
/// bounds with a deterministic 1/2/5 step; label text is rendered with the
/// crate's dependency-free 5x7 font. Deterministic end-to-end, so an HD
/// present frame is byte-reproducible and safe to commit as a regenerable
/// demo artifact.
pub fn render_present(
    scene: &Scene,
    cam: &Camera,
    clips: &[ClipPlane],
    width: u32,
    height: u32,
    opts: &PresentOptions,
) -> PresentFrame {
    let mut img = render(scene, cam, clips, width, height);
    let aspect = width as f64 / height as f64;
    let vp = cam.view_projection(aspect);
    let opacity = 255u8;
    let grid_color = theme::GRID_RGB;
    let mut grid_lines = 0usize;
    let mut axis_arms = 0usize;

    if opts.grid {
        // Extent from the model bounds on the sketch plane (X and Y), padded
        // so the grid reads as a surround, not a tight cage.
        let raw_extent = scene
            .bounds
            .map(|b| {
                [
                    b.min.x.abs().max(b.max.x.abs()),
                    b.min.y.abs().max(b.max.y.abs()),
                ]
            })
            .unwrap_or([100.0, 100.0]);
        let extent = raw_extent.into_iter().fold(1.0f64, f64::max);
        let spacing = overlay::nice_spacing(extent / 4.0);
        let segs = overlay::grid_segments(extent * 1.3, spacing);
        grid_lines = segs.len();
        for seg in &segs {
            draw_wire(
                &mut img,
                *seg,
                &vp,
                [grid_color[0], grid_color[1], grid_color[2], opacity],
            );
        }
    }
    if opts.axes {
        let origin = scene
            .bounds
            .map(|b| Vec3::new(0.0, 0.0, b.min.z.min(0.0)))
            .unwrap_or_else(Vec3::zero);
        // Anchor at the projected origin, clamped into the frame.
        let proj_origin = to_px(origin, &vp, width, height);
        let (ax, ay) = proj_origin.unwrap_or((0.0, 0.0));
        let ax = ax.clamp(8.0, width.saturating_sub(8) as f64);
        let ay = ay.clamp(8.0, height.saturating_sub(8) as f64);
        let arm_px = (width.min(height) as f64 * 0.07).max(24.0);
        let arm_world = scene
            .bounds
            .map(|b| b.sphere_radius())
            .unwrap_or(10.0)
            .max(1e-3);
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let colors = [theme::AXIS_X_RGB, theme::AXIS_Y_RGB, theme::AXIS_Z_RGB];
        let labels = ["X", "Y", "Z"];
        for (i, axis) in axes.iter().enumerate() {
            let p = origin.add(&Vec3::new(axis[0], axis[1], axis[2]).mul(arm_world));
            let Some(pp) = to_px(p, &vp, width, height) else {
                continue;
            };
            let anchor = proj_origin.unwrap_or((0.0, 0.0));
            let (dx, dy) = (pp.0 - anchor.0, pp.1 - anchor.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-6 {
                continue;
            }
            let (ux, uy) = (dx / len, dy / len);
            let (ex, ey) = (ax + ux * arm_px, ay + uy * arm_px);
            rasterize_line(
                &mut img,
                ax,
                ay,
                ex,
                ey,
                [colors[i][0], colors[i][1], colors[i][2], opacity],
            );
            // Axis letter at the tip, offset a little past it.
            let (tx, ty) = (ex + ux * 3.0, ey + uy * 3.0);
            let c = [colors[i][0], colors[i][1], colors[i][2], opacity];
            overlay::draw_text(&mut img, tx.round() as i64, ty.round() as i64, labels[i], c);
            axis_arms += 1;
        }
    }
    let label_color = theme::LABEL_RGB;
    let label_bg = theme::LABEL_BG_RGB;
    let label_n = opts.labels.len();
    if label_n > 0 {
        overlay::draw_label_chip(
            &mut img,
            12,
            12,
            &opts.labels,
            [label_color[0], label_color[1], label_color[2], opacity],
            [label_bg[0], label_bg[1], label_bg[2], opacity],
        );
    }
    PresentFrame {
        img,
        grid_lines,
        axis_arms,
        labels: label_n,
    }
}

/// Project a world-space point to pixel coordinates (top-left origin).
///
/// The point is expected in WORLD space: `clip_w` applies the combined
/// view-projection `vp` to the point it is given, so giving it a view-space
/// point (as the pre-D7 code did) applies the view transform a second time
/// and degenerates to `w == 0` on exact axis views (UI-D4's Back-view
/// blocker). Every caller here passes a world point.
fn to_px(world: oep_geo::math::Vec3, vp: &Mat4, width: u32, height: u32) -> Option<(f64, f64)> {
    let clip = vp.transform_point(&world);
    let w = clip_w(vp, &world);
    if w.abs() < 1e-9 {
        return None;
    }
    let nx = clip.x / w;
    let ny = clip.y / w;
    let nz = clip.z / w;
    if nz < -1.0 || nz > 1.0 {
        return None;
    }
    Some((
        (nx * 0.5 + 0.5) * width as f64,
        (0.5 - ny * 0.5) * height as f64,
    ))
}

/// The world-space polygon of one indexed mesh triangle.
fn tri_poly(t: &[u32; 3], positions: &[[f32; 3]]) -> Vec<Vec3> {
    let mut v = Vec::with_capacity(3);
    for i in t {
        if let Some(p) = pos(positions, *i as usize) {
            v.push(p);
        }
    }
    v
}

/// The world-space point at `positions[i]`, if in range.
fn pos(positions: &[[f32; 3]], i: usize) -> Option<Vec3> {
    positions
        .get(i)
        .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
}

/// Push one world-space polygon through clipping, shading, view/NDC transform
/// and near/far cull into the face list (the shared painter pipeline).
#[allow(clippy::too_many_arguments)]
fn push_polygon(
    faces: &mut Vec<FaceNdc>,
    poly: &[Vec3],
    color: [u8; 4],
    world_to_view: &Mat4,
    vp: &Mat4,
    clips: &[ClipPlane],
    light: &Vec3,
) {
    if poly.len() < 3 {
        return;
    }
    let mut clipped: Vec<Vec3> = poly.to_vec();
    for pl in clips {
        clipped = pl.clip_polygon(&clipped);
        if clipped.len() < 3 {
            break;
        }
    }
    if clipped.len() < 3 {
        return;
    }
    // Deterministic world-space face normal (Newell's method over the
    // clipped polygon) -> Lambert shade factor.
    let normal = newell_normal(&clipped);
    let shade = normal.dot(light).max(0.0) * 0.55 + 0.45;
    // View + NDC transform and view depth.
    let mut ok = true;
    let mut pts: Vec<[f64; 3]> = Vec::with_capacity(clipped.len());
    let mut view_depth_sum = 0.0f64;
    for p in &clipped {
        let view = world_to_view.transform_point(p);
        let clip = vp.transform_point(p);
        // `w` must be the clip w of the SAME world point `p` the clip xyz
        // came from (D7: the combined matrix applied to body space). Feeding
        // the view-space point here double-applied the view transform.
        let w = clip_w(vp, p);
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
        return;
    }
    faces.push(FaceNdc {
        _pts: pts,
        depth: view_depth_sum / clipped.len() as f64,
        color,
        shade,
    });
}

/// Overlay wire color for solid boundary edges (neutral light gray).
const EDGE_RGB: [u8; 4] = [170, 176, 184, 255];

/// Rasterize one wire segment: project both endpoints to NDC (world points,
/// direct through `vp`), clip the segment to the visible NDC cube, then draw
/// it as a 1px line.
fn draw_wire(img: &mut RgbaImage, seg: [Vec3; 2], vp: &Mat4, color: [u8; 4]) {
    let mut ndc = [[0.0f64; 3]; 2];
    for (k, p) in seg.iter().enumerate() {
        let clip = vp.transform_point(p);
        let w = clip_w(vp, p);
        if w.abs() < 1e-9 {
            return;
        }
        ndc[k] = [clip.x / w, clip.y / w, clip.z / w];
    }
    let Some((t0, t1)) = clip_ndc_line(ndc[0], ndc[1]) else {
        return;
    };
    let a = lerp3(ndc[0], ndc[1], t0);
    let b = lerp3(ndc[0], ndc[1], t1);
    let to_px = |p: &[f64; 3]| -> (f64, f64) {
        let w = img.width as f64;
        let h = img.height as f64;
        ((p[0] * 0.5 + 0.5) * w, (0.5 - p[1] * 0.5) * h)
    };
    let (ax, ay) = to_px(&a);
    let (bx, by) = to_px(&b);
    let (cx0, cy0) = clip_point_to_rect(ax.round(), ay.round(), img.width, img.height);
    let (cx1, cy1) = clip_point_to_rect(bx.round(), by.round(), img.width, img.height);
    rasterize_line(img, cx0, cy0, cx1, cy1, color);
}

/// Parametrically clip a segment in NDC space to the visible cube
/// `[-1,1]^3`; returns the surviving `t` interval (None when fully out).
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

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Integer Bresenham line from `(x0,y0)` to `(x1,y1)`, clipped to the image.
fn rasterize_line(img: &mut RgbaImage, x0: f64, y0: f64, x1: f64, y1: f64, color: [u8; 4]) {
    let (sx0, sy0) = clip_point_to_rect(x0, y0, img.width, img.height);
    let (sx1, sy1) = clip_point_to_rect(x1, y1, img.width, img.height);
    let (mut x0, mut y0) = (sx0 as i64, sy0 as i64);
    let (x1, y1) = (sx1 as i64, sy1 as i64);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as u32) < img.width && (y0 as u32) < img.height {
            img.set_pixel(x0 as u32, y0 as u32, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Clip a floating pixel coordinate into the image rectangle (inclusive).
fn clip_point_to_rect(x: f64, y: f64, w: u32, h: u32) -> (f64, f64) {
    let maxx = (w.saturating_sub(1)) as f64;
    let maxy = (h.saturating_sub(1)) as f64;
    (x.clamp(0.0, maxx), y.clamp(0.0, maxy))
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

/// The `w` clip coordinate for a WORLD point under the combined
/// view-projection matrix (the clip-space homogeneous component, which the
/// perspective divide uses).
///
/// The pre-D7 bug fed the VIEW-space point into this (the caller that
/// pre-transformed `p` through `world_to_view` passed that result), so the
/// combined matrix's 4th row was applied to an already view-space point —
/// the view transform was applied a second time purely inside `w`. For a
/// right-handed perspective camera the projection's 4th row is
/// `[0, 0, -1, 0]`, so `w = -view.z`, and the double application scaled the
/// whole frame ~1.6x (UI-D1's doc note) and forced `w == 0` at the exact
/// axis views whose near (z=0) face lay perpendicular to the view axis
/// (UI-D4's Back-view blocker: every corner at z=0 in world, hence a whole
/// face at exactly `w = 0`, dropped by `push_polygon`'s `|w| < 1e-9` guard).
///
/// Pass the point in the same space `vp` expects — body space (world) — and
/// `row4(vp) · (p, 1)` is the true clip-space `w`.
fn clip_w(vp: &Mat4, world: &Vec3) -> f64 {
    // Row 3 (clip.w) = row3 · (world, 1). Column-major: element (3,c) at c*4+3.
    let a = vp.to_array16();
    a[3] * world.x + a[7] * world.y + a[11] * world.z + a[15]
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
