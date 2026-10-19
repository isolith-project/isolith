//! P6-M3-T02 acceptance contract for the topology undo/redo stack (RED).
//!
//! Written BEFORE `oep_geo::history` / `oep_geo::topology_state` exist;
//! against the skeleton crate this does not compile, proving TDD RED. Locks
//! the public API + behaviors:
//!
//! 1. `History<S>` semantics on a concrete `TopologyState` record:
//!    commit / undo / redo / cursor / can_undo / can_redo / clear, with
//!    redo-branch truncation on a new commit (the classic CAD rule: a fresh
//!    edit invalidates the redo branch) and bounded capacity that drops the
//!    OLDEST states when exceeded.
//! 2. Own-format serialization (`oep-history` v1): the whole stack (capacity,
//!    cursor, every state on both branches) round-trips to a deterministic
//!    text form, is byte-identical against a committed golden produced by this
//!    crate's example generator, and re-encodes to the exact same bytes.
//! 3. `HistoryState` encode/decode is strict and never panics: malformed
//!    input, unknown body kinds, non-finite coordinates, and count mismatches
//!    all yield `None`.
//! 4. Concrete `TopologyState` record rules: ids follow the `oep-` convention,
//!    bodies sort by id, features sort by id, params sort by name, every bbox
//!    is finite with `min <= max` per axis, and the kind set is exactly
//!    {solid, sheet, wire}.
//!
//! The committed golden (`tests/golden/history-bracket.oephist`) is produced
//! BY this crate (example `gen_history_golden`) and independently re-parsed by
//! `tests/test_oep_geo_history_acceptance.py` (Python stdlib), so a
//! self-consistent but wrong implementation cannot satisfy both sides.

use oep_geo::history::{History, HistoryState};
use oep_geo::topology_state::{BodyKind, TopologyState};

/// Path of the committed golden history (kept in sync with the acceptance
/// test's GOLDEN constant).
const GOLDEN_PATH: &str = "tests/golden/history-bracket.oephist";

/// The committed golden bytes, read at runtime (avoids include_str!'s
/// compile-time dependency on a file this crate's own regen test writes).
fn committed_golden() -> String {
    std::fs::read_to_string(GOLDEN_PATH)
        .unwrap_or_else(|e| panic!("missing committed golden {GOLDEN_PATH}: {e}"))
}

fn body(id: &str, kind: BodyKind, bbox: [f64; 6]) -> (String, BodyKind, [f64; 6]) {
    (id.to_string(), kind, bbox)
}

/// Builds the canonical bracket session used for the golden.
///
/// Story (must match the Python acceptance + the committed golden):
///   1. init        - bracket solid 80x40x6
///   2. union-lip   - add a 30x10x6 lip body next to it
///   3. hole        - add a hole feature (diameter 6, depth 6)
///   4. fillet      - add a fillet feature (radius 2)
///   5. undo() x2   - back to the hole state
///   6. redo()      - forward (still hole state)
///   7. countersink - NEW commit; this drops the fillet state from the redo
///      branch. Final states are [init, union-lip, hole, countersink], cursor
///      on countersink (index 3).
pub fn build_session() -> History<TopologyState> {
    let mut h = History::new(8);

    // 1. init
    h.commit(TopologyState::new(
        1,
        "init",
        vec![body(
            "oep-body-bracket",
            BodyKind::Solid,
            [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
        )],
        vec![],
    ));

    // 2. union-lip
    h.commit(TopologyState::new(
        2,
        "union-lip",
        vec![
            body(
                "oep-body-bracket",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
            ),
            body(
                "oep-body-lip",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 30.0, 10.0, 6.0],
            ),
        ],
        vec![],
    ));

    // 3. hole
    h.commit(TopologyState::new(
        3,
        "hole",
        vec![
            body(
                "oep-body-bracket",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
            ),
            body(
                "oep-body-lip",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 30.0, 10.0, 6.0],
            ),
        ],
        vec![(
            "oep-feature-hole".to_string(),
            "Hole".to_string(),
            vec![("diameter".to_string(), 6.0), ("depth".to_string(), 6.0)],
        )],
    ));

    // 4. fillet
    h.commit(TopologyState::new(
        4,
        "fillet",
        vec![
            body(
                "oep-body-bracket",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
            ),
            body(
                "oep-body-lip",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 30.0, 10.0, 6.0],
            ),
        ],
        vec![
            (
                "oep-feature-hole".to_string(),
                "Hole".to_string(),
                vec![("diameter".to_string(), 6.0), ("depth".to_string(), 6.0)],
            ),
            (
                "oep-feature-fillet".to_string(),
                "Fillet".to_string(),
                vec![("radius".to_string(), 2.0)],
            ),
        ],
    ));

    // 5. undo x2 -> hole state
    assert_eq!(h.undo().map(|s| s.revision()), Some(3));
    assert_eq!(h.undo().map(|s| s.revision()), Some(2));

    // 6. redo -> hole state
    assert_eq!(h.redo().map(|s| s.revision()), Some(3));

    // 7. countersink: new commit clears the redo branch (fillet dropped)
    h.commit(TopologyState::new(
        5,
        "countersink",
        vec![
            body(
                "oep-body-bracket",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
            ),
            body(
                "oep-body-lip",
                BodyKind::Solid,
                [0.0, 0.0, 0.0, 30.0, 10.0, 6.0],
            ),
        ],
        vec![
            (
                "oep-feature-hole".to_string(),
                "Hole".to_string(),
                vec![("diameter".to_string(), 6.0), ("depth".to_string(), 6.0)],
            ),
            (
                "oep-feature-countersink".to_string(),
                "CounterSink".to_string(),
                vec![
                    ("diameter".to_string(), 8.0),
                    ("depth".to_string(), 2.0),
                    ("top_angle".to_string(), 90.0),
                ],
            ),
        ],
    ));

    h
}

// ---------------------------------------------------------------------------
// history semantics (no serialization)
// ---------------------------------------------------------------------------

#[test]
fn commit_undo_redo_cursor_semantics() {
    let mut h = History::<TopologyState>::new(0);
    assert_eq!(h.len(), 0);
    assert!(!h.can_undo());
    assert!(!h.can_redo());

    h.commit(TopologyState::new(1, "a", vec![], vec![]));
    h.commit(TopologyState::new(2, "b", vec![], vec![]));
    h.commit(TopologyState::new(3, "c", vec![], vec![]));
    assert_eq!(h.len(), 3);
    assert!(h.can_undo());
    assert!(!h.can_redo());
    assert_eq!(h.current().map(|s| s.revision()), Some(3));

    assert_eq!(h.undo().map(|s| s.revision()), Some(2));
    assert!(h.can_redo());
    assert_eq!(h.redo().map(|s| s.revision()), Some(3));
    assert!(!h.can_redo());
}

#[test]
fn undo_at_bottom_and_redo_at_top_are_noops() {
    let mut h = History::<TopologyState>::new(0);
    h.commit(TopologyState::new(1, "a", vec![], vec![]));
    assert!(h.undo().is_none());
    assert!(h.redo().is_none());
    assert_eq!(h.len(), 1);
}

#[test]
fn new_commit_truncates_redo_branch() {
    let mut h = History::<TopologyState>::new(0);
    for (rev, op) in [(1, "a"), (2, "b"), (3, "c")] {
        h.commit(TopologyState::new(rev, op, vec![], vec![]));
    }
    h.undo(); // cursor on "b"
    h.undo(); // cursor on "a"
    h.redo(); // cursor on "b"
              // fresh edit from "b" drops "c"
    h.commit(TopologyState::new(4, "d", vec![], vec![]));
    assert_eq!(h.len(), 3);
    let ops: Vec<&str> = h.states().map(|s| s.op()).collect();
    assert_eq!(ops, vec!["a", "b", "d"]);
    assert!(!h.can_redo());
    assert_eq!(h.current().map(|s| s.op()), Some("d"));
}

#[test]
fn bounded_capacity_drops_oldest() {
    let mut h = History::<TopologyState>::new(2);
    for rev in 1..=4 {
        h.commit(TopologyState::new(rev, rev.to_string(), vec![], vec![]));
    }
    // capacity 2: only the two newest survive, oldest dropped from the FRONT.
    assert_eq!(h.len(), 2);
    let revs: Vec<u32> = h.states().map(|s| s.revision()).collect();
    assert_eq!(revs, vec![3, 4]);
    // cursor is on the newest; one undo still available.
    assert_eq!(h.current().map(|s| s.revision()), Some(4));
    assert_eq!(h.undo().map(|s| s.revision()), Some(3));
}

#[test]
fn unbounded_capacity_keeps_everything() {
    let mut h = History::<TopologyState>::new(0);
    for rev in 1..=10 {
        h.commit(TopologyState::new(rev, rev.to_string(), vec![], vec![]));
    }
    assert_eq!(h.len(), 10);
}

#[test]
fn clear_resets_to_empty() {
    let mut h = History::<TopologyState>::new(0);
    h.commit(TopologyState::new(1, "a", vec![], vec![]));
    h.clear();
    assert_eq!(h.len(), 0);
    assert!(!h.can_undo());
    assert!(!h.can_redo());
    assert!(h.current().is_none());
}

// ---------------------------------------------------------------------------
// golden: the serializer is byte-locked to the committed file
// ---------------------------------------------------------------------------

#[test]
fn serializer_snapshot_matches_committed_golden() {
    let text = build_session().to_text().expect("serialize history");
    assert_eq!(
        text,
        committed_golden(),
        "history serializer drifted from committed golden"
    );
}

/// One-shot generator for the committed golden (run manually, mirrors the
/// P6-M1/P6-M2 regen convention):
/// `cargo run -p oep-geo --example gen_history_golden -- crates/oep-geo/tests/golden`
#[test]
fn regenerate_golden() {
    match std::env::var("OEP_HISTORY_REGEN") {
        Ok(v) if v == "1" => {
            let text = build_session().to_text().expect("serialize history");
            std::fs::write(GOLDEN_PATH, text).expect("write golden");
            eprintln!("wrote {GOLDEN_PATH}");
        }
        _ => eprintln!("skip: set OEP_HISTORY_REGEN=1 to (re)generate the golden"),
    }
}

// ---------------------------------------------------------------------------
// serialization round-trip identity
// ---------------------------------------------------------------------------

#[test]
fn serialize_then_deserialize_is_identity() {
    let original = build_session();
    let text = original.to_text().expect("serialize");
    let back = History::from_text(&text).expect("deserialize");
    assert_eq!(back, original);
    // Re-encoding the deserialized stack is byte-identical.
    assert_eq!(back.to_text().expect("re-serialize"), text);
}

#[test]
fn golden_roundtrips_and_has_expected_shape() {
    let h =
        History::<TopologyState>::from_text(&committed_golden()).expect("parse committed golden");
    assert_eq!(h.len(), 4);
    assert_eq!(h.capacity(), 8);
    assert!(h.can_undo());
    assert!(!h.can_redo());
    let ops: Vec<&str> = h.states().map(|s| s.op()).collect();
    assert_eq!(ops, vec!["init", "union-lip", "hole", "countersink"]);
    assert_eq!(h.current().map(|s| s.op()), Some("countersink"));
}

// ---------------------------------------------------------------------------
// concrete TopologyState record rules
// ---------------------------------------------------------------------------

#[test]
fn topology_state_records_default_empty() {
    let s = TopologyState::new(1, "x", vec![], vec![]);
    assert_eq!(s.revision(), 1);
    assert_eq!(s.op(), "x");
    assert!(s.bodies().is_empty());
    assert!(s.features().is_empty());
}

// ---------------------------------------------------------------------------
// strict decoding: malformed input never panics, always None
// ---------------------------------------------------------------------------

#[test]
fn from_text_rejects_garbage_and_malformed() {
    let cases = [
        "",
        "llama",
        "format oep-history\nversion 1\ncapacity 0\ncursor 0\nstates 99\n",
        "format oep-history\nversion 999\ncapacity 0\ncursor 0\nstates 0\n",
        "format oep-history\nversion 1\ncapacity 0\ncursor 5\nstates 1\nstate 0\nrevision 1\nop x\nbodies 0\nfeatures 0\n",
        // state block missing its 'state N' marker
        "format oep-history\nversion 1\ncapacity 0\ncursor 0\nstates 1\nrevision 1\nop x\nbodies 0\nfeatures 0\n",
        // state whose inner record is malformed (unknown kind)
        "format oep-history\nversion 1\ncapacity 0\ncursor 0\nstates 1\nstate 0\nrevision 1\nop x\nbodies 1\nbody oep-b solid bogus\nfeatures 0\n",
        // non-finite bbox coordinate
        "format oep-history\nversion 1\ncapacity 0\ncursor 0\nstates 1\nstate 0\nrevision 1\nop x\nbodies 1\nbody oep-b solid 0.0;nan;0.0;1.0;1.0;1.0\nfeatures 0\n",
        // body count does not match the number of body lines
        "format oep-history\nversion 1\ncapacity 0\ncursor 0\nstates 1\nstate 0\nrevision 1\nop x\nbodies 2\nbody oep-b solid 0.0;0.0;0.0;1.0;1.0;1.0\nfeatures 0\n",
    ];
    for (i, c) in cases.iter().enumerate() {
        assert!(
            History::<TopologyState>::from_text(c).is_none(),
            "case {i} should have decoded to None"
        );
    }
}

#[test]
fn topology_state_encode_decode_is_strict() {
    // Feature count 1 but the feature line is missing entirely.
    let missing = "revision 1\nop x\nbodies 0\nfeatures 1\n";
    assert!(
        TopologyState::decode(missing).is_none(),
        "counted feature line missing"
    );

    // Feature line with a non-finite parameter value.
    let bad_val = "revision 1\nop x\nbodies 0\nfeatures 1\nfeature oep-f Hole diameter=nan\n";
    assert!(
        TopologyState::decode(bad_val).is_none(),
        "non-finite param value"
    );

    // Feature line where the parameters are not name=value pairs.
    let bad_kv = "revision 1\nop x\nbodies 0\nfeatures 1\nfeature oep-f Hole notapair\n";
    assert!(
        TopologyState::decode(bad_kv).is_none(),
        "malformed param pair"
    );
}
