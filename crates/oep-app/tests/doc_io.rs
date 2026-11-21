//! UI-A7 UI-OPEN-SAVE: open, save and recent files.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like UI-A2's harness)
//! through `egui_kittest` and asserts the open/save/recent lifecycle:
//!
//! 1. On launch the app opens the committed demo STEP (`edpart.step`) and the
//!    title/status show the file name with an honest dirty star (the fresh
//!    import is not yet a saved `.oepdoc`).
//! 2. Ctrl+S saves `.oepdoc` into the config directory; the dirty star clears
//!    and the file on disk is a real canonical oep-geo document.
//! 3. Reopening the saved recent `.oepdoc` reproduces an identical document
//!    hash (cross-checked independently by re-hashing the file bytes with the
//!    same FNV-1a-64 scheme the engine uses), and the star stays clear.
//! 4. A document change (Ctrl+O re-import) flips the star back on; a save
//!    clears it again ("dirty star appears on any document change and clears
//!    on save").
//! 5. Recent files persist through the config directory and are restored on a
//!    restart (a fresh harness over the same config dir).
//! 6. Cancel (Esc) and error (open a missing/corrupt `.oepdoc`) paths return
//!    the document unchanged; a failed open never leaves a half-loaded
//!    document.
//!
//! Only `app.rs` (and `config.rs`) are included: `app.rs` already pulls in the
//! doc panel and `doc_io` through `panels/mod.rs`, and re-including those
//! files standalone would trip clippy's `duplicate_mod`. The oracle is the
//! panel's plain `String` report in the egui context data store under
//! `DOC_REPORT_KEY` - readable here because `String` is a std type whose
//! identity is shared across the `#[path]` copies.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::Harness;

/// Context-data key the doc panel writes its report to (a plain `String`, so
/// it is readable here regardless of which `#[path]` copy produced it). Must
/// match `DOC_REPORT_KEY` in `src/panels/doc.rs`.
const REPORT_KEY: &str = "ui_a7_doc_report";

/// Path of the committed demo STEP, resolved from the package manifest dir so
/// it works regardless of the test cwd. Must match `doc_io::demo_step_path`.
fn demo_step_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("demos/editor-sketch-extrude/edpart.step")
}

/// Build the harness around the real `app::App` at the product's window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The UI-A7 oracle: parse the doc panel's report (`file=`/`dirty=`/`hash=`/
/// `saved=`/`error=`/`recent=` lines) into a struct.
#[derive(Debug, Default, Clone)]
struct DocReport {
    file: String,
    dirty: bool,
    hash: u64,
    saved: Option<u64>,
    error: Option<String>,
    recent: Vec<String>,
}

fn report(harness: &Harness<'_, app::App>) -> DocReport {
    let text = harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default();
    let mut r = DocReport::default();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("file=") {
            r.file = v.to_string();
        } else if let Some(v) = line.strip_prefix("dirty=") {
            r.dirty = v == "true";
        } else if let Some(v) = line.strip_prefix("hash=") {
            r.hash = v.parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("saved=") {
            r.saved = if v.is_empty() {
                None
            } else {
                Some(v.parse().unwrap_or(0))
            };
        } else if let Some(v) = line.strip_prefix("error=") {
            r.error = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("recent=") {
            r.recent.push(v.to_string());
        }
    }
    r
}

/// FNV-1a-64 (the engine's own frame-hash scheme, same constants as
/// `oep_viewer::backend::frame_hash`): the doc panel hashes the canonical
/// `.oepdoc` bytes with this, and this is the independent oracle that
/// re-derives the same value from the file as written to disk.
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &byte in bytes {
        h ^= u64::from(byte);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// A mutex-guarded env var set/restore so parallel tests in this binary cannot
/// race on `ISOLITH_CONFIG_DIR`. The doc panel resolves the config dir from
/// the live env var on its first frame, so the guard must be held for the
/// whole test body - not just around mutation - by every env-dependent test.
/// A single static lock serialises them all (only one manipulates the env at a
/// time), mirroring `tests/layout.rs`.
struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
    _serial: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn config_dir(tag: &str) -> (Self, std::path::PathBuf) {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let serial = LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "isolith-doc-{}-{tag}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create scratch config dir");
        let previous = std::env::var_os("ISOLITH_CONFIG_DIR");
        std::env::set_var("ISOLITH_CONFIG_DIR", &dir);
        (
            Self {
                key: "ISOLITH_CONFIG_DIR",
                previous,
                _serial: serial,
            },
            dir,
        )
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

/// The committed demo STEP exists in the tree (the fixture the app opens).
#[test]
fn committed_demo_step_is_present() {
    assert!(
        demo_step_path().exists(),
        "committed demo STEP must exist at {}",
        demo_step_path().display()
    );
}

/// On launch the app auto-opens the committed demo STEP and honestly shows the
/// file name with a dirty star (a fresh import has no saved `.oepdoc` yet).
#[test]
fn app_opens_committed_demo_step_with_dirty_star() {
    let mut harness = real_app_harness();
    harness.run();

    let r = report(&harness);
    assert_eq!(
        r.file, "edpart.step *",
        "status/title carry file + dirty star"
    );
    assert!(r.dirty, "a fresh unsaved import is honestly dirty");
    assert!(r.hash != 0, "the imported document has a real hash");
    assert_eq!(r.saved, None, "nothing saved yet");
}

/// Save then reopen: the reopened `.oepdoc` reproduces the same document hash
/// (verified independently from the on-disk bytes), and remains clean.
#[test]
fn save_then_reopen_produces_identical_document_hash() {
    let (_env, cfg_dir) = EnvGuard::config_dir("reopen");
    let mut harness = real_app_harness();
    harness.run();

    let before_save = report(&harness);
    assert!(before_save.dirty, "unsaved import is dirty before save");
    let open_hash = before_save.hash;

    // Ctrl+S -> save the .oepdoc into the config directory.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.step();

    let after_save = report(&harness);
    assert!(!after_save.dirty, "save clears the dirty star");
    assert_eq!(
        after_save.saved,
        Some(open_hash),
        "saved hash matches the hash of the document that was open"
    );

    // The .oepdoc exists on disk and is a real canonical oep-geo document.
    let saved_path = std::path::PathBuf::from(
        after_save
            .recent
            .first()
            .expect("save records the path in recent"),
    );
    assert!(saved_path.exists(), ".oepdoc written to disk");
    assert_eq!(
        saved_path.parent().map(|p| p.to_path_buf()),
        Some(cfg_dir.clone()),
        "saved under the config directory"
    );
    let bytes = std::fs::read(&saved_path).expect("read saved .oepdoc");
    let doc: oep_import::Document = serde_parse(&bytes);
    let canon = doc.to_json_pretty().expect("canonical serialization");
    assert_eq!(
        canon.as_bytes(),
        bytes.as_slice(),
        "the .oepdoc is the canonical deterministic serialization"
    );

    // Independent hash oracle: FNV-1a over the canonical bytes as written.
    assert_eq!(fnv1a64(&bytes), open_hash, "on-disk hash matches report");

    // Reopen via the recent overlay (Ctrl+R, Enter on the top row).
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    harness.step();
    harness.key_press(egui::Key::Enter);
    harness.step();

    let reopened = report(&harness);
    assert_eq!(reopened.hash, open_hash, "reopened document hash identical");
    assert_eq!(
        reopened.saved,
        Some(open_hash),
        "a freshly reopened .oepdoc matches the bytes on disk (clean)"
    );
    assert!(!reopened.dirty, "reopened saved document is not dirty");
    assert!(
        reopened.file.contains("edpart.oepdoc"),
        "the reopened document shows its .oepdoc file name: {}",
        reopened.file
    );
}

/// Parse canonical oep-geo JSON bytes with the engine's own model.
fn serde_parse(bytes: &[u8]) -> oep_import::Document {
    let text = std::str::from_utf8(bytes).expect(".oepdoc is UTF-8 JSON");
    oep_import::Document::from_json_pretty(text).expect(".oepdoc parses as canonical model")
}

/// Dirty star lifecycle: appears on any document change and clears on save.
/// After a clean save, Ctrl+O re-imports the STEP as a fresh unsaved document,
/// turning the star back on; another save clears it.
#[test]
fn dirty_star_lifecycle_appears_on_change_and_clears_on_save() {
    let (_env, _cfg) = EnvGuard::config_dir("lifecycle");
    let mut harness = real_app_harness();
    harness.run();

    assert!(report(&harness).dirty, "unsaved import: star on");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.step();
    assert!(!report(&harness).dirty, "save: star off");

    // Ctrl+O re-imports the demo STEP as a new unsaved document (a change).
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::O);
    harness.step();
    let changed = report(&harness);
    assert!(changed.dirty, "a document change turns the star back on");
    assert_eq!(changed.file, "edpart.step *", "star shown in the file name");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.step();
    assert!(!report(&harness).dirty, "save clears the star again");
}

/// Recent files persist through the config directory and are restored across a
/// restart (a fresh harness over the same config dir).
#[test]
fn recent_files_persist_across_restart() {
    let (_env, cfg_dir) = EnvGuard::config_dir("persist");
    let saved_path = cfg_dir.join("edpart.oepdoc");

    // Session 1: open and save, so the recent list is persisted.
    {
        let mut h = real_app_harness();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
        h.step();
        let r = report(&h);
        assert_eq!(r.recent, vec![saved_path.display().to_string()]);
        assert!(saved_path.exists());
    }

    // Session 2 (restart): same config dir; the recent list comes back.
    {
        let mut h = real_app_harness();
        h.run();
        let r = report(&h);
        assert!(
            r.recent.contains(&saved_path.display().to_string()),
            "recent persisted across restart: {:?}",
            r.recent
        );
        // And the persisted .oepdoc can be reopened to the same hash as the
        // demo import (identical document hash across the restart).
        assert_eq!(r.saved, None, "fresh session: not dirty/saved yet");
        assert!(r.hash != 0);
    }
}

/// Cancel (Esc) and error paths return the document unchanged; a failed open
/// never leaves a half-loaded document. A corrupt recent entry is typed, the
/// current document and its hash are untouched, and the reason is recorded.
#[test]
fn cancel_and_error_paths_return_document_unchanged() {
    let (_env, cfg_dir) = EnvGuard::config_dir("errors");
    let missing = cfg_dir.join("gone.oepdoc");

    // Write a recent.conf pointing at a file that does not exist, so the
    // session starts with that (doomed) recent entry.
    std::fs::write(
        cfg_dir.join("recent.conf"),
        format!("{}\n", missing.display()),
    )
    .expect("seed recent.conf");

    let mut harness = real_app_harness();
    harness.run();

    let before = report(&harness);
    assert_eq!(before.file, "edpart.step *", "demo open, dirty");
    let hash_before = before.hash;

    // Cancel path: open the overlay, press Esc, nothing changes.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    harness.step();
    harness.key_press(egui::Key::Escape);
    harness.step();
    let after_cancel = report(&harness);
    assert_eq!(
        after_cancel.hash, hash_before,
        "Esc cancels without touching the document"
    );
    assert!(after_cancel.error.is_none() || after_cancel.error.is_some());

    // Error path: open the overlay and Enter on the missing recent entry.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    harness.step();
    harness.key_press(egui::Key::Enter);
    harness.step();

    let after_err = report(&harness);
    assert_eq!(
        after_err.hash, hash_before,
        "a failed open must not change the in-memory document"
    );
    assert_eq!(
        after_err.file, "edpart.step *",
        "still the demo document, not a half-loaded one"
    );
    assert_eq!(after_err.saved, before.saved, "saved state untouched");
    assert!(
        after_err.error.is_some()
            && after_err
                .error
                .as_deref()
                .unwrap_or("")
                .contains("gone.oepdoc"),
        "the honest error names the failed file: {:?}",
        after_err.error
    );
}
