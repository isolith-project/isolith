# UI-D5 UI-PERF: interactive frame budget on U1, recorded numbers — proof

Wave D card. Behaviour: one repeatable perf harness renders a 1M-triangle scene
at 1600x1000 through the real GPU viewport backend and records the frame-time
distribution, so "interactive" is a measured claim with a committed p95 — not a
hope. This card's honest outcome is a **BLOCK (`capability`)**: the committed
threshold p95 <= 16.6 ms is **NOT met on this device** (measured p95 = 438.6 ms
on the real NVIDIA GB10, not lavapipe). The bar is deliberately NOT lowered and
the real measured numbers are recorded here.

## STEP 0

`git fetch github && git merge github/wave/ui-d` ran first from the worktree
(`ui/d5-perf`). It brought the wave head 23aa051 (UI-D1/D2/D3/D4/D7 and their
merges) as a fast-forward — **not empty**. Working tree clean after the merge.

## Owned files (exactly the card's list)

- `scripts/ui/perf.sh` (new) — runs all three acceptance legs, writes
  `proof/UI-D5/perf.json`, exits non-zero when the budget is missed (loud miss);
  the report is written regardless.
- `crates/oep-viewer/tests/perf_budget.rs` (new) — the harness: an
  always-running fast smoke variant + an `#[ignore]`d heavy 1M measurement.
- `workstreams/modeler/ui/proof/UI-D5/` — this proof, the three transcripts and
  `perf.json`.
- `workstreams/modeler/ui/README.md` — this card's row (status
  `blocked-capability`, measured numbers included).

No other file touched.

## The harness (design)

`perf_budget.rs`:

- `perf_budget_fast_smoke` — **always runs** (the acceptance green leg):
  builds the deterministic 1M-triangle mesh, asserts `>= 1,000,000` triangles
  and that `mesh.triangles.len()` matches; computes the `CpuBackend` demo
  frame_hash and asserts it equals the committed golden
  `12597229237775278722` (CPU determinism unchanged — printed); and, when an
  adapter is reachable, renders the small demo scene once through the GPU
  backend, echo-hashes it twice for the determinism oracle, and prints the
  adapter. On a GPU-less host the GPU half is skipped and the CPU leg still
  passes.
- `perf_budget_1m_gpu_heavy` — `#[ignore]`d by default, run by `perf.sh`:
  builds the scene, creates the GPU backend, renders 20 warm-up + 120 measured
  frames of the SAME `(scene, camera, size)` via `re_render_with_current_shading()`
  (each call = one full `render_frame`: CPU projection + upload + draw + readback
  + GPU completion — the real interactive cost the seam pays per frame), sorts
  the 120 samples, reports p50/p95/p99/max + min/mean fps, writes the machine
  report JSON, prints the CPU determinism hash, then GATES on
  `p95 <= 16.6 ms`. A miss still writes the report and is stated plainly; the
  test then fails loudly (bar NOT lowered).

## Accepted/measured numbers (real, from this host)

Source: `transcripts/02-heavy-perf.txt` and `perf.json` (three independent
runs, all consistent):

```
scene: 1,002,528 triangles, 1600x1000, GPU backend (Vulkan / NVIDIA GB10 / 580.178.04)
warm-up 20 frames; 120 measured frames
min frame ms:  424.575     mean: 431.977
p50 431.835   p95 438.617   p99 444.862   max 447.043
min fps 2.24   mean fps 2.31
adapter: name=NVIDIA GB10 driver=NVIDIA driver_info=580.178.04 backend=Vulkan device_type=IntegratedGpu
CPU determinism: demo frame_hash = 12597229237775278722 == committed golden (match=true)
BUDGET: p95=438.617 ms vs committed threshold 16.6 ms -> THRESHOLD MISSED
```

(The first two exploratory runs measured p95 = 449.441 ms and 442.918 ms; the
committed run in `02-heavy-perf.txt` is 438.617 ms — the miss is not marginal,
it is ~26x over the bar, so run-to-run noise does not change the verdict.)

### Why the number is what it is

This measures the FULL interactive seam, not a raw microbenchmark. The
P6-M5-T03 1M budget (0.31 ms mean) drew a pre-uploaded indexed mesh headless
with no per-frame CPU scene work. The `GpuBackend` here — what the app drives —
re-assembles and re-uploads the scene every frame: CPU projects every vertex to
NDC and computes smooth normals, packs >100 MB of shaded vertices, creates a
fresh vertex buffer each frame, then reads back 6.4 MB of RGBA and waits for GPU
completion. At 1M triangles / 1600x1000 that full path sustains ~2.3 FPS
(≈430 ms/frame) on this U1, so the 16.6 ms p95 interactive budget is missed by
a wide margin on this device. The recorded number is the honest cost of the
current seam at this size.

## Acceptance legs

1. `cargo test -p oep-viewer --test perf_budget` — **green** (fast smoke
   always-runs leg; 1 passed, 0 failed; `01-fast-smoke.txt`).
2. `xvfb-run -a cargo run -p oep-app -- --smoke` — **exit 0**,
   `frames=8 smoke ok`, renderer=wgpu viewport-backend=gpu (`03-app-smoke.txt`).
3. `bash scripts/ui/perf.sh` — writes `perf.json`, **exits 1** because the
   committed threshold is missed on this device (this is the honest loud-miss
   signal, not a faked green).

Because the acceptance threshold (p95 <= 16.6 ms) is not reachable on this U1
(a real NVIDIA GB10), the card is a **legitimate `capability` block** per the
card body: "you may NOT lower the bar: record the measured p95, say plainly
that the threshold is missed on this device, and block the card `capability`
with the number in the reason." Card blocked with `438.6 ms` in the reason.

## No images produced

This card's proof is numbers (perf.json + transcripts); no PNG/screenshot was
produced, so there is no image sha256 to record. (The card requires a sha256
for every image produced; zero images were produced.)

## Transcripts

- `transcripts/00-step0-merge.txt` — STEP 0 merge log (wave head brought).
- `transcripts/01-fast-smoke.txt` — fast smoke leg output.
- `transcripts/02-heavy-perf.txt` — heavy 1M measurement (real numbers + loud miss).
- `transcripts/03-app-smoke.txt` — app smoke output.
