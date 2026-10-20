//! P7-M5-T02: accessibility on the viewer surface (oep-viewer), measured on
//! the REAL committed golden model.
//!
//! Each test maps to a row in workstreams/modeler/quality/accessibility-checklist.md:
//!
//! - VW-A11Y-01 structural identity without vision: pick returns the full
//!   structured identity (part/body/instance) with no pixel/color reading
//!   (selection carries no color fields by construction),
//! - VW-A11Y-02 identity is not color-only: the semantic channel is the
//!   structured pick result, part color is a stable reference cue only,
//! - VW-A11Y-03 render is a pure deterministic single-frame function (no
//!   animation loop, no flashing, no auto-motion),
//! - VW-A11Y-04 the default theme keeps every part color at or above the
//!   WCAG 1.4.11 non-text contrast floor (3.0:1) against the theme
//!   background, with the measured worst case asserted with margin,
//! - VW-A11Y-06 the renderer consumes the theme constants (single source).

use std::fs;
use std::path::PathBuf;

use oep_geo::Document;

use oep_viewer::camera::Camera;
use oep_viewer::pick;
use oep_viewer::render::render;
use oep_viewer::scene::build_scene;
use oep_viewer::theme::{self, BACKGROUND_RGB, MIN_NON_TEXT_CONTRAST};

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
fn worst_part_contrast_meets_wcag_floor() {
    // WCAG 1.4.11: graphical objects keep >= 3.0:1 against adjacent colors.
    let worst = theme::worst_part_contrast();
    assert!(
        worst >= MIN_NON_TEXT_CONTRAST,
        "worst part contrast {worst:.3} is below the {MIN_NON_TEXT_CONTRAST}:1 floor"
    );
    assert!(
        worst >= 3.2,
        "part palette has no margin over the floor (worst {worst:.3})"
    );
}

#[test]
fn every_part_color_across_hue_meets_the_floor() {
    for h in 0..360 {
        let c = theme::part_color_hue(h as f64);
        let ratio = theme::contrast_ratio([c[0], c[1], c[2]], BACKGROUND_RGB);
        assert!(
            ratio >= MIN_NON_TEXT_CONTRAST,
            "hue {h}: part color contrast {ratio:.3} below {MIN_NON_TEXT_CONTRAST}:1"
        );
    }
}

#[test]
fn renderer_uses_theme_constants() {
    let doc = load_golden();
    let scene = build_scene(&doc);
    let cam = Camera::framed(&scene.bounds.unwrap());
    let img = render(&scene, &cam, &[], 32, 24);
    // Background pixel must come from the theme single source.
    assert_eq!(
        img.pixel(0, 0),
        [BACKGROUND_RGB[0], BACKGROUND_RGB[1], BACKGROUND_RGB[2], 255]
    );
}

#[test]
fn render_is_deterministic_single_frame() {
    // Photosensitivity / no auto-motion: render is a pure function producing
    // exactly one frame; two identical calls give byte-identical output.
    let doc = load_golden();
    let scene = build_scene(&doc);
    let cam = Camera::framed(&scene.bounds.unwrap());
    let a = render(&scene, &cam, &[], 320, 240);
    let b = render(&scene, &cam, &[], 320, 240);
    assert_eq!(a, b, "render must be byte-deterministic (no auto-motion)");
}

#[test]
fn structural_identity_is_not_color_only() {
    // The identity channel is the structured pick result, available without
    // reading a single pixel; Selection intentionally carries no color field.
    let doc = load_golden();
    let scene = build_scene(&doc);
    let mut cam = Camera::framed(&scene.bounds.unwrap());
    cam.orbit(0.6, 0.35);
    let sel = pick(&scene, &cam, 320.0, 240.0, 640.0, 480.0)
        .expect("a centre pick must hit geometry on the golden model");
    assert!(!sel.id.is_empty(), "selection has a stable id");
    assert!(!sel.part_id.is_empty(), "selection names the part");
    assert!(!sel.body_id.is_empty(), "selection names the body");
    assert!(
        sel.part_id == "oep-part-bracket" || sel.part_id == "oep-part-fastener",
        "selection part {} is one of the golden parts",
        sel.part_id
    );
}
