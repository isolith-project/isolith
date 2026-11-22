//! UI-B1 UI-INPUT-ROUTER: one event map to command tokens (wave B).
//!
//! The app's single input table. Pointer buttons and keys plus their
//! modifiers resolve here to the command tokens that `oep_shell::commands`
//! already validates, so the command palette, the keyboard, the mouse and the
//! future toolbars all speak one language: no widget invents its own binding,
//! and nothing in this crate mutates a document directly (the guard in
//! `tests/input_router.rs` fails the build if any module under `src/` writes
//! to a document outside `doc_io.rs`).
//!
//! Structure:
//!
//! - [`KEY_BINDINGS`] and [`POINTER_BINDINGS`] are the one table - every row
//!   declares the event shape (key or pointer button plus required
//!   [`Mods`]) and the command token it resolves to. `token` is the dispatch
//!   token the workbench CLI matches on (text before the first ':', e.g.
//!   `undo`, `sk`, `view`), and [`unknown_tokens`] proves every row speaks
//!   only tokens `oep_shell::commands::validate_token` accepts.
//! - [`resolve_key`] / [`resolve_pointer`] answer "does this key/button plus
//!   modifiers map to a command" with exact modifier semantics, so a modifier
//!   the row did not ask for does not match.
//! - [`poll`] is the per-frame wiring: it consumes the global command keys no
//!   panel owns, routes each through the shared registry naming, and records
//!   the outcome in a plain `String` report (the UI-B1 spy) under
//!   [`REPORT_KEY`] in the egui context store, exactly the oracle pattern the
//!   palette and doc panels use.
//!
//! The panel seam is deliberate: the palette (Ctrl+K), the doc panel
//! (Ctrl+O/S/R, Escape) and the viewport (drags, wheel) keep the interaction
//! keys they own because they consume them earlier in the frame; `app.rs`
//! runs [`poll`] **last**, so it only ever sees key presses no panel claimed.
//! The two bindings the card names are therefore untouched by this router:
//! the palette still opens on Ctrl+K and the doc panel still cancels on
//! Escape, proven by the existing UI-A6/UI-A7 suites in `cargo test
//! -p oep-app` staying green.
//!
//! Nothing here mutates a document: a routed token is validated and named,
//! never executed against the model - the execution lane is wired by later
//! waves through the same registry (`oep_shell::commands`).

use eframe::egui;

/// Key under which the router writes its `String` report into the egui
/// context data store. The test reads it back as a plain `String`, the same
/// cross-`#[path]`-copy-safe oracle the palette (A6) and doc (A7) panels use.
pub const REPORT_KEY: &str = "ui_b1_input_report";

/// The modifier requirement of one binding row, normalised to the three bits
/// the table cares about.
///
/// `command` is the whole Ctrl / Command / Mac-Command family treated as one
/// bit: the doc panel's shortcut reader already treats them interchangeably,
/// and on Windows/Linux egui-winit reports Ctrl with both `ctrl` and `command`
/// set, while on macOS a `Cmd` press sets `mac_cmd`. A binding that asks for
/// the command family matches a press carrying any of the three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mods {
    /// Any of Ctrl / Command / Mac-Command is down.
    pub command: bool,
    /// Shift is down.
    pub shift: bool,
    /// Alt/Option is down.
    pub alt: bool,
}

impl Mods {
    /// No modifier (a plain press).
    pub const NONE: Self = Self {
        command: false,
        shift: false,
        alt: false,
    };
    /// The command family only (e.g. Ctrl+Z).
    pub const COMMAND: Self = Self {
        command: true,
        shift: false,
        alt: false,
    };
    /// The command family plus shift (e.g. Ctrl+Shift+Z).
    pub const COMMAND_SHIFT: Self = Self {
        command: true,
        shift: true,
        alt: false,
    };
}

/// The physical event a binding maps from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventShape {
    /// A key press (the logical key, plus required modifiers).
    Key { key: egui::Key, mods: Mods },
    /// A pointer button (plus required modifiers).
    Pointer {
        button: egui::PointerButton,
        mods: Mods,
    },
}

/// One row of the single input table: a human label (the future keymap
/// overlay reads it), the command token it resolves to, and the event shape.
///
/// `token` is the dispatch token `oep_shell::commands` validates - never a
/// hand-written string that could diverge from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// Human readable name shown by a keymap surface (e.g. "Undo").
    pub label: &'static str,
    /// The command token this binding resolves to (validated by oep-shell).
    pub token: &'static str,
    /// The physical event (key or pointer button + modifiers).
    pub event: EventShape,
}

impl Binding {
    /// The key of a [`EventShape::Key`] binding.
    pub fn key(&self) -> Option<egui::Key> {
        match self.event {
            EventShape::Key { key, .. } => Some(key),
            EventShape::Pointer { .. } => None,
        }
    }

    /// The pointer button of a [`EventShape::Pointer`] binding.
    pub fn button(&self) -> Option<egui::PointerButton> {
        match self.event {
            EventShape::Key { .. } => None,
            EventShape::Pointer { button, .. } => Some(button),
        }
    }

    /// The modifiers the binding requires.
    pub fn mods(&self) -> Mods {
        match self.event {
            EventShape::Key { mods, .. } | EventShape::Pointer { mods, .. } => mods,
        }
    }
}

/// The keyboard half of the one table.
///
/// Order is load-bearing: the runtime consumes with egui's logical matching
/// (an extra Shift/Alt on the press is ignored), so a more-specific row for
/// the same key MUST precede the plain row - Redo (`Ctrl+Shift+Z`) is declared
/// before Undo (`Ctrl+Z`) so `Ctrl+Shift+Z` can never be mis-read as Undo.
/// Every token below passes `oep_shell::commands::validate_token` (asserted
/// by `--test input_router`).
pub const KEY_BINDINGS: &[Binding] = &[
    // Redo (with shift) before Undo: same key, more-specific first.
    Binding {
        label: "Redo",
        token: "redo",
        event: EventShape::Key {
            key: egui::Key::Z,
            mods: Mods::COMMAND_SHIFT,
        },
    },
    Binding {
        label: "Undo",
        token: "undo",
        event: EventShape::Key {
            key: egui::Key::Z,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Redo",
        token: "redo",
        event: EventShape::Key {
            key: egui::Key::Y,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "New Session",
        token: "editor",
        event: EventShape::Key {
            key: egui::Key::N,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Properties",
        token: "props",
        event: EventShape::Key {
            key: egui::Key::P,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Journal",
        token: "journal",
        event: EventShape::Key {
            key: egui::Key::G,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Help",
        token: "help",
        event: EventShape::Key {
            key: egui::Key::Slash,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Help",
        token: "help",
        event: EventShape::Key {
            key: egui::Key::F1,
            mods: Mods::NONE,
        },
    },
    Binding {
        label: "Solve Sketch",
        token: "sk",
        event: EventShape::Key {
            key: egui::Key::Enter,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Find Command",
        token: "search",
        event: EventShape::Key {
            key: egui::Key::L,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Frame Model",
        token: "view",
        event: EventShape::Key {
            key: egui::Key::F,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Mates",
        token: "assem",
        event: EventShape::Key {
            key: egui::Key::M,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Feature",
        token: "feat",
        event: EventShape::Key {
            key: egui::Key::D,
            mods: Mods::COMMAND,
        },
    },
    Binding {
        label: "Export",
        token: "export",
        event: EventShape::Key {
            key: egui::Key::E,
            mods: Mods::COMMAND,
        },
    },
    // Save/Open with shift: the doc panel owns the plain Ctrl+S / Ctrl+O.
    Binding {
        label: "Save Copy",
        token: "save",
        event: EventShape::Key {
            key: egui::Key::S,
            mods: Mods::COMMAND_SHIFT,
        },
    },
    Binding {
        label: "Open Model",
        token: "load",
        event: EventShape::Key {
            key: egui::Key::O,
            mods: Mods::COMMAND_SHIFT,
        },
    },
    Binding {
        label: "Variables",
        token: "vars",
        event: EventShape::Key {
            key: egui::Key::V,
            mods: Mods::COMMAND,
        },
    },
];

/// The pointer half of the one table.
///
/// These document the pointer language the viewport seam already implements
/// (orbit/pick = left button, pan = middle button or shift+left button) plus
/// the two extra buttons that map to the same undo/redo tokens the keyboard
/// uses, so a future toolbar binds through this table and never invents a
/// third spelling.
pub const POINTER_BINDINGS: &[Binding] = &[
    Binding {
        label: "Orbit / Pick",
        token: "view",
        event: EventShape::Pointer {
            button: egui::PointerButton::Primary,
            mods: Mods::NONE,
        },
    },
    Binding {
        label: "Pan",
        token: "view",
        event: EventShape::Pointer {
            button: egui::PointerButton::Middle,
            mods: Mods::NONE,
        },
    },
    Binding {
        label: "Pan",
        token: "view",
        event: EventShape::Pointer {
            button: egui::PointerButton::Primary,
            mods: Mods::COMMAND_SHIFT,
        },
    },
    Binding {
        label: "Undo",
        token: "undo",
        event: EventShape::Pointer {
            button: egui::PointerButton::Extra1,
            mods: Mods::NONE,
        },
    },
    Binding {
        label: "Redo",
        token: "redo",
        event: EventShape::Pointer {
            button: egui::PointerButton::Extra2,
            mods: Mods::NONE,
        },
    },
];

/// Every binding row, keyboard then pointer.
///
/// The future keymap overlay (UI-H2) and the command-reachability audit
/// (UI-H1) read this same seam instead of a second copy.
#[allow(dead_code)]
pub fn all_bindings() -> Vec<&'static Binding> {
    KEY_BINDINGS.iter().chain(POINTER_BINDINGS.iter()).collect()
}

/// The command tokens the table declares that `oep_shell::commands` does not
/// validate. Empty when the table stays faithful to the registry - the
/// self-check the acceptance runs over every row.
#[allow(dead_code)]
pub fn unknown_tokens() -> Vec<&'static str> {
    all_bindings()
        .into_iter()
        .filter(|b| oep_shell::commands::validate_token(b.token).is_err())
        .map(|b| b.token)
        .collect()
}

/// The normalised modifiers of an egui press: the command family collapses to
/// one bit, so Ctrl, Command and Mac-Command reach the same rows.
fn mods_from_egui(m: egui::Modifiers) -> Mods {
    Mods {
        command: m.command || m.ctrl || m.mac_cmd,
        shift: m.shift,
        alt: m.alt,
    }
}

/// The egui `Modifiers` to present to `consume_key` for a required [`Mods`].
///
/// Only the `command` bit is set for the command family; egui's
/// `matches_logically` then accepts a press carrying Ctrl, Command or
/// Mac-Command, and a purely logical match ignores an extra Shift/Alt on the
/// press (which is why the more-specific rows precede the plain ones).
fn egui_mods(m: Mods) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: false,
        shift: m.shift,
        mac_cmd: false,
        command: m.command,
    }
}

/// Resolve a key press (logical key plus modifiers) against the keyboard half
/// of the one table with **exact** modifier semantics: a modifier the row did
/// not ask for does not match (so `Ctrl+Shift+Z` resolves to the redo row, not
/// the plain undo row).
#[allow(dead_code)]
pub fn resolve_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<&'static Binding> {
    let want = mods_from_egui(modifiers);
    KEY_BINDINGS.iter().find(|b| {
        matches!(
            b.event,
            EventShape::Key { key: k, mods: m } if k == key && m == want
        )
    })
}

/// Resolve a pointer press (button plus modifiers) against the pointer half of
/// the one table with the same exact modifier semantics.
#[allow(dead_code)]
pub fn resolve_pointer(
    button: egui::PointerButton,
    modifiers: egui::Modifiers,
) -> Option<&'static Binding> {
    let want = mods_from_egui(modifiers);
    POINTER_BINDINGS.iter().find(|b| {
        matches!(
            b.event,
            EventShape::Pointer { button: bb, mods: m } if bb == button && m == want
        )
    })
}

/// The router state, carried as a plain `String` report in the egui context
/// store (the same cross-copy-safe oracle the palette and doc panels use).
#[derive(Debug, Clone, Default)]
struct RouterReport {
    /// Number of key presses this router has consumed and routed.
    count: u32,
    /// The last routed binding, rendered as `label -> token`.
    last: String,
    /// Label of the pointer binding currently held, or `-`.
    active_pointer: String,
}

fn read_report(ctx: &egui::Context) -> RouterReport {
    let text = ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default();
    let mut r = RouterReport::default();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("count=") {
            r.count = v.parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("last=") {
            r.last = v.to_string();
        } else if let Some(v) = line.strip_prefix("active_pointer=") {
            r.active_pointer = v.to_string();
        }
    }
    r
}

fn write_report(ctx: &egui::Context, r: &RouterReport) {
    let text = format!(
        "count={}\nbindings={}\nlast={}\nactive_pointer={}\n",
        r.count,
        KEY_BINDINGS.len() + POINTER_BINDINGS.len(),
        r.last,
        r.active_pointer
    );
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(REPORT_KEY), text));
}

/// The pointer binding currently held (read-only; the viewport seam owns the
/// actual drag/wheel interaction over the panel).
fn active_pointer_binding(ctx: &egui::Context) -> Option<&'static Binding> {
    ctx.input(|i| {
        POINTER_BINDINGS.iter().find(|b| {
            let Some(button) = b.button() else {
                return false;
            };
            i.pointer.button_down(button) && mods_from_egui(i.modifiers) == b.mods()
        })
    })
}

/// Per-frame router wiring. `app.rs` calls this **after** the panels, so every
/// panel-owned key wins first (palette Ctrl+K, doc Ctrl+O/S/R and Escape,
/// viewport drags/wheel) and only the global command keys reach the table:
///
/// - keyboard rows that match a press are consumed (so nothing downstream
///   re-sees them) and routed through the shared command registry naming, the
///   outcome recorded in the report oracle;
/// - pointer rows are resolved read-only each frame so a future toolbar shares
///   the same mapping without stealing the viewport's surface.
///
/// A routed token is validated and named, never executed: document mutation
/// stays out of this crate by the `--test input_router` guard.
pub fn poll(ctx: &egui::Context) {
    let mut report = read_report(ctx);

    // Keyboard: first row in table order whose press matches wins. The
    // more-specific rows precede plain ones on the same key (Redo > Undo), so
    // a logical match (extra Shift/Alt ignored by egui) still lands on the
    // right command.
    for binding in KEY_BINDINGS {
        let Some(key) = binding.key() else {
            continue;
        };
        let mods = binding.mods();
        if ctx.input_mut(|i| i.consume_key(egui_mods(mods), key)) {
            report.count = report.count.saturating_add(1);
            report.last = format!("{} -> {}", binding.label, binding.token);
            break; // one routed command per frame
        }
    }

    report.active_pointer = active_pointer_binding(ctx)
        .map(|b| b.label)
        .unwrap_or("-")
        .to_string();

    write_report(ctx, &report);
}

