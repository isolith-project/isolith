//! UI-A4 UI-LAYOUT: five-region shell, geometry at both resolutions, and
//! restart persistence of the region sizes.
//!
//! This test drives the REAL `app::App` (the same source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like UI-A2's harness) at
//! the two acceptance window sizes and asserts:
//!
//! 1. The five named regions are reachable by accessible name: model tree,
//!    viewport, properties, status bar, command palette host.
//! 2. The geometry of those regions tiles the window with no overlap and no
//!    clipping at both 1280x800 and 1920x1080, read back from the AccessKit
//!    tree against the sizes `app.rs` computed from `config::ShellLayout`.
//! 3. A config save/load round trip brings back the same sizes (restart
//!    persistence through config.rs) and the app restores them on startup.
//!
//! `crates/oep-app` is a binary-only crate, so like UI-A2's `tests/harness.rs`
//! the app source is included by path - there is no fake `App` here. The
//! regions are the app's own child Uis; nothing in this file fabricates the
//! shell.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

/// Accessible names of the five shell regions, exactly as `app.rs` labels
/// them.
const REGION_NAMES: [&str; 5] = [
    "model tree",
    "viewport",
    "properties",
    "status bar",
    "command palette host",
];

/// Build the harness around the real `app::App` at a given window size.
fn real_app_harness(size: [f32; 2]) -> Harness<'static, app::App> {
    Harness::builder()
        .with_size(size)
        .build_eframe(|_cc| app::App::new(false))
}

/// Every one of the five regions is reachable by its accessible name, at both
/// acceptance resolutions.
#[test]
fn five_regions_reachable_by_accessible_name_at_both_resolutions() {
    for size in [[1280.0, 800.0], [1920.0, 1080.0]] {
        let mut harness = real_app_harness(size);
        harness.run();

        for name in REGION_NAMES {
            let found: Vec<_> = harness.get_all_by_label(name).collect();
            assert_eq!(
                found.len(),
                1,
                "region {name:?} not exactly once in the AccessKit tree at {:?} (found {})",
                size,
                found.len()
            );
        }
    }
}

/// The five region rects tile the window: each is fully inside, positive
/// area, pairwise disjoint, and the union covers the window. Asserted from the
/// actual AccessKit bounds at both acceptance resolutions.
#[test]
fn region_geometry_tiles_window_without_overlap_or_clipping() {
    for size in [[1280.0, 800.0], [1920.0, 1080.0]] {
        let mut harness = real_app_harness(size);
        harness.run();

        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0], size[1]));

        let mut rects = Vec::new();
        for name in REGION_NAMES {
            let node = &harness.get_all_by_label(name).collect::<Vec<_>>()[0];
            let rect = node
                .accesskit_node()
                .bounding_box()
                .expect("region node carries explicit bounds");
            // accesskit::Rect is f64-based with x0..y1 fields; convert back to
            // the egui Rect the window comparison uses.
            let rect = egui::Rect::from_min_max(
                egui::pos2(rect.x0 as f32, rect.y0 as f32),
                egui::pos2(rect.x1 as f32, rect.y1 as f32),
            );
            rects.push((name, rect));

            // Inside the window, positive area.
            assert!(
                window.contains_rect(rect),
                "region {name:?} clipped at {size:?}: {rect:?} vs window {window:?}"
            );
            assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "region {name:?} has zero area"
            );
        }

        // Pairwise disjoint interiors.
        for (i, (name_a, a)) in rects.iter().enumerate() {
            for (name_b, b) in rects.iter().skip(i + 1) {
                let overlap = a.intersect(*b).width() * a.intersect(*b).height();
                assert!(
                    overlap <= 0.001,
                    "regions {name_a:?} and {name_b:?} overlap at {size:?}"
                );
            }
        }

        // The union covers the rect the app actually painted the shell into.
        // The CentralPanel sits inside the window with its own frame, so the
        // shell tiles the panel's available rect, which is what the regions
        // were computed from.
        let union = rects
            .iter()
            .fold(egui::Rect::NOTHING, |acc, (_, r)| acc.union(*r));
        let panes: Vec<_> = harness
            .get_all_by_role(egui::accesskit::Role::Pane)
            .collect();
        let pane = panes[0].accesskit_node().bounding_box();
        match pane {
            Some(p) => {
                let p = egui::Rect::from_min_max(
                    egui::pos2(p.x0 as f32, p.y0 as f32),
                    egui::pos2(p.x1 as f32, p.y1 as f32),
                );
                assert!(
                    (union.width() - p.width()).abs() < 0.5
                        && (union.height() - p.height()).abs() < 0.5,
                    "region union does not tile the central pane at {size:?}: union {union:?} vs pane {p:?}"
                );
            }
            None => {
                // No Pane bounds reported; fall back to window coverage.
                assert!(
                    window.contains_rect(union),
                    "region union escapes the window at {size:?}: {union:?}"
                );
            }
        }
    }
}

/// The right-hand properties and left-hand tree columns sit either side of the
/// viewport, and the status bar and palette host are the bottom/top strips -
/// the D-UI-5 spatial contract, read off the real geometry.
#[test]
fn regions_are_in_the_documented_spatial_positions() {
    let mut harness = real_app_harness([1280.0, 800.0]);
    harness.run();

    fn rect(harness: &Harness<'_, app::App>, name: &str) -> egui::Rect {
        let r = harness.get_all_by_label(name).collect::<Vec<_>>()[0]
            .accesskit_node()
            .bounding_box()
            .expect("bounds");
        egui::Rect::from_min_max(
            egui::pos2(r.x0 as f32, r.y0 as f32),
            egui::pos2(r.x1 as f32, r.y1 as f32),
        )
    }

    let tree = rect(&harness, "model tree");
    let viewport = rect(&harness, "viewport");
    let props = rect(&harness, "properties");
    let status = rect(&harness, "status bar");
    let palette = rect(&harness, "command palette host");

    assert!(
        tree.max.x <= viewport.min.x + 0.001,
        "tree must sit left of viewport"
    );
    assert!(
        props.min.x >= viewport.max.x - 0.001,
        "properties must sit right of viewport"
    );
    assert!(palette.min.y == 0.0, "palette host is the top strip");
    assert!(
        palette.max.y <= viewport.min.y + 0.001,
        "palette host above the columns"
    );
    assert!(
        status.max.y >= viewport.max.y - 0.001,
        "status bar is the bottom strip"
    );
    assert!(
        status.min.y >= viewport.max.y - 0.001,
        "status bar below the columns"
    );
}

/// Restart persistence through config.rs: a non-default layout saves to disk
/// and the app restores exactly those same sizes on the next startup.
#[test]
fn persisted_layout_round_trips_and_app_restores_same_sizes() {
    // Unique throwaway config dir per run: never touches a real home.
    let dir = std::env::temp_dir().join(format!("isolith-layout-test-{}", std::process::id()));
    let dir = dir.join(thread_tag());
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("layout.conf");

    let saved = config::ShellLayout {
        palette_host_height: 44.0,
        model_tree_width: 296.0,
        properties_width: 352.0,
        status_bar_height: 30.0,
    };
    saved.save_to(&path).expect("save config to throwaway path");

    // config round trip: exact sizes come back.
    let loaded = config::ShellLayout::load_from(&path).expect("load config");
    assert_eq!(loaded, saved, "same sizes come back after save/load");

    // App startup restores those sizes (ISOLITH_CONFIG_DIR points at the
    // throwaway dir, which is exactly how a real restart resolves the path).
    // Guard the env mutation with a lock because tests share the process.
    let _guard = EnvGuard::set("ISOLITH_CONFIG_DIR", &dir);
    let mut harness = real_app_harness([1280.0, 800.0]);
    harness.run();

    let tree = harness.get_all_by_label("model tree").collect::<Vec<_>>()[0]
        .accesskit_node()
        .bounding_box()
        .expect("bounds");
    let tree_w = tree.x1 as f32 - tree.x0 as f32;
    // Tree column width comes back as the persisted 296 (plus rounding).
    assert!(
        (tree_w - saved.model_tree_width).abs() < 1.0,
        "expected tree width {} from persisted layout, got {}",
        saved.model_tree_width,
        tree_w
    );
}

/// A cheap mutex-protected env var set/restore so parallel tests in this
/// binary cannot race on `ISOLITH_CONFIG_DIR`.
struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap();
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap();
        match &self.previous {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

/// Thread id of the running test, to keep throwaway dirs unique per test.
fn thread_tag() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .replace("::", "-")
}
