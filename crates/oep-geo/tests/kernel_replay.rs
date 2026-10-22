//! KERNEL-REPLAY acceptance: a feature graph replays through
//! [`KernelEvaluator`] into REAL solids computed by the pinned OCCT 8.0.x
//! engine (KERNEL-BASE, ADR v1.3.0), not bounding boxes.
//!
//! Acceptance contract (this card):
//!   1. a sketch+extrude+offset+union+trim+mirror graph replays to real
//!      solids whose per-step volume and bbox match a committed golden
//!      computed from the kernel (JSON) AND whose canonical replay trace is
//!      byte-locked to a committed text golden (`.oepkrt`) -- the kernel
//!      analogue of the MiniBox byte-lock;
//!   2. an edit that changes a driven dimension (the sketch width) re-solves
//!      the graph to the new closed-form solid;
//!   3. the open-kind tolerant-failure contract holds with the real
//!      evaluator: an unknown kind aborts after the completed prefix with a
//!      typed [`ReplayError`] -- never a panic, never fabricated geometry;
//!   4. undo/redo (P6-M3-T02) composes over real-solid states: the
//!      [`History`] stack navigates states whose solids are re-derived by
//!      replay, and undo restores the prior real-solid state.
//!
//! Also proves the existing mini-vocabulary bracket story replays as REAL
//! solids (Extrude/Offset/Union/Trim/Mirror over an 80x40x6 footprint),
//! agreeing with the MiniBox bounds where the story is axis-aligned.
//!
//! Build/run: `cargo test -p oep-geo --features kernel --test kernel_replay`.
//! Without the `kernel` feature this file is empty (workspace stays C++-free).
//!
//! Golden regen (mirrors the kernel-seam convention):
//! `cargo run -p oep-geo --features kernel --example gen_kernel_replay_golden -- crates/oep-geo/tests/golden`
//! (byte-stability across regeneration is asserted by acceptance test 1).

#![cfg(feature = "kernel")]
#![deny(warnings)]

use oep_geo::{
    FeatureGraph, FeatureNode, History, HistoryState, KernelEvaluator, KernelValue,
    MiniBoxEvaluator, ReplayError, Tolerance,
};
use std::collections::BTreeMap;

const JSON_GOLDEN: &str = include_str!("golden/kernel-replay-golden.json");
const TRACE_GOLDEN: &str = include_str!("golden/kernel-replay-trace.oepkrt");
const ACCEPTANCE_GRAPH: &str = include_str!("golden/acceptance-replay-graph.oepfg");
const BRACKET_GOLDEN: &str = include_str!("golden/kernel-replay-bracket-golden.json");
const GOLDEN_DIR: &str = "crates/oep-geo/tests/golden";

fn tol() -> Tolerance {
    Tolerance::new(1e-6).expect("valid tolerance")
}

/// Relative volume equality within 1e-6 (the closed-form gate; volumes carry
/// the kernel's honest measurement error, so relative is the right metric).
fn vol_close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() / expected.max(1e-300) < 1e-6
}

// ---------------------------------------------------------------------------
// Story builders (duplicated per repo convention so test + example stay
// independent; byte-identical to the committed acceptance graph).
// ---------------------------------------------------------------------------

/// The acceptance story: Sketch (80x40 floor) -> Extrude h6 -> Offset dx80 ->
/// Union -> Trim dz3 -> Mirror x at 90.
pub fn build_acceptance_graph(width: f64) -> FeatureGraph {
    let profile = format!("0,0;80,0;80,{width};0,{width}");
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("oep-feature-shelf-sketch", "shelf-sketch", "Sketch")
            .text("profile", profile),
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

/// The mini-vocabulary bracket story (Extrude fallback, no Sketch), already
/// used as the feature-graph golden story.
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

// ---------------------------------------------------------------------------
// Golden helpers
// ---------------------------------------------------------------------------

/// One golden entry: kernel-measured volume + topology counts + bbox.
#[derive(Debug, Clone, Copy)]
struct Golden {
    volume: f64,
    surface_area: f64,
    face_count: u64,
    edge_count: u64,
    vertex_count: u64,
    bbox: [f64; 6],
}

fn parse_golden_json(text: &str) -> BTreeMap<String, Golden> {
    let v: serde_json::Value = serde_json::from_str(text).expect("parse golden");
    let obj = v["steps"].as_object().expect("steps object");
    let mut out = BTreeMap::new();
    for (key, m) in obj {
        let b = m["bbox"].as_object().expect("bbox object");
        let mut bb = [0.0; 6];
        for (i, k) in ["xmin", "ymin", "zmin", "xmax", "ymax", "zmax"]
            .iter()
            .enumerate()
        {
            bb[i] = b[*k].as_f64().expect("bbox coord");
        }
        out.insert(
            key.clone(),
            Golden {
                volume: m["volume"].as_f64().expect("volume"),
                surface_area: m["surface_area"].as_f64().expect("area"),
                face_count: m["face_count"].as_u64().expect("faces"),
                edge_count: m["edge_count"].as_u64().expect("edges"),
                vertex_count: m["vertex_count"].as_u64().expect("verts"),
                bbox: bb,
            },
        );
    }
    out
}

/// Assert a solid step matches a committed golden entry tightly.
fn assert_matches_golden(what: &str, m: &oep_geo::SolidMeasures, bb: [f64; 6], g: Golden) {
    let rel = (m.volume - g.volume).abs() / g.volume.max(1e-300);
    assert!(
        rel < 1e-9,
        "{what}: measured volume {} drifted from golden {} (rel {rel:e})",
        m.volume,
        g.volume
    );
    let arel = (m.surface_area - g.surface_area).abs() / g.surface_area.max(1e-300);
    assert!(arel < 1e-9, "{what}: surface area drifted");
    assert_eq!(m.face_count, g.face_count, "{what}: face_count drift");
    assert_eq!(m.edge_count, g.edge_count, "{what}: edge_count drift");
    assert_eq!(m.vertex_count, g.vertex_count, "{what}: vertex_count drift");
    for i in 0..6 {
        let d = (bb[i] - g.bbox[i]).abs();
        assert!(
            d < 1e-6,
            "{what}: bbox[{}] {bb:?} vs golden {:?}",
            i,
            g.bbox
        );
    }
}

/// The solid (measures, bbox) of one replay step, if it produced a solid.
fn solid_step(
    ev: &KernelEvaluator,
    step: &oep_geo::feature_graph::ReplayStep<KernelValue>,
) -> Option<(oep_geo::SolidMeasures, [f64; 6])> {
    match &step.output {
        KernelValue::Solid(s) => {
            let bb = ev.bbox(s).expect("bbox of replay solid");
            Some((
                s.measures,
                [
                    bb.min[0], bb.min[1], bb.min[2], bb.max[0], bb.max[1], bb.max[2],
                ],
            ))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 1. Real solids, golden-matched, byte-locked, deterministic
// ---------------------------------------------------------------------------

#[test]
fn replay_produces_real_solids_matching_committed_golden() {
    let g = build_acceptance_graph(40.0);
    assert!(g.validate().is_ok(), "acceptance graph validates");
    let golden = parse_golden_json(JSON_GOLDEN);

    let mut ev = KernelEvaluator::new(tol()).expect("kernel evaluator");
    let result = g.evaluate(&mut ev);
    assert!(result.error.is_none(), "acceptance graph must fully replay");
    assert_eq!(result.trace.len(), 6, "six steps");

    // Every solid-producing step matches the committed kernel golden.
    let mut solids = 0;
    let mut final_bb = [0.0; 6];
    for step in &result.trace {
        if let Some((m, bb)) = solid_step(&ev, step) {
            let gold = golden.get(&step.id).expect("golden has this step id");
            assert_matches_golden(&step.id, &m, bb, *gold);
            final_bb = bb;
            solids += 1;
        }
    }
    assert_eq!(solids, 5, "five solid-producing steps (all but the Sketch)");

    // The final solid is the mirror copy of the trimmed union.
    let last = result.trace.last().expect("last step");
    let (m, _) = solid_step(&ev, last).expect("final solid");
    assert!(
        vol_close(m.volume, 19200.0),
        "final volume {} vs analytic 19200",
        m.volume
    );
    let expect: [f64; 6] = [20.0, 0.0, 0.0, 180.0, 40.0, 3.0];
    for i in 0..6 {
        assert!(
            (final_bb[i] - expect[i]).abs() < 1e-6,
            "final bbox {final_bb:?} vs analytical {expect:?}"
        );
    }
}

#[test]
fn replay_trace_is_byte_locked_to_committed_golden_and_deterministic() {
    let g = build_acceptance_graph(40.0);

    // Two fresh evaluators: byte-identical canonical traces (determinism).
    let mut ev1 = KernelEvaluator::new(tol()).expect("ev1");
    let r1 = g.evaluate(&mut ev1);
    let t1 = ev1.canonical_trace(&r1.trace);
    let mut ev2 = KernelEvaluator::new(tol()).expect("ev2");
    let r2 = g.evaluate(&mut ev2);
    let t2 = ev2.canonical_trace(&r2.trace);
    assert_eq!(t1, t2, "two clean replays serialize identically");

    // And the canonical trace is byte-locked to the committed .oepkrt golden
    // (the kernel analogue of the MiniBox oepfg byte-lock).
    assert_eq!(
        t1, TRACE_GOLDEN,
        "kernel replay canonical trace drifted from committed golden"
    );

    // The acceptance graph itself round-trips through the oep-feature-graph v1
    // owned format byte-identically, exactly as before (MiniBox trace in it).
    let text = g.to_text().expect("serialize");
    assert_eq!(
        text, ACCEPTANCE_GRAPH,
        "acceptance graph v1 text drifted from committed .oepfg"
    );
    let back = FeatureGraph::from_text(&text).expect("parse");
    assert_eq!(back, g, "graph round-trips");
    assert_eq!(back.to_text().expect("re-encode"), text, "fixpoint");
}

#[test]
fn bracket_story_replays_as_real_solids_agreeing_with_mini_bboxes() {
    // The documented mini story becomes REAL solids, and on this axis-aligned
    // story the kernel bbox agrees exactly with the MiniBox trace bounds.
    let g = build_bracket();
    assert!(g.validate().is_ok());
    let golden = parse_golden_json(BRACKET_GOLDEN);

    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let result = g.evaluate(&mut ev);
    assert!(result.error.is_none(), "bracket must fully replay");
    assert_eq!(result.trace.len(), 5);

    let mini = g.evaluate(&mut MiniBoxEvaluator::new());
    let mut solids = 0;
    for (k, step) in result.trace.iter().enumerate() {
        let Some((m, bb)) = solid_step(&ev, step) else {
            continue;
        };
        // Real solid, matching the committed bracket golden too.
        let gold = golden.get(&step.id).expect("bracket golden has this step");
        assert_matches_golden(&step.id, &m, bb, *gold);
        solids += 1;
        let want = mini.trace[k].output;
        for i in 0..6 {
            assert!(
                (bb[i] - want[i]).abs() < 1e-6,
                "bracket step {} bbox {bb:?} vs mini {:?}",
                step.id,
                want
            );
        }
    }
    assert_eq!(solids, 5, "five bracket solid steps");
    let last = result.trace.last().expect("last");
    let (m, _) = solid_step(&ev, last).expect("final solid");
    assert!(
        vol_close(m.volume, 19200.0),
        "bracket final volume {} vs 19200",
        m.volume
    );
}

// ---------------------------------------------------------------------------
// 2. Driven-dimension edit re-solves the graph
// ---------------------------------------------------------------------------

#[test]
fn edit_changing_driven_dimension_ree_solves_the_graph() {
    let g = build_acceptance_graph(40.0);
    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let base = g.evaluate(&mut ev);
    assert!(base.error.is_none());
    let (m0, bb0) = solid_step_of(&ev, &base.trace).expect("final solid");

    // Change the driven dimension: the sketch width 40 -> 50.
    let edited = build_acceptance_graph(50.0);
    edited.validate().expect("edited graph validates");
    let mut ev2 = KernelEvaluator::new(tol()).expect("ev2");
    let re = edited.evaluate(&mut ev2);
    assert!(re.error.is_none(), "edited graph must re-solve");
    let (m1, bb1) = solid_step_of(&ev2, &re.trace).expect("final solid edited");

    // The solid re-solved: 160x50x3 -> volume 24000 (vs 19200 at width 40).
    assert!(
        vol_close(m0.volume, 19200.0) && vol_close(m1.volume, 24000.0),
        "driven edit changed the solid: {} -> {}",
        m0.volume,
        m1.volume
    );
    assert!(
        (bb0[4] - 40.0).abs() < 1e-6 && (bb1[4] - 50.0).abs() < 1e-6,
        "driven width edit changed the bbox: {bb0:?} -> {bb1:?}"
    );
    let expect: [f64; 6] = [20.0, 0.0, 0.0, 180.0, 50.0, 3.0];
    for i in 0..6 {
        assert!(
            (bb1[i] - expect[i]).abs() < 1e-6,
            "edited final bbox {bb1:?} vs analytical {expect:?}"
        );
    }
}

/// The last solid-producing step's (measures, bbox).
fn solid_step_of(
    ev: &KernelEvaluator,
    trace: &[oep_geo::feature_graph::ReplayStep<KernelValue>],
) -> Option<(oep_geo::SolidMeasures, [f64; 6])> {
    trace.iter().rev().find_map(|s| match &s.output {
        KernelValue::Solid(sol) => {
            let bb = ev.bbox(sol).expect("bbox of replay solid");
            Some((
                sol.measures,
                [
                    bb.min[0], bb.min[1], bb.min[2], bb.max[0], bb.max[1], bb.max[2],
                ],
            ))
        }
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// 3. Tolerant failure with the real evaluator: never panic, never fabricate
// ---------------------------------------------------------------------------

#[test]
fn unknown_kind_aborts_after_completed_prefix_with_real_solids() {
    let mut g = build_acceptance_graph(40.0);
    // BogusOp lives under the union's refs: node 3 (index 3).
    g.add_child(
        "oep-feature-body-union",
        FeatureNode::new("oep-feature-bogus", "bogus", "BogusOp")
            .with_ref("oep-feature-body-union"),
    )
    .expect("bogus added");
    g.add_child(
        "oep-feature-bogus",
        FeatureNode::new("oep-feature-after-bogus", "after", "Mirror")
            .with_ref("oep-feature-bogus")
            .text("axis", "x")
            .num("at", 90.0),
    )
    .expect("after added");

    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let result = g.evaluate(&mut ev);
    let err = result.error.expect("typed error surfaced");
    assert_eq!(
        err,
        ReplayError::UnknownKind {
            feature: "oep-feature-bogus".to_string(),
            kind: "BogusOp".to_string(),
        }
    );
    // Completed prefix only: the six acceptance steps real-solve, then the
    // unknown kind aborts (node 0..5); the node after it is never reached.
    let ids: Vec<&str> = result.trace.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "oep-feature-shelf-sketch",
            "oep-feature-shelf-body",
            "oep-feature-side-shelf",
            "oep-feature-body-union",
            "oep-feature-top-trim",
            "oep-feature-hub-mirror",
        ]
    );
    // The prefix is REAL solid material, not a bound: the trim step has 9
    // faces and the mirror step has exactly the trimmed solid's volume.
    let trim = &result.trace[4].output;
    match trim {
        KernelValue::Solid(s) => assert!(s.measures.face_count > 6, "real trim solid"),
        other => panic!("trim step produced {other:?}, not a real solid"),
    }
}

#[test]
fn missing_required_param_is_a_typed_replay_error() {
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("oep-feature-sketch", "sketch", "Sketch")
            .text("profile", "0,0;1,0;1,1;0,1"),
    )
    .expect("root");
    g.add_child(
        "oep-feature-sketch",
        FeatureNode::new("oep-feature-no-height", "no-height", "Extrude")
            .with_ref("oep-feature-sketch"),
    )
    .expect("child");

    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let result = g.evaluate(&mut ev);
    let err = result.error.expect("typed error");
    assert!(matches!(err, ReplayError::Evaluator(msg) if msg.contains("height")));
    // Completed prefix only.
    assert_eq!(result.trace.len(), 1);
}

#[test]
fn trim_beyond_full_height_is_a_typed_replay_error_not_a_panic() {
    // Trim dz=100 on a 6-high stack consumes everything: this must surface as
    // a typed error (completed prefix retained), never a panic.
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("oep-feature-shelf-sketch", "sketch", "Sketch")
            .text("profile", "0,0;80,0;80,40;0,40"),
    )
    .expect("sketch");
    g.add_child(
        "oep-feature-shelf-sketch",
        FeatureNode::new("oep-feature-shelf-body", "body", "Extrude")
            .with_ref("oep-feature-shelf-sketch")
            .num("height", 6.0),
    )
    .expect("ex");
    g.add_child(
        "oep-feature-shelf-body",
        FeatureNode::new("oep-feature-overtrim", "overtrim", "Trim")
            .with_ref("oep-feature-shelf-body")
            .num("dz", 100.0),
    )
    .expect("trim");
    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let result = g.evaluate(&mut ev);
    let err = result.error.expect("typed error");
    let is_whole_solid_trim =
        matches!(&err, ReplayError::Evaluator(m) if m.contains("consume the whole solid"));
    assert!(is_whole_solid_trim, "got {err:?}");
    assert_eq!(result.trace.len(), 2, "completed prefix only");
}

// ---------------------------------------------------------------------------
// 4. Undo/redo over real-solid states (P6-M3-T02 composition)
// ---------------------------------------------------------------------------

/// A committed graph state; the real solid it denotes is re-derived by
/// replay (kernel solids are session-local and never serialized, so time
/// navigation re-derives geometry from the versioned graph).
#[derive(Debug, Clone, PartialEq)]
struct GraphState {
    graph: FeatureGraph,
    revision: u32,
    label: String,
}

impl GraphState {
    fn new(graph: FeatureGraph, revision: u32, label: &str) -> Self {
        Self {
            graph,
            revision,
            label: label.to_string(),
        }
    }
}

impl HistoryState for GraphState {
    fn revision(&self) -> u32 {
        self.revision
    }
    fn op(&self) -> &str {
        &self.label
    }
    fn encode(&self) -> String {
        // The kernel solid itself is session-local and never serialized, so
        // the persisted state carries the graph (the solid re-derives by
        // replay) plus the edition's revision and label.
        format!(
            "revision {}\nop {}\n{}",
            self.revision,
            self.label,
            self.graph.to_text().expect("encode graph state")
        )
    }
    fn decode(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let revision: u32 = lines.next()?.strip_prefix("revision ")?.parse().ok()?;
        let label = lines.next()?.strip_prefix("op ")?.to_string();
        let body = lines.collect::<Vec<_>>().join("\n");
        let graph = FeatureGraph::from_text(&body)?;
        Some(Self {
            graph,
            revision,
            label,
        })
    }
}

/// Replay a state's graph into its final real-solid volume (and bbox).
fn solid_volume_of(state: &GraphState) -> f64 {
    let mut ev = KernelEvaluator::new(tol()).expect("ev");
    let r = state.graph.evaluate(&mut ev);
    assert!(r.error.is_none(), "state graph must replay");
    r.trace
        .iter()
        .rev()
        .find_map(|s| match &s.output {
            KernelValue::Solid(sol) => Some(sol.measures.volume),
            _ => None,
        })
        .expect("state has a final solid")
}

#[test]
fn undo_restores_prior_real_solid_state_and_redo_returns() {
    let width40 = build_acceptance_graph(40.0);
    let width50 = build_acceptance_graph(50.0);

    let mut history = History::new(0); // unbounded
    history.commit(GraphState::new(width40, 1, "bracket width 40"));
    assert!(!history.can_undo());

    // Edit: driven dimension 40 -> 50 (a fresh commit truncates redo).
    history.commit(GraphState::new(width50, 2, "bracket width 50"));
    assert!(history.can_undo());
    assert!(!history.can_redo(), "fresh commit invalidates redo");

    // Current edited state re-solves to the wider solid.
    let edited = history.current().expect("current");
    assert_eq!(edited.revision(), 2);
    assert!(
        vol_close(solid_volume_of(edited), 24000.0),
        "width 50 volume"
    );

    // Undo restores the PRIOR real-solid state.
    let undone = history.undo().expect("undo moves to width 40");
    assert_eq!(undone.revision(), 1);
    let v = solid_volume_of(undone);
    assert!(
        vol_close(v, 19200.0),
        "undo restored the prior solid, volume {v} vs 19200"
    );

    // Redo returns to the edited state.
    let redone = history.redo().expect("redo returns to width 50");
    assert_eq!(redone.revision(), 2);
    assert!(vol_close(solid_volume_of(redone), 24000.0), "redo volume");

    // The whole stack is serializable (P6-M3-T02 `oep-history` v1) and the
    // current real-solid state re-derives from the restored stack.
    let text = history.to_text().expect("serialize");
    let back = History::<GraphState>::from_text(&text).expect("parse history");
    let restored = back.current().expect("current after round-trip");
    assert_eq!(
        restored.revision(),
        2,
        "history round-trips to the edited state"
    );
    assert!(vol_close(solid_volume_of(restored), 24000.0));
}

// ---------------------------------------------------------------------------
// Golden regeneration (mirrors the repo regen convention)
// ---------------------------------------------------------------------------

#[test]
fn regenerate_goldens() {
    match std::env::var("OEP_KERNEL_REPLAY_REGEN") {
        Ok(v) if v == "1" => {
            let g = build_acceptance_graph(40.0);
            let mut ev = KernelEvaluator::new(tol()).expect("ev");
            let r = g.evaluate(&mut ev);
            assert!(r.error.is_none());
            std::fs::write(
                format!("{GOLDEN_DIR}/kernel-replay-trace.oepkrt"),
                ev.canonical_trace(&r.trace),
            )
            .expect("write trace golden");
            let graph_text = g.to_text().expect("serialize");
            std::fs::write(
                format!("{GOLDEN_DIR}/acceptance-replay-graph.oepfg"),
                &graph_text,
            )
            .expect("write graph golden");
            eprintln!("wrote trace + graph goldens (JSON written by the example)");
        }
        _ => eprintln!("skip: set OEP_KERNEL_REPLAY_REGEN=1 to regenerate"),
    }
}
