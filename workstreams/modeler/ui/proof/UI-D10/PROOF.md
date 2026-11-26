# UI-D10 UI-RESIDENT-SCENE — proof: geometry kept on the GPU, per-primitive cull, dirty-only re-upload

Wave D, the render-seam fix (UI-D9 ADR Option A, stages 1 + 2 of the prior-art
brief). Branch `ui/d10-resident-scene`, based on `github/wave/ui-d` (head
`d8e200f`, the UI-D5 perf harness; carries UI-D1/2/3/4/7/8). Every commit DCO-
signed by the commit-msg hook (`Signed-off-by: Don Allen
<don@redonealpha.com>`). All raw transcripts under `transcripts/`; the ADR and
the prior-art brief are copied byte-identical as reference files (sha256
below).

## STEP 0

`git fetch github && git merge github/wave/ui-d` -> `Already up to date.` The
wave head (`d8e200f`) was already this branch's base. The UI-D9 ADR lives on
the UNMERGED `ui/d9-perf-profile` branch, so this proof carries byte-identical
copies under this directory to stay self-contained on the wave tree.

## The pixel oracle outranks transport: why this is a merged-resident design

The card's hardest gate is unchanged GPU pixels — 21 committed proof frames
(md: 16 UI-D7 + 5 UI-D2) sha256-identical, `CPU_DEMO_GOLDEN_HASH` unchanged,
coverage band unchanged — and scope rule 4 keeps the global painter sort. That
forces the exact byte stream the pre-D10 path drew: ONE triangle list in global
painter-sorted order. That order is camera-dependent (it is the sort), so the
stream can only be reused verbatim when nothing that affects it changed.

Two candidate transports were weighed.

**A. World normals in the resident buffer, rotated in the vertex stage** (the
ADR's "normals computed once at upload"). The pixel math is NOT bit-identical:
the CPU keeps view-space normals (`normalize_f64(R·n`) then f32-truncates them
per frame, and `trunc_f32(normalize_f64(R·n)) != normalize_f32(R_f32 ·
trunc_f32(n))` in the last ulp. A pure-math probe over exactly the committed
golden fixtures (demo box at all 8 D7 views, the gpu_shading overlap fixture)
counts **292 per-vertex f32 normal mismatches** (`probe-normal-bits.txt`,
committed). Any one of them can round a lit pixel differently, failing the
whole-frame sha256. Design A is therefore REFUTED for this renderer by the
card's own hierarchy: I1/I5 (byte pixels) outrank transport in the ADR's
invariant contract, and the card says stop-and-block rather than regolden.

**B. Keep the exact CPU per-frame view-space normal derivation; make the
painter-sorted stream itself resident.** The stream is created once in
device-resident buffers and re-uploaded via `queue.write_buffer` only when it
genuinely changes; identical-input frames (the perf protocol's fixed camera)
re-derive nothing, upload only the 64-byte MVP, and draw the resident ranges as
is. Because the algorithm, order and bytes are identical, rasterization is
byte-identical (all 21 goldens prove it; tables below). The card's "per-mesh
buffers / one range per mesh" is deliberately NOT taken: a per-mesh layout
would fix each mesh's internal face order and break the global painter sort,
moving pixels by construction (the thing the card forbids), for no steady-state
gain — the only camera-dependent per-vertex data (view-space normals) changes
for every visible mesh on a camera move regardless of layout. This deviation
from the ADR's transport sketch is recorded here because the ADR's own
highest-ranked invariant is the byte-identity it would break.

## What lands (the seam change)

`crates/oep-viewer/src/gpu/resident.rs` (new):
- `ResidentScene`: one device-resident face buffer + one line buffer, created
  once (`usage: VERTEX | COPY_DST`), written only on change, with per-frame
  counters (upload bytes, buffer allocations, draw calls) and a cumulative
  allocation guard; `queue.write_buffer(buffer, 0, bytes)` on dirty only.
- `frame_key(epoch, camera, size, mode, alpha)`: the per-frame dirty key. The
  scene is owned by `GpuBackend` and immutable between `set_scene` calls, so a
  monotone scene generation is an exact, allocation-free oracle for every
  `(prim.id, version)` the card names (mesh bytes via `MeshData: PartialEq`,
  transform, highlighted, base colour); camera/size/shading complete the key.
  Equal key => resident streams are byte-identical to this frame's => reuse.
- `FaceArena` + `FaceMeta`: the reused CPU face arena (flat world/normal
  scratch + metadata). `push_face`'s ~1M per-face `Vec` pairs become ranges
  into flat buffers; `clear()` keeps the backing store, so full-path frames
  allocate no per-face storage and the sort/pack operate on survivors only.
- `aabb_visible(world, vp)`: conservative per-primitive world-AABB frustum
  cull (Gribb-Hartmann on the single `view_projection`), before assembling
  anything; the CPU only touches survivors (faces and edges).

`crates/oep-viewer/src/gpu/mod.rs` (seam): `render_frame` computes the dirty
key and takes the **fast path** when nothing changed (only the 64-byte MVP
uniform is written, the resident ranges are drawn, zero buffer allocations,
single-digit draws) or the **full path** otherwise (cull -> assemble survivors
into the arena -> global painter sort -> pack -> re-upload dirty streams). New
`set_scene` (mesh edit / highlight / re-tessellation entry, bumps the epoch),
and seam counters: `last_upload_bytes`, `last_buffer_allocs`,
`total_buffer_allocs`, `last_draw_calls`, `last_assemble_time`,
`last_faces_submitted`, `last_scene_triangles`, `last_culled_prims`,
`last_fast_path`.

Scope item 5 (colour/alpha out of the vertex stream) is DECLINED with reason:
moving per-primitive base RGB out requires a per-primitive draw (one draw per
visible prim), which breaks the single-digit-draw rule; the alpha channel is
already constant-per-primitive and moving it to a uniform buys an unmeasured
slider edge at the cost of touching the shader + bind group — the byte-identity
risk is not worth an unmeasured gain. 16 of the 40 B/vertex stay as D2/D8
shipped. Stated, not hidden.

## Acceptance legs (raw transcripts committed)

| Leg | Result | Transcript |
|---|---|---|
| STEP 0 merge | `Already up to date` | (log) |
| `cargo test -p oep-viewer --test resident_scene` | **6/6 green** (the new acceptance harness) | `02-resident-scene.txt` |
| `cargo test -p oep-viewer --test gpu_backend` | **6/6 green** (CPU golden round-trip + 5% coverage band + determinism oracle) | `03-gpu-backend.txt` |
| `cargo test -p oep-app --test pick_gpu` | **123/0 green** (20 sampled pixels idbuf==analytic pick, all match=true) | `04-pick-gpu.txt` |
| `cargo test -p oep-viewer --test render_axis_views` | **4/4 green** (all 16 D7 proof frames sha256 re-render identical) | `05-render-axis-views.txt` |
| `cargo test -p oep-app --test gpu_shading` | **125/0 green** (5 D2 snapshots sha256 identical, edges 24==24, 50%-alpha readback) | `06-gpu-shading.txt` |
| `cargo test --workspace` | **exit 0** (4608 passed / 0 failed) | `01-workspace.txt` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | exit 0, `frames=8 smoke ok`, `renderer=wgpu viewport-backend=gpu` | `07-app-smoke.txt` |
| `bash scripts/ui/perf.sh` | **exit 0 — BUDGET MET** (see perf section) | `08-perf-full.txt` + `perf-legs/ui5-{01,02,03}.txt`, `perf-after.json` |

The workspace gate ran as `cargo test --workspace --jobs 4` (one clean exit-0
pass). This host's documented pre-existing egui_kittest **exit-time SIGSEGV
flake** (layout / feature_from_sketch / pick_gpu / feature_tree / dimensions,
D7-and-D8-recorded as reproducing on the UNMODIFIED base, all assertions pass,
crash at teardown) hit on several full-parallel and even isolated attempts;
limiting concurrent test binaries is the standard dodge for that documented
race, exactly the same class the D4/D7/D8 proofs report (`--jobs 4` is still
`cargo test --workspace`, no test weakened or skipped; the exit-0 transcript
is committed).

## The numbers this card owns

### 1. Upload bytes per frame (the headline)

The 1M grid, steady state (all acceptance protocol frames, fixed camera):

| metric | before UI-D10 | after UI-D10 |
|---|---|---|
| vertex bytes uploaded/frame | 120,303,360 B (3,007,584 verts x 40 B) | **64 B** (the MVP uniform only) |
| device-buffer allocations/frame | 2 (create_buffer_init x2) | **0** (resident buffers created once) |
| draw calls/frame | 2 | **2** |
| fast path taken | n/a | true (every steady frame) |

Quoted from `02-resident-scene.txt`: steady frames print `upload=64 B,
buffer_allocs=0 (total 2), draw_calls=2, fast_path=true` across the measured
run, with the harness assertion `steady-state uploads must be 64 B` green.

### 2. The counter is real (it responds to real dirty events)

Same transcript: a camera move prints `upload bytes = 181464 (was 64 at
steady state; per-frame geometry before UI-D10 was 120303360)` with
`buffer_allocs=0` (resident buffers reused, not re-created); a one-mesh edit
through `set_scene` prints `upload bytes = 181464, frame hash ... -> ...
(changed = true)`. Both move the counter; allocations stay 0; the next steady
frame is 64 again.

### 3. Per-primitive cull is a real reduction

Framed close view of a 1331-primitive assembly: `total_prims=1331
culled_prims=1044 submitted_faces=3444 total_triangles=15972
culled_ratio=0.784 assemble_ms=9.225` — 78.4% of the scene's triangles never
reach the CPU assemble, and the harness asserts `culled > 0`,
`submitted < total` and `ratio > 0.05`. The cull is conservative: the fully-
framed demo box is NEVER culled at any of the 8 committed D7 views
(`culled_prims = 0` across Front/Back/Left/Right/Top/Bottom/Iso), and the
all-culled camera (near plane inside the model) culls exactly the box — the
D7 empty-frame semantics preserved.

### 4. Fast path is byte-identical (determinism through reuse)

`fast_path_frame_is_byte_identical_to_full_path`: a resident-reused render
hashes exactly like the preceding full-derivation render
(hash `11436386787023745229` on both), bytes equal, an independently
constructed backend agrees, and a shading change (alpha 0.5) is a genuine
dirty event that selects the full path and changes pixels.

## Pixel oracle — all gates held

- **21 / 21 proof frames sha256-identical.** `render_axis_views` re-rendered
  the 16 D7 frames and `gpu_shading` the 5 D2 snapshots; every sha256 equals
  the committed value (full table in `transcripts/05` and `06`; the D8
  `golden_table.sh` re-run prints `goldens checked=21 pass=21 fail=0`, logged
  as `09-golden-table.txt`). `git status` after these runs is clean on
  `workstreams/modeler/ui/proof/` — the re-rendered PNGs are byte-identical to
  the committed files.
- **CPU golden unchanged**: `CPU_DEMO_GOLDEN_HASH 12597229237775278722` —
  `gpu_backend` round-trip and the perf smoke both print
  `equal = true` / `match = true`.
- **GPU determinism oracle**: perf smoke and `gpu_backend` both re-prove
  identical inputs hash identically; the structural echo
  `frame_hash 9287941873092625150 -> 9287941873092625150, identical = true`
  matches the D9-recorded pre-card echo value.
- **Coverage band unchanged**: `gpu_backend` prints demo GPU 45866 vs CPU
  45357 (identical to the D8 recorded numbers), bracket within the 5% band.
- **Picking parity unchanged**: `pick_gpu` 123/0, every sampled pixel
  `idbuf == oep_viewer::pick`, match=true.
- **Transparency unchanged**: `gpu_shading` prints the overlap-region readback
  alpha opaque 255.0 -> 50% 146.7 (the committed threshold asserts inside the
  green test) and the transparent-50 snapshot is sha256-identical.

## perf: p95/p50/p99 after the change vs the committed reference

Same UI-D5 protocol (1,002,528 triangles, 1600x1000, 20 warm-up + 120 measured
frames; the committed `scripts/ui/perf.sh` on this tree — `perf-exit.txt`:

| metric | UI-D5 committed reference | after UI-D10 (this run) |
|---|---|---|
| p95 | 434.94 ms (UI-D5 record; D9 clean run 424.81) | **6.449 ms** |
| p50 | 431.84 ms | **6.309 ms** |
| p99 | 444.86 ms | **6.552 ms** |
| max / min | 447.04 / 424.57 ms | 6.779 / 6.037 ms |
| fps mean / min | 2.31 / 2.24 | **158.9 / 147.5** |
| budget_met (<= 16.6 ms) | false | **true** |

`perf.sh` printed `BUDGET MET — interactive at 1M triangles on this device.`
and exited 0; the machine report is committed as `perf-after.json` (the
script wrote the current tree's numbers into `proof/UI-D5/perf.json`, which is
UI-D5's owned file — restored to its committed content afterwards; the new
report and the three leg transcripts live under `perf-legs/`).

Honest boundary of this number: the protocol measures STEADY STATE (fixed
camera), which is exactly the card's BEHAVIOUR ("a steady-state frame uploads
no geometry ... draws in single-digit draw calls"). An interactive camera move
at the full 1M grid is a genuine dirty event: the seam re-derives the
painter-sorted stream (cull survivors + project + sort + pack, arena-reused)
and re-uploads the view-space normals it must — the measured camera-move upload
for a small assembly jumps from 64 B to 181,464 B (harness transcript), and at
the 1M grid a move pays, for that one dirty frame, the derivation cost the D9
profile already measured (assemble ≈ 293 ms + the ~120 MB view-normal
re-upload) because per-vertex view-space normals cannot live in a camera-
independent buffer without moving pixels (Design A refuted, above). The
committed 16.6 ms interactive bar is therefore met at steady state on this
device; removing the full-visible camera-move tail is the documented next-stage
work (the 3b depth card the ADR sequences after A), not hidden and not
estimated.

## Reference files (byte-identical copies for the D9-unmerged tree)

| file | sha256 |
|---|---|
| `render-seam-performance.md` (UI-D9 ADR v1.0.1, 2026-11-25) | `2d56d08519f49105a79cbe9375db8ab444dd1e4e66abafb0ad2490f49f04eafe` |
| `prior-art.md` (the 36-source UI-D9 brief) | `c5120c2b21bbbe55683a425eca55d6ec2bc7a340137cf866cbbfad595e6dc706` (identical to the D9-recorded brief hash) |
| `probe-normal-bits.txt` (Design-A probe, 292 mismatches) | recorded in the artifact manifest |

## Pre-existing flake note

Same documented class as D4/D7/D8: egui_kittest harness binaries occasionally
SIGSEGV at exit after all assertions pass (layout / feature_from_sketch /
pick_gpu / feature_tree / dimensions observed this run; D8's proof verified it
reproduces on the unmodified base). My diff touches no oep-app file; `pick_gpu`
passed 123/0 both isolated and again in the exit-0 workspace run. The
committed workspace transcript is the clean `--jobs 4` run.

## Changed files (owned list only)

`crates/oep-viewer/src/gpu/resident.rs` (new), `crates/oep-viewer/src/gpu/mod.rs`,
`crates/oep-viewer/tests/resident_scene.rs` (new),
`workstreams/modeler/ui/proof/UI-D10/`, `workstreams/modeler/ui/README.md`.
Not touched: `render.rs`, `gpu/idbuffer.rs`, `pick.rs`, `gpu/target.rs`,
`gpu/device.rs`, `tests/perf_budget.rs`, `scripts/ui/perf.sh`, `gpu/shading.rs`,
`gpu/shaders/shading.wgsl`, `scene.rs`, or any other card's tests/goldens.
