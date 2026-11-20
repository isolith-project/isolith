//! Model tree panel (wave A, card UI-A4).
//!
//! Renders the left "model tree" region of the editor shell. On this card it
//! is a compiling stub: a named region with a placeholder line, so the shell
//! is a stable home the later document/feature-tree cards fill in. The region
//! rect itself is owned and sized by `app.rs` from [`crate::config`].

use eframe::egui;

use crate::app::App;

/// Draw the model tree panel content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    ui.label(egui::RichText::new("Model Tree").strong().size(13.0));
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("No model open").weak().italics());
}
