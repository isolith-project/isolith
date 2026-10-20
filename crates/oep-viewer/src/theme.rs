//! P7-M5-T02: theme baseline for the viewer surface.
//!
//! The theme is the single source for background and part colors: the
//! renderer consumes these constants (never a private copy), and the theme is
//! accessible by construction:
//!
//! - the background is a dark neutral (`BACKGROUND_RGB`),
//! - part colors are stable per part id (a simple hash) with a fixed
//!   saturation/value so every hue keeps a WCAG 1.4.11 non-text contrast
//!   ratio of at least [`MIN_NON_TEXT_CONTRAST`] against the background,
//!   asserted over the full hue sweep by
//!   `crates/oep-viewer/tests/accessibility_theme.rs`,
//! - selection identity is never encoded in color (it is the structured
//!   [`crate::pick::Selection`]), so the palette is a reference cue only.
//!
//! Contrast math follows the WCAG relative-luminance definition and uses
//! `sRGB -> linear` per the spec.
//!
//! Accessibility checklist rows: VW-A11Y-04 (contrast floor) and
//! VW-A11Y-06 (single source) in `workstreams/modeler/quality/accessibility-checklist.md`.

/// Theme background color (dark neutral RGB, opaque).
pub const BACKGROUND_RGB: [u8; 3] = [28, 30, 34];

/// WCAG 1.4.11 non-text contrast floor for graphical objects (3.0:1).
pub const MIN_NON_TEXT_CONTRAST: f64 = 3.0;

/// Saturation of the part palette, fixed so all hues stay bright enough.
const PALETTE_SATURATION: f64 = 0.65;

/// Value (brightness) of the part palette, tuned so the darkest hue (blue)
/// still clears the non-text contrast floor with margin.
const PALETTE_VALUE: f64 = 1.0;

/// Convert one sRGB channel (0..255) to linear light per WCAG.
fn srgb_to_linear(c: u8) -> f64 {
    let u = c as f64 / 255.0;
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an sRGB color.
pub fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let [r, g, b] = rgb;
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

/// WCAG contrast ratio between two sRGB colors (1.0 .. 21.0).
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Convert HSV (h in degrees, s and v in [0,1]) to an opaque RGBA color.
fn hsv_to_rgba(h: f64, s: f64, v: f64) -> [u8; 4] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
        255,
    ]
}

/// The part palette color for a hue in degrees, using the fixed theme
/// saturation and value.
pub fn part_color_hue(hue_degrees: f64) -> [u8; 4] {
    hsv_to_rgba(hue_degrees, PALETTE_SATURATION, PALETTE_VALUE)
}

/// Stable per-part color from a simple string hash (fixed across runs, no std
/// HashMap randomness involved). Reference cue only: selection identity is the
/// structured [`crate::pick::Selection`], never pixel color.
pub fn part_color(part_id: &str) -> [u8; 4] {
    let mut h: u32 = 2166136261;
    for b in part_id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let hue = (h >> 8) % 360;
    part_color_hue(hue as f64)
}

/// The worst (lowest) WCAG contrast ratio of any part palette color against
/// the theme background, measured over the full hue sweep.
pub fn worst_part_contrast() -> f64 {
    let mut worst = f64::INFINITY;
    for h in 0..360 {
        let c = part_color_hue(h as f64);
        let ratio = contrast_ratio([c[0], c[1], c[2]], BACKGROUND_RGB);
        if ratio < worst {
            worst = ratio;
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_of_a_color_with_itself_is_1() {
        assert!((contrast_ratio([10, 20, 30], [10, 20, 30]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn white_on_black_exceeds_17() {
        assert!(contrast_ratio([255, 255, 255], [0, 0, 0]) > 17.0);
    }
}
