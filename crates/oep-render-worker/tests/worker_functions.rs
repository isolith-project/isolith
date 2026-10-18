//! P9-M3-T01 contract test: the render worker registers with the scheduler
//! AND renders the real golden oep-geo model through the worker function.
//!
//! This is the two-sided gate the P9-M1 job spec wants (P9-M3-T01 owns
//! worker registry / function resolution): resolving the registration for
//! `render/oep-geo-render` succeeds, and using the worker function on the
//! real golden document (crates/oep-geo/tests/golden/example-bracket.oepgeo.json)
//! produces a non-blank image at the requested resolution, differing across
//! fidelity levels. All through the library surface, not shell-outs.

use oep_geo::Document;
use oep_render_worker::options::{Fidelity, RenderOptions, Resolution};
use oep_render_worker::registry::get;
use oep_render_worker::worker::render_document;

/// Load the real golden bracket model from the repo.
fn golden_document() -> Document {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../oep-geo/tests/golden/example-bracket.oepgeo.json"
    );
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read golden example-bracket: {e}"));
    Document::from_json_pretty(&text)
        .unwrap_or_else(|e| panic!("parse golden example-bracket: {e}"))
}

#[test]
fn registry_resolves_render_worker_function() {
    let w = get("render", "oep-geo-render")
        .expect("render worker must register as render/oep-geo-render");
    assert_eq!(w.kind, "render");
    assert_eq!(w.function, "oep-geo-render");
    assert_eq!(w.binary, "oep-render-worker");
    assert_eq!(w.fidelity_levels, &["low", "medium", "high"]);
}

#[test]
fn worker_renders_real_golden_model() {
    let doc = golden_document();
    let opts = RenderOptions::new(
        Resolution {
            width: 320,
            height: 240,
        },
        Fidelity::Low,
    );
    let out = render_document(&doc, opts);
    assert_eq!(out.image.width, 320);
    assert_eq!(out.image.height, 240);
    // Not blank: the bracket assembly draws geometry over the dark background.
    let bg = [28, 30, 34, 255];
    let mut any_drawn = false;
    for y in (0..240).step_by(8) {
        for x in (0..320).step_by(8) {
            if out.image.pixel(x, y) != bg {
                any_drawn = true;
            }
        }
    }
    assert!(
        any_drawn,
        "golden bracket render should contain drawn geometry"
    );
}

#[test]
fn fidelity_levels_change_output_pixels() {
    let doc = golden_document();
    let res = Resolution {
        width: 320,
        height: 240,
    };
    let low = render_document(&doc, RenderOptions::new(res, Fidelity::Low));
    let medium = render_document(&doc, RenderOptions::new(res, Fidelity::Medium));
    let high = render_document(&doc, RenderOptions::new(res, Fidelity::High));
    assert_eq!(low.image.width, high.image.width);
    assert_eq!(low.image.height, high.image.height);
    assert_ne!(
        low.image.data, high.image.data,
        "low and high fidelity must produce different pixels (supersample is a real knob)"
    );
    assert_ne!(medium.image.data, high.image.data);
}
