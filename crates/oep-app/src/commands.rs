//! The editor app's routing seam onto the oep-shell command registry
//! (wave A, card UI-A6, UI-COMMAND-PALETTE).
//!
//! D-UI-2 resolves honestly: the palette, later menus/toolbars, keyboard
//! shortcuts and the CLI all route a typed command through the SAME oep-shell
//! registry - `oep_shell::commands`. This module is the app's half of that
//! seam. It deliberately owns no command list and forks nothing:
//!
//! - the search surface and the registry come straight from
//!   `oep_shell::commands::{search, commands, find, find_by_token,
//!   validate_token}` (re-exported below so the palette has exactly one
//!   module boundary to import from);
//! - routing validates the token with `oep_shell::commands::validate_token`,
//!   the exact function the workbench CLI (`oep-shell/examples/workbench.rs`)
//!   calls for every argument before it dispatches, so a command run from the
//!   palette and the same command run from the CLI can never drift.
//!
//! It is included into the crate from `panels/palette.rs` via `#[path]`
//! (module path `panels::palette::commands`), which is how this card owns only
//! the files it was granted without touching `main.rs` or `app.rs`.

pub use oep_shell::commands::{commands, find_by_token, search, validate_token, CommandSpec};

/// The honest marker shown in a palette row for a command that has neither a
/// shortcut nor a palette path - visible as a gap, never silently hidden.
pub const GAP_NOTE: &str = "(no shortcut or palette path)";

/// Which wave of the editor parity program wires a registered command's
/// execution lane into `oep-app`. The palette routes today; executing the
/// engine command is the job of the named wiring card.
fn lane_for(category: &str) -> &'static str {
    match category {
        "editor" | "inspection" => "execution lane lands in UI-B1 (editor input router)",
        "assembly" => "execution lane lands in UI-E (assembly editing)",
        "pdm" => "execution lane lands in UI-G (PDM lifecycle)",
        "interchange" => "execution lane lands in UI-A7 (open/save lifecycle)",
        _ => "handled by this palette surface",
    }
}

/// The typed outcome of routing one command line through the shared registry.
/// Never a panic, never fabricated output - the routing result, not a model
/// edit pretending to have happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// Not a registered token; carries the CLI's own actionable message
    /// (from `validate_token`).
    Unknown { token: String, message: String },
    /// A registered command. `lane` names the wave that wires its execution
    /// into the app.
    Registered {
        name: &'static str,
        lane: &'static str,
    },
}

/// Route one command line the way the CLI routes its arguments: validate the
/// first token against the shared registry, then name the command and the wave
/// that executes it. This is the single function the palette calls on Enter.
pub fn dispatch(line: &str) -> Routed {
    let token = line.trim().split(':').next().unwrap_or_default();
    match validate_token(token) {
        Err(message) => Routed::Unknown {
            token: token.to_string(),
            message,
        },
        Ok(_) => {
            let spec =
                find_by_token(token).expect("a token that validate_token accepts is registered");
            Routed::Registered {
                name: spec.name,
                lane: lane_for(spec.category),
            }
        }
    }
}

/// The shortcut currently bound to a command, if any. Today no binding map
/// exists, so every command falls back to its palette path below; the keymap
/// overlay (UI-H2) and the command-reachability audit (UI-H1) read this same
/// seam instead of adding a second source of truth.
pub fn shortcut_for(_name: &str) -> Option<&'static str> {
    None
}

/// The text a palette row shows next to the command label: the shortcut when
/// one is bound, else the palette path (the CLI usage line a user would type),
/// else an explicit [`GAP_NOTE`] so a command with neither stays visible as a
/// gap rather than being silently hidden.
pub fn row_note(spec: &CommandSpec) -> String {
    match shortcut_for(spec.name) {
        Some(shortcut) if !shortcut.trim().is_empty() => shortcut.to_string(),
        _ if !spec.usage.trim().is_empty() => spec.usage.to_string(),
        _ => GAP_NOTE.to_string(),
    }
}
