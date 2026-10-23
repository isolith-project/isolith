//! MEDIA-RENDERER: the present-mode renderer composes grid, axes triad and
//! labels into the same raster, is deterministic, and leaves the default
//! (non-present) render byte-identical.

use oep_geo::math::Vec3;
use oep_viewer::camera::Camera;
use oep_viewer::render::{encode_png, render, render_present, PresentOptions, RgbaImage};
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::theme;

/// A small solid box mesh in world space (the kind the editor viewport draws).
fn box_scene() -> Scene {
    let mut positions = Vec::new();
    let (x0, x1) = (-10.0_f32, 10.0_f32);
    let (y0, y1) = (-4.0_f32, 4.0_f32);
    let (z0, z1) = (-2.0_f32, 2.0_f32);
    let corners = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    positions.extend_from_slice(&corners);
    // 12 face triangles (two per face) so the painter has real fill.
    let triangles: Vec<[u32; 3]> = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [3, 2, 6],
        [3, 6, 7],
        [0, 7, 4],
        [0, 3, 7],
        [1, 5, 6],
        [1, 6, 2],
    ];
    let prim = Primitive::from_mesh(
        "part:p1:body:b1",
        "p1",
        "b1",
        MeshData {
            positions,
            triangles,
            edges: Vec::new(),
        },
    );
    let mut scene = Scene::default();
    scene.primitives.push(prim);
    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    scene
}

fn chips_painted(frame: &RgbaImage, expected_bg: [u8; 3]) -> usize {
    // Count pixels in the top-left corner that match the label chip backdrop.
    let mut n = 0usize;
    let (w, h) = (frame.width.min(120), frame.height.min(120));
    for y in 0..h {
        for x in 0..w {
            let p = frame.pixel(x, y);
            if p[0] == expected_bg[0] && p[1] == expected_bg[1] && p[2] == expected_bg[2] {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn default_present_options_equal_base_render() {
    let scene = box_scene();
    let cam = Camera::framed(&scene.bounds.expect("scene bounds"));
    let base = render(&scene, &cam, &[], 320, 180);
    let present = render_present(&scene, &cam, &[], 320, 180, &PresentOptions::default());
    assert_eq!(
        present.img.data, base.data,
        "empty present opts must not alter the raster"
    );
    assert_eq!(present.grid_lines, 0);
    assert_eq!(present.axis_arms, 0);
    assert_eq!(present.labels, 0);
}

#[test]
fn present_paints_grid_axes_and_labels() {
    let scene = box_scene();
    let cam = Camera::framed(&scene.bounds.expect("scene bounds"));
    let labels = vec!["width = 20.000".to_string(), "height = 8.000".to_string()];
    let opts = PresentOptions {
        grid: true,
        axes: true,
        labels: labels.clone(),
    };
    let fr = render_present(&scene, &cam, &[], 640, 360, &opts);
    assert_eq!(fr.img.width, 640);
    assert_eq!(fr.img.height, 360);
    assert!(fr.grid_lines > 0, "grid segments drawn");
    assert_eq!(fr.axis_arms, 3, "XYZ triad drawn");
    assert_eq!(fr.labels, labels.len());

    // The overlays actually painted pixels into the raster: the label chip
    // backdrop is present in the top-left corner, and the frame differs from
    // the bare render.
    assert!(
        chips_painted(&fr.img, theme::LABEL_BG_RGB) >= 20,
        "label chip backdrop visible"
    );
    let base = render(&scene, &cam, &[], 640, 360);
    assert_ne!(fr.img.data, base.data, "overlays must change the raster");
}

#[test]
fn present_is_deterministic_and_png_reproducible() {
    let scene = box_scene();
    let cam = Camera::framed(&scene.bounds.expect("scene bounds"));
    let opts = PresentOptions {
        grid: true,
        axes: true,
        labels: vec!["width = 20.000".to_string(), "height = 8.000".to_string()],
    };
    let a = render_present(&scene, &cam, &[], 320, 180, &opts);
    let b = render_present(&scene, &cam, &[], 320, 180, &opts);
    assert_eq!(a.img.data, b.img.data);
    let pa = encode_png(&a.img);
    let pb = encode_png(&b.img);
    assert_eq!(
        pa, pb,
        "PNG bytes must be reproducible for committed artifacts"
    );
}

#[test]
fn hd_present_render_has_real_dimensions() {
    let scene = box_scene();
    let cam = Camera::framed(&scene.bounds.expect("scene bounds"));
    let fr = render_present(
        &scene,
        &cam,
        &[],
        1920,
        1080,
        &PresentOptions {
            grid: true,
            axes: true,
            labels: vec!["width = 20.000".to_string()],
        },
    );
    assert_eq!((fr.img.width, fr.img.height), (1920, 1080));
    assert!(fr.grid_lines > 0);
    assert_eq!(fr.axis_arms, 3);
    // A real HD frame is far from empty.
    let non_bg = fr
        .img
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            !(px[0] == theme::BACKGROUND_RGB[0]
                && px[1] == theme::BACKGROUND_RGB[1]
                && px[2] == theme::BACKGROUND_RGB[2])
        })
        .count();
    assert!(non_bg > 1000, "HD present frame should be mostly non-empty");
}

#[test]
fn overlay_label_chip_is_legible_pixels() {
    let mut img = RgbaImage {
        width: 200,
        height: 100,
        data: vec![0; 200 * 100 * 4],
    };
    let labels = vec!["width = 80.000".to_string(), "height = 40.000".to_string()];
    let (cw, ch) = oep_viewer::overlay::draw_label_chip(
        &mut img,
        10,
        10,
        &labels,
        [
            theme::LABEL_RGB[0],
            theme::LABEL_RGB[1],
            theme::LABEL_RGB[2],
            255,
        ],
        [
            theme::LABEL_BG_RGB[0],
            theme::LABEL_BG_RGB[1],
            theme::LABEL_BG_RGB[2],
            255,
        ],
    );
    assert!(cw > 30 && ch > 20, "chip sized for two lines: {cw}x{ch}");
    // Foreground text pixels exist inside the chip.
    let mut fg = 0usize;
    for y in 10..(10 + ch as i64) {
        for x in 10..(10 + cw as i64) {
            let p = img.pixel(x as u32, y as u32);
            if p[0] == theme::LABEL_RGB[0] && p[1] == theme::LABEL_RGB[1] {
                fg += 1;
            }
        }
    }
    assert!(fg >= 20, "label text pixels present, got {fg}");
}

#[test]
fn axis_triad_direction_is_meaningful() {
    // The three world axes project to three distinct screen directions for a
    // generic camera, so the triad reflects the actual camera orientation.
    let scene = box_scene();
    let cam = Camera::framed(&scene.bounds.expect("scene bounds"));
    let w2v = cam.view_matrix();
    let vp = cam.view_projection(640.0 / 360.0);
    let origin = Vec3::zero();
    let mut dirs = Vec::new();
    for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        let po = project_impl(&w2v, &vp, &origin, 640, 360);
        let pp = project_impl(
            &w2v,
            &vp,
            &origin.add(&Vec3::new(axis[0], axis[1], axis[2])),
            640,
            360,
        );
        if let (Some(po), Some(pp)) = (po, pp) {
            dirs.push((pp.0 - po.0, pp.1 - po.1));
        }
    }
    assert_eq!(dirs.len(), 3, "all three axes project");
    let (a, b, c) = (dirs[0], dirs[1], dirs[2]);
    // Distinct directions: the pairwise cross products are non-zero.
    assert!((a.0 * b.1 - a.1 * b.0).abs() > 1e-9);
    assert!((a.0 * c.1 - a.1 * c.0).abs() > 1e-9);
    assert!((b.0 * c.1 - b.1 * c.0).abs() > 1e-9);
}

// Mirror of render.rs's projection so this test can reason about directions
// without reaching into a private fn: view-space point -> pixel.
fn project_impl(
    w2v: &oep_geo::math::Mat4,
    vp: &oep_geo::math::Mat4,
    p: &Vec3,
    width: u32,
    height: u32,
) -> Option<(f64, f64)> {
    let view = w2v.transform_point(p);
    let clip = vp.transform_point(&view);
    let a = vp.to_array16();
    let w = a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15];
    if w.abs() < 1e-9 {
        return None;
    }
    let (nx, ny, nz) = (clip.x / w, clip.y / w, clip.z / w);
    if nz < -1.0 || nz > 1.0 {
        return None;
    }
    Some((
        (nx * 0.5 + 0.5) * width as f64,
        (0.5 - ny * 0.5) * height as f64,
    ))
}
