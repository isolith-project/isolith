//! Modeler shell panels (wave A, card UI-A4).
//!
//! Each of the five named regions of the editor shell is rendered by one
//! module here. `app.rs` owns the region layout ([`crate::config::ShellLayout`
//! `::regions`]) and calls into these as the panel seam, so later cards fill a
//! panel without having to touch `app.rs`:
//!
//! - [`tree`]        - model tree (left column)
//! - [`viewport`]    - viewport (centre column); UI-A5 fills the live frame
//! - [`properties`]  - properties panel (right column)
//! - [`status`]      - status bar (bottom strip): document, selection, mode
//! - [`palette`]     - command palette host (top strip); UI-A6 fills it
//! - [`doc`]         - document lifecycle seam (open/save); UI-A7 fills it

pub mod doc;
pub mod palette;
pub mod properties;
pub mod status;
pub mod tree;
pub mod viewport;

// Wave-E (UI-E1 UI-ASSY-TREE): the assembly tree panel, hosted as the nested
// lower portion of the model-tree column (see `config::ShellRegions::assembly`).
// Like viewport.rs it will `#[path]`-include the E2-E4 assembly sub-panels
// (mates, diagnostics, explode) as those cards land.
pub mod assy_tree;

// UI-E6 UI-NAV-CUBE: the 3D orientation cube panel (registered here per the
// card; driven by one `nav_cube::update` call at the end of `nav::update`, so
// nav.rs is the registration call site and viewport.rs stays untouched).
pub mod nav_cube;

// The settings surface (UI-E6): `src/settings.rs`, reached by the panels as
// `panels::settings`. Registered in this module so no sibling file (app.rs,
// main.rs) has to change to host the settings surface.
#[path = "../settings.rs"]
pub mod settings;

use eframe::egui;

use oep_ui_theme::{Mode, Palette};

/// One of the named shell regions, as the shell-contrast treatment knows
/// them (UI-A9 UI-SHELL-CONTRAST).
///
/// `app.rs` owns the region rects ([`crate::config::ShellLayout::regions`])
/// and paints, behind each region's content, the frame [`region_frame`]
/// returns for that region - a raised fill distinct from the flat window
/// surface plus a 1px hairline drawn from the BORDER token, in both themes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionKind {
    /// Left column: the model tree.
    ModelTree,
    /// Centre column: the navigable viewport (CPU frame inside the hairline).
    Viewport,
    /// Right column: the properties inspector.
    Properties,
    /// Bottom strip: the status bar.
    StatusBar,
    /// Top strip: the command palette host.
    PaletteHost,
    /// Wave-E (UI-E1): the assembly tree, nested in the model-tree column's
    /// lower portion. Same raised panel treatment as every region.
    Assembly,
}

impl RegionKind {
    /// Every region, for tests and any whole-shell iteration.
    #[cfg(test)]
    pub const ALL: [RegionKind; 6] = [
        RegionKind::PaletteHost,
        RegionKind::ModelTree,
        RegionKind::Viewport,
        RegionKind::Properties,
        RegionKind::StatusBar,
        RegionKind::Assembly,
    ];
}

/// The `egui::Frame` treatment for a shell region: a raised fill (distinct
/// from the window [`Palette::surface`]) plus a 1px hairline from the BORDER
/// token, resolved for `mode` from the semantic tokens.
///
/// Every region - the two side panels named by the card, the viewport it
/// frames, and both strips - shares the raised fill + hairline so each reads
/// as a panel instead of labels on one flat surface; the window behind them
/// stays `SURFACE`. `mode` is resolved (never inverted by hand), so dark gets
/// the same deliberate treatment as light.
pub fn region_frame(kind: RegionKind, mode: Mode) -> egui::Frame {
    let p = Palette::resolve(mode);
    let _ = kind; // every region carries this wave's raised panel treatment
    egui::Frame::new()
        .fill(p.surface_raised)
        .stroke(egui::Stroke::new(1.0, p.border))
}

#[cfg(test)]
mod contrast_test {
    use super::*;
    use oep_ui_theme::{Mode, Palette};

    /// UI-A9 UI-SHELL-CONTRAST structural guard: if the shell is ever
    /// flattened again (a region's frame losing its hairline, or its fill
    /// collapsing back onto the window surface), this test fails. Asserted on
    /// the [`egui::Frame`]s the shell actually paints - never raw pixels.
    #[test]
    fn every_region_frame_carries_stroke_and_fill_distinct_from_window_surface() {
        for mode in [Mode::Light, Mode::Dark] {
            let p = Palette::resolve(mode);
            for kind in RegionKind::ALL {
                let frame = region_frame(kind, mode);
                assert!(
                    frame.stroke.width >= 1.0,
                    "{kind:?} ({mode:?}) frame must carry a hairline stroke"
                );
                assert_eq!(
                    frame.stroke.color, p.border,
                    "{kind:?} ({mode:?}) hairline must come from the BORDER token"
                );
                assert_eq!(
                    frame.fill, p.surface_raised,
                    "{kind:?} ({mode:?}) frame must be raised (SURFACE_RAISED)"
                );
                assert_ne!(
                    frame.fill, p.surface,
                    "{kind:?} ({mode:?}) frame fill must differ from the window surface"
                );
            }
        }
    }
}
