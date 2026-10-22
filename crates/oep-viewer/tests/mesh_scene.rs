//! INTERACTIVE-SHELL: the viewer renders real solid meshes and overlay wires
//! with selection highlighting, and picking works on mesh primitives.
//!
//! The box-proxy path (existing tests) is untouched; this file locks the new
//! mesh path: a [`MeshData`] primitive drawn through the same painter
//! pipeline still renders deterministically, an edge overlay visibly changes
//! the frame, a highlighted mesh uses the theme highlight color, and
//! screen-space picking selects the mesh primitive by its world AABB.

use oep_geo::kernel::Point2D;
use oep_geo::tessellate::{extrude_profile, TriMesh};
use oep_viewer::camera::Camera;
use oep_viewer::render::{render, RgbaImage};
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::{pick, theme};

fn cube_mesh() -> TriMesh {
    // Unit cube profile swept by 1: positions at z=0 and z=1.
    let profile = [
        Point2D::new(0.0, 0.0),
        Point2D::new(1.0, 0.0),
        Point2D::new(1.0, 1.0),
        Point2D::new(0.0, 1.0),
    ];
    extrude_profile(&profile, 1.0).expect("cube extrudes")
}

fn scene_with_cube() -> (Scene, Camera) {
    let mesh: TriMesh = cube_mesh();
    let data = MeshData {
        positions: mesh
            .positions
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect(),
        triangles: mesh.triangles,
        edges: mesh.edges,
    };
    let mut prim = Primitive::from_mesh("body:oep-body-p1", "part-1", "oep-body-p1", data);
    prim.id = "part:part-1:body:oep-body-p1".to_string();
    let mut scene = Scene::default();
    scene.primitives.push(prim);
    scene.bounds = scene
        .primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    let cam = Camera::framed(&scene.bounds.expect("bounds"));
    let mut cam = cam;
    // A three-quarter view so faces are not edge-on and shading is visible.
    cam.yaw = 0.8;
    cam.pitch = 0.6;
    (scene, cam)
}

#[test]
fn mesh_primitive_renders_non_background_and_deterministically() {
    let (scene, cam) = scene_with_cube();
    let img: RgbaImage = render(&scene, &cam, &[], 128, 128);
    let mut solid_pixels = 0usize;
    let bg = [
        theme::BACKGROUND_RGB[0],
        theme::BACKGROUND_RGB[1],
        theme::BACKGROUND_RGB[2],
        255,
    ];
    for y in 0..img.height {
        for x in 0..img.width {
            if img.pixel(x, y) != bg {
                solid_pixels += 1;
            }
        }
    }
    assert!(
        solid_pixels > 512,
        "the cube mesh must occupy a visible region (got {solid_pixels} px)"
    );
    let again = render(&scene, &cam, &[], 128, 128);
    assert_eq!(img, again, "mesh render must be deterministic");
}

#[test]
fn edge_overlay_changes_the_frame() {
    let (scene, cam) = scene_with_cube();
    let plain = render(&scene, &cam, &[], 128, 128);
    // A wire-only overlay of the same cube edges (no fill): it must draw
    // lines (corner stays background) and produce a different frame than the
    // filled solid.
    let mesh = cube_mesh();
    let data = MeshData {
        positions: mesh
            .positions
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect(),
        triangles: Vec::new(),
        edges: mesh.edges,
    };
    let prim = Primitive::from_mesh("w", "part-1", "oep-body-p1", data);
    let mut wire_scene = Scene::default();
    wire_scene.primitives.push(prim);
    wire_scene.bounds = Some(wire_scene.primitives[0].world_aabb());
    let no_fill = render(&wire_scene, &cam, &[], 128, 128);
    let bg = [
        theme::BACKGROUND_RGB[0],
        theme::BACKGROUND_RGB[1],
        theme::BACKGROUND_RGB[2],
        255,
    ];
    assert_eq!(
        no_fill.pixel(0, 0),
        bg,
        "corner stays background (a wire does not fill)"
    );
    assert_ne!(plain, no_fill, "wires visibly change the frame");
}

#[test]
fn highlighted_mesh_uses_theme_highlight_color() {
    let (mut scene, cam) = scene_with_cube();
    scene.primitives[0].highlighted = true;
    let img = render(&scene, &cam, &[], 96, 96);
    let mut found = false;
    for y in 0..img.height {
        for x in 0..img.width {
            let px = img.pixel(x, y);
            // Highlight tint: r/g/b near the theme highlight hue (shaded so
            // not exactly equal); simply require a strongly amber-dominant
            // pixel (r > g > b by a clear margin), which the part palette
            // (hue from the hash) is not guaranteed to produce.
            if px[0] > px[1] && px[1] > px[2] && (px[0] as i32 - px[2] as i32) > 40 {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }
    assert!(found, "highlighted mesh must show amber-dominant pixels");
}

#[test]
fn pick_selects_the_mesh_primitive() {
    let (scene, cam) = scene_with_cube();
    // The cube spans [0,1]^3 and the camera frames its bounds; the screen
    // centre ray must hit the cube's AABB and resolve to the right primitive.
    let sel = pick(&scene, &cam, 64.0, 64.0, 128.0, 128.0).expect("centre hits the cube");
    assert_eq!(sel.part_id, "part-1");
    assert_eq!(sel.body_id, "oep-body-p1");
}

#[test]
fn wire_overlay_primitive_renders_lines() {
    // A 1-segment wire: from (0,0,0) to (80,40,0).
    let points: [[f32; 3]; 2] = [[0.0, 0.0, 0.0], [80.0, 40.0, 0.0]];
    let edges = [[0usize, 1usize]];
    let mut scene = Scene::default();
    scene
        .primitives
        .push(Primitive::wire("sk:l0", "sketch", &points, &edges));
    let cam = Camera::framed(&scene.primitives[0].world_aabb());
    let img = render(&scene, &cam, &[], 128, 128);
    let bg = [
        theme::BACKGROUND_RGB[0],
        theme::BACKGROUND_RGB[1],
        theme::BACKGROUND_RGB[2],
        255,
    ];
    let mut drawn = 0usize;
    for y in 0..img.height {
        for x in 0..img.width {
            if img.pixel(x, y) != bg {
                drawn += 1;
            }
        }
    }
    assert!(
        drawn > 8,
        "wire line must draw visible pixels (got {drawn})"
    );
}
