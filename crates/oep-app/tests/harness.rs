//! UI-A2 UI-HARNESS: headless UI test driving the real Isolith editor app.
//!
//! Behaviour: a test drives the real `App` type (the same `crates/oep-app`
//! source shipped in the `isolith` binary, not a stub) through injected pointer
//! and key events and asserts on the resulting egui input state and on the
//! AccessKit tree produced by that same app.
//!
//! `crates/oep-app` is a binary-only crate, so an integration test cannot
//! `use oep_app::...`. To still drive the *real* app type, the two source
//! modules are pulled in verbatim via `#[path]`. The test therefore compiles
//! the exact `app.rs` / `config.rs` from `src/`; there is no fake `App` here.
//!
//! The harness is `egui_kittest` 0.37.0 (the version matching egui 0.37, per
//! the dependency card UI-A1) with only the `eframe` feature: `HarnessBuilder
//! ::build_eframe` wraps a real `eframe::App`, so a click and a key press are
//! delivered through egui's normal input path and the app's per-frame `ui()`
//! runs against them. The tests never fabricate a widget tree by hand.

#[path = "../src/app.rs"]
mod app;

// config.rs is compiled into the real binary too; in the test context the
// launch-only items (title, smoke budget) are legitimately unused, so silence
// dead-code for it alone. The app module below stays under normal lints.
#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

/// Build the harness around the real `app::App` at the product's window size.
/// `smoke = false`: the shell stays open exactly as it does at runtime, so the
/// frame is driven purely by injected input.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The real app produces an AccessKit tree with its central panel as a Pane
/// node, i.e. the real `App::ui` ran against the harness context and
/// registered itself in the a11y tree. (kittest drives the app with no native
/// OS window, so there is no Window/title-bar role; the app's own widget layer
/// is what appears.)
#[test]
fn real_app_registers_window_in_accesskit_tree() {
    let mut harness = real_app_harness();
    harness.run();

    // The empty shell renders a CentralPanel, which egui exposes to AccessKit
    // as a Pane. Its presence proves the real app's widget layer reached the
    // a11y tree (kittest cannot invent this node; only a real panel creates
    // it).
    let panes: Vec<_> = harness
        .get_all_by_role(egui::accesskit::Role::Pane)
        .collect();
    assert_eq!(
        panes.len(),
        1,
        "expected exactly the app's one central panel in the AccessKit tree"
    );

    // The tree is queryable end-to-end: the panel has at least the app's own
    // content child (the shell's GenericContainer), proving a real, populated
    // tree is driven by the real App type rather than an empty stub tree.
    let descendants: usize = panes[0].children_recursive().count();
    assert!(descendants >= 1, "expected app content in the panel tree");
}

/// A primary-button click injected into the harness reaches the real app:
/// after the click is processed the app's egui context reports the pointer
/// event that the app's own `ui()` observes.
#[test]
fn click_reaches_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    let target = egui::pos2(700.0, 400.0);

    // Inject the pointer and a press + release at that position.
    harness.event(egui::Event::PointerMoved(target));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    });
    harness.step();

    // While the button is down, the app's context must report it down.
    assert!(
        harness.ctx.input(|i| i.pointer.any_down()),
        "click press did not reach the app input state"
    );
    assert!(
        harness
            .ctx
            .input(|i| i.pointer.button_down(egui::PointerButton::Primary)),
        "primary button down not visible to the app input state"
    );
    assert_eq!(harness.ctx.input(|i| i.pointer.latest_pos()), Some(target));

    // Release: the app context must now register a completed primary click.
    harness.event(egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    });
    harness.step();
    assert!(
        harness.ctx.input(|i| i.pointer.any_click()),
        "click release did not register as a click in the app input state"
    );
}

/// A key press injected into the harness reaches the real app: the app's egui
/// context reports the key as pressed on the frame that delivers it.
#[test]
fn key_press_reaches_real_app() {
    let mut harness = real_app_harness();
    harness.run();

    harness.key_down(egui::Key::B);
    harness.step();

    assert!(
        harness.ctx.input(|i| i.key_pressed(egui::Key::B)),
        "key B press did not reach the app input state"
    );

    harness.key_up(egui::Key::B);
    harness.step();
    assert!(
        !harness.ctx.input(|i| i.key_pressed(egui::Key::B)),
        "key B should not be reported after release"
    );
}
