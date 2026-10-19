//! A tiny own software renderer for the mates/kinematics demo recording.
//!
//! Clean-room choice: the demo records its animation with a dependency-free
//! painter's-algorithm / z-buffer rasterizer (std only) written for this card.
//! No image, GUI or 3D vendor library is used; the frames are emitted as PPM
//! (P6) images that a packaging step (ffmpeg on the recording host) wraps into
//! a playable GIF/MP4.
//!
//! The meshes are **visualization-only approximations** derived from `oep-geo`
//! body bounding boxes and demo fixture dimensions (the `oep-geo` bodies carry
//! a `bbox` only at this program wave; B-Rep surfaces land later). The mates
//! and the recorded motion act purely on instance transforms, so the rendering
//! never influences the mechanics.

use std::f64::consts::TAU;

/// RGB color.
pub type Color = [u8; 3];

/// A triangle mesh in a fixed (local or world) frame.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Vertices, `[x, y, z]`.
    pub verts: Vec<[f64; 3]>,
    /// Triangles as vertex index triples (CCW front).
    pub tris: Vec<[usize; 3]>,
}

impl Mesh {
    /// An axis-aligned box from `min` to `max` (local frame).
    pub fn box_mesh(min: [f64; 3], max: [f64; 3]) -> Mesh {
        let (x0, y0, z0) = (min[0], min[1], min[2]);
        let (x1, y1, z1) = (max[0], max[1], max[2]);
        let v: Vec<[f64; 3]> = vec![
            [x0, y0, z0], // 0
            [x1, y0, z0], // 1
            [x1, y1, z0], // 2
            [x0, y1, z0], // 3
            [x0, y0, z1], // 4
            [x1, y0, z1], // 5
            [x1, y1, z1], // 6
            [x0, y1, z1], // 7
        ];
        let t: Vec<[usize; 3]> = vec![
            [0, 2, 1], // bottom
            [0, 3, 2],
            [4, 5, 6], // top
            [4, 6, 7],
            [1, 2, 6], // +x side
            [1, 6, 5],
            [0, 4, 7], // -x side
            [0, 7, 3],
            [0, 1, 5], // +y side
            [0, 5, 4],
            [3, 7, 6], // -y side
            [3, 6, 2],
        ];
        Mesh { verts: v, tris: t }
    }

    /// A cylinder of radius `r`, height `h`, axis along local +Z from z=0 to
    /// z=h, approximated as an `n`-sided prism.
    pub fn prism_cylinder(r: f64, h: f64, n: usize) -> Mesh {
        let mut verts = Vec::with_capacity(2 * n);
        let mut tris = Vec::with_capacity(4 * n);
        for i in 0..n {
            let a = TAU * i as f64 / n as f64;
            let (cy, cx) = (a.sin(), a.cos());
            verts.push([cx * r, cy * r, 0.0]);
            verts.push([cx * r, cy * r, h]);
        }
        // side walls (two tris each)
        for i in 0..n {
            let j = (i + 1) % n;
            let (b0, t0, b1, t1) = (2 * i, 2 * i + 1, 2 * j, 2 * j + 1);
            tris.push([b0, t0, b1]);
            tris.push([b1, t0, t1]);
        }
        // bottom fan
        let cb = verts.len();
        verts.push([0.0, 0.0, 0.0]);
        let ct = verts.len();
        verts.push([0.0, 0.0, h]);
        for i in 0..n {
            let j = (i + 1) % n;
            tris.push([cb, 2 * j, 2 * i]);
            tris.push([ct, 2 * i, 2 * j]);
        }
        Mesh { verts, tris }
    }

    /// Apply a world pose transform (position + orientation as column vectors)
    /// to every vertex, returning a world-frame mesh.
    pub fn transformed(&self, pos: [f64; 3], rot: &[f64; 9]) -> Mesh {
        let tr = |v: &[f64; 3]| -> [f64; 3] {
            [
                rot[0] * v[0] + rot[3] * v[1] + rot[6] * v[2] + pos[0],
                rot[1] * v[0] + rot[4] * v[1] + rot[7] * v[2] + pos[1],
                rot[2] * v[0] + rot[5] * v[1] + rot[8] * v[2] + pos[2],
            ]
        };
        Mesh {
            verts: self.verts.iter().map(tr).collect(),
            tris: self.tris.clone(),
        }
    }
}

/// A mesh plus a flat color, ready to rasterize.
#[derive(Debug, Clone)]
pub struct SceneItem {
    /// World-frame mesh.
    pub mesh: Mesh,
    /// Fill color.
    pub color: Color,
}

/// Perspective camera.
pub struct Camera {
    /// Eye position.
    pub eye: [f64; 3],
    /// Look-at target.
    pub target: [f64; 3],
    /// Approximate up vector.
    pub up: [f64; 3],
    /// Vertical field of view in degrees.
    pub fov_deg: f64,
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels.
    pub height: usize,
}

impl Camera {
    /// Rasterize a scene, write a PPM (P6 binary) file, return bytes written.
    pub fn render_ppm(&self, scene: &[SceneItem], path: &str) -> std::io::Result<usize> {
        let (w, h) = (self.width, self.height);
        let mut img = vec![[0_u8; 3]; w * h];
        let mut zbuf = vec![f64::INFINITY; w * h];

        // flat light background
        let bg: Color = [244, 247, 250];
        for px in img.iter_mut() {
            *px = bg;
        }

        // fill (with z-buffer) then edges.
        for item in scene {
            self.draw_mesh_filled(item, &mut img, &mut zbuf);
        }
        for item in scene {
            self.draw_mesh_edges(item, &mut img);
        }

        let mut out = Vec::with_capacity(w * h * 3 + 64);
        out.extend_from_slice(format!("P6\n{} {}\n255\n", w, h).as_bytes());
        for px in &img {
            out.extend_from_slice(px);
        }
        std::fs::write(path, &out)?;
        Ok(out.len())
    }

    fn view_basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let sub = |a: &[f64; 3], b: &[f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let nor = |a: &[f64; 3]| {
            let l = dot(a, a).sqrt();
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let cross = |a: &[f64; 3], b: &[f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let fwd = nor(&sub(&self.target, &self.eye));
        let right = nor(&cross(&fwd, &self.up));
        let upv = cross(&right, &fwd);
        (right, upv, fwd)
    }

    /// Project a world point; returns screen `(x, y)` and camera-space depth.
    fn project(&self, p: &[f64; 3]) -> Option<(f64, f64, f64)> {
        let (right, upv, fwd) = self.view_basis();
        let d = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let z = d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2];
        if z < 1.0 {
            return None;
        }
        let x = d[0] * right[0] + d[1] * right[1] + d[2] * right[2];
        let y = d[0] * upv[0] + d[1] * upv[1] + d[2] * upv[2];
        let f = (self.height as f64 / 2.0) / (self.fov_deg.to_radians() / 2.0).tan();
        let sx = self.width as f64 / 2.0 + f * x / z;
        let sy = self.height as f64 / 2.0 - f * y / z;
        Some((sx, sy, z))
    }

    fn draw_mesh_filled(&self, item: &SceneItem, img: &mut [[u8; 3]], zbuf: &mut [f64]) {
        let (w, h) = (self.width as f64, self.height as f64);
        for tri in &item.mesh.tris {
            let Some((ax0, ay0, cd_a)) = self.project(&item.mesh.verts[tri[0]]) else {
                continue;
            };
            let Some((bx0, by0, cd_b)) = self.project(&item.mesh.verts[tri[1]]) else {
                continue;
            };
            let Some((cx0, cy0, cd_c)) = self.project(&item.mesh.verts[tri[2]]) else {
                continue;
            };
            let (a, b, c) = ((ax0, ay0), (bx0, by0), (cx0, cy0));
            // bounding box
            let x0 = a.0.min(b.0).min(c.0).max(0.0).floor() as usize;
            let x1 = ((a.0.max(b.0).max(c.0)).min(w - 1.0).ceil() as usize).max(x0);
            let y0 = a.1.min(b.1).min(c.1).max(0.0).floor() as usize;
            let y1 = ((a.1.max(b.1).max(c.1)).min(h - 1.0).ceil() as usize).max(y0);
            let area = edge(a, b, c);
            if area.abs() < 1e-12 {
                continue;
            }
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let p = (px as f64 + 0.5, py as f64 + 0.5);
                    let (wa, wb, wc) = bary(a, b, c, p);
                    let tot = wa + wb + wc;
                    let (wa, wb, wc) = (wa / tot, wb / tot, wc / tot);
                    if wa < -1e-6 || wb < -1e-6 || wc < -1e-6 {
                        continue;
                    }
                    let z = wa * cd_a + wb * cd_b + wc * cd_c;
                    let off = py * self.width + px;
                    if z < zbuf[off] - 1e-9 {
                        zbuf[off] = z;
                        img[off] = item.color;
                    }
                }
            }
        }
    }

    fn draw_mesh_edges(&self, item: &SceneItem, img: &mut [[u8; 3]]) {
        let dark: Color = [30, 34, 40];
        for tri in &item.mesh.tris {
            let pts = [tri[0], tri[1], tri[2], tri[0]];
            for k in 0..3 {
                let Some((ax, ay, _)) = self.project(&item.mesh.verts[pts[k]]) else {
                    continue;
                };
                let Some((bx, by, _)) = self.project(&item.mesh.verts[pts[k + 1]]) else {
                    continue;
                };
                self.line((ax, ay), (bx, by), dark, img);
            }
        }
    }

    fn line(&self, a: (f64, f64), b: (f64, f64), color: Color, img: &mut [[u8; 3]]) {
        let (w, h) = (self.width as isize, self.height as isize);
        let mut x0 = a.0.round() as isize;
        let mut y0 = a.1.round() as isize;
        let x1 = b.0.round() as isize;
        let y1 = b.1.round() as isize;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
                img[(y0 * w + x0) as usize] = color;
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
}

fn edge(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn bary(a: (f64, f64), b: (f64, f64), c: (f64, f64), p: (f64, f64)) -> (f64, f64, f64) {
    let area = edge(a, b, c);
    (
        edge(p, b, c) / area,
        edge(a, p, c) / area,
        edge(a, b, p) / area,
    )
}
