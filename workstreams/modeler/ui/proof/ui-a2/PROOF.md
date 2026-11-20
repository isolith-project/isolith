# UI-A2 UI-HARNESS proof

Card: UI-A2 UI-HARNESS - headless UI test and deterministic screenshot harness.
Branch: ui/a2-harness. Wave branch: wave/ui-a (github remote).

Behaviour: one command produces a deterministic frame from the real
application, and a test drives the real app through injected input and asserts
on its state.

## What landed

- `crates/oep-app/tests/harness.rs` - integration test driving the REAL
  `app::App` via `egui_kittest` 0.37 (eframe feature only; no wgpu, no
  snapshot). Since oep-app is a binary-only crate, the test pulls the real
  `src/app.rs` and `src/config.rs` in with `#[path]` module includes - it
  compiles the shipped app code, not a stub. Three tests:
  - real app registers its central panel Pane in the AccessKit tree,
  - a primary-button click (pointer move, press, release) reaches the app
    context: `pointer.any_down`, `button_down(Primary)`, `latest_pos() ==
    Some(target)` and `pointer.any_click()` all asserted,
  - a key press (`Key::B` down) reaches the app context: `key_pressed(B)` true,
    then false after release.
- `scripts/ui/shot.sh` - runs the real binary under `xvfb-run` on a fixed
  1280x800x24 screen, waits for the window, captures the root with `xwd`,
  decodes XWD to PNG deterministically (embedded Python/PIL decoder, strict on
  geometry), writes `proof/ui-a2/frame.png` and `frame.png.sha256`, captures
  twice and exits non-zero if the two hashes differ OR if a committed golden
  reference drifts. Fails loudly, never warns.
- `crates/oep-app/Cargo.toml` - dev-dependency `egui_kittest 0.37` added
  (normal dependencies untouched; UI-A3's `oep-ui-theme` normal dep arrived via
  the wave merge).

## Acceptance transcripts (real command output)

| Step | Command | Exit | File |
|---|---|---|---|
| step0 | fetch + merge wave/ui-a | 0 (union Cargo.lock) | `transcripts/01-step0.txt` |
| harness | `cargo test -p oep-app --test harness` | 0 (3 passed) | `transcripts/02-harness-test.txt` |
| shot run 1 | `bash scripts/ui/shot.sh` | 0 | `transcripts/03-shot-run1.txt` |
| shot run 2 | `bash scripts/ui/shot.sh` | 0, identical sha256 | `transcripts/04-shot-run2.txt` |
| drift active | one-pixel app change + `bash scripts/ui/shot.sh` | 1 (guard trips) | `transcripts/05-drift-active.txt` |
| drift reverted | revert + `bash scripts/ui/shot.sh` | 0 | `transcripts/06-drift-reverted.txt` |
| gate | fmt, clippy, harness test on merged head | 0 | `transcripts/07-gate.txt` |

`transcripts/02-harness-test.txt` is kept separate from the gate transcript so
the acceptance command appears uncontaminated; `transcripts/07-gate.txt`
re-runs the same test on the merged head.

## Image provenance

`frame.png` is the deterministic frame from the real `isolith` binary on a
1280x800 Xvfb screen, captured twice; the sha256 is written to
`frame.png.sha256` and reproduced identically by two independent shot.sh runs:

```
$ sha256sum workstreams/modeler/ui/proof/ui-a2/frame.png
e20b6d5d78466dcc7ce0f8531c2a9de243f7c0e6b28f3fd8aba6fc1442bb0da1  workstreams/modeler/ui/proof/ui-a2/frame.png
$ cat workstreams/modeler/ui/proof/ui-a2/frame.png.sha256
e20b6d5d78466dcc7ce0f8531c2a9de243f7c0e6b28f3fd8aba6fc1442bb0da1
```

The frame shows the UI-A3 light theme (the wave merge brought UI-A3's
`oep-ui-theme` into this tree and into the rendered shell).

## Notes for the review of this card

- The shot.sh internal double-capture runs two independent app launches and
  requires byte-identical PNGs; the golden-reference guard additionally refuses
  to silently rewrite a committed `frame.png.sha256` when the app drifts. Both
  paths fail loudly with both hashes printed.
- The deliberate drift transcript applies a one-pixel rendering change to
  `src/app.rs`, rebuilds, and shows `bash scripts/ui/shot.sh` exiting 1
  (drift detected), then reverts the change and shows exit 0. The app source is
  byte-identical to the wave head at the end of the proof (no diff).
- Loading a real X surface needs an EGL/GL context; the two warnings printed by
  libEGL (DRI3) are expected on the lavapipe/software stack and do not affect
  the captured frame.
