//! Command palette host (wave A, card UI-A4).
//!
//! The top "command palette host" region of the editor shell. On this card it
//! is a compiling stub - the seam UI-A6 fills with the searchable command
//! surface - so UI-A6 can own this one file without touching `app.rs`.
//!
//! The empty body is intentional and documented: UI-A6 replaces it with a
//! palette over `oep_shell::commands::search` opened on Ctrl+K, keeping the
//! same `(ctx, app)` shape `app.rs` already calls.

use eframe::egui;

use crate::app::App;

/// Draw the command palette layer (UI-A6 fills the body).
pub fn show(ctx: &egui::Context, app: &mut App) {
    // TODO(UI-A6): searchable command surface over oep_shell::commands::search;
    // opened on Ctrl+K, filters as you type, Enter runs through the same router
    // as the CLI. Until then the host region renders as an empty strip.
    let _ = (ctx, app);
}
