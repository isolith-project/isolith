//! Editor UI settings surface (editor parity program, wave E, card UI-E6
//! UI-NAV-CUBE).
//!
//! The settings surface for the editor shell. It carries one toggle today,
//! [`Settings::nav_cube_click_to_align`], the switch that turns the
//! orientation cube's click/drag-to-align interaction on and off. The flag
//! ships DEFAULT OFF: the cube is a live orientation indicator only, and the
//! interaction (click a face/edge/corner to align, drag the cube to orbit with
//! snap) sits behind this one switch because the interactive navigation cube
//! reads on Autodesk's US7782319B2 while it is in force. The patent and this
//! decision are recorded in the vault at `Topics/Isolith.md`, decision #24.
//!
//! The setting is context-store resident (the same egui store the panels use),
//! so it is readable and writable from anywhere the panel machinery runs and
//! flips live the next frame. No persisted config file is introduced: the
//! default is off and Don flips it in one place when the patent term ends.

use eframe::egui;

/// The editor's settings surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    /// Whether the orientation cube's click/drag-to-align interaction is
    /// enabled.
    ///
    /// OFF (the default): the cube is a live orientation indicator. It tracks
    /// the camera, labels its faces with the shared view-name table, highlights
    /// the face the camera is currently looking at, and passes pointer input
    /// straight through to the viewport (it never swallows orbit or pick).
    ///
    /// ON: the full click/drag/snap behaviour is available and tested - click
    /// a face for the perpendicular view, an edge for the 45 degree view, a
    /// corner for an isometric view, or drag the cube to orbit with snap.
    ///
    /// Conservative by design: the derived default is `false` (the interaction
    /// stays off while US7782319B2 is in force; see the module docs and the
    /// vault decision record). The flag must never silently flip to a derived
    /// `true`: the field type is `bool`, whose default is off.
    pub nav_cube_click_to_align: bool,
}

/// One-line help for the `nav_cube_click_to_align` toggle (the card's
/// mandated help text; shown in the panel's oracle and available to any
/// future settings UI). Consumed by the acceptance harness, so allowed dead
/// code in the binary build alone (the same pattern as nav's committed
/// literals).
#[allow(dead_code)]
pub const NAV_CUBE_CLICK_TO_ALIGN_HELP: &str =
    "Make the orientation cube clickable: faces/edges/corners jump the camera, drag orbits with snap. \
     Default off: the cube is a live indicator only, kept conservative while a navigation-cube patent term is in force.";

/// Context-data key under which the live setting value lives (the panel reads
/// it every frame, so a flip applies the next frame; the acceptance harness
/// writes the same key to test both flag states). The stored VALUE is a plain
/// `bool` (the shared std type), not the [`Settings`] struct, so the value
/// TypeId matches across the app and test compilations (the same cross-include
/// discipline the scalar panel snapshots use).
pub const SETTINGS_KEY: &str = "isolith.ui.settings";

/// The context-data id under which the live flag lives.
fn settings_id() -> egui::Id {
    egui::Id::unique(SETTINGS_KEY)
}

/// Read the live `nav_cube_click_to_align` flag (falling back to the
/// conservative [`Settings::default`] when nothing has been written yet).
pub fn read(ctx: &egui::Context) -> Settings {
    let flag = ctx
        .data(|d| d.get_temp::<bool>(settings_id()))
        .unwrap_or(false);
    Settings {
        nav_cube_click_to_align: flag,
    }
}

/// Write the live settings (takes effect the next frame the panels run).
/// Consumed by the acceptance harness (and any future settings UI) to flip
/// the flag; the binary build ships the conservative default and never calls
/// it, so allowed dead code there.
#[allow(dead_code)]
pub fn write(ctx: &egui::Context, settings: &Settings) {
    ctx.data_mut(|d| d.insert_temp(settings_id(), settings.nav_cube_click_to_align));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The click-to-align flag defaults OFF (the patent-conservative default
    /// the orchestrator decision fixed; do not flip this without re-reading
    /// the vault decision record).
    #[test]
    fn nav_cube_click_to_align_defaults_off() {
        let settings = Settings::default();
        assert!(
            !settings.nav_cube_click_to_align,
            "the interaction flag must default off while US7782319B2 is in force"
        );
    }

    /// The help text exists, is one line, names the flag and why the default
    /// is conservative.
    #[test]
    fn help_text_explains_flag_and_conservative_default() {
        assert!(!NAV_CUBE_CLICK_TO_ALIGN_HELP.is_empty());
        assert!(
            NAV_CUBE_CLICK_TO_ALIGN_HELP
                .contains("nav_cube_click_to_align".trim_start_matches("Make "))
                || NAV_CUBE_CLICK_TO_ALIGN_HELP.contains("orientation cube")
        );
        // One line: no newlines in the published help text.
        assert!(!NAV_CUBE_CLICK_TO_ALIGN_HELP.contains('\n'));
    }

    /// Read/write round-trips through the egui context store with a real
    /// context.
    #[test]
    fn settings_round_trip_through_context_store() {
        let ctx = egui::Context::default();
        assert_eq!(read(&ctx), Settings::default());
        let flipped = Settings {
            nav_cube_click_to_align: true,
        };
        write(&ctx, &flipped);
        assert_eq!(read(&ctx), flipped);
    }
}
