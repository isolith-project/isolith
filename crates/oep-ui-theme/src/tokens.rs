//! Semantic design tokens for the Isolith editor UI (editor parity program,
//! wave A, card UI-A3).
//!
//! Every colour the app draws is a [`Token`] carrying a light and a dark
//! value. No widget reads a literal colour: [`Palette::resolve`] resolves the
//! whole set for one [`Mode`], and `crates/oep-app/src/theme.rs` maps it onto
//! egui's `Visuals` at startup.
//!
//! Light is the default mode, per the product's standing preference. Dark
//! exists and is reachable (`oep_ui_theme::Mode::Dark`), but nothing defaults
//! to it.
//!
//! Contrast is tested against WCAG thresholds in `tests/contrast.rs`:
//! 4.5:1 for text-bearing tokens, 3:1 for non-text (graphical) tokens, both
//! modes.

use egui::Color32;

/// The supported theme modes. [`Default`] is [`Mode::Light`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Default mode: light surfaces, dark text, blue accent.
    #[default]
    Light,
    /// Optional dark mode. Reachable, never the default.
    Dark,
}

/// A semantic colour token: one value per theme mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// Value used by [`Mode::Light`].
    pub light: Color32,
    /// Value used by [`Mode::Dark`].
    pub dark: Color32,
}

impl Token {
    /// Build a token from its two mode values.
    pub const fn new(light: Color32, dark: Color32) -> Self {
        Self { light, dark }
    }

    /// Resolve this token for `mode`.
    pub fn for_mode(self, mode: Mode) -> Color32 {
        match mode {
            Mode::Light => self.light,
            Mode::Dark => self.dark,
        }
    }
}

// --- Semantic tokens ----------------------------------------------------
//
// Values chosen against the two mode surfaces (`SURFACE`, `SURFACE_RAISED`)
// so `tests/contrast.rs` passes WCAG 4.5:1 (text) and 3:1 (non-text) in both
// modes. Do not edit values here without re-running the contrast test.

/// Base application background.
pub const SURFACE: Token = Token::new(
    Color32::from_rgb(0xF6, 0xF7, 0xF9),
    Color32::from_rgb(0x1E, 0x21, 0x26),
);

/// Elevated surface: windows, panels, cards over [`SURFACE`].
pub const SURFACE_RAISED: Token = Token::new(
    Color32::from_rgb(0xFF, 0xFF, 0xFF),
    Color32::from_rgb(0x2A, 0x2E, 0x35),
);

/// Primary text on [`SURFACE`] or [`SURFACE_RAISED`].
pub const TEXT_PRIMARY: Token = Token::new(
    Color32::from_rgb(0x1B, 0x1F, 0x24),
    Color32::from_rgb(0xE6, 0xE8, 0xEB),
);

/// Secondary/muted text, placeholders, labels.
pub const TEXT_MUTED: Token = Token::new(
    Color32::from_rgb(0x5A, 0x64, 0x72),
    Color32::from_rgb(0xA8, 0xB0, 0xBC),
);

/// Brand/action accent: selection, primary buttons, active states.
pub const ACCENT: Token = Token::new(
    Color32::from_rgb(0x1F, 0x6F, 0xEB),
    Color32::from_rgb(0x58, 0xA6, 0xFF),
);

/// Control and window outlines.
pub const BORDER: Token = Token::new(
    Color32::from_rgb(0x82, 0x8B, 0x96),
    Color32::from_rgb(0x70, 0x7B, 0x8F),
);

/// Keyboard-focus indication.
pub const FOCUS_RING: Token = Token::new(
    Color32::from_rgb(0x1F, 0x6F, 0xEB),
    Color32::from_rgb(0x58, 0xA6, 0xFF),
);

/// Errors, destructive actions, warnings that must read as danger.
pub const DANGER: Token = Token::new(
    Color32::from_rgb(0xC6, 0x28, 0x28),
    Color32::from_rgb(0xF2, 0x6D, 0x5B),
);

/// A resolved set of semantic colours for one [`Mode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Base application background.
    pub surface: Color32,
    /// Elevated surfaces over [`Palette::surface`].
    pub surface_raised: Color32,
    /// Primary text.
    pub text_primary: Color32,
    /// Muted text.
    pub text_muted: Color32,
    /// Accent / action colour.
    pub accent: Color32,
    /// Borders.
    pub border: Color32,
    /// Focus indication.
    pub focus_ring: Color32,
    /// Danger / error colour.
    pub danger: Color32,
}

impl Palette {
    /// Resolve every token for `mode`.
    pub fn resolve(mode: Mode) -> Self {
        Self {
            surface: SURFACE.for_mode(mode),
            surface_raised: SURFACE_RAISED.for_mode(mode),
            text_primary: TEXT_PRIMARY.for_mode(mode),
            text_muted: TEXT_MUTED.for_mode(mode),
            accent: ACCENT.for_mode(mode),
            border: BORDER.for_mode(mode),
            focus_ring: FOCUS_RING.for_mode(mode),
            danger: DANGER.for_mode(mode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_light() {
        assert_eq!(Mode::default(), Mode::Light);
    }

    #[test]
    fn token_resolves_per_mode() {
        assert_eq!(TEXT_PRIMARY.for_mode(Mode::Light), TEXT_PRIMARY.light);
        assert_eq!(TEXT_PRIMARY.for_mode(Mode::Dark), TEXT_PRIMARY.dark);
        assert_ne!(TEXT_PRIMARY.light, TEXT_PRIMARY.dark);
    }

    #[test]
    fn every_token_differs_between_modes() {
        // Semantic tokens should actually re-skin the app between modes, not
        // silently collapse to the same colour.
        for token in [
            SURFACE,
            SURFACE_RAISED,
            TEXT_PRIMARY,
            TEXT_MUTED,
            ACCENT,
            BORDER,
            FOCUS_RING,
            DANGER,
        ] {
            assert_ne!(
                token.light, token.dark,
                "token must differ between modes: {token:?}"
            );
        }
    }

    #[test]
    fn palette_resolves_all_tokens() {
        for mode in [Mode::Light, Mode::Dark] {
            let p = Palette::resolve(mode);
            assert_eq!(p.surface, SURFACE.for_mode(mode));
            assert_eq!(p.surface_raised, SURFACE_RAISED.for_mode(mode));
            assert_eq!(p.text_primary, TEXT_PRIMARY.for_mode(mode));
            assert_eq!(p.text_muted, TEXT_MUTED.for_mode(mode));
            assert_eq!(p.accent, ACCENT.for_mode(mode));
            assert_eq!(p.border, BORDER.for_mode(mode));
            assert_eq!(p.focus_ring, FOCUS_RING.for_mode(mode));
            assert_eq!(p.danger, DANGER.for_mode(mode));
        }
    }
}
