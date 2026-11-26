//! Launch configuration for the Isolith editor UI (wave A, card UI-A1).
//!
//! Window geometry, the product title bar string, and the headless smoke-mode
//! frame budget live here so the wave-A cards that follow can tune them in one
//! place.
//!
//! Since card UI-A4 the shell geometry also lives here: the five named
//! regions of the editor shell (model tree, viewport, properties, status bar,
//! command palette host) are laid out from a persisted [`ShellLayout`] so the
//! sizes survive a restart. Persistence is a tiny hand-rolled text format
//! (key=value lines); no serde or config crate is pulled in, keeping the
//! dependency surface exactly what the wave assigned.

use std::io;
use std::path::{Path, PathBuf};

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

/// Persisted geometry of the five-region editor shell.
///
/// Each field is the region's initial size on the corresponding axis. The
/// viewport takes whichever space is left between the side panels; it has no
/// size of its own. All sizes are clamped into a safe range when a layout is
/// built (see [`ShellLayout::regions`]), so a persisted value from an older or
/// hand-edited config can never make the shell overlap or clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellLayout {
    /// Height in points of the command palette host strip (top).
    pub palette_host_height: f32,
    /// Width in points of the model tree panel (left).
    pub model_tree_width: f32,
    /// Width in points of the properties panel (right).
    pub properties_width: f32,
    /// Height in points of the status bar strip (bottom).
    pub status_bar_height: f32,
}

impl Default for ShellLayout {
    fn default() -> Self {
        Self {
            palette_host_height: 32.0,
            model_tree_width: 240.0,
            properties_width: 300.0,
            status_bar_height: 26.0,
        }
    }
}

/// The computed region rects of the shell, in points.
///
/// The first five are the locked shell layout (UI-A4): palette host, model
/// tree, viewport, properties, status bar. [`ShellRegions::assembly`] is the
/// wave-E assembly-tree sub-region: it lives INSIDE the model-tree column
/// (the left column's lower portion) so the five named regions keep their
/// exact rects and the UI-A4 tile/no-overlap contract is untouched. The
/// assembly tree renders as a nested child Ui at that rect, like the model
/// tree content renders inside its own region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellRegions {
    pub palette_host: egui::Rect,
    pub model_tree: egui::Rect,
    pub viewport: egui::Rect,
    pub properties: egui::Rect,
    pub status_bar: egui::Rect,
    /// Wave-E (UI-E1): the assembly-tree region, nested in the lower portion
    /// of the model-tree column (same x-span, bottom 48%).
    pub assembly: egui::Rect,
}

impl ShellLayout {
    /// Serialise to the persisted key=value text form.
    ///
    /// `f32` values are written with Rust's shortest round-trip representation
    /// (the `{}` format, which `.parse::<f32>()` reads back to the same binary
    /// value), so a save/load round trip is lossless.
    pub fn as_text(&self) -> String {
        format!(
            "palette_host_height={}\nmodel_tree_width={}\nproperties_width={}\nstatus_bar_height={}\n",
            self.palette_host_height,
            self.model_tree_width,
            self.properties_width,
            self.status_bar_height
        )
    }

    /// Parse the persisted text form.
    ///
    /// Unknown or malformed lines are ignored; `None` is reserved for an
    /// unreadable file, so callers fall back to [`ShellLayout::default`].
    pub fn from_text(text: &str) -> Option<Self> {
        let mut layout = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Ok(v) = value.trim().parse::<f32>() else {
                continue;
            };
            match key.trim() {
                "palette_host_height" => layout.palette_host_height = v,
                "model_tree_width" => layout.model_tree_width = v,
                "properties_width" => layout.properties_width = v,
                "status_bar_height" => layout.status_bar_height = v,
                _ => {}
            }
        }
        Some(layout)
    }

    /// Persist the layout to `path`.
    ///
    /// The parent directory must already exist ([`Self::save_to`] never
    /// creates directories), so whoever owns the config directory is explicit
    /// about where state lives instead of sprinkling files across the host.
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.as_text())
    }

    /// Load a persisted layout from `path`; [`None`] when the file is absent
    /// or unreadable, so callers fall back to [`ShellLayout::default`].
    pub fn load_from(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        Self::from_text(&text)
    }

    /// Compute the five region rects inside `window`.
    ///
    /// The layout is anchored to the window edges:
    ///
    /// ```text
    /// +-----------------------------------+
    /// |         palette host             |  (top strip)
    /// +----------+-----------+-----------+
    /// | model    |  viewport | properties|  (three columns)
    /// | tree     |           |           |
    /// +----------+-----------+-----------+
    /// |              status bar          |  (bottom strip)
    /// +-----------------------------------+
    /// ```
    ///
    /// Sizes are clamped so the region rects always partition `window` with no
    /// overlap and positive area - never overlap or clip at any window size,
    /// in particular the 1280x800 and 1920x1080 acceptance sizes.
    pub fn regions(&self, window: egui::Rect) -> ShellRegions {
        let w = window.width().max(320.0);
        let h = window.height().max(240.0);

        // Top / bottom strips: keep each between a readable minimum and a
        // quarter of the window, so the side panels always keep room.
        let palette_height = self.palette_host_height.clamp(24.0, h * 0.25);
        let status_height = self.status_bar_height.clamp(22.0, h * 0.25);
        let middle_top = palette_height;
        let middle_bottom = h - status_height;

        // Side panels: each between a minimum width and 40% of the window, and
        // the pair may never claim more than the window minus a guaranteed
        // viewport strip (>= 180 pt), so the centre column never collapses.
        let tree_width = self.model_tree_width.clamp(140.0, w * 0.40);
        let props_max = (w - tree_width - 180.0).max(140.0);
        let props_width = self.properties_width.clamp(140.0, props_max.min(w * 0.40));

        let to_rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
        };

        ShellRegions {
            palette_host: to_rect(0.0, 0.0, w, middle_top),
            model_tree: to_rect(0.0, middle_top, tree_width, middle_bottom),
            viewport: to_rect(tree_width, middle_top, w - props_width, middle_bottom),
            properties: to_rect(w - props_width, middle_top, w, middle_bottom),
            status_bar: to_rect(0.0, middle_bottom, w, h),
            // Wave-E (UI-E1): the assembly-tree region is the lower 48% of the
            // model-tree column, nested INSIDE it so the five locked regions
            // keep their exact rects (UI-A4 tile/no-overlap contract intact).
            assembly: to_rect(
                0.0,
                middle_top + (middle_bottom - middle_top) * 0.52,
                tree_width,
                middle_bottom,
            ),
        }
    }
}

/// Where persisted layout state lives.
///
/// `ISOLITH_CONFIG_DIR` overrides the location so tests can point the app at a
/// throwaway directory (and so the app never touches a test runner's real
/// home). When unset the D-UI-8 default is used: `~/.config/isolith` on
/// Unix-like hosts.
pub fn layout_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("ISOLITH_CONFIG_DIR") {
        return PathBuf::from(dir).join("layout.conf");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("isolith")
            .join("layout.conf");
    }
    PathBuf::from("layout.conf")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test writes to its own throwaway directory so the tests never
    /// touch a real config dir and never race each other.
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "isolith-config-{}-{tag}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// The persisted layout round-trips losslessly: the same five sizes come
    /// back after a save, and the region rects they produce are identical
    /// before and after the round trip. This is the "restart persistence"
    /// contract of UI-A4.
    #[test]
    fn shell_layout_round_trips_same_sizes() {
        let layout = ShellLayout {
            palette_host_height: 48.0,
            model_tree_width: 300.0,
            properties_width: 380.0,
            status_bar_height: 30.0,
        };
        let text = layout.as_text();
        let back = ShellLayout::from_text(&text).expect("persisted text parses");
        assert_eq!(back, layout, "same sizes come back from the persisted text");

        // File round trip against a throwaway path.
        let dir = scratch_dir("roundtrip");
        let path = dir.join("round-trip-layout.conf");
        layout.save_to(&path).expect("save to temp config path");
        let loaded = ShellLayout::load_from(&path).expect("load from temp config path");
        assert_eq!(
            loaded, layout,
            "same sizes come back from a saved config file"
        );

        // And the geometry those sizes produce is unchanged by the round trip.
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
        assert_eq!(
            layout.regions(window).model_tree,
            loaded.regions(window).model_tree
        );
    }

    /// The region partition fills the window without overlap or clipping at
    /// both acceptance resolutions.
    #[test]
    fn regions_partition_window_without_overlap_at_acceptance_sizes() {
        for (w, h) in [(1280.0, 800.0), (1920.0, 1080.0)] {
            let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
            let layout = ShellLayout::default();
            let region = layout.regions(window);
            let rects = [
                region.palette_host,
                region.model_tree,
                region.viewport,
                region.properties,
                region.status_bar,
            ];

            // Every region inside the window, positive area.
            for rect in rects {
                assert!(
                    window.contains_rect(rect),
                    "region {rect:?} clipped at {w}x{h}"
                );
                assert!(
                    rect.width() > 0.0 && rect.height() > 0.0,
                    "region {rect:?} has non-positive area at {w}x{h}"
                );
            }

            // No two regions overlap: interiors are pairwise disjoint.
            for (i, a) in rects.iter().enumerate() {
                for b in rects.iter().skip(i + 1) {
                    let overlap = a.intersect(*b).width() * a.intersect(*b).height();
                    assert!(
                        overlap <= 0.001,
                        "regions {a:?} and {b:?} overlap at {w}x{h}"
                    );
                }
            }

            // The union covers the whole window (nothing stranded behind them).
            let union = rects
                .iter()
                .fold(egui::Rect::NOTHING, |acc, rect| acc.union(*rect));
            assert!(
                (union.width() - window.width()).abs() < 0.5
                    && (union.height() - window.height()).abs() < 0.5,
                "regions do not tile the window at {w}x{h}: union {union:?} vs {window:?}"
            );
        }
    }

    /// Unreadable or missing config falls back to the default layout, and a
    /// malformed line in an otherwise valid file is ignored rather than
    /// failing the parse.
    #[test]
    fn missing_or_malformed_config_falls_back_safely() {
        let dir = scratch_dir("fallback");
        let missing = dir.join("missing.conf");
        assert!(ShellLayout::load_from(&missing).is_none());

        let parsed =
            ShellLayout::from_text("model_tree_width=270\nbogus=oops\npalette_host_height=36\n");
        let layout = parsed.expect("malformed line ignored, rest parses");
        assert_eq!(layout.model_tree_width, 270.0);
        assert_eq!(layout.palette_host_height, 36.0);
        assert_eq!(
            layout.properties_width,
            ShellLayout::default().properties_width
        );
    }
}
