//! Launch configuration for the Isolith editor UI (wave A, card UI-A1).
//!
//! Window geometry, the product title bar string, and the headless smoke-mode
//! frame budget live here so the wave-A cards that follow can tune them in one
//! place.

/// Product name shown in the window title bar.
pub const APP_NAME: &str = "Isolith";

/// Initial window size (width, height) in logical points.
pub const WINDOW_SIZE: [f32; 2] = [1280.0, 800.0];

/// Bounded number of frames the `--smoke` mode runs before requesting close.
pub const SMOKE_FRAMES: u32 = 8;

/// Single title-bar string naming the product and version.
pub fn title() -> String {
    format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"))
}
