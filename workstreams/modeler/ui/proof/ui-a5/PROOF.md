# UI-A5 UI-VIEWPORT-CPU proof

Card: UI-A5 UI-VIEWPORT-CPU - the viewport backend seam and a live frame.
Branch: `ui/a5-viewport`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: the existing oep-viewer renderer is shown inside the app viewport
panel and is navigable with the mouse, behind a backend trait that Wave D swaps
for a GPU implementation.

## What landed (files owned by this card)

- `crates/oep-viewer/src/backend.rs` (new) - `ViewportBackend` trait (the seam:
  `resize`, `camera`/`set_camera`, `frame`/`frame_hash`, `frame_time_secs`) and
  `CpuBackend`, a deterministic wrapper over the existing `render_present` path.
  `frame_hash()` is a dependency-free FNV-1a 64 of the rendered pixels; unit
  tests prove determinism, numeric orbit/pan/zoom camera deltas, resize, and
  that the seam hands back byte-identical frames to a direct render call (the
  CPU backend never alters the committed golden output).
- `crates/oep-viewer/src/lib.rs` - one new `pub mod backend;` and re-exports
  (`CpuBackend`, `ViewportBackend`, `Vec3`); nothing else changed.
- `crates/oep-app/src/panels/viewport.rs` - the live frame: the committed demo
  scene (`CpuBackend::demo`, the canonical 80x40x10 solid) rendered at the
  panel size, drawn into the panel via an egui texture, navigated with the
  mouse (orbit = left drag, pan = middle drag or shift+drag, zoom = wheel or
  pinch), and reporting the frame time in the status-bar mode field UI-A4
  created.
- `crates/oep-app/tests/viewport.rs` (new) - the `--test viewport` target:
  kittest drives real drags; asserts camera deltas NUMERICALLY (yaw moves by
  exactly `dx * ORBIT_RAD_PER_PX`; pan target lands exactly where
  `Camera::pan(dx*wpp, dy*wpp)` puts it; zoom distance scales by the gesture
  factor) and that the frame hash changes with each interaction.
- `crates/oep-app/Cargo.toml` - adds `oep-viewer` as a path dependency: the
  card's whole behaviour is showing the oep-viewer renderer in the app's panel,
  so the panel must link the crate that owns the seam. Workspace-internal
  crate, Apache-2.0, no new external dependency.
- `workstreams/modeler/ui/README.md` - this card's row appended.

Files outside the list, touched with reason:
- `crates/oep-app/Cargo.toml` - acceptance REQUIRES the app to use
  `oep_viewer::CpuBackend` (files owned include the oep-viewer seam AND the
  oep-app panel); a path dep is the only way the panel can name the seam crate.
  No external crate added, no licence-scan change.
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - the shot.sh
  golden. UI-A5's whole behaviour changes the shell frame (the viewport now
  renders a model instead of the stub), so the drift guard fires exactly as A2
  designed it; per the wave precedent established by UI-A4 the new frame is
  recorded as the golden, and the pre-viewport golden is preserved beside this
  proof (`ui-a2-golden-before-viewport.png`).

## Acceptance (real command output)

| Step | Command | Exit | File |
|---|---|---|---|
| step0 | `git fetch github && git merge github/wave/ui-a` | 0 (brought 11 commits) | `transcripts/01-step0.txt` |
| oep-viewer | `cargo test -p oep-viewer` | 0 (31 lib + 7 + 5 + 6 + 5 + 5 doc) | `transcripts/02-viewer-tests.txt` |
| smoke | `xvfb-run -a cargo run -p oep-app -- --smoke` | 0 (`frames=8 smoke ok`) | `transcripts/02-viewer-tests.txt` (appended) |
| viewport | `cargo test -p oep-app --test viewport` | 0 (3 viewport + 3 config tests, numeric camera deltas + hash change) | `transcripts/02-viewer-tests.txt` (appended) / `03-shot-drift.txt` |
| gate | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | 0 | `transcripts/06-gate.txt` |
| shot drift | `bash scripts/ui/shot.sh` against A4 golden | 1 (expected: viewport now renders) | `transcripts/03-shot-drift.txt` |
| shot green | `bash scripts/ui/shot.sh` after recording new golden | 0, sha256 `35182d8d…9f5f` | `transcripts/04-shot-green-run1.txt`, `05-shot-green-run2.txt` |
| merged head | merge `github/wave/ui-a` (UI-A6 landed during this card) into this branch | 0 (union deps, both README rows; golden re-derived on merged head) | landing section |
| shot merged | `bash scripts/ui/shot.sh` on the merged head | 0, sha256 `97f0a035…7672` (byte-identical x2) | `transcripts/07-shot-merged-run1.txt`, `08-shot-merged-run2.txt` |
| gate | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | 0 | `transcripts/06-gate.txt` |

### 1. `cargo test -p oep-viewer` (exit 0)

The `ViewportBackend` seam and `CpuBackend` unit tests plus the crate's
existing suite: 31 library tests pass, including the new backend tests
(determinism, orbit/zoom/pan numeric deltas, resize, camera round-trip, frame
time, and `seam_does_not_alter_output` which byte-compares the backend frame to
a direct `render_present` call).

### 2. `cargo test -p oep-app --test viewport` (exit 0)

kittest drives the real `app::App`:

- **Left drag** 80px right over the viewport: yaw moves by exactly
  `80 * ORBIT_RAD_PER_PX = 0.4` rad (assert delta `< 1e-9`); pitch unchanged;
  frame hash changes.
- **Middle drag** 60px right + 30px down: orbit target lands exactly where
  `Camera::pan(60*wpp, 30*wpp)` puts it (per-component `< 1e-6`); frame hash
  changes.
- **Zoom** gesture 1.2x in: distance becomes `before/1.2` (`< 1e-4`); frame
  hash changes.

The assertions read the scalar snapshot the panel publishes into the egui
context data (same key the panel uses), so they check the real seam state, not
a test double.

### 3. `bash scripts/ui/shot.sh` - viewport frame, hash recorded

`frame.png` is the deterministic frame of the real `isolith` binary on a
1280x800 Xvfb screen after UI-A5: the five-region shell with the live viewport
(80x40x10 solid + grid + axes, status bar reporting `Select · 10 ms`). The
stub that UI-A4 left in the viewport is gone; the refactor is exactly what A5
set out to prove, so the shell golden moved (validated: drift guard fired once
loudly, old golden preserved).

During this card's run the wave head advanced (UI-A6 landed on `wave/ui-a`),
so the FINAL golden is the one recorded on the merged head, where the shell
shows BOTH the A6 palette and the A5 viewport. Two independent merged-head
shot.sh runs are byte-identical:

```
$ sha256sum workstreams/modeler/ui/proof/ui-a2/frame.png
97f0a035a70219c035e0c27702aabf441e7f9eddb6c094e26f602675b3557672
```

Same hash recorded in `proof/ui-a5/frame.png.sha256`. (The pre-merge
viewport-only frame recorded `35182d8d…9f5f`; the A6 golden preserved beside
it is `66d7010e…a694`.)

## Image provenance

- `frame.png` - 1280x800 xvfb capture on the merged head (A5 viewport + A6
  palette); sha256 `97f0a035…7672`; byte-identical across two merged-head
  shot.sh runs.
- `ui-a2-golden-before-viewport.png` - the UI-A4 shell golden (hash
  `49749550…9a9d`), preserved so the drift is auditable.

## Notes for the review of this card

- The seam is the Card's own four-element contract (resize, camera,
  set_camera, frame hash) plus the pixel/time accessors the panel needs to draw
  and report; the CPU backend wraps `render_present` unchanged
  (`seam_does_not_alter_output` proves it byte-for-byte).
- Frame time in the status bar is quantized to whole tens of milliseconds
  (`Select · 10 ms`), a deliberate choice so the deterministic screenshot
  golden never sees a wall-clock digit flip between two captures; on this host
  the ~12 ms CPU render is stably in the 10 ms bucket.
- No test was weakened or deleted; all A1-A4 wave tests still pass.
