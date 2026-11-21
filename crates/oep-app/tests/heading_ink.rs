//! UI-A12 UI-HEADING-INK: strong text is painted in the accent ink and
//! disappears on light panels.
//!
//! Behaviour this card restores: every heading and emphasised label in the
//! shell reads in both themes - dark on light surfaces, light on dark
//! surfaces. Root cause (verified against the vendored egui 0.37 source):
//! `RichText::strong()` resolves its colour from
//! `Visuals::strong_text_color()`, which is `widgets.active.fg_stroke` - the
//! best-on-accent ink, correct only on an accent-filled button. The app
//! installed that accent ink as `widgets.active` (white on the light blue),
//! so every `.strong()` heading was painted in the panel's own fill colour
//! and vanished.
//!
//! The fix: headings go through [`theme::heading`], which pins the
//! `TEXT_PRIMARY` token for the installed mode as an explicit `RichText`
//! colour (egui resolves an explicit colour before the `strong()` ink), and
//! the one emphasised label genuinely painted on the accent fill (the
//! selected command-palette row) goes through [`theme::on_accent`], which
//! pins the best-on-accent ink instead.
//!
//! These tests assert never on literal rgb: every colour is compared against
//! the `oep_ui_theme` token values (or the values [`theme::visuals`] derives
//! from them), and "the app's headings are produced through the helper" is
//! asserted in source, because colour is not carried in the AccessKit tree.

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use oep_ui_theme::{Mode, Palette, TEXT_PRIMARY};

/// `theme::heading` in light carries `TEXT_PRIMARY` - dark ink on the light
/// panels - not the accent/active ink that was making headings vanish.
#[test]
fn heading_carries_text_primary_in_light() {
    assert_eq!(
        theme::heading(Mode::Light, "Model Tree").color_override(),
        Some(TEXT_PRIMARY.for_mode(Mode::Light)),
        "light heading must resolve TEXT_PRIMARY, not the accent ink"
    );
}

/// `theme::heading` in dark carries `TEXT_PRIMARY` - light ink on the dark
/// panels.
#[test]
fn heading_carries_text_primary_in_dark() {
    assert_eq!(
        theme::heading(Mode::Dark, "Model Tree").color_override(),
        Some(TEXT_PRIMARY.for_mode(Mode::Dark)),
        "dark heading must resolve TEXT_PRIMARY, not the accent ink"
    );
}

/// The fix, stated directly: a heading is never the ink a bare `.strong()`
/// would paint (`strong_text_color()`, i.e. the active-widget ink). It is
/// `TEXT_PRIMARY` in both modes, and in both it differs from the value
/// `RichText::strong()` derives.
#[test]
fn heading_is_not_the_strong_text_ink() {
    for mode in [Mode::Light, Mode::Dark] {
        let strong_ink = theme::visuals(mode).strong_text_color();
        let heading_ink = theme::heading(mode, "Model Tree").color_override();
        assert_eq!(heading_ink, Some(TEXT_PRIMARY.for_mode(mode)));
        assert_ne!(
            heading_ink,
            Some(strong_ink),
            "{mode:?}: heading must not paint the active/strong ink"
        );
    }
}

/// The two helpers are distinct: headings use `TEXT_PRIMARY` (neutral
/// surfaces) while the selected palette row uses the best-on-accent ink
/// (it paints on the accent fill). Proving they differ in both modes keeps a
/// future `on_accent` slip (e.g. swapping to `TEXT_PRIMARY`) visible as a
/// test failure, and keeps the selected-row accent behaviour exactly what a
/// `.strong()` painted there before.
#[test]
fn on_accent_carries_the_accent_ink_and_differs_from_heading() {
    for mode in [Mode::Light, Mode::Dark] {
        let accent_ink = theme::visuals(mode).strong_text_color();
        assert_eq!(
            theme::on_accent(mode, "row").color_override(),
            Some(accent_ink),
            "{mode:?}: selected-row ink must match the accent fill ink"
        );
        assert_ne!(
            theme::on_accent(mode, "row").color_override(),
            theme::heading(mode, "row").color_override(),
            "{mode:?}: the accent-fill ink and the heading ink must differ"
        );
        // in light the accent ink is the raised surface (white), asserted
        // from the token, never a literal rgb.
        assert_eq!(
            theme::on_accent(Mode::Light, "row").color_override(),
            Some(Palette::resolve(Mode::Light).surface_raised),
        );
    }
}

// -------------------------------------------------------------------------
// The app's heading strings are produced through the helper. Asserted in
// source because a rendered heading's colour is not exposed through the
// AccessKit tree; the no-strong guard in theme.rs additionally fails the
// whole crate on any bare `.strong()`.
// -------------------------------------------------------------------------

fn read_src(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Each fixed shell heading (matched by its quoted string so prose mentions
/// do not qualify) is produced through `theme::heading(` on the line that
/// carries it.
#[test]
fn shell_heading_strings_are_produced_through_heading_helper() {
    let headings: &[(&str, &str)] = &[
        ("app.rs", "\"Command Palette Host\""),
        ("panels/tree.rs", "\"Model Tree\""),
        ("panels/status.rs", "\"Status Bar\""),
        ("panels/properties.rs", "\"Properties\""),
        ("panels/palette.rs", "\"Command Palette\""),
        ("panels/doc.rs", "\"Recent Documents (Esc to close)\""),
    ];
    for (rel, needle) in headings {
        let text = read_src(rel);
        let lines: Vec<&str> = text.lines().collect();
        let hits: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(needle))
            .map(|(i, _)| i + 1) // 1-indexed line numbers
            .collect();
        assert!(
            !hits.is_empty(),
            "{needle} must appear in {rel} (did the panel rename its heading?)"
        );
        for hit in hits {
            // rustfmt wraps long heading calls onto several lines, so check a
            // generous window around the label: the `theme::heading(` opener
            // must be present nearby and no `.strong(` anywhere in the call.
            let lo = (hit - 1).saturating_sub(5);
            let hi = (hit + 1).min(lines.len());
            let window = &lines[lo..hi];
            assert!(
                window.iter().any(|l| l.contains("theme::heading(")),
                "{needle} (near line {hit} in {rel}) must be produced through theme::heading, got:\n  {}",
                window.join("\n  ")
            );
            assert!(
                !window.iter().any(|l| l.contains(".strong(")),
                "{needle} (near line {hit} in {rel}) must not use .strong(), got:\n  {}",
                window.join("\n  ")
            );
        }
    }
}

/// The dynamic emphasised labels - the tree's file-name line and the palette's
/// selected row - go through the sanctioned helpers too: `heading` for the
/// neutral-surface file name, `on_accent` for the row painted on the accent
/// fill.
#[test]
fn dynamic_heading_sites_use_the_sanctioned_helpers() {
    let tree = read_src("panels/tree.rs");
    let file_line = tree
        .lines()
        .find(|l| l.contains("theme::heading(") && l.contains("file_name"))
        .expect("tree file-name heading must be produced through theme::heading");
    assert!(
        !file_line.contains(".strong("),
        "tree file-name must not use .strong(): {file_line}"
    );

    let palette = read_src("panels/palette.rs");
    let palette_lines: Vec<&str> = palette.lines().collect();
    // The palette has exactly one emphasised text path - the selected row,
    // which paints on the accent fill - routed through `theme::on_accent`.
    // Its only other text path is the plain unselected monospace branch, so
    // the presence of `on_accent` (and the absence of `.strong`, above) pins
    // the selected row to the sanctioned accent helper.
    let on_accent_line = palette_lines
        .iter()
        .find(|l| l.contains("theme::on_accent("))
        .expect("selected palette row must be produced through theme::on_accent");
    assert!(
        !on_accent_line.contains(".strong("),
        "selected palette row must not use .strong(): {on_accent_line}"
    );
    assert!(
        palette
            .lines()
            .any(|l| l.contains("is_selected") && l.contains("let text =")),
        "palette must keep its selected/unselected row distinction"
    );
}
