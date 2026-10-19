//! P6-M3-T03 acceptance contract for the feature graph (history) model with
//! replay — a mini feature tree for native open models (RED).
//!
//! Written BEFORE `oep_geo::feature_graph` exists; against the skeleton crate
//! this does not compile, proving TDD RED. Locks the public API + behaviors:
//!
//! 1. A [`FeatureGraph`] is a rooted **mini feature tree** in creation order:
//!    every node has an id, name, an **open** PascalCase `kind` vocabulary,
//!    an optional `parent`, explicit `refs` (dependency inputs) and sorted
//!    `(name, value)` params. Building helpers (`add_root` / `add_child`)
//!    refuse duplicate ids and unknown parents; `validate` additionally
//!    rejects unknown/forward `refs` (strictly-backward invariant ⇒ the graph
//!    is a DAG and replay is deterministic without a topological sort).
//! 2. **Replay**: `evaluate` walks the tree in creation order, resolves each
//!    node's inputs (refs' outputs, else parent's output, else the evaluator's
//!    `start_value` for roots) and calls the pluggable [`Evaluator`]. The
//!    reference [`MiniBoxEvaluator`] folds each feature into a bbox `[f64;6]`;
//!    the bracket story below has known-good, closed-form outputs that the
//!    stdlib Python acceptance test independently re-derives from the graph.
//!    Replay is deterministic (same graph × same evaluator ⇒ identical trace)
//!    and tolerant: an unknown kind aborts **after** the completed prefix and
//!    returns the partial trace instead of panicking.
//! 3. **Own-format serialization** (`oep-feature-graph` v1): graph + canonical
//!    replay trace round-trip to a deterministic text form, are byte-identical
//!    against the committed golden produced by this crate's example generator,
//!    and re-encode to the exact same bytes. Decoding is strict and never
//!    panics: malformed structure, bad counts, non-finite numbers, empty ids
//!    all yield `None` — while an unknown `kind` still decodes (open
//!    vocabulary, per P2-M1-T04 tolerant-failure doctrine).
//!
//! The committed golden (`tests/golden/bracket-feature-graph.oepfg`) is
//! produced BY this crate (example `gen_feature_graph_golden`) and
//! independently re-derived by `tests/test_oep_geo_feature_graph_acceptance.py`
//! (Python stdlib), so a self-consistent but wrong evaluator cannot satisfy
//! both sides.

use oep_geo::feature_graph::{
    Evaluator, FeatureGraph, FeatureNode, GraphError, MiniBoxEvaluator, ParamValue, ReplayError,
};

/// Path of the committed golden (kept in sync with the acceptance test's
/// GOLDEN constant).
const GOLDEN_PATH: &str = "crates/oep-geo/tests/golden/bracket-feature-graph.oepfg";

/// Raw bytes of the committed golden (compile-time, absent until GREEN).
const GOLDEN: &str = include_str!("golden/bracket-feature-graph.oepfg");

/// The bracket part's mini feature tree (native open model story, not CATIA
/// semantics). Every node's output bbox is closed-form and re-verified by the
/// Python acceptance test.
///
///   0 Extrude 80x40x6                    -> [0,0,0,80,40,6]
///   1 Offset  dx=80 (child of 0)         -> [80,0,0,160,40,6]
///   2 Union   0 + 1                      -> [0,0,0,160,40,6]
///   3 Trim    dz=3 (child of 2)          -> [0,0,0,160,40,3]
///   4 Mirror  about x at=90 (child of 3) -> [20,0,0,180,40,3]
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

/// Known-good replay trace for the bracket (closed form, also in the golden).
fn expected_trace() -> Vec<[f64; 6]> {
    vec![
        [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
        [80.0, 0.0, 0.0, 160.0, 40.0, 6.0],
        [0.0, 0.0, 0.0, 160.0, 40.0, 6.0],
        [0.0, 0.0, 0.0, 160.0, 40.0, 3.0],
        [20.0, 0.0, 0.0, 180.0, 40.0, 3.0],
    ]
}

// ---------------------------------------------------------------------------
// build helpers: graph structure
// ---------------------------------------------------------------------------

#[test]
fn builds_bracket_tree_with_parent_and_ref_edges() {
    let g = build_bracket();
    assert_eq!(g.len(), 5);
    assert!(g.validate().is_ok(), "bracket graph must validate");

    // The tree: root bracket-body owns the shelf and the union;
    // the union owns the trim; the trim owns the mirror.
    let root = g.node("oep-feature-bracket-body").expect("root");
    assert_eq!(root.parent, None);
    assert_eq!(root.kind, "Extrude");

    let shelf = g.node("oep-feature-side-shelf").expect("shelf");
    assert_eq!(shelf.parent.as_deref(), Some("oep-feature-bracket-body"));
    assert_eq!(shelf.refs, vec!["oep-feature-bracket-body".to_string()]);
    assert_eq!(shelf.param_num("dx"), Some(80.0));

    let u = g.node("oep-feature-body-union").expect("union");
    assert_eq!(u.refs.len(), 2);
    assert_eq!(u.refs[0], "oep-feature-bracket-body");
    assert_eq!(u.refs[1], "oep-feature-side-shelf");

    let mirror = g.node("oep-feature-hub-mirror").expect("mirror");
    assert_eq!(mirror.param_str("axis"), Some("x"));
    assert_eq!(mirror.param_num("at"), Some(90.0));
}

#[test]
fn params_are_sorted_and_typed() {
    let node = FeatureNode::new("f", "f", "Extrude")
        .num("z", 1.0)
        .num("a", 2.0)
        .text("plane", "yz");
    let keys: Vec<&str> = node.params.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["a", "plane", "z"], "params stay sorted by key");
    assert_eq!(node.params[1].1, ParamValue::Str("yz".to_string()));
    assert_eq!(node.param_num("z"), Some(1.0));
}

// ---------------------------------------------------------------------------
// build helpers: rejection rules
// ---------------------------------------------------------------------------

#[test]
fn add_root_and_add_child_reject_dup_ids_and_unknown_parents() {
    let mut g = FeatureGraph::new();
    g.add_root(FeatureNode::new("a", "a", "Extrude"))
        .expect("root");
    assert_eq!(
        g.add_root(FeatureNode::new("a", "a2", "Extrude")),
        Err(GraphError::DuplicateId("a".to_string()))
    );
    assert_eq!(
        g.add_child("nope", FeatureNode::new("b", "b", "Extrude")),
        Err(GraphError::UnknownParent {
            child: "b".to_string(),
            parent: "nope".to_string()
        })
    );
    // duplicate id via add_child is also refused
    assert_eq!(
        g.add_child("a", FeatureNode::new("a", "dup", "Extrude")),
        Err(GraphError::DuplicateId("a".to_string()))
    );
}

#[test]
fn validate_rejects_unknown_and_forward_refs() {
    // Unknown ref: build by hand (build helpers never admit it).
    let mut g = FeatureGraph::new();
    g.add_root(FeatureNode::new("n0", "n0", "Extrude"))
        .expect("root");
    g.add_child(
        "n0",
        FeatureNode::new("n1", "n1", "Offset").with_ref("missing"),
    )
    .expect("child");
    assert_eq!(
        g.validate(),
        Err(vec![GraphError::UnknownRef {
            feature: "n1".to_string(),
            reference: "missing".to_string()
        }])
    );

    // Forward ref: hand-construct a graph whose edge points ahead in creation
    // order (breaks the strictly-backward invariant).
    let mut g2 = FeatureGraph::new();
    g2.add_root(FeatureNode::new("a", "a", "Extrude"))
        .expect("r");
    g2.add_root(FeatureNode::new("b", "b", "Offset").with_ref("c"))
        .expect("r2");
    g2.add_root(FeatureNode::new("c", "c", "Trim")).expect("r3");
    assert_eq!(
        g2.validate(),
        Err(vec![GraphError::ForwardRef {
            feature: "b".to_string(),
            reference: "c".to_string()
        }])
    );
}

// ---------------------------------------------------------------------------
// replay: deterministic, closed-form, tolerant
// ---------------------------------------------------------------------------

#[test]
fn replay_produces_known_good_trace_in_creation_order() {
    let g = build_bracket();
    let mut ev = MiniBoxEvaluator::new();
    let result = g.evaluate(&mut ev);
    assert!(result.error.is_none(), "bracket must fully replay");

    let ids: Vec<&str> = result.trace.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "oep-feature-bracket-body",
            "oep-feature-side-shelf",
            "oep-feature-body-union",
            "oep-feature-top-trim",
            "oep-feature-hub-mirror",
        ]
    );

    let got: Vec<[f64; 6]> = result.trace.iter().map(|s| s.output).collect();
    assert_eq!(got, expected_trace(), "mini-bbox evaluator outputs");
}

#[test]
fn replay_is_deterministic() {
    let g = build_bracket();
    let a = g.evaluate(&mut MiniBoxEvaluator::new());
    let b = g.evaluate(&mut MiniBoxEvaluator::new());
    assert_eq!(a.trace, b.trace);
    assert_eq!(a.error, b.error);
}

#[test]
fn parent_feeds_input_when_no_refs() {
    // A child without explicit refs receives the parent's output.
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("base", "base", "Extrude")
            .num("length", 10.0)
            .num("width", 5.0)
            .num("height", 2.0),
    )
    .expect("root");
    g.add_child(
        "base",
        FeatureNode::new("cut", "cut", "Trim").num("dz", 1.0),
    )
    .expect("child");
    let result = g.evaluate(&mut MiniBoxEvaluator::new());
    assert!(result.error.is_none());
    assert_eq!(
        result.trace.iter().map(|s| s.output).collect::<Vec<_>>(),
        vec![
            [0.0, 0.0, 0.0, 10.0, 5.0, 2.0],
            [0.0, 0.0, 0.0, 10.0, 5.0, 1.0]
        ]
    );
}

#[test]
fn unknown_kind_is_tolerant_with_partial_trace() {
    // Node 3 (index 3) has an unknown kind: replay must return the completed
    // prefix (nodes 0..2) plus the error, never panic.
    let mut g = FeatureGraph::new();
    g.add_root(
        FeatureNode::new("n0", "n0", "Extrude")
            .num("length", 1.0)
            .num("width", 1.0)
            .num("height", 1.0),
    )
    .expect("r");
    g.add_child(
        "n0",
        FeatureNode::new("n1", "n1", "Offset")
            .with_ref("n0")
            .num("dx", 1.0)
            .num("dy", 0.0)
            .num("dz", 0.0),
    )
    .expect("c1");
    g.add_child(
        "n1",
        FeatureNode::new("n2", "n2", "Union")
            .with_ref("n0")
            .with_ref("n1"),
    )
    .expect("c2");
    g.add_child("n2", FeatureNode::new("n3", "n3", "BogusOp").with_ref("n2"))
        .expect("c3");
    g.add_child(
        "n3",
        FeatureNode::new("n4", "n4", "Trim")
            .with_ref("n3")
            .num("dz", 1.0),
    )
    .expect("c4");

    let result = g.evaluate(&mut MiniBoxEvaluator::new());
    let err = result.error.expect("unknown kind surfaced");
    assert_eq!(
        err,
        ReplayError::UnknownKind {
            feature: "n3".to_string(),
            kind: "BogusOp".to_string()
        }
    );
    // Completed prefix only: n0, n1, n2 (n4 never reached).
    let ids: Vec<&str> = result.trace.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["n0", "n1", "n2"]);
}

// ---------------------------------------------------------------------------
// serialization: golden byte-lock, round-trip identity, strict decode
// ---------------------------------------------------------------------------

#[test]
fn serializer_matches_committed_golden() {
    let text = build_bracket().to_text().expect("serialize bracket");
    assert_eq!(
        text, GOLDEN,
        "feature-graph serializer drifted from committed golden"
    );
}

#[test]
fn serialize_deserialize_is_identity_and_canonical() {
    let g = build_bracket();
    let text = g.to_text().expect("serialize");
    let back = FeatureGraph::from_text(&text).expect("parse");
    assert_eq!(back, g);
    // Re-encoding the parsed graph is byte-identical (trace is deterministic).
    assert_eq!(back.to_text().expect("re-serialize"), text);
}

#[test]
fn committed_golden_roundtrips() {
    let g = FeatureGraph::from_text(GOLDEN).expect("parse committed golden");
    let result = g.evaluate(&mut MiniBoxEvaluator::new());
    assert!(result.error.is_none());
    let got: Vec<[f64; 6]> = result.trace.iter().map(|s| s.output).collect();
    assert_eq!(got, expected_trace());
    // Canonical fixpoint: re-encoding equals the committed bytes.
    assert_eq!(g.to_text().expect("re-encode"), GOLDEN);
}

/// One-shot generator for the committed golden (run manually, mirrors the
/// P6-M1/P6-M2 regen convention):
/// `cargo run -p oep-geo --example gen_feature_graph_golden -- crates/oep-geo/tests/golden`
#[test]
fn regenerate_golden() {
    match std::env::var("OEP_FEATURE_GRAPH_REGEN") {
        Ok(v) if v == "1" => {
            let text = build_bracket().to_text().expect("serialize");
            std::fs::write(GOLDEN_PATH, text).expect("write golden");
            eprintln!("wrote {GOLDEN_PATH}");
        }
        _ => eprintln!("skip: set OEP_FEATURE_GRAPH_REGEN=1 to (re)generate the golden"),
    }
}

#[test]
fn from_text_rejects_garbage_and_malformed() {
    let bad = [
        "",
        "llama",
        "format oep-geo-math-golden\nversion 1\nnodes 0\ntrace 0\n",
        "format oep-feature-graph\nversion 999\nnodes 0\ntrace 0\n",
        // nodes count does not match body
        "format oep-feature-graph\nversion 1\nnodes 5\ntrace 0\n",
        // missing 'node 0' marker
        "format oep-feature-graph\nversion 1\nnodes 1\nid x\nname x\nkind Extrude\nparent -\nrefs 0\nparams 0\ntrace 0\n",
        // non-sequential node indices
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 1\nid x\nname x\nkind Extrude\nparent -\nrefs 0\nparams 0\ntrace 0\n",
        // refs count mismatch (says 1, gives 0)
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid x\nname x\nkind Extrude\nparent -\nrefs 1\nparams 0\ntrace 0\n",
        // params count mismatch
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid x\nname x\nkind Extrude\nparent -\nrefs 0\nparams 1\ntrace 0\n",
        // non-finite param value
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid x\nname x\nkind Extrude\nparent -\nrefs 0\nparams 1\na=nan\ntrace 0\n",
        // empty id
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid \nname x\nkind Extrude\nparent -\nrefs 0\nparams 0\ntrace 0\n",
        // empty param name
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid x\nname x\nkind Extrude\nparent -\nrefs 0\nparams 1\n=1\ntrace 0\n",
        // trace count mismatch (says 1, gives 0)
        "format oep-feature-graph\nversion 1\nnodes 0\ntrace 1\n",
        // non-finite number inside a trace box
        "format oep-feature-graph\nversion 1\nnodes 0\ntrace 1\nstep 0\nid x\nkind Extrude\nbox inf;0;0;1;1;1\n",
        // junk trailing line
        "format oep-feature-graph\nversion 1\nnodes 0\ntrace 0\njunk\n",
        // blank line anywhere
        "format oep-feature-graph\nversion 1\nnodes 0\n\ntrace 0\n",
    ];
    for (i, case) in bad.iter().enumerate() {
        assert!(
            FeatureGraph::from_text(case).is_none(),
            "case {i} should have decoded to None"
        );
    }
}

#[test]
fn unknown_kind_and_unknown_ids_still_decode() {
    // Open kind vocabulary + tolerant linking: structurally clean text with an
    // unknown kind or unknown ids READS back (semantics deferred to replay /
    // validate, never a decode panic).
    let unknown_kind =
        "format oep-feature-graph\nversion 1\nnodes 1\nnode 0\nid x\nname x\nkind BogusOp\nparent -\nrefs 0\nparams 0\ntrace 1\nstep 0\nid x\nkind BogusOp\nbox 0;0;0;0;0;0\n";
    assert!(FeatureGraph::from_text(unknown_kind).is_some());
}

#[test]
fn evaluator_is_pluggable_with_custom_value_type() {
    // Prove replay is kernel-agnostic: a string-truncating evaluator.
    struct LenEvaluator;
    impl Evaluator for LenEvaluator {
        type Value = usize;
        fn start_value(&mut self) -> usize {
            0
        }
        fn evaluate_node(
            &mut self,
            _node: &FeatureNode,
            inputs: &[usize],
        ) -> Result<usize, ReplayError> {
            let base = inputs.iter().copied().max().unwrap_or(0);
            Ok(base + 1)
        }
    }
    let mut g = FeatureGraph::new();
    g.add_root(FeatureNode::new("a", "a", "X")).expect("r");
    g.add_child("a", FeatureNode::new("b", "b", "Y"))
        .expect("c");
    g.add_child("b", FeatureNode::new("c", "c", "Z").with_ref("b"))
        .expect("c2");
    let r = g.evaluate(&mut LenEvaluator);
    assert!(r.error.is_none());
    let lens: Vec<usize> = r.trace.iter().map(|s| s.output).collect();
    assert_eq!(lens, vec![1, 2, 3], "inputs cascade through the graph");
}
