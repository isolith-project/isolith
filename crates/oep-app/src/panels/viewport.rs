//! Viewport panel (wave A, card UI-A4).
//!
//! Renders the centre "viewport" region of the editor shell. On this card it
//! is a compiling stub: a named region with a placeholder line. UI-A5 fills
//! this file (the `ViewportBackend` seam and live frame), and UI-A7's doc
//! lifecycle, UI-A6's palette and this wave's other content cards build on
//! the stable region `app.rs` allocates here.

use eframe::egui;

use crate::app::App;

/// Draw the viewport panel content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    ui.label(egui::RichText::new("Viewport").strong().size(13.0));
    ui.separator();
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Viewport backend lands in UI-A5")
            .weak()
            .italics(),
    );
}
