//! One-shot generator for the KERNEL-REPLAY goldens.
//!
//! Mirrors the kernel-seam regen convention:
//! `cargo run -p oep-geo --features kernel --example gen_kernel_replay_golden -- crates/oep-geo/tests/golden`
//!
//! Replays the acceptance feature graph (Sketch -> Extrude -> Offset -> Union
//! -> Trim -> Mirror over an 80x40x6 footprint) and the mini-vocabulary
//! bracket story through the REAL OCCT 8.0.x kernel behind the oep-geo seam,
//! cross-checks every solid step against its closed-form analytic volume and
//! bbox, and writes:
//!
//!   - `kernel-replay-golden.json` -- per-step kernel measurements (JSON)
//!   - `kernel-replay-trace.oepkrt` -- the canonical byte-locked trace text
//!   - `acceptance-replay-graph.oepfg` -- the acceptance graph in the
//!     oep-feature-graph v1 owned format
//!   - `kernel-replay-bracket-golden.json` -- per-step measurements of the
//!     bracket mini story as REAL solids
//!
//! A mismatch against the closed form fails the generator loudly, so the
//! committed goldens cannot drift from the real kernel (the acceptance test
//! then asserts BOTH the closed form AND these committed goldens).
//!
//! Requires the `kernel` feature (links the real OCCT engine). The canonical
//! trace and the .oepfg must be byte-stable across regenerations (proved by
//! the acceptance test).

use oep_geo::{FeatureGraph, FeatureNode, KernelEvaluator, KernelValue, SolidMeasures, Tolerance};
use std::path::PathBuf;
use std::process::ExitCode;

fn tol() -> Tolerance {
    Tolerance::new(1e-6).expect("valid tolerance")
}

/// The acceptance story (width 40): Sketch -> Extrude h6 -> Offset dx80 ->
/// Union -> Trim dz3 -> Mirror x at 90.
fn build_acceptance_graph() -> FeatureGraph {
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("oep-feature-shelf-sketch", "shelf-sketch", "Sketch")
            .text("profile", "0,0;80,0;80,40;0,40"),
    )
    .expect("sketch root");
    g.add_child(
        "oep-feature-shelf-sketch",
        FeatureNode::new("oep-feature-shelf-body", "shelf-body", "Extrude")
            .with_ref("oep-feature-shelf-sketch")
            .num("height", 6.0),
    )
    .expect("extrude added");
    g.add_child(
        "oep-feature-shelf-body",
        FeatureNode::new("oep-feature-side-shelf", "side-shelf", "Offset")
            .with_ref("oep-feature-shelf-body")
            .num("dx", 80.0)
            .num("dy", 0.0)
            .num("dz", 0.0),
    )
    .expect("offset added");
    g.add_child(
        "oep-feature-shelf-body",
        FeatureNode::new("oep-feature-body-union", "body-union", "Union")
            .with_ref("oep-feature-shelf-body")
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

/// The mini-vocabulary bracket story (Extrude rectangle fallback, no Sketch).
fn build_bracket() -> FeatureGraph {
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

/// Closed-form expectations for the 80x40-footprint stories. The acceptance
/// story extrudes the Sketch-driven `oep-feature-shelf-body`; the mini bracket
/// story extrudes `oep-feature-bracket-body`. Only that first id differs.
fn expectations(extrude_id: String) -> Vec<(String, f64, [f64; 6])> {
    vec![
        (extrude_id, 19200.0, [0.0, 0.0, 0.0, 80.0, 40.0, 6.0]),
        (
            "oep-feature-side-shelf".to_string(),
            19200.0,
            [80.0, 0.0, 0.0, 160.0, 40.0, 6.0],
        ),
        (
            "oep-feature-body-union".to_string(),
            38400.0,
            [0.0, 0.0, 0.0, 160.0, 40.0, 6.0],
        ),
        (
            "oep-feature-top-trim".to_string(),
            19200.0,
            [0.0, 0.0, 0.0, 160.0, 40.0, 3.0],
        ),
        (
            "oep-feature-hub-mirror".to_string(),
            19200.0,
            [20.0, 0.0, 0.0, 180.0, 40.0, 3.0],
        ),
    ]
}

fn assert_volume(actual: f64, expected: f64, what: &str) {
    let rel = (actual - expected).abs() / expected.max(1e-300);
    assert!(
        rel < 1e-6,
        "{what}: volume {actual} not within 1e-6 of {expected} (rel {rel:e})"
    );
}

/// Replay a graph, cross-check every solid step against the closed-form
/// expectations, and return the per-step measurements as a JSON map.
fn replay_and_measure(
    name: &str,
    g: &FeatureGraph,
    expect: &[(String, f64, [f64; 6])],
    mut ev: KernelEvaluator,
) -> serde_json::Map<String, serde_json::Value> {
    g.validate().expect("graph validates");
    let result = g.evaluate(&mut ev);
    assert!(
        result.error.is_none(),
        "{name}: full replay failed: {:?}",
        result.error
    );
    let mut steps = serde_json::Map::new();
    let mut solids: Vec<(&str, SolidMeasures, [f64; 6])> = Vec::new();
    for step in &result.trace {
        if let KernelValue::Solid(sol) = &step.output {
            let bb = ev.bbox(sol).expect("bbox");
            let b = [
                bb.min[0], bb.min[1], bb.min[2], bb.max[0], bb.max[1], bb.max[2],
            ];
            steps.insert(
                step.id.clone(),
                serde_json::json!({
                    "volume": sol.measures.volume,
                    "surface_area": sol.measures.surface_area,
                    "face_count": sol.measures.face_count,
                    "edge_count": sol.measures.edge_count,
                    "vertex_count": sol.measures.vertex_count,
                    "bbox": {
                        "xmin": b[0], "ymin": b[1], "zmin": b[2],
                        "xmax": b[3], "ymax": b[4], "zmax": b[5],
                    },
                }),
            );
            solids.push((step.id.as_str(), sol.measures, b));
        }
    }
    assert_eq!(
        solids.len(),
        expect.len(),
        "{name}: solid steps {}/{}",
        solids.len(),
        expect.len()
    );
    for ((id, measures, bb), (wid, vol, wbb)) in solids.iter().zip(expect.iter()) {
        assert_eq!(id, wid, "{name}: step order/id mismatch");
        assert_volume(measures.volume, *vol, &format!("{name} {id}"));
        for i in 0..6 {
            assert!(
                (bb[i] - wbb[i]).abs() < 1e-6,
                "{name} {id}: bbox {bb:?} vs analytic {wbb:?}"
            );
        }
    }
    steps
}

fn main() -> ExitCode {
    let out_dir = std::env::args()
        .nth(1)
        .expect("usage: gen_kernel_replay_golden <output-dir>");

    // ---- acceptance story: real solids + byte-locked canonical trace -------
    let acceptance = build_acceptance_graph();
    let md = replay_and_measure(
        "acceptance",
        &acceptance,
        &expectations("oep-feature-shelf-body".to_string()),
        KernelEvaluator::new(tol()).expect("kernel evaluator"),
    );

    let mut ev = KernelEvaluator::new(tol()).expect("trace evaluator");
    let trace_result = acceptance.evaluate(&mut ev);
    assert!(trace_result.error.is_none());
    let trace_text = ev.canonical_trace(&trace_result.trace);

    let graph_text = acceptance.to_text().expect("acceptance graph text");

    // ---- bracket story: the mini vocabulary as REAL solids ----------------
    let bracket = build_bracket();
    let bracket_md = replay_and_measure(
        "bracket",
        &bracket,
        &expectations("oep-feature-bracket-body".to_string()),
        KernelEvaluator::new(tol()).expect("bracket evaluator"),
    );

    let dir = PathBuf::from(&out_dir);
    let json = serde_json::json!({
        "format": "oep-kernel-replay-golden",
        "version": 1,
        "generator": "gen_kernel_replay_golden",
        "occt_line": "8.0.x",
        "story": "sketch+extrude+offset+union+trim+mirror over an 80x40x6 footprint (acceptance width 40)",
        "steps": md,
    });
    let json_text = serde_json::to_string_pretty(&json).expect("serialize golden");
    std::fs::write(dir.join("kernel-replay-golden.json"), json_text.as_bytes())
        .expect("write kernel-replay-golden.json");

    let bracket_json = serde_json::json!({
        "format": "oep-kernel-replay-bracket-golden",
        "version": 1,
        "generator": "gen_kernel_replay_golden",
        "occt_line": "8.0.x",
        "story": "mini-vocabulary bracket (Extrude/Offset/Union/Trim/Mirror) as REAL solids",
        "steps": bracket_md,
    });
    let bracket_text = serde_json::to_string_pretty(&bracket_json).expect("serialize bracket");
    std::fs::write(
        dir.join("kernel-replay-bracket-golden.json"),
        bracket_text.as_bytes(),
    )
    .expect("write kernel-replay-bracket-golden.json");

    std::fs::write(
        dir.join("kernel-replay-trace.oepkrt"),
        trace_text.as_bytes(),
    )
    .expect("write kernel-replay-trace.oepkrt");
    std::fs::write(
        dir.join("acceptance-replay-graph.oepfg"),
        graph_text.as_bytes(),
    )
    .expect("write acceptance-replay-graph.oepfg");

    println!("wrote 4 files under {}", dir.display());
    println!("{}", json_text);
    println!("--- trace ({} bytes) ---", trace_text.len());
    print!("{}", trace_text);
    ExitCode::SUCCESS
}
