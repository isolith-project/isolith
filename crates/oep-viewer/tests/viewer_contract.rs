//! P6-M5-T02 contract test: the preview viewer works on the REAL golden
//! oep-geo document (crates/oep-geo/tests/golden/example-bracket.oepgeo.json)
//! with camera, selection, clipping and rendering.
//!
//! This is the acceptance harness behind the Python test
//! `tests/test_preview_viewer_acceptance.py`: it must exercise camera,
//! pick, clip and the golden model by name, so the Python gate can assert this
//! file covers the real material.

use std::fs;
use std::path::PathBuf;

use oep_geo::Document;

use oep_viewer::camera::Camera;
use oep_viewer::clip::{ClipPlane, Side};
use oep_viewer::pick;
use oep_viewer::render::{encode_png, render, RgbaImage};
use oep_viewer::scene::build_scene;

/// Locate the committed golden document (the REAL material this viewer must
/// show), independent of the working directory.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("oep-geo")
        .join("tests")
        .join("golden")
        .join("example-bracket.oepgeo.json")
}

fn load_golden() -> Document {
    let p = golden_path();
    let text = fs::read_to_string(&p).expect("golden example-bracket.oepgeo.json must exist");
    let doc = Document::from_json_pretty(&text).expect("golden must parse as oep-geo v1");
    assert!(doc.validate().is_empty(), "golden must validate clean");
    doc
}

#[test]
fn scene_builds_full_assembly_from_golden() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    // Bracket body + 4 fastener instances = 5 primitives.
    assert_eq!(scene.primitives.len(), 5, "expected bracket + 4 fasteners");

    let bracket = scene
        .primitives
        .iter()
        .find(|p| p.part_id == "oep-part-bracket")
        .expect("bracket part must be present");
    assert_eq!(bracket.body_id, "oep-body-bracket");
    assert_eq!(bracket.world_aabb().min.x, 0.0);
    assert_eq!(bracket.world_aabb().max.x, 80.0);

    // The fasteners must be instanced: instance chain recorded, translated.
    let fasteners: Vec<_> = scene
        .primitives
        .iter()
        .filter(|p| p.part_id == "oep-part-fastener")
        .collect();
    assert_eq!(fasteners.len(), 4);
    for f in &fasteners {
        assert!(
            !f.instance_ids.is_empty(),
            "fastener must carry its instance chain"
        );
        assert!(
            f.world_aabb().min.z == 0.0 && f.world_aabb().max.z == 20.0,
            "fastener placed at z 0..20"
        );
    }
    // F1 sits at x=10, F2 at x=70 (from the golden transforms).
    let xs: Vec<f64> = fasteners.iter().map(|f| f.world_aabb().min.x).collect();
    assert!(
        xs.contains(&10.0) && xs.contains(&70.0),
        "golden x placements"
    );
}

#[test]
fn framed_camera_projects_bracket_centroid_near_view_centre() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    let bounds = scene.bounds.expect("non-empty scene");
    let mut cam = Camera::framed(&bounds);
    // Slight orbit so the assembly reads as 3D.
    cam.orbit(0.6, 0.35);

    let width = 640.0;
    let height = 480.0;
    let aspect = width / height;
    let vp = cam.view_projection(aspect);
    let c = bounds.centroid();
    let clip = vp.transform_point(&c);
    let w = clip_w(&vp, &c);
    let ndc_x = clip.x / w;
    let ndc_y = clip.y / w;
    // The centroid must project near the screen centre.
    assert!((ndc_x).abs() < 0.15, "centroid ndc.x {} off-centre", ndc_x);
    assert!((ndc_y).abs() < 0.15, "centroid ndc.y {} off-centre", ndc_y);
}

#[test]
fn pick_returns_full_identity_from_golden() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    let bounds = scene.bounds.unwrap();
    let mut cam = Camera::framed(&bounds);
    cam.orbit(0.6, 0.35);
    let w = 640.0;
    let h = 480.0;

    // Screen centre should hit SOMETHING (the nearest body behind it).
    let sel = pick(&scene, &cam, w / 2.0, h / 2.0, w, h);
    assert!(sel.is_some(), "centre pick must hit a primitive");
    if let Some(s) = sel {
        assert!(!s.id.is_empty());
        assert!(!s.part_id.is_empty());
        assert!(!s.body_id.is_empty());
        assert!(s.distance > 0.0);
        assert!(
            s.instance_ids.len() <= 2,
            "instance chain bounded by assembly depth"
        );
    }
}

#[test]
fn picking_ray_direction_is_normalized_and_origins_at_eye() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    let cam = Camera::framed(&scene.bounds.unwrap());
    let ray = pick::unproject(&cam, 320.0, 240.0, 640.0, 480.0);
    assert!((ray.dir.norm() - 1.0).abs() < 1e-9, "unit ray direction");
    assert_eq!(ray.origin, cam.eye());
}

#[test]
fn clip_plane_cuts_golden_bracket_assembly() {
    let doc = load_golden();
    let scene = build_scene(&doc);

    // Clip plane x = 40, kept side x <= 40 (right half removed).
    let clip = ClipPlane::point_normal(
        oep_geo::math::Vec3::new(40.0, 0.0, 0.0),
        oep_geo::math::Vec3::new(1.0, 0.0, 0.0),
    );

    let bracket = scene
        .primitives
        .iter()
        .find(|p| p.part_id == "oep-part-bracket")
        .unwrap();
    // Bracket spans x 0..80 so it intersects the x=40 plane.
    assert_eq!(
        clip.classify_aabb(&bracket.world_aabb()),
        Side::Intersecting
    );

    // Fastener F1 at x=10 is fully kept; F2 at x=70 fully removed.
    let f1 = scene
        .primitives
        .iter()
        .find(|p| p.part_id == "oep-part-fastener" && p.world_aabb().min.x < 20.0)
        .unwrap();
    let f2 = scene
        .primitives
        .iter()
        .find(|p| p.part_id == "oep-part-fastener" && p.world_aabb().min.x > 50.0)
        .unwrap();
    assert_eq!(clip.classify_aabb(&f1.world_aabb()), Side::Inside);
    assert_eq!(clip.classify_aabb(&f2.world_aabb()), Side::Outside);
}

#[test]
fn render_produces_distinct_full_and_clipped_images() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    let cam = Camera::framed(&scene.bounds.unwrap());

    let full = render(&scene, &cam, &[], 320, 240);
    assert_eq!(full.data.len(), 320 * 240 * 4);

    let cut = ClipPlane::point_normal(
        oep_geo::math::Vec3::new(40.0, 0.0, 0.0),
        oep_geo::math::Vec3::new(1.0, 0.0, 0.0),
    );
    let clipped = render(&scene, &cam, &[cut], 320, 240);

    let non_bg = |img: &RgbaImage| {
        img.data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] != 28 || px[1] != 30 || px[2] != 34)
            .count()
    };
    assert!(non_bg(&full) > 100, "full render must have drawn geometry");
    assert!(
        non_bg(&clipped) < non_bg(&full),
        "clipping must remove geometry from the render"
    );
}

#[test]
fn png_encoder_roundtrips_with_valid_magic() {
    let img = RgbaImage {
        width: 4,
        height: 4,
        data: vec![255u8, 0, 0, 255]
            .into_iter()
            .cycle()
            .take(4 * 4 * 4)
            .collect(),
    };
    let png = encode_png(&img);
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(png.windows(4).any(|w| w == b"IDAT"));
    assert!(png.windows(4).any(|w| w == b"IEND"));
}

/// The `w` clip coordinate produced by the projection matrix (row 3 dot view).
fn clip_w(vp: &oep_geo::math::Mat4, view: &oep_geo::math::Vec3) -> f64 {
    let a = vp.to_array16();
    a[3] * view.x + a[7] * view.y + a[11] * view.z + a[15]
}
