//! PRACTITIONER-INPUT-CONTRACT (card t_5b00868e): the two reachable rows
//! of the `practitioner_input_contract` section of the scope register are
//! enforced on real output, not mocked:
//!
//! 1. `input-command-search` - command search on a single key (SolidWorks S
//!    key / Fusion search) plus a keyboard reachability path to EVERY
//!    command. The workbench is a keyboard-first command surface, so the
//!    reachability half is: every registered command resolves through its
//!    dispatch token (`validate_token`), is findable by canonical name
//!    (`find`), and is advertised in the generated plain-text, markdown and
//!    JSON indexes (no command can be added undocumented). The search half
//!    is `commands::search(query)`: the type-ahead affordance that narrows
//!    the whole reachable command set to what matches, with an empty query
//!    returning the complete index rather than a dead end.
//!
//! 2. `input-undo-redo-parity` - single-press undo/redo parity and a visible
//!    undo step boundary per committed dimension. Committing a dimension in
//!    the variable set is one real document snapshot on the undo stack; ONE
//!    `undo()` press reverts exactly that committed dimension, ONE `redo()`
//!    press replays it, and the operation journal records each committed
//!    dimension as its own line (the visible boundary). The history is over
//!    the canonical oep-geo document (snapshots of the real model), never a
//!    proxy, and an undo at the oldest state is a typed `HistoryBoundary`
//!    error, never a panic.

use std::fs;
use std::path::PathBuf;

use oep_shell::commands;
use oep_shell::editor::{EditorError, EditorSession};

fn committed_index_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("re-notes")
        .join("oep-workbench-help-index.md")
}

// ---------------------------------------------------------------------------
// input-command-search
// ---------------------------------------------------------------------------

#[test]
fn every_command_is_keyboard_reachable_and_findable() {
    let specs: Vec<commands::CommandSpec> = commands::commands().to_vec();
    assert!(!specs.is_empty(), "registry must not be empty");
    let index = commands::render_index();
    let markdown = commands::render_index_markdown();
    let json = commands::render_index_json();
    for c in &specs {
        // Keyboard reachability: the dispatch token resolves; the canonical
        // name resolves. Both paths are real, callable, non-panicking.
        assert_eq!(
            commands::validate_token(c.token),
            Ok(c.token),
            "{} token {} not dispatchable",
            c.name,
            c.token
        );
        assert!(
            commands::find(c.name).is_some(),
            "{} not findable by canonical name",
            c.name
        );
        // Discoverability: the command is advertised in every generated index
        // (plain text, markdown, JSON), so no command is keyboard-reachable
        // without being documented.
        assert!(
            index.contains(c.usage),
            "{} missing from text index",
            c.name
        );
        assert!(
            markdown.contains(c.name),
            "{} missing from generated markdown index",
            c.name
        );
        assert!(
            json.contains(&format!("\"name\":\"{}\"", c.name)),
            "{} missing from JSON index",
            c.name
        );
    }
}

#[test]
fn committed_help_index_covers_the_search_surface() {
    // The committed, drift-guarded help index must already carry the search
    // command: keyboard command search is part of the documented surface.
    let committed = fs::read_to_string(committed_index_path()).expect(
        "missing committed help index (regenerate with: \
         cargo run --example workbench -- --write-help-index \
         docs/re-notes/oep-workbench-help-index.md)",
    );
    assert!(
        committed.starts_with(&commands::render_index_markdown()),
        "committed help index drifted from command metadata"
    );
    assert!(
        committed.contains("search:QUERY"),
        "committed help index does not document the search command"
    );
}

#[test]
fn command_search_narrows_the_reachable_set() {
    // Empty query: the search affordance returns the whole reachable command
    // set, never a dead end.
    let all = commands::search("");
    assert_eq!(all.len(), commands::commands().len());
    assert_eq!(
        all.len(),
        commands::search("  ").len(),
        "whitespace-only query is a full-index search"
    );

    // A few characters narrow to what matches; the exact command is always a
    // member of the returned set.
    let hits = commands::search("ext");
    assert!(
        hits.iter().any(|c| c.name == "feat:extrude"),
        "search 'ext' must surface feat:extrude, got {}",
        hits.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")
    );
    assert!(
        hits.iter().all(|c| commits_spec(c)),
        "search results must be members of the registry"
    );

    // Searching a canonical name returns that command (case-insensitive).
    let n = commands::search("UNDO");
    assert!(n.iter().any(|c| c.name == "undo"));

    // Every command is findable by searching its own name: no command is
    // reachable only by reading the full index.
    for c in commands::commands() {
        assert!(
            commands::search(c.name).iter().any(|h| h.name == c.name),
            "search of {} does not surface it",
            c.name
        );
    }
}

fn commits_spec(c: &commands::CommandSpec) -> bool {
    commands::find(c.name).is_some()
}

// ---------------------------------------------------------------------------
// input-undo-redo-parity
// ---------------------------------------------------------------------------

fn sorted_vars(s: &EditorSession) -> Vec<(String, f64)> {
    let mut v = s.vars();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn undo_redo_single_press_parity_and_dimension_step_boundaries() {
    let mut s = EditorSession::new();
    // Fresh session: no committed dimensions, no dimension journal lines.
    assert!(
        s.vars().is_empty(),
        "fresh session has no committed dimensions"
    );
    assert!(
        s.journal().iter().all(|l| !l.starts_with("variable ")),
        "no dimension commit before any set_var"
    );

    // Commit two dimensions: each commit is its own undo step (a visible
    // boundary between committed dimensions).
    s.set_var("width", 80.0).expect("commit dimension width");
    s.set_var("height", 40.0).expect("commit dimension height");
    assert_eq!(
        sorted_vars(&s),
        vec![("height".to_string(), 40.0), ("width".to_string(), 80.0)]
    );
    let dim_lines: Vec<&str> = s
        .journal()
        .iter()
        .map(String::as_str)
        .filter(|l| l.starts_with("variable "))
        .collect();
    assert_eq!(
        dim_lines,
        vec!["variable width = 80", "variable height = 40"],
        "each committed dimension is its own journal line (visible boundary)"
    );

    // ONE press undoes exactly the last committed dimension, not both.
    let r1 = s.undo().expect("one undo press");
    assert_eq!(r1.op, "previous edit", "undo names the reverted step");
    assert_eq!(
        sorted_vars(&s),
        vec![("width".to_string(), 80.0)],
        "one undo press reverts exactly one committed dimension"
    );

    // ONE more press undoes the first committed dimension.
    s.undo().expect("second undo press");
    assert!(
        s.vars().is_empty(),
        "each committed dimension reverted on its own undo press"
    );

    // At the oldest state an undo is a typed boundary, never a panic: the
    // single-press contract keeps its bounds honest and measured.
    assert!(
        matches!(s.undo(), Err(EditorError::HistoryBoundary(_))),
        "undo at the oldest state is a typed HistoryBoundary, not a panic"
    );

    // Single-press redo parity: replays exactly one committed dimension per
    // press, in commit order.
    s.redo().expect("one redo press");
    assert_eq!(
        sorted_vars(&s),
        vec![("width".to_string(), 80.0)],
        "one redo press replays exactly one committed dimension"
    );
    s.redo().expect("second redo press");
    assert_eq!(
        sorted_vars(&s),
        vec![("height".to_string(), 40.0), ("width".to_string(), 80.0)],
        "second redo press replays the second committed dimension"
    );

    // Parity closed loop: undo + redo returns to the exact committed state
    // (the journey is a real document round trip, not cosmetic).
    s.undo().expect("undo for parity");
    assert_eq!(sorted_vars(&s), vec![("width".to_string(), 80.0)]);
    s.redo().expect("redo for parity");
    assert_eq!(
        sorted_vars(&s),
        vec![("height".to_string(), 40.0), ("width".to_string(), 80.0)]
    );
}
