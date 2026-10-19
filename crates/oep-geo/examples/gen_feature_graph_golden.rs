//! One-shot generator for the P6-M3-T03 feature-graph golden.
//!
//! Mirrors the P6-M1/P6-M2 regen convention:
//! `cargo run -p oep-geo --example gen_feature_graph_golden -- crates/oep-geo/tests/golden`
//!
//! Builds the bracket mini feature tree and writes its deterministic
//! `oep-feature-graph` v1 text (graph + canonical replay trace) to
//! `<output-dir>/bracket-feature-graph.oepfg`, then prints the text so the
//! PR body can quote the proof artifact.

use oep_geo::{FeatureGraph, FeatureNode};
use std::process::ExitCode;

/// The bracket story: a native (non-CATIA) mini feature tree whose replay
/// outputs are closed-form bboxes, independently re-derived by the Python
/// acceptance test.
pub fn build_bracket() -> FeatureGraph {
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("oep-feature-bracket-body", "bracket-body", "Extrude")
            .num("length", 80.0)
            .num("width", 40.0)
            .num("height", 6.0),
    )
    .expect("root added");
    g.add_child(
        "oep-feature-bracket-body",
        FeatureNode::new("oep-feature-side-shelf", "side-shelf", "Offset")
            .with_ref("oep-feature-bracket-body")
            .num("dx", 80.0)
            .num("dy", 0.0)
            .num("dz", 0.0),
    )
    .expect("offset added");
    g.add_child(
        "oep-feature-bracket-body",
        FeatureNode::new("oep-feature-body-union", "body-union", "Union")
            .with_ref("oep-feature-bracket-body")
            .with_ref("oep-feature-side-shelf"),
    )
    .expect("union added");
    g.add_child(
        "oep-feature-body-union",
        FeatureNode::new("oep-feature-top-trim", "top-trim", "Trim")
            .with_ref("oep-feature-body-union")
            .num("dz", 3.0),
    )
    .expect("trim added");
    g.add_child(
        "oep-feature-top-trim",
        FeatureNode::new("oep-feature-hub-mirror", "hub-mirror", "Mirror")
            .with_ref("oep-feature-top-trim")
            .text("axis", "x")
            .num("at", 90.0),
    )
    .expect("mirror added");
    g
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let dir = match args.next() {
        Some(d) => d,
        None => {
            eprintln!("usage: gen_feature_graph_golden <output-dir>");
            return ExitCode::FAILURE;
        }
    };
    let g = build_bracket();
    let text = match g.to_text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("serialize error: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let path = std::path::Path::new(&dir).join("bracket-feature-graph.oepfg");
    if let Err(e) = std::fs::write(&path, &text) {
        eprintln!("write error: {e}");
        return ExitCode::FAILURE;
    }
    println!("wrote {} ({} bytes)", path.display(), text.len());
    println!("{}", text);
    ExitCode::SUCCESS
}
