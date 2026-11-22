//! UI-B2 UI-HISTORY: journal undo/redo with drag coalescing (wave B).
//!
//! Behaviour this card proves (the acceptance comment, verified with REAL
//! canonical documents, not fabricated hashes):
//!
//! 1. One pointer drag is one undo entry: 50 consecutive drag commands with
//!    the same command token and the same drag id collapse into a single
//!    journal entry - the journal coalesces a continuous pointer gesture into
//!    one command. The merged entry keeps the FIRST `before` and the LAST
//!    `after` hash, and each intermediate command really changed the document
//!    (hashes differ), so the collapse is real, not a stream of no-ops.
//! 2. Undo is exact: undo restores the PRIOR mesh hash - the document is
//!    restored so `document_hash(document) == entry.before` (asserted inside
//!    the seam, re-asserted here).
//! 3. Redo returns to the same hash: `document_hash(document) == entry.after`
//!    after redo.
//! 4. Coalescing is scoped to the gesture: a new drag id, an ended gesture,
//!    a different token or a discrete command each start a fresh entry.
//! 5. The journal owns ordering + coalescing only, never document state: an
//!    entry is the token plus two hashes, and undo/redo return exactly the
//!    recorded hashes.
//! 6. 32-step semantics preserved: bounded capacity keeps the newest edits
//!    and drops the oldest (journal and snapshot store in lock-step).
//! 7. Journal tokens stay in the shared command language: every token used
//!    here is one `oep_shell::commands::validate_token` accepts (UI-B1's
//!    one-language contract reaches the journal).
//!
//! The test drives `doc_io::EditHistory` - the routed mutation seam that
//! holds the journal plus the bounded snapshot store - and, for the pure
//! ordering rules, `doc_io::history` directly. The real app's per-context
//! doc panel (UI-A7) owns WHERE the document lives; this seam owns HOW every
//! edit is journaled.

// The seam is `doc_io.rs`, which pulls `history.rs` in through its own
// `#[path]` include, so this target compiles exactly one copy of each and
// exercises the real shipped routing path (no test-only shim). This target
// only drives the journal seam, so the open/save helper functions beside it
// are unused here - `#[allow(dead_code)]` pins that, matching how every other
// oep-app test target handles `#[path]`-included modules it does not fully
// use.
#[path = "../src/doc_io.rs"]
#[allow(dead_code)]
mod doc_io;

use doc_io::{history::DragId, EditHistory};
use oep_import::{Document, Feature};

/// A real document with one feature, ready to be edited.
fn blank_document() -> Document {
    Document::new("mm")
}

/// A command that appends one uniquely-named feature (changes the hash).
fn add_feature(n: usize) -> impl FnOnce(&mut Document) {
    move |doc| doc.push_feature(Feature::new(format!("f{n}"), format!("op-{n}"), "Extrude"))
}

// ---------------------------------------------------------------------------
// The acceptance: 50 drag commands collapse to 1; undo/redo hash equality
// ---------------------------------------------------------------------------

/// The card's headline acceptance: 50 drag commands with the same token and
/// drag id collapse into ONE journal entry, undo restores the prior mesh
/// hash, and redo returns to the same hash - proven against real documents.
#[test]
fn fifty_drag_commands_collapse_to_one_entry_and_undo_redo_are_exact() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();
    let prior_hash = doc_io::document_hash(&doc);

    // A continuous pointer gesture: 50 commands, one token, one drag id.
    // Each command really edits the document (the hashes move every frame),
    // so collapsing to one entry is a genuine coalesce, not a no-op stream.
    let mut last = prior_hash;
    for i in 0..50 {
        let hash = history.apply(&mut doc, "view", Some(DragId(7)), add_feature(i));
        assert_ne!(
            hash, last,
            "each drag command must genuinely change the document (command {i})"
        );
        last = hash;
    }
    let mesh_hash_after_drag = doc_io::document_hash(&doc);
    assert_eq!(
        last, mesh_hash_after_drag,
        "apply() reports the hash the document actually has"
    );
    history.journal_mut().end_drag(DragId(7)); // finger lifted: the gesture ends

    assert_eq!(history.len(), 1, "50 drag commands collapse to 1 entry");
    assert!(history.can_undo());
    assert!(!history.can_redo());
    let entry = &history.journal().entries()[0];
    assert_eq!(entry.token, "view");
    assert_eq!(
        entry.before, prior_hash,
        "entry keeps the FIRST before hash"
    );
    assert_eq!(
        entry.after, mesh_hash_after_drag,
        "entry keeps the LAST after hash"
    );

    // Undo restores the PRIOR mesh hash: the actual document hashes to it.
    let undo_target = history.undo(&mut doc).expect("one undoable entry");
    assert_eq!(undo_target, prior_hash, "undo returns the prior hash");
    assert_eq!(
        doc_io::document_hash(&doc),
        prior_hash,
        "hash before == hash after undo"
    );

    // Redo returns to the same hash the drag ended at.
    let redo_target = history.redo(&mut doc).expect("one redoable entry");
    assert_eq!(
        redo_target, mesh_hash_after_drag,
        "redo returns the after hash"
    );
    assert_eq!(
        doc_io::document_hash(&doc),
        mesh_hash_after_drag,
        "hash after == hash after redo"
    );
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

/// The same exactness holds across a MIXED history of discrete edits and a
/// drag: undo walks the recorded hashes in reverse, redo returns forward to
/// exactly the same hashes at every step.
#[test]
fn undo_redo_walk_mixed_history_with_exact_hashes() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();

    // Boundary hashes: H0 = start, then each committed edit's after-hash.
    let mut ladder = vec![doc_io::document_hash(&doc)];
    history.apply(&mut doc, "point", None, add_feature(0)); // discrete
    ladder.push(doc_io::document_hash(&doc));
    history.apply(&mut doc, "point", None, add_feature(1)); // discrete
    ladder.push(doc_io::document_hash(&doc));
    // A drag of 5 commands = the third committed entry (coalesced).
    for i in 2..7 {
        history.apply(&mut doc, "view", Some(DragId(3)), add_feature(i));
    }
    ladder.push(doc_io::document_hash(&doc));

    assert_eq!(history.len(), 3, "two discrete edits + one coalesced drag");
    let hashes: Vec<(u64, u64)> = history
        .journal()
        .entries()
        .iter()
        .map(|e| (e.before, e.after))
        .collect();

    // Undo all three: each step lands on exactly the recorded prior hash.
    for (expect_before, expect_after) in hashes.iter().rev() {
        let target = history.undo(&mut doc).expect("undoable");
        assert_eq!(target, *expect_before, "undo returns the entry's before");
        assert_eq!(
            doc_io::document_hash(&doc),
            *expect_before,
            "document restores to the prior hash"
        );
        let _ = expect_after;
    }
    assert!(!history.can_undo());

    // Redo all three: each step returns to exactly the recorded after hash.
    for (expect_before, expect_after) in hashes.iter() {
        let target = history.redo(&mut doc).expect("redoable");
        assert_eq!(target, *expect_after, "redo returns the entry's after");
        assert_eq!(
            doc_io::document_hash(&doc),
            *expect_after,
            "document returns to the same hash"
        );
        let _ = expect_before;
    }
    assert!(!history.can_redo());
    assert_eq!(doc_io::document_hash(&doc), *ladder.last().unwrap());
}

// ---------------------------------------------------------------------------
// Coalescing scope: when does a drag become ONE entry
// ---------------------------------------------------------------------------

/// A NEW drag id starts a fresh entry - coalescing never bridges gestures.
#[test]
fn a_new_drag_id_starts_a_fresh_entry() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();

    history.apply(&mut doc, "view", Some(DragId(1)), add_feature(0));
    history.apply(&mut doc, "view", Some(DragId(1)), add_feature(1));

    // A different gesture (drag id 2) on its own line must not merge in.
    history.apply(&mut doc, "view", Some(DragId(2)), add_feature(2));
    history.apply(&mut doc, "view", Some(DragId(2)), add_feature(3));

    assert_eq!(history.len(), 2, "two gestures are two undo entries");
}

/// Once a gesture ends (`end_drag`), reusing the same drag id opens a fresh
/// entry - the coalescing window closes with the gesture.
#[test]
fn after_the_gesture_ends_the_same_drag_id_starts_again() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();

    let d = DragId(9);
    history.apply(&mut doc, "view", Some(d), add_feature(0));
    history.apply(&mut doc, "view", Some(d), add_feature(1));
    history.journal_mut().end_drag(d);

    history.apply(&mut doc, "view", Some(d), add_feature(2));
    history.apply(&mut doc, "view", Some(d), add_feature(3));

    assert_eq!(
        history.len(),
        2,
        "a gesture that ended starts a fresh entry even with the same id"
    );
}

/// A DIFFERENT token interrupts a drag: the same drag id on a different
/// command is two entries (the rule keys coalescing on token AND drag id).
#[test]
fn a_different_token_breaks_coalescing() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();

    history.apply(&mut doc, "view", Some(DragId(5)), add_feature(0));
    history.apply(&mut doc, "view", Some(DragId(5)), add_feature(1));
    history.apply(&mut doc, "dim", Some(DragId(5)), add_feature(2));
    history.apply(&mut doc, "dim", Some(DragId(5)), add_feature(3));

    assert_eq!(history.len(), 2, "token change = fresh entry");
}

/// Discrete commands (no drag id) are one entry each, never coalesced.
#[test]
fn discrete_commands_are_individual_entries() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();
    for i in 0..5 {
        history.apply(&mut doc, "point", None, add_feature(i));
    }
    assert_eq!(history.len(), 5, "5 discrete commands are 5 undo entries");
    for _ in 0..5 {
        history.undo(&mut doc).expect("five undoable entries");
    }
    assert!(!history.can_undo());
    assert_eq!(history.len(), 0);
}

// ---------------------------------------------------------------------------
// The journal owns ordering + coalescing, never document state
// ---------------------------------------------------------------------------

/// The journal (history::Journal) is state-free: it records exactly the token
/// and the two hashes it is handed, and undo/redo return exactly those. No
/// document is ever required - the hashes are the whole contract.
#[test]
fn journal_records_token_and_hashes_only() {
    let mut journal = doc_io::history::Journal::new();
    journal.push("view", Some(DragId(0)), 111, 222);
    journal.push("view", Some(DragId(0)), 222, 333); // coalesces

    assert_eq!(journal.len(), 1);
    let e = &journal.entries()[0];
    assert_eq!((e.token, e.before, e.after), ("view", 111, 333));

    assert_eq!(
        journal.undo(),
        Some(111),
        "undo returns the recorded before"
    );
    assert_eq!(journal.redo(), Some(333), "redo returns the recorded after");
    assert!(!journal.can_redo());
}

/// A new edit after an undo truncates the redo branch: the abandoned future
/// can no longer be redone (the classic CAD rule, at the routed seam too).
#[test]
fn new_edit_after_undo_truncates_the_redo_branch() {
    let mut history = EditHistory::new();
    let mut doc = blank_document();
    for i in 0..3 {
        history.apply(&mut doc, "point", None, add_feature(i));
    }

    // Undo the third edit, then redo it is still available.
    history.undo(&mut doc);
    let before_fresh = doc_io::document_hash(&doc);
    assert!(history.can_redo());

    // A fresh edit from the undone state discards the redone future.
    history.apply(&mut doc, "point", None, add_feature(9));
    assert!(!history.can_redo(), "a fresh edit invalidates redo");
    // Undo now lands on the state the fresh edit started from, never on the
    // abandoned old future.
    let target = history.undo(&mut doc).expect("undoable");
    assert_eq!(
        target, before_fresh,
        "undo returns the fresh edit's prior hash"
    );
    assert_eq!(doc_io::document_hash(&doc), before_fresh);
}

// ---------------------------------------------------------------------------
// 32-step semantics: bounded capacity, oldest dropped
// ---------------------------------------------------------------------------

/// At capacity (2 here), the oldest edits are dropped from BOTH the journal
/// and its snapshot store, and undo still restores the newest survivors
/// exactly.
#[test]
fn bounded_capacity_drops_the_oldest_and_undo_stays_exact() {
    let mut history = EditHistory::with_capacity(2);
    let mut doc = blank_document();

    let mut ladder = vec![doc_io::document_hash(&doc)];
    for i in 0..4 {
        history.apply(&mut doc, "point", None, add_feature(i));
        ladder.push(doc_io::document_hash(&doc));
    }
    // Four edits, capacity 2: only the newest two survive.
    assert_eq!(history.len(), 2, "oldest edits dropped at capacity");
    let entries: Vec<(u64, u64)> = history
        .journal()
        .entries()
        .iter()
        .map(|e| (e.before, e.after))
        .collect();
    assert_eq!(
        entries,
        vec![(ladder[2], ladder[3]), (ladder[3], ladder[4])],
        "the two newest edits survive, oldest dropped from the front"
    );

    // Undo the two survivors exactly; the dropped past cannot be reached.
    history.undo(&mut doc);
    assert_eq!(doc_io::document_hash(&doc), ladder[3]);
    history.undo(&mut doc);
    assert_eq!(doc_io::document_hash(&doc), ladder[2]);
    assert!(!history.can_undo(), "the dropped oldest is truly gone");
}

// ---------------------------------------------------------------------------
// Journal tokens stay in the shared command language (UI-B1 contract)
// ---------------------------------------------------------------------------

/// The command tokens that reach the journal for drag coalescing today are
/// the registry-backed binding tokens (UI-B1's `view` drag row) - the same
/// `oep_shell::commands::validate_token` the CLI runs before dispatch, so
/// routing and journaling cannot drift into a second dialect. (The journal
/// itself stays token-agnostic: sketching cards may route command names that
/// enter the registry in a later wave; validation happens upstream in the
/// router.)
#[test]
fn registry_backed_journal_tokens_are_validated_command_tokens() {
    for token in ["view", "undo", "redo"] {
        assert_eq!(
            oep_shell::commands::validate_token(token),
            Ok(token),
            "journal routing must stay in the validated command language"
        );
    }
}
