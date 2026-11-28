//! UI-F4 UI-DRAWING-EXPORT: SVG and PDF export from the UI.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts the card acceptance exactly
//! as written, each clause with printed numbers from the real panel:
//!
//! 1. **SVG is deterministic**: two exports of the same sheet are byte-identical
//!    - export the committed bracket twice through the real Export SVG button
//!    - into two paths, read both files, print both FNV-1a-64 hashes, assert bytes
//!    - equal;
//! 2. **PDF matches the committed bracket golden within a documented tolerance**:
//!    export the PDF through the real Export PDF button, compare it against the
//!    committed `bracket-drawing.pdf` golden (`crates/oep-drafting/tests/golden`),
//!    state the metric - page geometry (MediaBox / sheet size) and element
//!    positions (the content-stream drawing ops) - and print golden vs actual;
//! 3. **The print preview shows the same sheet content as the export**: assert
//!    the preview's graphical element count equals the element count the SVG and
//!    PDF writers actually serialize (printed);
//! 4. **A failed export (unwritable path) surfaces a named error in the UI and
//!    leaves no partial file**: point the path at a non-writable location, click
//!    Export PDF, read back the report oracle's named `ExportError::Io`, and
//!    assert no file exists at the target path;
//! 5. **The export runs from the real UI path in a kittest harness**: every
//!    clause above goes through the real Export buttons in the rendered UI, and
//!    the files are read back from disk (not synthesized by a unit call).
//!
//! The panel is seeded with NO override on the first frame: it opens the
//! committed bracket drawing through `oep_drafting::builder::drive_bracket_drawing`
//! (the existing drafting seam), so every number traces to the engine's own
//! committed bracket - never a second fixture. The export path is seeded under
//! [`EXPORT_PATH_KEY`] before the first frame, exactly like the F1 sheet seed,
//! so each test writes into its own temp dir.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

/// The export panel's `String` report oracle key (must match `drawing_export.rs`).
const EXPORT_REPORT_KEY: &str = "ui_f4_export_report";
/// The export path seed key (must match `drawing_export.rs`).
const EXPORT_PATH_KEY: &str = "ui_f4_export_path";

/// FNV-1a 64 - the wave's hash language (same as `doc_io::document_hash`'s
/// family), so the two exported hashes printed are comparable.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The export base path for one test run (a fresh temp dir per call).
fn temp_path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ui-f4-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join("drawing")
}

/// The committed bracket golden directory (this test reads the golden, never
/// writes it).
fn golden_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("oep-drafting")
        .join("tests")
        .join("golden")
}

/// Build the harness with `path` seeded as the export target BEFORE the first
/// frame (the export panel reads it on its first frame).
fn harness_with_path(path: &std::path::Path) -> Harness<'static, app::App> {
    let path = path.to_string_lossy().to_string();
    Harness::builder()
        .with_size([1280.0, 2100.0])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(EXPORT_PATH_KEY), path.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

/// The export panel's `String` report oracle, read back through the same
/// context store the panel writes to.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(EXPORT_REPORT_KEY)))
        .unwrap_or_default()
}

/// One report field, e.g. `("elements", "67")`.
fn field(report_text: &str, key: &str) -> Option<String> {
    report_text
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// Click a labelled node exactly, scrolling it into view first (the export
/// section is the LAST section of the drafting scroll and starts below the
/// fold at the acceptance window). `scroll_to_me` moves the ScrollArea over a
/// few frames; the probe confirmed the button lands inside the window after
/// several steps, then we click through the real input path.
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let first = harness
        .get_all_by_label(label)
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("label {label:?} not found in tree"));
    first.scroll_to_me();
    for _ in 0..10 {
        harness.step();
    }
    let node = harness
        .get_all_by_label(label)
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("label {label:?} not found after scroll"));
    let r = node.rect();
    assert!(
        r.min.y < 2100.0,
        "label {label:?} still below the drafting fold after scroll: {r:?}"
    );
    node.click();
    harness.step();
}

/// Run the real app to a settled first frame.
fn settle(harness: &mut Harness<'_, app::App>) {
    harness.run();
    harness.step();
}

// ---------------------------------------------------------------------------
// 1. SVG is deterministic: two exports of the same sheet are byte-identical
// ---------------------------------------------------------------------------

#[test]
fn svg_export_is_deterministic_two_exports_byte_identical() {
    // First export.
    let p1 = temp_path("svg-a");
    let mut h1 = harness_with_path(&p1);
    settle(&mut h1);
    click_label(&mut h1, "Export SVG");
    let r1 = report(&h1);
    println!("first export report:\n{r1}");
    assert_eq!(
        field(&r1, "svg_bytes").map(|s| s.parse::<usize>().unwrap()),
        Some(3158),
        "SVG byte count (golden is 3158)"
    );
    let svg1 = std::fs::read(format!("{}.svg", p1.to_string_lossy())).expect("first svg on disk");
    assert!(svg1.starts_with(b"<?xml"), "real SVG from the UI path");

    // Second export (independent launch, same sheet).
    let p2 = temp_path("svg-b");
    let mut h2 = harness_with_path(&p2);
    settle(&mut h2);
    click_label(&mut h2, "Export SVG");
    let r2 = report(&h2);
    let svg2 = std::fs::read(format!("{}.svg", p2.to_string_lossy())).expect("second svg on disk");

    let h1 = fnv1a64(&svg1);
    let h2 = fnv1a64(&svg2);
    println!(
        "SVG #1 hash = {h1} ({} bytes)\nSVG #2 hash = {h2} ({} bytes)",
        svg1.len(),
        svg2.len()
    );
    assert_eq!(
        svg1, svg2,
        "two exports of the same sheet must be byte-identical"
    );
    assert_eq!(h1, h2, "both hashes must match");
    let _ = r2;
}

// ---------------------------------------------------------------------------
// 2. PDF matches the committed bracket golden within a documented tolerance
// ---------------------------------------------------------------------------

#[test]
fn pdf_matches_the_committed_bracket_golden_within_tolerance() {
    let p = temp_path("pdf");
    let mut harness = harness_with_path(&p);
    settle(&mut harness);
    click_label(&mut harness, "Export PDF");
    let r = report(&harness);
    println!("pdf export report:\n{r}");

    let actual_path = format!("{}.pdf", p.to_string_lossy());
    let actual = std::fs::read(&actual_path).expect("exported pdf on disk");
    assert!(actual.starts_with(b"%PDF-"), "real PDF from the UI path");

    let golden_path = golden_dir().join("bracket-drawing.pdf");
    let golden = std::fs::read(&golden_path).expect("committed golden pdf");

    // Metric: page geometry (MediaBox) + element positions (content-stream ops).
    let (g_geometry, g_elements) = pdf_metric(&golden);
    let (a_geometry, a_elements) = pdf_metric(&actual);
    println!("GOLDEN pdf: page {g_geometry}, content ops {g_elements}");
    println!("ACTUAL pdf: page {a_geometry}, content ops {a_elements}");

    // Documented tolerance: page geometry identical (the same sheet), element
    // positions identical within 1e-6 (same computed geometry through the same
    // writer), byte length equal - the strongest claim, byte-identical.
    assert_eq!(
        g_geometry, a_geometry,
        "page geometry (MediaBox) must match the golden"
    );
    assert_eq!(
        g_elements, a_elements,
        "element positions (content ops) must match the golden"
    );
    assert_eq!(
        golden.len(),
        actual.len(),
        "PDF length must match the golden byte-for-byte"
    );
    assert_eq!(
        golden, actual,
        "exported PDF must be byte-identical to the golden"
    );
    println!(
        "PDF byte-identical to golden: {} == {} bytes",
        golden.len(),
        actual.len()
    );
}

/// Minimal PDF metric reader: the MediaBox page geometry and the count of
/// content-stream drawing ops (line/rect/text), enough to compare golden vs
/// actual on a documented, content-derived metric.
fn pdf_metric(pdf: &[u8]) -> (String, usize) {
    let text = String::from_utf8_lossy(pdf);
    // Page geometry: 595 pts wide, 841.95 pts tall for A4? This writer emits a
    // MediaBox in mm (297 x 210). Capture it verbatim from the file.
    let geometry = text
        .split_once("/MediaBox [")
        .map(|(_, rest)| {
            rest.split(']')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| "-".to_string());
    // Content ops: every line ending in "S", "re S" or "Tj ET" is one element
    // position drawn by the writer.
    let mut ops = 0usize;
    if let Some((_, after)) = text.split_once("stream\n") {
        let content = after.split("\nendstream").next().unwrap_or_default();
        for line in content.lines() {
            let line = line.trim();
            if line.ends_with(" l S") || line.ends_with(" re S") || line.contains("Tj ET") {
                ops += 1;
            }
        }
    }
    (geometry, ops)
}

// ---------------------------------------------------------------------------
// 3. The print preview shows the same sheet content as the export (element count)
// ---------------------------------------------------------------------------

#[test]
fn print_preview_element_count_matches_the_export() {
    let p = temp_path("preview");
    let mut harness = harness_with_path(&p);
    settle(&mut harness);

    let r0 = report(&harness);
    println!("preview report (before export):\n{r0}");
    let preview_elements: usize = field(&r0, "elements")
        .expect("elements field")
        .parse()
        .expect("elements number");
    assert!(preview_elements > 0, "preview renders the sheet content");
    assert_eq!(
        field(&r0, "preview").as_deref(),
        Some("1"),
        "preview rendered"
    );

    // Export both formats; each must serialize EXACTLY the preview's count.
    click_label(&mut harness, "Export SVG");
    let svg = std::fs::read(format!("{}.svg", p.to_string_lossy())).expect("svg");
    let svg_elements = svg_element_count(&svg);
    println!("preview elements = {preview_elements}, SVG serialized elements = {svg_elements}");
    assert_eq!(
        svg_elements, preview_elements,
        "the SVG export writes exactly the preview's sheet content"
    );

    click_label(&mut harness, "Export PDF");
    let pdf = std::fs::read(format!("{}.pdf", p.to_string_lossy())).expect("pdf");
    let (_, pdf_elements) = pdf_metric(&pdf);
    println!("PDF serialized elements = {pdf_elements}");
    assert_eq!(
        pdf_elements, preview_elements,
        "the PDF export writes exactly the preview's sheet content"
    );
}

/// Count the graphical elements the SVG writer serialized (`<path`, `<rect`,
/// `<text` - the same set the preview's `graphical_count` counts; group
/// markers are structure and excluded exactly like the panel excludes them).
fn svg_element_count(svg: &[u8]) -> usize {
    let text = String::from_utf8_lossy(svg);
    text.split('<')
        .filter(|t| t.starts_with("path ") || t.starts_with("rect ") || t.starts_with("text "))
        .count()
}

// ---------------------------------------------------------------------------
// 4. A failed export (unwritable path) surfaces a named error, no partial file
// ---------------------------------------------------------------------------

#[test]
fn unwritable_export_path_surfaces_named_error_and_leaves_no_partial_file() {
    // Point the path at a location that cannot be created: a nested path under
    // a path component that is a FILE (so opening it for write fails).
    let base = temp_path("unwritable");
    let blocker = base.with_extension("blocker");
    std::fs::write(&blocker, b"x").expect("blocker file");
    let bad = blocker.join("out"); // a file cannot be a directory -> write fails

    let mut harness = harness_with_path(&bad);
    settle(&mut harness);
    click_label(&mut harness, "Export PDF");

    let r = report(&harness);
    println!("failed export report:\n{r}");
    let err = field(&r, "error").unwrap_or_default();
    assert!(
        err.contains("ExportError::Io"),
        "a named Io error must surface in the UI, got: {err}"
    );
    assert!(
        err.contains(&bad.to_string_lossy().to_string()),
        "the error names the unwritable path: {err}"
    );

    // No partial file at the target (the write failed before creating it).
    let target = format!("{}.pdf", bad.to_string_lossy());
    assert!(
        !std::path::Path::new(&target).exists(),
        "no partial file may remain at the target path"
    );
}

// ---------------------------------------------------------------------------
// 5. The export runs from the real UI path in a kittest harness
// ---------------------------------------------------------------------------

#[test]
fn export_runs_from_the_real_ui_path() {
    // The committed golden proves both writers; this clause proves the UI path:
    // the button click in the rendered app produced a real, parseable file on
    // disk, and the panel's report carries the byte count + hash of exactly
    // what the writers serialized.
    let p = temp_path("uipath");
    let mut harness = harness_with_path(&p);
    settle(&mut harness);

    // Both buttons are real UI nodes (AccessKit labels) - not unit calls.
    let svg_btn = harness
        .get_all_by_label("Export SVG")
        .next()
        .expect("Export SVG button in the rendered UI");
    let pdf_btn = harness
        .get_all_by_label("Export PDF")
        .next()
        .expect("Export PDF button in the rendered UI");
    println!(
        "real UI buttons reachable: Export SVG at {}, Export PDF at {}",
        svg_btn.rect(),
        pdf_btn.rect()
    );

    click_label(&mut harness, "Export SVG");
    let r = report(&harness);
    println!("export report:\n{r}");
    let svg_bytes: usize = field(&r, "svg_bytes").unwrap().parse().unwrap();
    let svg_hash: u64 = field(&r, "svg_hash").unwrap().parse().unwrap();
    let disk_svg = std::fs::read(format!("{}.svg", p.to_string_lossy())).expect("svg on disk");
    assert_eq!(
        disk_svg.len(),
        svg_bytes,
        "panel-reported SVG bytes == on-disk bytes"
    );
    assert_eq!(
        fnv1a64(&disk_svg),
        svg_hash,
        "panel-reported SVG hash == on-disk hash (the UI wrote exactly this)"
    );
}
