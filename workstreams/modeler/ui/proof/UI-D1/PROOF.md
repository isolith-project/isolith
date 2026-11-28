# UI-D1 UI-GPU-BACKEND: wgpu viewport behind the seam, shared device with egui

Card: `t_adbd567c`, wave D. Real command output lives in `transcripts/`; every
image's sha256 is in `images.sha256`.

## What landed

`GpuBackend` (new `crates/oep-viewer/src/gpu/`) is a second implementation of
the `ViewportBackend` seam that renders the SAME `(scene, camera, size)` with
wgpu into an offscreen colour target and reads the frame back into the seam's
`RgbaImage`. The app selects it at runtime:

- `oep_viewer::select_backend(scene)` returns the GPU backend when a wgpu
  adapter is reachable and `ISOLITH_GPU` is not `off`; `CpuBackend` otherwise.
- `main.rs` picks eframe's wgpu renderer when an adapter is available and the
  glow renderer otherwise, and registers egui's own wgpu device with the
  viewport (`gpu::device::register_egui_device`) so the GPU viewport shares
  egui's exact device/queue. One wgpu version in the lock (verified below).
- `viewport.rs` constructs through `select_backend` instead of `CpuBackend`,
  holding the seam behind `Arc<Mutex<Box<dyn ViewportBackend>>>` (the egui
  context store needs cloned, Send+Sync state).

## Coverage parity (exact numbers, printed by the seam test)

800x600, same framed+orbit camera, CPU `CpuBackend::render_present` vs GPU:

| fixture | GPU covered px | CPU covered px | |delta| | within 5% |
|---|---|---|---|---|
| demo (80x40x10) | 23658 | 23524 | 134 (0.57%) | yes |
| bracket (golden, 5 bodies) | 25833 | 25755 | 78 (0.30%) | yes |

GPU and CPU pixels are NOT byte-equal and are not claimed to be: the GPU
rasterizes 1px wgpu lines, skips the 5x7 axis LETTERS, and its fill rule
differs at triangle edges. What is identical is the projected geometry.

## Why the GPU consumes CPU-projected NDC (important, documented in gpu/mod.rs)

The CPU renderer's committed goldens project with `w = row4(mvp) * (view * p)`
(its `clip_w`). The mathematically raw `row4(mvp) * p` does not reproduce it:
measured on the demo view, the raw form renders the same box ~1.6x larger on
screen (corner bbox 325..511 px vs the golden 340..456 px). A GPU that
projects with the textbook formula shows a genuinely different view than the
CPU goldens, so `GpuBackend` projects every vertex with the exact CPU math and
hands wgpu pre-divided NDC positions (the same immediate-mode model egui
itself uses). The GPU owns the rasterization; the CPU owns the matrix
pipeline. Wave D2+ may move the transform into the vertex shader once the CPU
path's projection is corrected and re-goldened.

## Acceptance (all green, transcripts in transcripts/)

| acceptance | result |
|---|---|
| `cargo test -p oep-viewer --test gpu_backend` | 6/6 pass x3 parallel runs (numbered output in `01-gpu-backend-test.txt`; the lavapipe ICD segfaulted under concurrent device churn, so the device-touching tests hold a process-wide lock - measured fix, documented in the test) |
| `cargo tree -d -p oep-app \| grep -c '^wgpu '` | `0` duplicate lines = a single `wgpu v30.0.2`, shared by `egui-wgpu` (eframe 0.37) and `oep-viewer` (`02-tree-single-wgpu.txt`) |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | exit 0; `isolith smoke: renderer=wgpu viewport-backend=gpu` (`03-smoke-gpu.txt`) |
| `ISOLITH_GPU=off xvfb-run ... -- --smoke` | exit 0; `renderer=glow viewport-backend=cpu` - honest fallback (`04-smoke-cpu.txt`) |
| `cargo test -p oep-app --test viewport` | 117/117 pass with the GPU backend in-app (`05-viewport-test.txt`) |
| `cargo test -p oep-app` (all targets) | 22 targets / all green, no segfault (`07-oep-app-suite.txt`) |
| `cargo test -p oep-viewer` (all targets) | all green incl. the 6 new gpu_backend tests |
| fmt / clippy `-D warnings` (touched crates) | exit 0 (`06-fmt.txt`, `06-clippy-viewer.txt`, `06-clippy-app.txt`) |
| `cargo check --workspace` | clean |

The seam test additionally proves: adapter present -> `select_backend` returns
the GPU backend and `GpuBackend::new` is Some; the demo and bracket fixtures
render non-empty frames at 800x600 (28136 and 26088 non-background px);
rendering the same view twice is byte-identical (determinism oracle on GPU);
and with `ISOLITH_GPU=off` `select_backend` returns `CpuBackend` whose demo
frame hash equals the pre-card golden `11436915591341135581` exactly.

## Launch frame drift (the integrate card owns the golden refresh)

The wave-D viewport now renders on the GPU, so the launch frame changed from
the wave-C golden. Per the card's explicit instruction the shot.sh golden was
NOT refreshed here; the `before` (wave-C golden) and the `after` (observed with
this change, captured with shot.sh's own xwd+decode path) are:

```
before (proof/ui-a2/frame.png.sha256) : a7594b124ac8ea49c95ef49d2c25106bf37cf309ca808d652e9943bb417586ef
after  (launch-frame-after-gpu.png)   : 0641915bb23413f7701425679dcf1b4d990bb233ac16a3e1fabd9679afe3f97f
```

The after frame is included as a normal image (verified: full shell layout,
GPU viewport rendering the yellow demo box on the grid with the RGB triad).
The wave-D integrate card owns refreshing `scripts/ui/shot.sh`'s golden.

## Owned-files deviation flagged per the card's rule

- **`crates/oep-app/src/app.rs`** (NOT in the card's owned list): the card's
  "renderer selection: wgpu when available, glow otherwise" requires eframe's
  `glow` feature, which changes `eframe::App::on_exit` to also receive the
  glow context. `on_exit(&mut self)` became `on_exit(&mut self,
  _gl: Option<&eframe::glow::Context>)` - a signature-only change preserving
  the prior persist-layout behaviour, flagged here as card-required. No other
  file outside the owned list was touched.

## Notes for the integrate card

- `NOTICE` and `docs/licensing-boundary.{json,md}` are NOT in this card's
  owned list; `wgpu` 30.0.2 (MIT OR Apache-2.0) joined oep-viewer's direct
  deps, so the licensing gate needs the boundary/notice refresh at the wave
  gate (the precedent wave used the same boundary at integrate time).
- The `.worktree` target/ was left for the integrate/dispatch cleanup; this
  card has not deleted it (proof must be committed first).
