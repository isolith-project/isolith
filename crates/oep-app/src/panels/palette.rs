//! Searchable command palette (wave A, card UI-A6, UI-COMMAND-PALETTE).
//!
//! Fills the UI-A4 "command palette host" stub:
//!
//! - Ctrl+K opens (and closes) a palette overlay over the shell;
//! - typing filters live against `oep_shell::commands::search` - the shared
//!   registry, never a fork and never a copied command list;
//! - every row shows the command label plus its shortcut or its palette path,
//!   so a command with neither is visible as an explicit gap
//!   ([`commands::GAP_NOTE`]) rather than silently hidden;
//! - Enter routes the selected command through [`commands::dispatch`], the
//!   same validate-then-dispatch router step the workbench CLI uses;
//! - an unknown filter shows an honest "no command matches" line instead of an
//!   empty list.
//!
//! State and the spy: `app.rs` calls `show(ctx, app)` every frame and this
//! card owns only this file (plus `commands.rs`), so the palette's per-app
//! state lives in the egui context data store - one instance per
//! `egui::Context`, which keeps every test/harness isolated (no global
//! variable). `route_count` is the UI-A6 spy the acceptance asks for. The
//! test reads it back as a plain `String` report (see [`report`]); a `String`
//! is a std type whose [`std::any::TypeId`] is identical across the
//! `#[path]`-included copies of this file, so the test observes the same
//! per-context state the app writes - never by shelling out to the binary.
//!
//! AccessKit/layout contract: open or closed, this module only lays out either
//! a single top-level [`egui::Area`] overlay or a single hinted strip (again
//! an [`egui::Area`]) - never a native egui panel - so the UI-A2 one-Pane
//! contract and the UI-A4 five-region geometry keep holding with the palette
//! both open and closed.

use eframe::egui;

use crate::app::App;

#[path = "../commands.rs"]
mod commands;

/// Closed-strip hint shown in the command palette host strip.
const HINT: &str = "Ctrl+K to open the command palette";
/// Key under which the per-app palette state lives in the egui data store.
const STATE_KEY: &str = "ui_a6_palette_state";
/// Key under which the UI-A6 spy report (a `String`) lives in the same store.
const REPORT_KEY: &str = "ui_a6_palette_report";
/// Stable id of the filter text field (so focus can be requested on it).
const QUERY_ID: &str = "ui_a6_palette_query";
/// Fixed width (points) of the palette overlay.
const OVERLAY_WIDTH: f32 = 480.0;
/// Max height of the row scroll area inside the overlay.
const ROWS_HEIGHT: f32 = 300.0;

/// Per-app palette state, kept in the context data store (the only place this
/// card may keep mutable state without editing `app.rs`).
#[derive(Debug, Clone, Default, PartialEq)]
struct PaletteState {
    /// The palette overlay is currently open.
    open: bool,
    /// Live filter query in the palette's search field.
    query: String,
    /// Query the rows were last computed with (a filter edit resets the
    /// selection to the top row).
    last_query: String,
    /// Index of the currently selected row within the search results.
    selected: usize,
    /// Router-invocation counter (the UI-A6 spy): increments once per Enter
    /// that routed a command through [`commands::dispatch`].
    route_count: u32,
    /// Short, human-readable line for the last routed outcome, shown as
    /// feedback under the closed strip.
    last_outcome: Option<String>,
}

fn read_state(ctx: &egui::Context) -> PaletteState {
    ctx.data(|d| d.get_temp(egui::Id::unique(STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, state: &PaletteState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(STATE_KEY), state.clone()));
    write_report(ctx, state);
}

/// The UI-A6 routing spy, read back through the same `egui::Context` the app
/// renders into. Lines: `query=<filter text>`, `open=<true|false>`,
/// `count=<router invocations>`, `outcome=<last routed outcome>`. Stored as a
/// plain `String` so the palette test observes the app's real per-context state
/// even though this file is compiled twice under `#[path]` (the `String`
/// TypeId is shared; the app's private [`PaletteState`] is not). The binary
/// never reads it (the palette integration test does, via `#[path]`), hence
/// the dead-code allowance.
#[allow(dead_code)]
pub fn report(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default()
}

fn write_report(ctx: &egui::Context, state: &PaletteState) {
    let text = format!(
        "query={}\nopen={}\ncount={}\noutcome={}",
        state.query,
        state.open,
        state.route_count,
        state.last_outcome.as_deref().unwrap_or_default()
    );
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(REPORT_KEY), text));
}

/// Draw the command palette layer on top of the shell: the overlay when open,
/// otherwise a one-line hint in the palette host strip.
pub fn show(ctx: &egui::Context, app: &mut App) {
    let _ = app; // wave-A seam; B+ wiring mutates the editor session through it.
    let mut state = read_state(ctx);

    // Ctrl+K toggles the palette (Command on macOS, Ctrl elsewhere). Accepting
    // both keeps the app correct on every host and in every harness.
    let ctrl_k = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K))
        || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::K));
    if ctrl_k {
        state.open = !state.open;
        if state.open {
            state.selected = 0;
        }
    }

    if state.open {
        state = open_overlay(ctx, state);
    } else {
        closed_hint(ctx, &state);
    }

    write_state(ctx, &state);
}

/// Render the open palette overlay and handle its keyboard interaction.
fn open_overlay(ctx: &egui::Context, mut state: PaletteState) -> PaletteState {
    // Enter routes the selected row through the shared router. Consumed here
    // before the search field is drawn, so a focused field cannot also act on
    // it. Enter with no matching row routes nothing (the spy stays flat) and
    // the palette stays open so the query can be fixed - only a routed command
    // dismisses the overlay.
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
        let rows = commands::search(&state.query);
        if let Some(spec) = rows.get(state.selected) {
            match commands::dispatch(spec.name) {
                commands::Routed::Unknown { message, .. } => {
                    state.last_outcome = Some(format!("no route: {message}"));
                }
                commands::Routed::Registered { name, lane } => {
                    state.last_outcome = Some(format!("'{name}' routed - {lane}"));
                }
            }
            state.route_count = state.route_count.saturating_add(1);
            state.open = false;
        }
        return state;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        state.open = false;
        return state;
    }
    // Arrow keys move the selection (consumed so the focused search field does
    // not steal them for cursor movement - a command palette navigates rows).
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
        state.selected = state.selected.saturating_add(1);
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
        state.selected = state.selected.saturating_sub(1);
    }
    // A filter change resets the selection to the top matching row.
    if state.query != state.last_query {
        state.selected = 0;
        state.last_query = state.query.clone();
    }

    // The palette's command search is the shared registry, computed once per
    // frame from the current filter (empty filter = whole index, like the CLI).
    let rows = commands::search(&state.query);
    let total = commands::commands().len();
    let display = state.selected.min(rows.len().saturating_sub(1));

    // Keep the search field focused so typing flows straight into the filter.
    ctx.memory_mut(|m| m.request_focus(egui::Id::unique(QUERY_ID)));

    egui::Area::new(egui::Id::unique("ui_a6_palette_overlay"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .constrain(true)
        .fade_in(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(OVERLAY_WIDTH);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.label(egui::RichText::new("Command Palette").strong());
                    ui.add_space(4.0);
                    let edit = egui::TextEdit::singleline(&mut state.query)
                        .id(egui::Id::unique(QUERY_ID))
                        .hint_text("Type to filter commands")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(8.0, 6.0));
                    let _ = ui.add(edit);
                    ui.add_space(4.0);

                    if rows.is_empty() {
                        // Honest line for an unknown filter, never an empty
                        // list. Rendered as an unselected row so it stays a
                        // queryable, labelled surface (plain labels carry no
                        // AccessKit name in this egui version).
                        ui.add_space(2.0);
                        let response = ui.selectable_label(
                            false,
                            egui::RichText::new(format!(
                                "no command matches '{}'; run 'help' for the full index",
                                state.query
                            ))
                            .weak()
                            .italics(),
                        );
                        let _ = response;
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(ROWS_HEIGHT)
                            .show(ui, |ui| {
                                ui.add_space(2.0);
                                for (i, spec) in rows.iter().enumerate() {
                                    let is_selected = i == display;
                                    let label =
                                        format!("{} | {}", spec.name, commands::row_note(spec));
                                    let text = if is_selected {
                                        egui::RichText::new(&label).strong().monospace()
                                    } else {
                                        egui::RichText::new(&label).monospace()
                                    };
                                    let response = ui.selectable_label(is_selected, text);
                                    if response.on_hover_text(spec.summary).clicked() {
                                        state.selected = i;
                                    }
                                }
                            });
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("{total} commands registered"))
                                .weak()
                                .small(),
                        );
                    }
                });
            });
        });

    state.selected = display;
    state
}

/// One hinted line in the command palette host strip when the palette is
/// closed, so the surface stays discoverable (and the deterministic frame
/// shows the palette affordance). Right-anchored clear of the host label;
/// "last outcome" feedback rides the same line.
fn closed_hint(ctx: &egui::Context, state: &PaletteState) {
    let mut text = String::from(HINT);
    if let Some(outcome) = &state.last_outcome {
        text.push_str(&format!("  |  last: {outcome}"));
    }
    egui::Area::new(egui::Id::unique("ui_a6_palette_hint"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 5.0))
        .fade_in(false)
        .show(ctx, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(text).weak()));
        });
}
