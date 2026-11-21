//! Status bar (wave A, card UI-A4).
//!
//! The bottom "status bar" region shows three fields - the active document
//! name, the current selection count and a mode field - each backed by a plain
//! field on [`Status`] that is stubbed to a safe default now and wired to real
//! model/session state by later cards (UI-A5 reports frame time here, UI-A7
//! reports the active document, and so on). Reading from a struct instead of
//! pulling live values keeps this panel a stable seam those cards can fill
//! without touching `app.rs` beyond the defaults.

use eframe::egui;

/// The three status bar fields, stubbed to safe defaults.
#[derive(Debug, Clone)]
pub struct Status {
    /// Name of the active document.
    pub document_name: String,
    /// Number of selected entities.
    pub selection_count: usize,
    /// Current interaction/mode field (e.g. orbit, sketch, select).
    pub mode: String,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            document_name: "Untitled".to_owned(),
            selection_count: 0,
            mode: "Select".to_owned(),
        }
    }
}

/// Draw the status bar content inside its already-positioned region.
pub fn show(ui: &mut egui::Ui, status: &Status) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Status Bar").strong().size(13.0));
        ui.separator();
        ui.label(egui::RichText::new(&status.document_name));
        ui.separator();
        ui.label(format!("{} selected", status.selection_count));
        ui.separator();
        ui.label(egui::RichText::new(&status.mode));
    });
}
