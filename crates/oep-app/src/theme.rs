//! Installs the semantic design tokens as the app-wide egui style/visuals
//! (editor parity program, wave A, card UI-A3).
//!
//! This is the single place outside `crates/oep-ui-theme` that knows how
//! tokens become visuals. Every colour this module installs comes from
//! [`oep_ui_theme::Palette`]; no widget anywhere else in `crates/oep-app` may
//! read a literal colour. The `guard` test module enforces that rule by
//! scanning this crate's `src/` for raw hex colour literals and direct
//! `Color32::from_*` constructions outside of this file.

use eframe::egui;
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{Color32, CornerRadius, Stroke, Visuals};

use oep_ui_theme::{Mode, Palette};

/// Build the egui [`Visuals`] for `mode` from the semantic palette.
pub fn visuals(mode: Mode) -> Visuals {
    let p = Palette::resolve(mode);
    let mut v = match mode {
        Mode::Light => Visuals::light(),
        Mode::Dark => Visuals::dark(),
    };
    v.dark_mode = mode == Mode::Dark;

    // --- Surfaces ---------------------------------------------------------
    v.panel_fill = p.surface;
    v.window_fill = p.surface_raised;
    v.extreme_bg_color = p.surface_raised;
    v.faint_bg_color = p.surface_raised;
    v.code_bg_color = p.surface_raised;
    v.window_stroke.color = p.border;

    // --- Text & links -----------------------------------------------------
    // override_text_color makes every widget's text resolve from the token,
    // so no widget needs its own colour.
    v.override_text_color = Some(p.text_primary);
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.danger;

    // --- Selection ---------------------------------------------------------
    let selection_ink = best_on(p.accent, &[p.surface_raised, p.text_primary, p.text_muted]);
    v.selection = Selection {
        bg_fill: p.accent,
        stroke: Stroke::new(1.0, selection_ink),
    };

    // --- Widget states ------------------------------------------------------
    let active_ink = best_on(p.accent, &[p.surface_raised, p.text_primary]);
    v.widgets = Widgets {
        noninteractive: widget(p.surface_raised, p.border, p.text_primary),
        inactive: widget(p.surface_raised, p.border, p.text_primary),
        hovered: widget(p.surface_raised, p.focus_ring, p.text_primary),
        active: widget(p.accent, p.focus_ring, active_ink),
        ..v.widgets
    };

    v
}

/// A [`WidgetVisuals`] whose surfaces, outline and foreground all come from
/// tokens, with the same corner radius egui uses at rest and zero expansion.
fn widget(bg: Color32, outline: Color32, fg: Color32) -> WidgetVisuals {
    WidgetVisuals {
        weak_bg_fill: bg,
        bg_fill: bg,
        bg_stroke: Stroke::new(1.0, outline),
        fg_stroke: Stroke::new(1.0, fg),
        corner_radius: CornerRadius::same(6),
        expansion: 0.0,
    }
}

/// Pick, from `candidates`, the token whose contrast against `bg` is best,
/// so text painted on a coloured fill (e.g. an accent button) stays readable
/// without ever naming a literal colour.
fn best_on(bg: Color32, candidates: &[Color32]) -> Color32 {
    candidates
        .iter()
        .copied()
        .max_by(|a, b| {
            contrast(*a, bg)
                .partial_cmp(&contrast(*b, bg))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or_default()
}

/// WCAG 2.x contrast ratio between two opaque colours (>= 1.0).
fn contrast(a: Color32, b: Color32) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// WCAG 2.x relative luminance of an opaque colour (0.0..=1.0).
fn relative_luminance(c: Color32) -> f64 {
    fn channel(ch: u8) -> f64 {
        let c = ch as f64 / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// Install the tokens as the egui style for `mode` on `ctx`.
///
/// `main.rs` calls this at startup with [`Mode::default()`] (light), which is
/// the product's standing default. Dark is reachable by passing
/// [`Mode::Dark`]; nothing defaults to it.
pub fn install(ctx: &egui::Context, mode: Mode) {
    ctx.set_visuals(visuals(mode));
}

// ---------------------------------------------------------------------------
// Guard: no raw hex colour literal, and no direct colour constructor, anywhere
// in crates/oep-app/src except this file. Enforced as a test so the rule is
// load-bearing, not documented.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod guard {
    #[test]
    fn no_raw_hex_or_direct_colour_construction_in_crates_oep_app_src() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut violations: Vec<(std::path::PathBuf, usize, String)> = Vec::new();

        collect_rust_files(&root, &mut |path, line_no, line| {
            if path.ends_with("theme.rs") {
                return; // this file is the sanctioned mapping seam
            }
            for hit in hex_literals_in(line) {
                violations.push((path.to_path_buf(), line_no, hit));
            }
            for hit in colour_constructors_in(line) {
                violations.push((path.to_path_buf(), line_no, hit));
            }
        });

        assert!(
            violations.is_empty(),
            "raw colour literals outside crates/oep-app/src/theme.rs:\n{}",
            violations
                .iter()
                .map(|(p, n, hit)| format!("  {}:{}: {hit}", p.display(), n))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    fn collect_rust_files(
        dir: &std::path::Path,
        visit: &mut impl FnMut(&std::path::Path, usize, &str),
    ) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, visit);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("read source file");
                for (idx, line) in text.lines().enumerate() {
                    visit(&path, idx + 1, line);
                }
            }
        }
    }

    fn is_hex(b: u8) -> bool {
        b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || (b'A'..=b'F').contains(&b)
    }

    /// Find raw hex colour literals: `0xRRGGBB`, `0xRRGGBBAA` and `#RRGGBB`,
    /// `#RRGGBBAA`.
    fn hex_literals_in(line: &str) -> Vec<String> {
        let bytes = line.as_bytes();
        let mut hits = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'0'
                && i + 1 < bytes.len()
                && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
            {
                let start = i;
                i += 2;
                let mut digits = 0;
                while i < bytes.len() && is_hex(bytes[i]) {
                    digits += 1;
                    i += 1;
                }
                if digits == 6 || digits == 8 {
                    hits.push(format!("hex literal {}", &line[start..i]));
                }
                continue;
            }
            if bytes[i] == b'#' {
                let start = i;
                i += 1;
                let mut digits = 0;
                while i < bytes.len() && is_hex(bytes[i]) {
                    digits += 1;
                    i += 1;
                }
                if digits == 6 || digits == 8 {
                    hits.push(format!("hex colour {}", &line[start..i]));
                }
                continue;
            }
            i += 1;
        }
        hits
    }

    const CONSTRUCTORS: &[&str] = &[
        "Color32::from_rgb",
        "Color32::from_rgba",
        "Color32::from_rgba_premultiplied",
        "Color32::from_rgba_unmultiplied",
        "Color32::from_gray",
        "Color32::from_black_alpha",
        "Color32::from_white_alpha",
        "Color32::from_additive_luminance",
        "Color32::from_rgb_additive",
    ];

    /// Find direct `Color32::from_*` constructions (a literal colour written
    /// in place instead of resolved from a token).
    fn colour_constructors_in(line: &str) -> Vec<String> {
        CONSTRUCTORS
            .iter()
            .filter(|c| line.contains(**c))
            .map(|c| format!("direct colour construction `{c}`"))
            .collect()
    }

    #[test]
    fn the_guard_tools_themselves_detect_literals() {
        // Prove the scanner sees what it is meant to see (sanity of the guard
        // itself): symbolic colours and short hex numbers pass, full colour
        // hex literals and direct constructors trip.
        assert!(hex_literals_in("let x = Color32::WHITE;").is_empty());
        assert!(hex_literals_in("let c = 0xF6F7F9;").len() == 1);
        assert!(hex_literals_in(r##"let css = "#1F6FEB";"##).len() == 1);
        assert!(colour_constructors_in("Color32::from_rgb(1,2,3)").len() == 1);
        assert!(hex_literals_in("let n = 0x10; break;").is_empty());
    }
}
