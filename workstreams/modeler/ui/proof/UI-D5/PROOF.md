# UI-D5 UI-PERF: interactive frame budget on U1, recorded numbers — proof

Wave D card. Behaviour: one repeatable perf harness renders a 1M-triangle scene
at 1600x1000 through the real GPU viewport backend and records the frame-time
distribution, so "interactive" is a measured claim with a committed p95 — not a
hope.

## Outcome (this run, merged wave head)

CARD PROMOTED by the orchestrator after the renderer changed under it. On the
merged wave head (`ba7e3bc`, carrying UI-D10's GPU-resident scene) this card's
own harness now measures the budget MET: p95 = 6.408 ms at 1,002,528 triangles /
1600x1000 on the real NVIDIA GB10, against the committed 16.6 ms threshold.
`bash scripts/ui/perf.sh` exits 0. The bar is unchanged (16.6 ms, never
lowered).

The earlier miss (p95 = 438.617 ms recorded on the pre-D10 seam at `43572a8`)
was a real finding, and it is preserved, not erased: the historical miss lives
in this card's kanban thread, in the render-seam performance ADR
(`workstreams/modeler/ui/decisions/render-seam-performance.md`, Option A being
the adopted fix), and in the README row which names it as history before the
current state. UI-D9 profiled the miss (CPU re-derivation + 120 MB re-upload per
frame dominated); UI-D10 implemented the ADR's Option A — geometry resident on
the GPU, per-primitive world-AABB cull, dirty-only re-upload, painter sort kept,
pixels byte-identical (21/21 goldens).

## STEP 0

`git fetch github && git merge github/wave/ui-d` from the worktree
(`ui/d5-perf` at the pushed tip 43572a8) fast-forwarded to the wave head ba7e3bc
— **not empty** (brought UI-D8/UI-D9/UI-D10, 60 files). Working tree clean
after the merge. Transcript in `transcripts/00-step0-merge.txt`.

## Owned files (exactly the card's list)

- `scripts/ui/perf.sh` — runs all three acceptance legs, writes
  `proof/UI-D5/perf.json`, exits 0 when the committed threshold is met (loud
  miss when it is not; the report is written regardless).
- `crates/oep-viewer/tests/perf_budget.rs` — the harness: an always-running fast
  smoke variant + an `#[ignore]`d heavy 1M measurement.
- `workstreams/modeler/ui/proof/UI-D5/` — this proof, the transcripts and
  `perf.json`.
- `workstreams/modeler/ui/README.md` — this card's row (status `done`, measured
  numbers included, historical miss named).

No other file touched.

## The harness (design)

`perf_budget.rs`:

- `perf_budget_fast_smoke` — **always runs** (the acceptance green leg): builds
  the deterministic 1M-triangle mesh, asserts `>= 1,000,000` triangles and that
  `mesh.triangles.len()` matches; computes the `CpuBackend` demo frame_hash and
  asserts it equals the committed golden `12597229237775278722` (CPU
  determinism unchanged — printed); and, when an adapter is reachable, renders
  the small demo scene once through the GPU backend, echo-hashes it twice for
  the determinism oracle, and prints the adapter. On a GPU-less host the GPU
  half is skipped and the CPU leg still passes.
- `perf_budget_1m_gpu_heavy` — `#[ignore]`d by default, run by `perf.sh`: builds
  the scene, creates the GPU backend, renders 20 warm-up + 120 measured frames
  of the SAME `(scene, camera, size)` via `re_render_with_current_shading()`,
  sorts the 120 samples, reports p50/p95/p99/max + min/mean fps, writes the
  machine report JSON, prints the CPU determinism hash, then GATES on
  `p95 <= 16.6 ms`. A miss still writes the report and is stated plainly; the
  test then fails loudly (bar NOT lowered).

## Accepted/measured numbers (this run, real, from this host)

Source: `transcripts/02-heavy-perf.txt` and `perf.json`:

```
scene: 1,002,528 triangles, 1600x1000, GPU backend (Vulkan / NVIDIA GB10 / 580.178.04)
warm-up 20 frames; 120 measured frames
min frame ms:  6.090     mean: 6.302
p50 6.313   p95 6.408   p99 6.471   max 6.823
min fps 146.6   mean fps 158.7
adapter: name=NVIDIA GB10 driver=NVIDIA driver_info=580.178.04 backend=Vulkan device_type=IntegratedGpu
CPU determinism: demo frame_hash = 12597229237775278722 == committed golden (match=true)
BUDGET: p95=6.408 ms vs committed threshold 16.6 ms -> THRESHOLD MET
```

The budget is met with margin (~2.6x under the 16.6 ms bar), and the CPU golden
is unchanged from the pre-D10 runs, so the fix moved the seam's per-frame cost
without disturbing CPU determinism.

## Acceptance legs

1. `cargo test -p oep-viewer --test perf_budget` — **green** (fast smoke
   always-runs leg; 1 passed, 0 failed; `01-fast-smoke.txt`).
2. `xvfb-run -a cargo run -p oep-app -- --smoke` — **exit 0**,
   `frames=8 smoke ok`, renderer=wgpu viewport-backend=gpu (`03-app-smoke.txt`).
3. `bash scripts/ui/perf.sh` — **exit 0**, writes `perf.json`, verdict
   BUDGET MET (perf.json `budget_met: true`).

## Historical miss (attempt 1, preserved)

Measured on the pre-D10 seam at `43572a8` (same host, adapter and scene):
p50 431.835 ms, p95 438.617 ms, p99 444.862 ms, max 447.043 ms, min fps 2.24,
mean fps 2.31 — p95 ~26x over the 16.6 ms bar, so the card was a legitimate
`capability` block. Root cause (profiled by UI-D9): the seam re-derived and
re-uploaded the whole scene every frame (CPU per-frame projection/assemble +
~120 MB vertex re-upload + 6.4 MB readback), sustaining ~2.3 FPS at 1M
triangles. Fixed by UI-D10 (render-seam ADR Option A). The bar was never
lowered.

## No images produced

This card's proof is numbers (perf.json + transcripts); no PNG/screenshot was
produced, so there is no image sha256 to record. (The card requires a sha256
for every image produced; zero images were produced.)

## Transcripts

- `transcripts/00-step0-merge.txt` — STEP 0 merge log (this run's fast-forward to ba7e3bc).
- `transcripts/01-fast-smoke.txt` — fast smoke leg output.
- `transcripts/02-heavy-perf.txt` — heavy 1M measurement (real numbers, THRESHOLD MET).
- `transcripts/03-app-smoke.txt` — app smoke output.
