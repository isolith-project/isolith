//! Document lifecycle seam (wave A, card UI-A4).
//!
//! Not one of the five fixed shell regions - this is the open/save seam UI-A7
//! owns. On this card it is a compiling stub so UI-A7 can fill it without
//! touching `app.rs`.
//!
//! The empty body is intentional and documented: UI-A7 renders open/save
//! (load STEP/CGR/3DXML, save `.oepdoc`, recent files, dirty-star title) at
//! whatever layer that card chooses, keeping the same `(ctx, app)` shape
//! `app.rs` already calls.

use eframe::egui;

use crate::app::App;

/// Draw the document lifecycle layer (UI-A7 fills the body).
pub fn show(ctx: &egui::Context, app: &mut App) {
    // TODO(UI-A7): open/save/recent-files surface over oep_geo serde and the
    // oep-import step bridge; title-bar document name + dirty star.
    let _ = (ctx, app);
}
