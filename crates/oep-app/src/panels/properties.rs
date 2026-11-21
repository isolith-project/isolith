//! Properties panel (wave A, card UI-A4).
//!
//! Renders the right "properties" region of the editor shell. On this card it
//! is a compiling stub: a named region with placeholder lines for a future
//! selection inspector (per-feature parameters land in wave C). The region
//! rect is owned and sized by `app.rs` from [`crate::config`].

use eframe::egui;

use crate::app::App;
use crate::theme;

/// Draw the properties panel content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    ui.label(theme::heading(theme::mode_of(ui), "Properties").size(13.0));
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Nothing selected").weak().italics());
}
