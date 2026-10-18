//! P6-M5-T02 real-material proof generator.
//!
//! Loads the committed golden oep-geo document
//! (crates/oep-geo/tests/golden/example-bracket.oepgeo.json), builds the
//! viewer scene, frames the orbit camera, renders the full assembly and a
//! clip-plane variant with the reference software renderer, and writes the
//! two proof PNGs the acceptance test requires:
//!   P6-M5/proof/preview-full.png
//!   P6-M5/proof/preview-clipped.png
//!
//! Run:  cargo run -p oep-viewer --example render_bracket
//! Output path is resolved relative to the crate manifest (two levels up from
//! crates/oep-viewer => the repo root, then P6-M5/proof/).

use std::fs;
use std::path::PathBuf;

use oep_geo::Document;

use oep_viewer::camera::Camera;
use oep_viewer::clip::ClipPlane;
use oep_viewer::pick;
use oep_viewer::render::{encode_png, render, RgbaImage};
use oep_viewer::scene::build_scene;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn golden_path() -> PathBuf {
    repo_root()
        .join("crates")
        .join("oep-geo")
        .join("tests")
        .join("golden")
        .join("example-bracket.oepgeo.json")
}

fn write_png(path: &PathBuf, img: &RgbaImage) {
    let bytes = encode_png(img);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create proof dir");
    }
    fs::write(path, &bytes).expect("write proof png");
    println!(
        "wrote {} ({}x{}, {} bytes)",
        path.display(),
        img.width,
        img.height,
        bytes.len()
    );
}

fn main() {
    let gpath = golden_path();
    println!("golden: {}", gpath.display());
    let text = fs::read_to_string(&gpath).expect("golden example-bracket.oepgeo.json must exist");
    let doc = Document::from_json_pretty(&text).expect("parse golden document");

    let scene = build_scene(&doc);
    let n_parts = scene
        .primitives
        .iter()
        .map(|p| p.part_id.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    println!(
        "scene: {} primitives, {} parts, bounds {:?}",
        scene.primitives.len(),
        n_parts,
        scene.bounds
    );

    // Frame the camera to fit the assembly, then orbit to a readable angle.
    let bounds = scene.bounds.expect("non-empty scene");
    let mut cam = Camera::framed(&bounds);
    cam.orbit(0.6, 0.35);
    println!("camera eye {:?}", cam.eye());

    // 1) Full assembly render.
    let full = render(&scene, &cam, &[], 640, 480);
    let proof_dir = repo_root().join("P6-M5").join("proof");
    write_png(&proof_dir.join("preview-full.png"), &full);

    // 2) Clipped render: keep x <= 40 (right half of the bracket removed,
    //    the two right-hand fasteners fully removed).
    let cut = ClipPlane::point_normal(
        oep_geo::math::Vec3::new(40.0, 0.0, 0.0),
        oep_geo::math::Vec3::new(1.0, 0.0, 0.0),
    );
    let clipped = render(&scene, &cam, &[cut], 640, 480);
    write_png(&proof_dir.join("preview-clipped.png"), &clipped);

    // 3) Demonstrate selection: unproject the screen centre and report the
    //    picked identity chain (the actual desktop selection path).
    let sel = pick(&scene, &cam, 320.0, 240.0, 640.0, 480.0);
    match sel {
        Some(s) => println!(
            "pick@centre -> id={} part={} body={} instances={:?} distance={:.3}",
            s.id, s.part_id, s.body_id, s.instance_ids, s.distance
        ),
        None => println!("pick@centre -> (miss)"),
    }
}
