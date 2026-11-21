//! Semantic design tokens for the Isolith editor UI (editor parity program,
//! wave A, card UI-A3).
//!
//! The crate is intentionally tiny: tokens and their WCAG contrast proof.
//! [`tokens::Mode`] defaults to light; `crates/oep-app/src/theme.rs` installs
//! the resolved palette as the egui `Visuals` at app startup.

pub mod tokens;

pub use tokens::ACCENT;
pub use tokens::BORDER;
pub use tokens::DANGER;
pub use tokens::FOCUS_RING;
pub use tokens::SURFACE;
pub use tokens::SURFACE_RAISED;
pub use tokens::TEXT_MUTED;
pub use tokens::TEXT_PRIMARY;
pub use tokens::{Mode, Palette, Token};
