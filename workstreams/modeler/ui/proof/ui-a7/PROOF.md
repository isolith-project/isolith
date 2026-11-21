# UI-A7 UI-OPEN-SAVE proof

Card: UI-A7 UI-OPEN-SAVE - open, save and recent files.
Branch: `ui/a7-open-save`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: the app opens the committed demo STEP, saves a `.oepdoc` document,
reopens it to an identical document hash, and shows the file name with an
honest dirty marker.

## What landed (files owned by this card)

- `crates/oep-app/src/doc_io.rs` (new) - the engine-side document lifecycle,
  egui-free: `import_step`/`import_step_bytes` through `oep_import` (the
  existing import path, never a new parser - `SourceKind::Step` + hub
  detection), `read_oepdoc`/`write_oepdoc` over the canonical oep-geo serde,
  `document_hash` (FNV-1a-64 over the canonical `.oepdoc` bytes, the SAME
  hashing scheme the engine already uses for frame hashes), and the
  recent-list store (`load_recent`/`save_recent`/`push_recent`, capped at
  `RECENT_MAX`, persisted in the config directory next to `layout.conf`).
- `crates/oep-app/src/panels/doc.rs` - fills the UI-A4 stub. State lives in
  the egui context data store (palette pattern; `app.rs` untouched). The
  native title bar is updated via `ViewportCommand::Title` to
  `Isolith - <file>*`; the A4 status-bar document field shows the same
  string; `document_hash`/`saved_hash` are derived-not-stored, so `dirty ==
  saved_hash != Some(current_hash)`. Ctrl+O re-imports the demo STEP, Ctrl+S
  saves `.oepdoc`, Ctrl+R toggles the recent overlay (Enter opens, Esc
  cancels). A failed open records the typed reason and leaves the current
  document + hash untouched (never half-loaded).
- `crates/oep-app/tests/doc_io.rs` (new) - the `--test doc_io` target: open,
  save, reopen identical-hash (cross-checked independently by re-hashing the
  file bytes with FNV-1a-64), dirty-star lifecycle, recent persistence across
  a restart, cancel + error paths returning the document unchanged. Driven
  through the real `app::App` by `egui_kittest`; the oracle is the panel's
  plain `String` report in context data.
- `workstreams/modeler/ui/proof/ui-a7/` - this proof + transcripts + frame.

Files outside the list, touched with reason:
- `crates/oep-app/Cargo.toml` - DETAIL mandates the import path goes through
  oep-import/oep-step; `doc_io.rs` opens STEP via `oep_import`, and a direct
  path dep is the only way the app can name the hub. Already in the build
  graph transitively via oep-shell, so no new external crate lands
  (Cargo.lock gains only the oep-app -> oep-import edge).
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - the shot.sh
  golden. UI-A7's whole behaviour shows the document name in the shell frame
  (status bar now reads `edpart.step *`), so the drift guard fires as A2
  designed; per the A4/A5/A6 precedent the new frame is recorded as the
  golden and the old A5/A6 golden preserved beside this proof
  (`ui-a2-golden-before-opensave.png`).

## Document hash

The engine computes no document-level hash (its only hashes are the rendered
frame FNV-1a in oep-viewer and SHA-256 of archive blobs in oep-archive), so
per the card instruction `document_hash` is defined in `doc_io.rs`. It is the
engine's OWN FNV-1a-64 scheme (identical constants to
`oep_viewer::backend::frame_hash`) applied to the canonical deterministic
`.oepdoc` serialization (`to_json_pretty`, which oep-geo documents as
deterministic) - not a second hashing scheme. The test cross-checks the
reported hash independently by re-hashing the bytes actually written to disk.

## Acceptance (real command output)

| Step | Command | Exit | File |
|---|---|---|---|
| step0 | `git fetch github && git merge github/wave/ui-a` | 0 (wave already at HEAD a3abdb6; this branch fast-forwarded to it at the start of the run) | `transcripts/01-step0.txt` |
| doc_io | `cargo test -p oep-app --test doc_io` | 0 (9 passed) | `transcripts/02-doc-io-test.txt` |
| smoke | `xvfb-run -a cargo run -p oep-app -- --smoke` | 0 (`frames=8 smoke ok`) | `transcripts/03-smoke.txt` |
| shot drift | `bash scripts/ui/shot.sh` vs A5/A6 golden | 1 (drift guard, intended) | `transcripts/04-shot-golden-drift.txt` |
| shot run 1 | `bash scripts/ui/shot.sh` after refresh | 0 | `transcripts/05-shot-green-run1.txt` |
| shot run 2 | `bash scripts/ui/shot.sh` | 0, identical sha256 | `transcripts/06-shot-green-run2.txt` |
| gate | fmt check, clippy `-D warnings` all targets, full `cargo test -p oep-app` (41 tests incl. all prior wave tests) | 0 | `transcripts/07-gate.txt` |

### 1. `cargo test -p oep-app --test doc_io` (exit 0)

```
running 9 tests
test config::tests::region_... ... ok
test committed_demo_step_is_present ... ok
test app_opens_committed_demo_step_with_dirty_star ... ok
test dirty_star_lifecycle_appears_on_change_and_clears_on_save ... ok
test save_then_reopen_produces_identical_document_hash ... ok
test recent_files_persist_across_restart ... ok
test cancel_and_error_paths_return_document_unchanged ... ok
test result: ok. 9 passed; 0 failed
```

Highlights:
- The `--test doc_io` suite asserts, through the real app: on launch the
  report/show carries `edpart.step *` (dirty star on, fresh import unsaved);
  Ctrl+S writes a canonical `.oepdoc` into the config directory and clears the
  star; reopening it reproduces the identical hash (asserted both from the
  app state and independently by FNV-1a-64 over the on-disk bytes); the dirty
  star comes back on a re-import (Ctrl+O) and clears again on save; the
  recent list persists across a restart; Esc-cancel and a failed (missing)
  `.oepdoc` open both leave the current document and hash unchanged with the
  typed error reported.

### 2. `xvfb-run -a cargo run -p oep-app -- --smoke` (exit 0)

```
frames=8 smoke ok
```

### 3. `bash scripts/ui/shot.sh` - doc frame, hash recorded

Transcript 04 shows the drift guard firing loudly while the golden was still
the A5/A6 shell frame - expected: the status bar now reports `edpart.step *`.
The golden was then refreshed and two independent runs produced byte-identical
PNGs:

```
shot.sh: ok
  frame    : workstreams/modeler/ui/proof/ui-a2/frame.png
  sha256   : 2695a608982b2eaa797a259f7c5796b2714f5021e903659eec61041255bcb0dc
```

## Image provenance

- `frame.png` - 1280x800 xvfb capture of the real `isolith` binary on the
  merged head: five-region shell, live viewport showing the demo part
  (80x40x10 solid + grid + axes), status bar `Status Bar | edpart.step * |
  0 selected | Select · 10 ms`. sha256
  `2695a608982b2eaa797a259f7c5796b2714f5021e903659eec61041255bcb0dc`,
  byte-identical across two independent shot.sh runs.
- `ui-a2-golden-before-opensave.png` - the A5/A6 shell golden (hash
  `97f0a035…7672`), preserved so the drift is auditable.

## Notes for the review of this card

- No test was weakened or deleted; all A1-A6 wave tests still pass
  (`cargo test -p oep-app` = 41: main 5, doc_io 9, harness 6, layout 7,
  palette 8, viewport 6).
- "Use the hash the engine already computes": the engine has no document
  hash; `document_hash` reuses the engine's FNV-1a-64 scheme (oep-viewer
  frame-hash constants) over the canonical `.oepdoc` bytes. One scheme, one
  place (doc_io.rs).
- Import goes through `oep_import` (the hub), never a new parser; save/reopen
  through `oep_geo::Document` serde (`.oepdoc` = canonical JSON, deterministic
  round trip).
- Recent files persist in the same config directory `config::layout_path`
  resolves (`recent.conf` beside `layout.conf`), restored on a restart - the
  test builds a second harness over the same `ISOLITH_CONFIG_DIR` and asserts
  the list came back.
- Cancel (Esc on the recent overlay) and error (open a missing/corrupt
  `.oepdoc`) paths are asserted to return the document unchanged; a failed
  open sets the error line and never touches the in-memory document.
- egui 0.37 notes encountered: `ViewportCommand::Title` for the native title
  bar; plain `selectable_label` rows are keyboard-reachable; the recent
  overlay is an `egui::Area` (Order::Foreground) so the UI-A2 one-Pane and
  UI-A4 five-region contracts hold.
