//! WCAG 2.x contrast proof for the semantic tokens (editor parity program,
//! wave A, card UI-A3).
//!
//! Every token is tested in BOTH modes:
//! - text tokens (`text_primary`, `text_muted`)     >= 4.5:1 against both surfaces
//! - non-text tokens (accent, border, focus_ring, danger) >= 3:1 against both surfaces
//!
//! The thresholds come from the card acceptance: "4.5:1 text, 3:1 non-text,
//! light and dark". The relative-luminance function is the WCAG 2.x
//! definition, verified in Python at authoring time against the whole palette.

use egui::Color32;

use oep_ui_theme::{Mode, Palette};

/// WCAG 2.x sRGB relative luminance for one channel (0.0..=1.0).
fn channel_luminance(channel: u8) -> f64 {
    let c = channel as f64 / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance of an opaque colour (0.0..=1.0).
fn relative_luminance(c: Color32) -> f64 {
    0.2126 * channel_luminance(c.r())
        + 0.7152 * channel_luminance(c.g())
        + 0.0722 * channel_luminance(c.b())
}

/// WCAG 2.x contrast ratio between two opaque colours (>= 1.0).
fn contrast(a: Color32, b: Color32) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn assert_text_pair(mode: Mode, fg: Color32, bg: Color32, label: &str) {
    let ratio = contrast(fg, bg);
    assert!(
        ratio >= 4.5,
        "{mode:?} text token {label}: contrast {ratio:.2}:1 < 4.5:1 ({fg:?} on {bg:?})"
    );
}

fn assert_non_text_pair(mode: Mode, fg: Color32, bg: Color32, label: &str) {
    let ratio = contrast(fg, bg);
    assert!(
        ratio >= 3.0,
        "{mode:?} non-text token {label}: contrast {ratio:.2}:1 < 3:1 ({fg:?} on {bg:?})"
    );
}

#[test]
fn text_contrast_light() {
    let p = Palette::resolve(Mode::Light);
    // Text must clear 4.5:1 on every surface it can appear on.
    assert_text_pair(
        Mode::Light,
        p.text_primary,
        p.surface,
        "text_primary/surface",
    );
    assert_text_pair(
        Mode::Light,
        p.text_primary,
        p.surface_raised,
        "text_primary/surface_raised",
    );
    assert_text_pair(Mode::Light, p.text_muted, p.surface, "text_muted/surface");
    assert_text_pair(
        Mode::Light,
        p.text_muted,
        p.surface_raised,
        "text_muted/surface_raised",
    );
}

#[test]
fn text_contrast_dark() {
    let p = Palette::resolve(Mode::Dark);
    assert_text_pair(
        Mode::Dark,
        p.text_primary,
        p.surface,
        "text_primary/surface",
    );
    assert_text_pair(
        Mode::Dark,
        p.text_primary,
        p.surface_raised,
        "text_primary/surface_raised",
    );
    assert_text_pair(Mode::Dark, p.text_muted, p.surface, "text_muted/surface");
    assert_text_pair(
        Mode::Dark,
        p.text_muted,
        p.surface_raised,
        "text_muted/surface_raised",
    );
}

#[test]
fn non_text_contrast_light() {
    let p = Palette::resolve(Mode::Light);
    for (name, color) in [
        ("accent", p.accent),
        ("border", p.border),
        ("focus_ring", p.focus_ring),
        ("danger", p.danger),
    ] {
        assert_non_text_pair(Mode::Light, color, p.surface, &format!("{name}/surface"));
        assert_non_text_pair(
            Mode::Light,
            color,
            p.surface_raised,
            &format!("{name}/surface_raised"),
        );
    }
}

#[test]
fn non_text_contrast_dark() {
    let p = Palette::resolve(Mode::Dark);
    for (name, color) in [
        ("accent", p.accent),
        ("border", p.border),
        ("focus_ring", p.focus_ring),
        ("danger", p.danger),
    ] {
        assert_non_text_pair(Mode::Dark, color, p.surface, &format!("{name}/surface"));
        assert_non_text_pair(
            Mode::Dark,
            color,
            p.surface_raised,
            &format!("{name}/surface_raised"),
        );
    }
}
