//! UI-A6 UI-COMMAND-PALETTE: searchable command surface.
//!
//! Drives the REAL `app::App` (the same source shipped in the `isolith`
//! binary, pulled in verbatim via `#[path]` like UI-A2's harness) through
//! `egui_kittest` and asserts:
//!
//! 1. Ctrl+K opens the palette and Esc closes it.
//! 2. Typing filters the rows to exactly `oep_shell::commands::search` (the
//!    shared registry - the test cross-checks the rendered rows against the
//!    registry directly, so the palette cannot be a fork or a copied list).
//! 3. Enter routes the selected command through the same router the CLI uses
//!    (both sides resolve through `oep_shell::commands`), proven by the UI-A6
//!    spy: the palette writes a router-invocation counter into the same
//!    `egui::Context`, read back here as a plain `String`. Never a shell-out
//!    to the binary.
//! 4. Unknown input shows an honest "no command matches" line and Enter with
//!    no match does NOT invoke the router.
//! 5. Every row shows the command label plus its shortcut or its palette path,
//!    so nothing is silently hidden (a command with neither would render an
//!    explicit gap).
//!
//! Only `app.rs` (and `config.rs`) are included: `app.rs` already pulls in the
//! palette and its command seam through `panels/mod.rs`, and re-including
//! those files standalone would trip clippy's `duplicate_mod`. The spy is the
//! deterministic `String` the palette writes into the egui context data store
//! under a documented key - readable from here because `String` is a std type
//! whose identity is shared across the `#[path]` copies.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

use egui_kittest::Harness;

/// Context-data key the palette writes its routing spy to (a plain `String`,
/// so it is readable here regardless of which `#[path]` copy produced it).
/// Must match `REPORT_KEY` in `src/panels/palette.rs`.
const REPORT_KEY: &str = "ui_a6_palette_report";

/// Build the harness around the real `app::App` at the product's window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The UI-A6 spy: parse the palette's report (`query=`/`open=`/`count=`/
/// `outcome=` lines stored in the shared context) into a tuple.
fn spy(harness: &Harness<'_, app::App>) -> (String, bool, u32, String) {
    let report = harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default();
    let mut query = String::new();
    let mut open = false;
    let mut count = 0u32;
    let mut outcome = String::new();
    for line in report.lines() {
        if let Some(value) = line.strip_prefix("query=") {
            query = value.to_string();
        } else if let Some(value) = line.strip_prefix("open=") {
            open = value == "true";
        } else if let Some(value) = line.strip_prefix("count=") {
            count = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("outcome=") {
            outcome = value.to_string();
        }
    }
    (query, open, count, outcome)
}

/// Every accessible label currently in the tree (the palette's rows and lines
/// are labels the real app produced; nothing here is fabricated).
fn all_labels(harness: &Harness<'_, app::App>) -> Vec<String> {
    use egui_kittest::kittest::NodeT;
    harness
        .root()
        .children_recursive()
        .filter_map(|n| n.accesskit_node().label().map(|s| s.to_string()))
        .collect()
}

/// Open the palette the way a user does: press Ctrl+K and let the queued
/// modifier/key events settle.
fn open_palette(harness: &mut Harness<'static, app::App>) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    harness.step();
}

/// Type into the (focused) palette filter and let one extra frame recompute the
/// rows from the new query (egui applies text during the frame it arrives, the
/// live search runs from the next frame onward).
fn type_filter(harness: &mut Harness<'static, app::App>, text: &str) {
    harness.event(egui::Event::Text(text.to_string()));
    harness.step();
    harness.step();
}

/// Ctrl+K opens the palette over the shell; Escape closes it again.
#[test]
fn ctrl_k_opens_the_palette_and_escape_closes_it() {
    let mut harness = real_app_harness();
    harness.run();
    assert!(!spy(&harness).1, "palette starts closed");

    open_palette(&mut harness);
    assert!(spy(&harness).1, "Ctrl+K opens the palette");

    harness.key_press(egui::Key::Escape);
    harness.step();
    assert!(!spy(&harness).1, "Escape closes the palette");
}

/// Typing filters the palette to EXACTLY the shared registry's search result,
/// cross-checked against `oep_shell::commands::search` directly - proof the
/// palette draws its rows from the one registry and does not hold a fork.
#[test]
fn typing_filters_through_the_shared_registry_not_a_fork() {
    let mut harness = real_app_harness();
    harness.run();
    open_palette(&mut harness);

    type_filter(&mut harness, "extrude");

    let expected = oep_shell::commands::search("extrude");
    assert!(
        !expected.is_empty(),
        "fixture: 'extrude' must hit the registry"
    );
    let labels = all_labels(&harness);
    let rows: Vec<&String> = labels.iter().filter(|l| l.contains(" | ")).collect();
    assert_eq!(
        rows.len(),
        expected.len(),
        "rendered rows must equal the shared-registry hit count, no more no fewer"
    );
    for spec in &expected {
        assert!(
            labels.iter().any(|l| l.contains(spec.name)),
            "shared-registry hit {:?} must be a visible palette row",
            spec.name
        );
    }
    // A command that does not match the filter must not appear.
    assert!(
        !labels.iter().any(|l| l.contains("assem:open"))
            && !labels.iter().any(|l| l.contains("vars:set")),
        "non-matching commands must be filtered out"
    );
}

/// Enter on a matching row routes it through the same router the CLI uses (the
/// shared oep-shell registry), and the UI-A6 spy counts the invocation.
#[test]
fn enter_routes_through_the_shared_router_spy_counts_invocations() {
    let mut harness = real_app_harness();
    harness.run();
    let before = spy(&harness).2;

    open_palette(&mut harness);
    type_filter(&mut harness, "undo");

    harness.key_press(egui::Key::Enter);
    harness.step();

    let (query, open, count, outcome) = spy(&harness);
    assert_eq!(query, "undo", "the filter text is the palette's live query");
    assert_eq!(
        count,
        before + 1,
        "spy: typing then Enter must invoke the router exactly once"
    );
    assert!(!open, "routing a command dismisses the palette");
    assert!(
        outcome.contains("undo"),
        "outcome names the routed command: {outcome:?}"
    );

    // The router is the CLI's router step, on the shared registry: the routed
    // command is exactly what `find`/`validate_token` - the functions the
    // workbench CLI calls before dispatch - accept for that token.
    assert!(oep_shell::commands::find("undo").is_some());
    assert_eq!(
        oep_shell::commands::validate_token("undo"),
        Ok("undo"),
        "the CLI's validate_token accepts the same token the palette routes"
    );
    assert_eq!(
        oep_shell::commands::find("undo").map(|c| c.name),
        Some("undo"),
        "the routed name is the shared registry's canonical name"
    );

    // A later bare Enter with the palette closed routes nothing.
    harness.key_press(egui::Key::Enter);
    harness.step();
    assert_eq!(
        spy(&harness).2,
        before + 1,
        "Enter with the palette closed must not invoke the router"
    );
}

/// Unknown input shows an honest "no command matches" line (never an empty
/// list) and Enter with no match does not invoke the router.
#[test]
fn unknown_input_shows_an_honest_no_match_line_and_does_not_route() {
    let mut harness = real_app_harness();
    harness.run();
    open_palette(&mut harness);

    type_filter(&mut harness, "n0-such-c0mmand");

    let labels = all_labels(&harness);
    assert!(
        labels
            .iter()
            .any(|l| l.contains("no command matches") && l.contains("n0-such-c0mmand")),
        "unknown filter must show an honest 'no command matches' line: {labels:?}"
    );

    let before = spy(&harness).2;
    harness.key_press(egui::Key::Enter);
    harness.step();
    let (_query, open, count, _outcome) = spy(&harness);
    assert_eq!(
        count, before,
        "Enter with no matching row must not invoke the router"
    );
    assert!(
        open,
        "palette stays open when nothing matched so the query can be fixed"
    );
}

/// With no filter, the whole shared index is listed and every row carries the
/// command label plus its shortcut or its palette path - so nothing is
/// silently hidden. (The palette-path text is the command's CLI usage line; a
/// command with neither shortcut nor path would render the explicit gap
/// marker, but every registered command today has a usage line, which this
/// test pins.)
#[test]
fn rows_show_label_and_shortcut_or_path_with_nothing_hidden() {
    let mut harness = real_app_harness();
    harness.run();
    open_palette(&mut harness);

    let labels = all_labels(&harness);
    for spec in oep_shell::commands::commands() {
        // label + palette path (the CLI usage line) on every row of the whole
        // index - the label is the canonical command name, the path how a user
        // would type it in the CLI.
        let row = format!("{} | {}", spec.name, spec.usage);
        assert!(
            labels.iter().any(|l| l.contains(&row)),
            "command {} must be a visible row with label + path ({row:?})",
            spec.name
        );
    }
    // The empty filter lists the whole index, one non-blank row per command.
    let rows: Vec<&String> = labels.iter().filter(|l| l.contains(" | ")).collect();
    assert_eq!(
        rows.len(),
        oep_shell::commands::commands().len(),
        "the empty filter lists the whole index, one row per command"
    );
    for row in &rows {
        assert!(
            !row.trim().is_empty() && row.contains(" | "),
            "a row must never render blank (a silently hidden command would): {row:?}"
        );
    }
}
