//! UI-C6 UI-KERNEL-FEATURES: fillet, chamfer, shell and boolean dialogs over
//! the OCC seam (wave C).
//!
//! Drives the REAL `kernel_features::show` panel (the exact source that ships
//! with the app, pulled in verbatim via `#[path]` like every oep-app panel
//! test) through `egui_kittest` and asserts the card's acceptance:
//!
//! - **availability is detected, never assumed**: the panel opens a real
//!   `oep_geo` session at runtime. With the oep-geo `kernel` feature on (the
//!   real OCCT engine linked) the oracle reports `kernel=1` and the dialogs
//!   run REAL geometry; with it off (the default workspace build, no OCCT)
//!   every dialog honestly reports the feature unavailable with ZERO fake
//!   geometry (`geometry=0`, `faces=-`). Both builds compile and green the
//!   SAME test file: each test reads `kernel=` from the panel's own oracle
//!   and asserts the branch the detection produced.
//! - **with the kernel: fillet produces the expected face count on the
//!   fixture**: running the fillet dialog (default radius 0.2) on the
//!   committed box-reference unit cube yields exactly the seam golden's face
//!   count (7) at the seam golden's volume.
//! - **the unavailable path is a tested state with an honest message**: shell
//!   (which has no operation in the seam) and every dialog under a missing
//!   kernel report a readable reason and never fabricate a face count or
//!   volume. Invalid dialog inputs are refused with the field + reason named
//!   (a typed refusal, never a panic).
//!
//! All assertions read the app's real state: the panel's oracle
//! ([`kernel_features::KERNEL_REPORT_KEY`]), the AccessKit tree (Run buttons,
//! text inputs by rect) and the deterministic widget rects the renderer uses.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact kernel-features panel source, included verbatim (the same
// `#[path]` copy pattern every wave-B/C panel card uses).
#[path = "../src/panels/kernel_features.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod kernel_features;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use egui::PointerButton;

/// The panel region as the harness lays the panel out (the `ui.max_rect()` of
/// `build_ui_state`, the same convention the C3 variables test uses).
fn window_region() -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(8.0, 8.0),
        egui::pos2(config::WINDOW_SIZE[0] - 8.0, config::WINDOW_SIZE[1] - 8.0),
    )
}

/// Build a harness around the REAL `kernel_features::show` with a real
/// `app::App` as state (no seed: the panel builds the committed box-reference
/// fixture itself the moment a kernel session is detected).
fn panel_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_ui_state(kernel_features::show, app::App::new(false))
}

/// The panel oracle, read back through the same context.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(kernel_features::KERNEL_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of the oracle, e.g. `("faces", "7")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// Whether the panel's own detection found a real kernel session.
fn kernel_available(harness: &Harness<'_, app::App>) -> bool {
    field(&report(harness), "kernel").as_deref() == Some("1")
}

fn click(harness: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step();
}

/// Click the Run button of `dialog`.
fn click_run(harness: &mut Harness<'_, app::App>, dialog: kernel_features::KernelDialog) {
    let rects = kernel_features::dialog_rects(window_region(), dialog);
    click(harness, rects.run.center());
}

/// Set a dialog's parameter field text through the assistive SetValue action
/// (the proven B7/C2 pattern), by rect overlap with the renderer's rect.
fn set_field(
    harness: &mut Harness<'_, app::App>,
    dialog: kernel_features::KernelDialog,
    text: &str,
) {
    let rects = kernel_features::dialog_rects(window_region(), dialog);
    let field_rect = rects.field.expect("dialog has a parameter field");
    let input = harness
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| {
            let r = n.rect();
            r.max.x >= field_rect.min.x
                && r.min.x <= field_rect.max.x
                && r.max.y >= field_rect.min.y
                && r.min.y <= field_rect.max.y
        })
        .expect("dialog TextInput present inside its widget rect");
    input.set_text_value_accesskit(text);
    harness.step();
    harness.step();
}

// ---------------------------------------------------------------------------
// 1. Availability is DETECTED, never assumed -- a tested state, not a guess.
//    In BOTH builds the panel publishes its own honest detection to the
//    oracle, and every dialog's unavailable path yields NO fake geometry.
// ---------------------------------------------------------------------------

/// The panel detects kernel availability at runtime and reports it honestly:
/// `kernel=1` with a real session (kernel build) or `kernel=0` with a reason
/// naming the seam state (default workspace build). The fixture is present
/// exactly when a session is.
#[test]
fn availability_is_detected_and_reported_honestly() {
    let mut harness = panel_harness();
    harness.run();

    let r = report(&harness);
    let kernel = field(&r, "kernel").as_deref().unwrap_or("?").to_string();
    assert!(
        kernel == "1" || kernel == "0",
        "kernel detection must be a runtime fact, got '{kernel}': {r}"
    );

    // The fixture's edge-occurrence count is the real seam answer (24 for a
    // cube) -- only ever reported with a session, never fabricated.
    if kernel == "1" {
        assert_eq!(field(&r, "reason").as_deref(), Some(""), "{r}");
        assert_eq!(
            field(&r, "edge_count").as_deref(),
            Some("24"),
            "box-reference fixture edge occurrences: {r}"
        );
    } else {
        assert!(
            field(&r, "reason")
                .unwrap_or_default()
                .contains("unavailable"),
            "the unavailable reason is honest: {r}"
        );
        assert_eq!(field(&r, "edge_count").as_deref(), Some("0"), "{r}");
    }

    // Before any run there is no geometry, real or fake.
    assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
    assert_eq!(field(&r, "faces").as_deref(), Some("-"), "{r}");
    assert_eq!(field(&r, "volume").as_deref(), Some("-"), "{r}");
}

/// Every dialog with a missing kernel says the feature is unavailable with an
/// honest message and NEVER fabricates a face count or volume. (With a kernel
/// this same test asserts the real geometry instead -- see section 2.)
#[test]
fn dialogs_without_kernel_are_honest_unavailable_not_fake_geometry() {
    let mut harness = panel_harness();
    harness.run();

    if !kernel_available(&harness) {
        for dialog in kernel_features::KernelDialog::ALL {
            click_run(&mut harness, dialog);
            let r = report(&harness);
            let res_name = match dialog {
                kernel_features::KernelDialog::Fillet => "run_fillet",
                kernel_features::KernelDialog::Chamfer => "run_chamfer",
                kernel_features::KernelDialog::Boolean => "run_boolean",
                kernel_features::KernelDialog::Shell => "run_shell",
            };
            assert_eq!(
                field(&r, "geometry").as_deref(),
                Some("0"),
                "no geometry after {dialog:?} Run: {r}"
            );
            assert_eq!(field(&r, "faces").as_deref(), Some("-"), "{r}");
            assert_eq!(field(&r, "volume").as_deref(), Some("-"), "{r}");
            let last = field(&r, "last").unwrap_or_default();
            assert!(
                !last.is_empty()
                    && (last.contains("unavailable") || last.contains("no shell operation")),
                "honest unavailable message for {dialog:?}, got: {last}"
            );
            if dialog != kernel_features::KernelDialog::Shell {
                assert_eq!(field(&r, res_name).as_deref(), Some("0"), "{r}");
            }
        }
    } else {
        // With a kernel the unavailable path is exercised by the shell dialog
        // and by the invalid-input refusals below; this branch exists so the
        // file stays meaningful green in the `--features kernel` build too.
        click_run(&mut harness, kernel_features::KernelDialog::Shell);
        let r = report(&harness);
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("no shell operation"),
            "{r}"
        );
    }
}

/// The shell dialog is honest in BOTH builds: the seam has no shell operation,
/// so it says so and produces no geometry (never a fake hollow).
#[test]
fn shell_dialog_is_honestly_no_geometry() {
    let mut harness = panel_harness();
    harness.run();
    click_run(&mut harness, kernel_features::KernelDialog::Shell);

    let r = report(&harness);
    assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
    assert_eq!(field(&r, "faces").as_deref(), Some("-"), "{r}");
    assert_eq!(field(&r, "run_shell").as_deref(), Some("1"), "{r}");
    assert!(
        field(&r, "last")
            .unwrap_or_default()
            .contains("no shell operation in the kernel seam"),
        "shell states the real reason: {r}"
    );
}

// ---------------------------------------------------------------------------
// 2. With the kernel: REAL geometry on the fixture. Running fillet on one
//    edge of the committed box-reference unit cube (default radius 0.2) must
//    produce the seam golden's face count (7) at its volume.
// ---------------------------------------------------------------------------

/// The seam golden for a radius-0.2 fillet of one unit-cube edge, straight
/// from `crates/oep-geo/tests/golden/kernel-seam-golden.json` (the same
/// numbers the oep-geo kernel-seam acceptance asserts, so an OCCT bump that
/// moves the topology is a reviewed change, not a silent drift).
const FILLET_GOLDEN_FACES: u64 = 7;
const FILLET_GOLDEN_VOLUME: f64 = 0.9914159265358977;
/// Union of the two half-overlapping unit boxes (face count from the golden).
const UNION_GOLDEN_FACES: u64 = 14;
const UNION_GOLDEN_VOLUME: f64 = 1.5;

/// With the kernel, the fillet dialog produces the EXPECTED FACE COUNT on the
/// fixture -- the card's headline acceptance. Without it, the same run is the
/// honest unavailable state (asserted here too, keeping this file green in the
/// default workspace build).
#[test]
fn fillet_produces_expected_face_count_on_the_fixture() {
    let mut harness = panel_harness();
    harness.run();
    click_run(&mut harness, kernel_features::KernelDialog::Fillet);

    let r = report(&harness);
    if !kernel_available(&harness) {
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("unavailable"),
            "{r}"
        );
        return;
    }

    // Real geometry ran and measured real numbers.
    assert_eq!(field(&r, "run_fillet").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "geometry").as_deref(), Some("1"), "{r}");
    assert_eq!(
        field(&r, "faces").as_deref(),
        Some(FILLET_GOLDEN_FACES.to_string().as_str()),
        "fillet r0.2 on the unit-cube fixture edge must yield the seam golden face count: {r}"
    );
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("measured volume");
    assert!(
        (volume - FILLET_GOLDEN_VOLUME).abs() / FILLET_GOLDEN_VOLUME < 1e-9,
        "fillet volume {volume} vs seam golden {FILLET_GOLDEN_VOLUME}"
    );
    // The honest readout names what actually ran.
    assert!(
        field(&r, "last")
            .unwrap_or_default()
            .contains("fillet radius 0.2"),
        "{r}"
    );
}

/// Chamfer on the same fixture edge is also real kernel geometry (golden: 7
/// faces at volume 0.98); without a kernel it is the same honest unavailable.
#[test]
fn chamfer_produces_real_geometry_with_the_kernel() {
    let mut harness = panel_harness();
    harness.run();
    click_run(&mut harness, kernel_features::KernelDialog::Chamfer);

    let r = report(&harness);
    if !kernel_available(&harness) {
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("unavailable"),
            "{r}"
        );
        return;
    }
    assert_eq!(field(&r, "run_chamfer").as_deref(), Some("1"), "{r}");
    assert_eq!(
        field(&r, "faces").as_deref(),
        Some("7"),
        "chamfer golden faces: {r}"
    );
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("measured volume");
    assert!(
        (volume - 0.98).abs() / 0.98 < 1e-6,
        "chamfer volume {volume}: {r}"
    );
}

/// A boolean union of the two fixture solids is real kernel geometry (golden:
/// 14 faces at volume 1.5); without a kernel it is the honest unavailable.
#[test]
fn boolean_union_produces_real_geometry_with_the_kernel() {
    let mut harness = panel_harness();
    harness.run();
    click_run(&mut harness, kernel_features::KernelDialog::Boolean);

    let r = report(&harness);
    if !kernel_available(&harness) {
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("unavailable"),
            "{r}"
        );
        return;
    }
    assert_eq!(field(&r, "run_boolean").as_deref(), Some("1"), "{r}");
    assert_eq!(
        field(&r, "faces").as_deref(),
        Some(UNION_GOLDEN_FACES.to_string().as_str()),
        "union of the half-overlapping fixture boxes must hit the golden face count: {r}"
    );
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("measured volume");
    assert!(
        (volume - UNION_GOLDEN_VOLUME).abs() / UNION_GOLDEN_VOLUME < 1e-6,
        "union volume {volume} vs seam golden {UNION_GOLDEN_VOLUME}"
    );
}

/// The boolean dialog accepts only the three seam op tokens; anything else is
/// a named refusal that never fabricates geometry. With a kernel this is the
/// typed-refusal path; without one, no validation runs behind the unavailable
/// state, which is still honest (a second unavailable assertion).
#[test]
fn invalid_boolean_op_is_a_named_refusal_not_a_panic() {
    let mut harness = panel_harness();
    harness.run();

    if !kernel_available(&harness) {
        click_run(&mut harness, kernel_features::KernelDialog::Boolean);
        let r = report(&harness);
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("unavailable"),
            "{r}"
        );
        return;
    }

    set_field(
        &mut harness,
        kernel_features::KernelDialog::Boolean,
        "banana",
    );
    click_run(&mut harness, kernel_features::KernelDialog::Boolean);
    let r = report(&harness);
    assert_eq!(
        field(&r, "error_field").as_deref(),
        Some("op"),
        "refusal names the op field: {r}"
    );
    let reason = field(&r, "error").unwrap_or_default();
    assert!(
        reason.contains("union | difference | intersection"),
        "refusal names the accepted tokens: {reason}"
    );
    assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
    assert_eq!(field(&r, "faces").as_deref(), Some("-"), "{r}");
}

/// A non-positive / non-numeric fillet amount is a named refusal (field +
/// reason), never a kernel panic and never fake geometry.
#[test]
fn invalid_fillet_amount_is_a_named_refusal() {
    let mut harness = panel_harness();
    harness.run();

    if !kernel_available(&harness) {
        click_run(&mut harness, kernel_features::KernelDialog::Fillet);
        let r = report(&harness);
        assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
        assert!(
            field(&r, "last")
                .unwrap_or_default()
                .contains("unavailable"),
            "{r}"
        );
        return;
    }

    // A negative radius is refused before it reaches the kernel.
    set_field(&mut harness, kernel_features::KernelDialog::Fillet, "-1");
    click_run(&mut harness, kernel_features::KernelDialog::Fillet);
    let r = report(&harness);
    assert_eq!(
        field(&r, "error_field").as_deref(),
        Some("radius"),
        "refusal names the field: {r}"
    );
    assert!(
        field(&r, "error").unwrap_or_default().contains("positive"),
        "{r}"
    );
    assert_eq!(field(&r, "geometry").as_deref(), Some("0"), "{r}");
    assert_eq!(field(&r, "faces").as_deref(), Some("-"), "{r}");

    // A non-numeric radius is refused the same way (still no panic).
    set_field(&mut harness, kernel_features::KernelDialog::Fillet, "abc");
    click_run(&mut harness, kernel_features::KernelDialog::Fillet);
    let r = report(&harness);
    assert_eq!(field(&r, "error_field").as_deref(), Some("radius"), "{r}");
    assert!(
        field(&r, "error").unwrap_or_default().contains("number"),
        "{r}"
    );

    // A positive amount runs for real.
    set_field(&mut harness, kernel_features::KernelDialog::Fillet, "0.5");
    click_run(&mut harness, kernel_features::KernelDialog::Fillet);
    let r = report(&harness);
    assert_eq!(field(&r, "geometry").as_deref(), Some("1"), "{r}");
    let faces: u64 = field(&r, "faces")
        .and_then(|v| v.parse().ok())
        .expect("fillet faces");
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("fillet volume");
    assert_eq!(faces, 7, "a radius-0.5 fillet keeps one fillet face: {r}");
    // Radius 0.5 removes more material than 0.2, so the volume must be lower
    // than the golden 0.991416 (real kernel behaviour, not a constant).
    assert!(
        volume < FILLET_GOLDEN_VOLUME,
        "volume {volume} vs {FILLET_GOLDEN_VOLUME}"
    );
}
