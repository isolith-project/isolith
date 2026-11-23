//! UI-B1 UI-INPUT-ROUTER: one event map to command tokens (wave B).
//!
//! Behaviour this card proves:
//!
//! 1. A single table (`src/input.rs`) maps (key, modifiers) and (pointer
//!    button, modifiers) to command tokens, and every token is one
//!    `oep_shell::commands` already validates - the exact `validate_token`
//!    the workbench CLI calls before dispatch, so the palette, the keyboard
//!    and the future toolbars speak one language. Table-driven over all rows:
//!    21 bindings declared, 21 resolve back to themselves with exact
//!    modifiers, 21 tokens accepted by the shared registry.
//! 2. The panel seam is real: the router does not claim the toggles the
//!    panels own (palette Ctrl+K, doc Escape / Ctrl+O / Ctrl+S), and the
//!    existing UI-A6 palette and UI-A7 doc suites stay green underneath.
//! 3. End to end: a real `app::App` frame consumes Ctrl+Z through the router
//!    and records `Undo -> undo` in the report oracle, while a plain key
//!    routes nothing.
//! 4. The mutation guard: no module under `crates/oep-app/src` may write to a
//!    document outside `doc_io.rs`. The scanner is proven load-bearing - every
//!    signature it is built to catch is caught on a planted line, and the real
//!    tree passes clean. (A live planted probe under `src/` failing the guard
//!    is captured in `workstreams/modeler/ui/proof/UI-B1/`.)
//!
//! Only `app.rs` (plus `config.rs`, `theme.rs`) is included, exactly like the
//! other oep-app test targets: `app.rs` already pulls in the router through
//! its `#[path] mod input;`, so every assert reads the app's real code.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::Harness;

/// The router's report oracle, read back through the same `egui::Context`.
/// `app::input::REPORT_KEY` is the exact key the app's router writes.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(app::input::REPORT_KEY)))
        .unwrap_or_default()
}

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The egui `Modifiers` that normalize back to `mods` (the command family as
/// one bit, exactly how the router sees a press).
fn egui_mods_of(m: app::input::Mods) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: false,
        shift: m.shift,
        mac_cmd: false,
        command: m.command,
    }
}

// ---------------------------------------------------------------------------
// The table: at least twenty table-driven bindings through the shared registry
// ---------------------------------------------------------------------------

/// The one table declares at least twenty bindings (the card's acceptance
/// floor); every row carries a label, a token and an event.
#[test]
fn the_table_declares_at_least_twenty_bindings() {
    let bindings = app::input::all_bindings();
    assert!(
        bindings.len() >= 20,
        "the single input table must declare at least 20 bindings, got {}",
        bindings.len()
    );
    for b in &bindings {
        assert!(!b.label.trim().is_empty(), "every binding has a label");
        assert!(!b.token.trim().is_empty(), "every binding has a token");
        assert!(
            b.key().is_some() || b.button().is_some(),
            "every binding has an event (key or pointer button)"
        );
    }
}

/// Table-driven over every row: each token is accepted by the SAME
/// `validate_token` the workbench CLI calls before it dispatches - the palette
/// and this router can never drift from the CLI, because both validate against
/// the one registry.
#[test]
fn every_binding_token_validates_through_the_shared_registry() {
    let bindings = app::input::all_bindings();
    assert!(
        bindings.len() >= 20,
        "fixture: at least 20 bindings declared"
    );
    let unknown = app::input::unknown_tokens();
    assert!(
        unknown.is_empty(),
        "bindings must speak only validated command tokens, unknown: {unknown:?}"
    );
    for b in &bindings {
        // The CLI's exact validate step: the canonical dispatch token comes
        // back unchanged.
        assert_eq!(
            oep_shell::commands::validate_token(b.token),
            Ok(b.token),
            "binding {:?} uses an unvalidated token",
            b.label
        );
        assert!(
            oep_shell::commands::find_by_token(b.token).is_some(),
            "find_by_token({:?}) must resolve the binding's token",
            b.token
        );
    }
}

/// Table-driven over every keyboard row: resolving the row's own (key,
/// modifiers) returns that same row - the table is self-consistent, one row
/// per press.
#[test]
fn every_key_binding_resolves_back_to_itself() {
    for b in app::input::KEY_BINDINGS {
        let key = b.key().expect("key binding has a key");
        let resolved = app::input::resolve_key(key, egui_mods_of(b.mods()));
        assert_eq!(
            resolved,
            Some(b),
            "key {key:?} + {:?} must resolve to its own row {:?}",
            b.mods(),
            b.label
        );
    }
}

/// Table-driven over every pointer row: resolving the row's own (button,
/// modifiers) returns that same row.
#[test]
fn every_pointer_binding_resolves_back_to_itself() {
    for b in app::input::POINTER_BINDINGS {
        let button = b.button().expect("pointer binding has a button");
        let resolved = app::input::resolve_pointer(button, egui_mods_of(b.mods()));
        assert_eq!(
            resolved,
            Some(b),
            "button {button:?} + {:?} must resolve to its own row {:?}",
            b.mods(),
            b.label
        );
    }
}

/// Exact modifiers: a modifier the row did not ask for does not match, so the
/// same key with different modifiers lands on the row that asked for them.
#[test]
fn modifier_exactness_routes_shifted_and_plain_combos_apart() {
    let cmd = egui::Modifiers::COMMAND;
    let cmd_shift = egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT);

    // Ctrl+Shift+Z is Redo (the more-specific row); Ctrl+Z is Undo.
    assert_eq!(
        app::input::resolve_key(egui::Key::Z, cmd_shift).map(|b| b.token),
        Some("redo"),
        "Ctrl+Shift+Z must resolve to redo, not undo"
    );
    assert_eq!(
        app::input::resolve_key(egui::Key::Z, cmd).map(|b| b.token),
        Some("undo"),
        "Ctrl+Z must resolve to undo"
    );
    // Shared modifier per row; a key no row declares resolves to nothing.
    assert!(app::input::resolve_key(egui::Key::Q, cmd).is_none());
    assert!(app::input::resolve_key(egui::Key::Z, egui::Modifiers::NONE).is_none());
    // Pointer exactness: shift+left is Pan, plain left is Orbit / Pick.
    assert_eq!(
        app::input::resolve_pointer(egui::PointerButton::Primary, cmd_shift).map(|b| b.label),
        Some("Pan")
    );
    assert_eq!(
        app::input::resolve_pointer(egui::PointerButton::Primary, egui::Modifiers::NONE)
            .map(|b| b.label),
        Some("Orbit / Pick")
    );
}

/// The command family is one bit: Ctrl, Command and a real Linux Ctrl press
/// (egui-winit sets both `ctrl` and `command`) all reach the same row, exactly
/// the interchangeability the app's own shortcut reader already uses.
#[test]
fn command_family_modifiers_resolve_to_the_same_binding() {
    let ctrl = egui::Modifiers::CTRL;
    let linux_ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    for modifiers in [
        egui::Modifiers::COMMAND,
        ctrl,
        linux_ctrl,
        egui::Modifiers::MAC_CMD,
    ] {
        assert_eq!(
            app::input::resolve_key(egui::Key::Z, modifiers).map(|b| b.token),
            Some("undo"),
            "the whole command family must reach the same undo row: {modifiers:?}"
        );
    }
    // A plain press (no command family) must not trigger a command binding.
    assert!(app::input::resolve_key(egui::Key::Z, egui::Modifiers::NONE).is_none());
}

/// The panel seam stays outside the table: the toggles the panels own (palette
/// Ctrl+K, doc panel Escape / Ctrl+O / Ctrl+S, plain Enter/arrows) are not
/// claimed as command bindings by the router, so the UI-A6/UI-A7 behaviour is
/// untouched by construction.
#[test]
fn panel_seam_toggles_are_not_claimed_by_the_table() {
    let cmd = egui::Modifiers::COMMAND;
    // palette Ctrl+K and doc Escape must keep going to their panels.
    assert!(app::input::resolve_key(egui::Key::K, cmd).is_none());
    assert!(app::input::resolve_key(egui::Key::Escape, egui::Modifiers::NONE).is_none());
    // doc Ctrl+O / Ctrl+S (save/open the loaded document) are panel-owned;
    // the router only claims the shifted variants (Open Model / Save Copy).
    assert!(app::input::resolve_key(egui::Key::O, cmd).is_none());
    assert!(app::input::resolve_key(egui::Key::S, cmd).is_none());
    assert!(app::input::resolve_key(egui::Key::Enter, egui::Modifiers::NONE).is_none());
}

// ---------------------------------------------------------------------------
// End to end through the real app frame
// ---------------------------------------------------------------------------

/// A real `app::App` frame consumes Ctrl+Z in the router, records it in the
/// report oracle, and routes it to the exact token the table declares - the
/// proof that nothing else in the crate invented the binding.
#[test]
fn ctrl_z_routes_through_the_single_table_in_the_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    let before = report(&harness);
    assert!(
        before.contains("count=0"),
        "fresh session routes nothing yet: {before:?}"
    );

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.step();

    let after = report(&harness);
    assert!(
        after.contains("count=1"),
        "Ctrl+Z must route exactly once through the table: {after:?}"
    );
    assert!(
        after.contains("Undo -> undo"),
        "the routed command is the table's own undo row: {after:?}"
    );

    // A second press routes a second time (one per frame, no double-route).
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.step();
    assert!(
        report(&harness).contains("count=2"),
        "a second Ctrl+Z routes again: {:?}",
        report(&harness)
    );
}

/// Ctrl+Shift+Z routes to redo (the more-specific row), not undo, through the
/// real app.
#[test]
fn ctrl_shift_z_routes_to_redo_in_the_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    harness.key_press_modifiers(
        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
        egui::Key::Z,
    );
    harness.step();

    let after = report(&harness);
    assert!(
        after.contains("Redo -> redo"),
        "Ctrl+Shift+Z must route to redo, the more-specific row: {after:?}"
    );
    assert!(
        after.contains("count=1"),
        "exactly one route for one press: {after:?}"
    );
}

/// A plain key that no row declares routes nothing - the router is not a key
/// thief.
#[test]
fn plain_key_routes_nothing_in_the_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    harness.key_press(egui::Key::Z); // no plain-Z command binding exists
    harness.step();
    assert!(
        report(&harness).contains("count=0"),
        "a plain key must not route a command: {:?}",
        report(&harness)
    );
}

/// The pointer half of the table is live in the real app: holding the forward
/// mouse button resolves to the redo binding in the report (read-only; the
/// viewport seam still owns drags over the panel).
#[test]
fn held_pointer_button_resolves_to_its_binding_in_the_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    let pos = egui::pos2(50.0, 50.0);
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Extra2,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    assert!(
        report(&harness).contains("active_pointer=Redo"),
        "holding the forward button must resolve to the redo row: {:?}",
        report(&harness)
    );

    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Extra2,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    assert!(
        report(&harness).contains("active_pointer=-"),
        "releasing the button clears the active pointer binding: {:?}",
        report(&harness)
    );
}

// ---------------------------------------------------------------------------
// Mutation guard: no module under crates/oep-app/src writes to a document
// outside doc_io.rs
// ---------------------------------------------------------------------------

/// Signatures of a DIRECT in-place write to a loaded document. The engine's
/// document (`oep_geo::Document`, re-exported as `oep_import::Document`) is
/// mutated only through its own `push_*` mutators or by pushing/removing
/// members of its collections; reading a count (`bodies.len()`) is not a
/// mutation and must pass below.
///
/// This is a static contract: any module under `src/` (other than `doc_io.rs`,
/// the one sanctioned document-I/O module) that calls one of these writes
/// fails the build. The list is deliberately specific so a read is never
/// mistaken for a mutation.
const MUTATION_SIGNATURES: &[&str] = &[
    // The document's own &mut-self mutators.
    ".push_part(",
    ".push_body(",
    ".push_feature(",
    ".push_assembly(",
    ".push_drawing(",
    // Direct mutation of the document's collections (doc.rs only ever reads
    // `len()` from them when reporting counts).
    ".parts.push(",
    ".parts.insert(",
    ".parts.remove(",
    ".parts.clear(",
    ".parts.pop(",
    ".bodies.push(",
    ".bodies.insert(",
    ".bodies.remove(",
    ".bodies.clear(",
    ".bodies.pop(",
    ".features.push(",
    ".features.insert(",
    ".features.remove(",
    ".features.clear(",
    ".features.pop(",
    ".assemblies.push(",
    ".assemblies.insert(",
    ".assemblies.remove(",
    ".assemblies.clear(",
    ".assemblies.pop(",
    ".drawings.push(",
    ".drawings.insert(",
    ".drawings.remove(",
    ".drawings.clear(",
    ".drawings.pop(",
    // In-place field reassignment of a document's collections.
    ".parts = ",
    ".bodies = ",
    ".features = ",
    ".assemblies = ",
    ".drawings = ",
];

/// Every mutation signature present in `line`.
fn document_mutation_hits(line: &str) -> Vec<&'static str> {
    MUTATION_SIGNATURES
        .iter()
        .copied()
        .filter(|sig| line.contains(sig))
        .collect()
}

/// Walk every `.rs` file under `dir`, invoking `visit` per line.
fn collect_rust_files(
    dir: &std::path::Path,
    visit: &mut impl FnMut(&std::path::Path, usize, &str),
) {
    for entry in std::fs::read_dir(dir).expect("read src dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, visit);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("read source file");
            for (idx, line) in text.lines().enumerate() {
                visit(&path, idx + 1, line);
            }
        }
    }
}

/// The guard itself: no module under `crates/oep-app/src` mutates a document
/// outside `doc_io.rs`. Runs on every `cargo test -p oep-app` invocation
/// (this target is part of that suite), so a direct document write cannot
/// merge silently.
#[test]
fn no_module_mutates_the_document_outside_doc_io_rs() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations: Vec<(std::path::PathBuf, usize, &'static str)> = Vec::new();
    collect_rust_files(&root, &mut |path, line_no, line| {
        if path.ends_with("doc_io.rs") {
            return; // doc_io.rs is the sanctioned document I/O module
        }
        for hit in document_mutation_hits(line) {
            violations.push((path.to_path_buf(), line_no, hit));
        }
    });
    assert!(
        violations.is_empty(),
        "direct document mutation outside crates/oep-app/src/doc_io.rs:\n{}",
        violations
            .iter()
            .map(|(p, n, hit)| format!("  {}:{}: {hit}", p.display(), n))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The scanner is load-bearing, proven by the same principle the guard fires
/// on: each signature it is built to catch really is caught (a planted
/// violation must fail), and the read-only document access doc.rs legitimately
/// uses is NOT caught.
#[test]
fn the_scanner_flags_planted_direct_document_mutation_and_passes_reads() {
    // Every signature, planted on a line the way it would appear in code.
    for sig in MUTATION_SIGNATURES {
        let probe = format!("document{sig}x");
        assert!(
            !document_mutation_hits(&probe).is_empty(),
            "the scanner must catch its own planted signature {sig:?}"
        );
    }
    // Reads are not mutations: the exact pattern doc.rs uses to report body
    // and feature counts must pass.
    assert!(
        document_mutation_hits("let n = d.bodies.len() + d.features.len();").is_empty(),
        "a read of body/feature counts is not a document mutation"
    );
    assert!(document_mutation_hits("d.features.len()").is_empty());
    // Field access of unrelated collections is not a document mutation either.
    assert!(document_mutation_hits("self.recent.push(path.clone());").is_empty());
}
