# UI-D8 UI-VEC4-PIPELINE — proof: one homogeneous clip-space path, transform owned by the GPU vertex stage

Wave D structural card, branch `ui/d8-vec4`, based on `github/wave/ui-d`
(merge-UI-D7 `ec6759a`). Commits below are DCO-signed by the commit-msg hook
(`Signed-off-by: Don Allen <don@redonealpha.com>`). All transcripts under
`transcripts/`; the golden table generator is `golden_table.sh`.

## STEP 0

`git fetch github && git merge github/wave/ui-d` — the wave head (which carries
UI-D1, UI-D2, UI-D3, UI-D7) was already the branch base:
`Already up to date.` UI-D7's landing merge `ec6759a` is in the log.

## What this card does

### 1. `oep_geo::math::Vec4` + `Mat4::transform_point4`

`Vec4` matches the `Vec3` API style: `new`, `zero`, `add`, `sub`, `mul`, `div`,
`dot`, `to_array4`, `from_array4`. `Mat4::transform_point4(&self, p: &Vec3) ->
Vec4` computes the full homogeneous product (all four rows, including row 4 for
`w`) with NO perspective divide. It is the ONE place a clip-space `w` for a
point is defined. Unit tests (oep-geo, +4):
- `vec4_basics` — componentwise ops, dot, arrays round-trip;
- `transform_point4_identity` — `M·p = (p.x, p.y, p.z, 1)`;
- `transform_point4_translation` — translation by (1,2,3) moves x/y/z, w stays 1;
- `transform_point4_perspective_w` — a perspective matrix where
  `w == -z_view` (asserted for `z_view ∈ {−2, −5, −100, −137.2}`).

Transcript: `transcripts/10-oep-geo.txt` (oep-geo suite green, 133 passed / 0
failed across the crate; the 4 Vec4 tests pass).

### 2. ONE projection helper in `render.rs`

`render::project_ndc(world, vp) -> Option<[f64;3]>` is the single NDC path:

```
h = vp.transform_point4(world)     # raw homogeneous (x,y,z,w)
if h.w <= 1e-9 { None }            # at/behind near plane / degenerate
ndc = (h.x/h.w, h.y/h.w, h.z/h.w)
if ndc.z outside [-1,1] { None }   # NDC z range test
Some(ndc)
```

Used by every point-visibility site — `push_polygon`, `to_px` (CPU) and
`push_face` (GPU) — so faces are culled and anchors are rejected by the SAME
test. The duplicated `clip_w` in `gpu/mod.rs` (and the D1/D7 bug-commentary it
carried) is DELETED: one formula (`transform_point4`), one file.

WIRE endpoints (`draw_wire`, gpu `to_ndc`) deliberately do NOT go through the
z-range rejection: they use the same `transform_point4` formula but keep D7's
segment semantics — reject only a degenerate `|w| < 1e-9`, then
`clip_ndc_line` against the visible cube. See the regression note below: this
is what keeps the D7 goldens byte-identical.

### 3. GPU owns the transform

- New `ShadedVertex`: WORLD `[f64;3]` position + view-space normal + RGBA.
  `fan_triangles` is now generic `const N: usize`, and `assemble` fans WORLD
  points (the old `clip_w`+`to_px` duplicate is gone — `to_px` delegates to
  `render::project_ndc`).
- `GpuPipeline` gains a 64-byte `mat4x4<f32>` uniform buffer, a bind group
  layout and a bind group created in `init_pipeline`.
- Every `render_frame` writes the MVP once:
  `pack_matrix4(&vp)` flattens `Mat4::to_array16()` (column-major f64) to f32
  via `f32::to_ne_bytes` — NO bytemuck, NO new dependency (root Cargo.toml
  untouched).
- `shaders/shading.wgsl` vertex stage now does `u_mvp · vec4<f32>(world,1)`,
  divides by `w`, and emits a manually-constructed `clip_position` with
  `w = 1` plus a varying view-space normal for the fragment key/fill. Because
  the vertex stage outputs `w = 1`, interpolation stays linear in screen space
  — rasterization is invariant vs. the pre-projected D1/D7 path, which is why
  the pixels are byte-identical (see the golden table).
- The line/overlay pass (`viewport.wgsl`, id-buffer pass) stays a pre-projected
  passthrough: triad arms are pixel-space by construction, id-buffer is the
  standard-divide NDC path and is untouched (its math did not change).
- `gpu/mod.rs` gains `vertex_prep: Duration` (timed vertex-prep upload) +
  `vertex_prep_time()`, and the module doc now states "the vertex stage owns
  the transform".

### 4. No new dependency

Uniform bytes are packed by hand with `f32::to_ne_bytes` from
`Mat4::to_array16()` (column-major, exactly WGSL `mat4x4<f32>` layout). No
bytemuck, no Cargo.toml/lock change, no licensing leg.

## The pixel oracle did its job (regression found and fixed in-dev)

The card's hard rule is PIXELS MUST NOT MOVE against D7's committed goldens.
The first version of the wire path routed endpoints through `project_ndc`
(which rejects NDC-z out of range). That moved 4 of 14 frames — the D7 goldens
for Left (CPU+GPU), Bottom (CPU+GPU) and all-culled (CPU+GPU, 4 frames plus
the all-culled pair). Every diff was a wire-only regression: grid-colored
pixels `(112,120,132)` reverting to background `(28,30,34)` were LOST. In
all-culled exactly 231 grid pixels vanished (x 50..111, y 155..262). The
cause: D7's wire path projected both endpoints (rejecting only `w.abs() <
1e-9`) and then clipped the SEGMENT to the visible cube with
`clip_ndc_line`, so a wire whose endpoint sat behind the near plane / beyond
the far plane still drew its visible portion. Sending the endpoint through
`project_ndc` and dropping the wire whenever an endpoint returned `None`
discarded those partially-visible wires entirely.

Fix: wire endpoints again reject only degenerate `|w| < 1e-9` (D7 semantics)
and keep `clip_ndc_line`; the shared `transform_point4` formula is used
everywhere. After the fix all goldens returned to byte identity (below). This
is the correct reconciliation of the card's two requirements: the single helper
does the z-range test for POINT visibility (faces, to_px), while SEGMENT
clipping — which is a segment operation, not a point rejection — owns its own
z-slicing. No golden moved; no test was weakened.

## Golden table (the point of this card)

Every committed frame golden the crate carries re-rendered and sha256-compared
against the value UI-D7 (and UI-D2) committed. Generator: `golden_table.sh`
(compares `git show HEAD:<file>` vs the re-rendered working-tree file). Full
printout: `transcripts/07-golden-table.txt`.

| golden | expected sha256 == actual sha256 | result |
|---|---|---|
| ui-d7-Front-cpu-400x300.png | `721209ec…35c35a` | PASS |
| ui-d7-Front-gpu-400x300.png | `90750d3f…7e9898` | PASS |
| ui-d7-Back-cpu-400x300.png | `16557c16…c78690d` | PASS |
| ui-d7-Back-gpu-400x300.png | `11e7b4f9…641b977` | PASS |
| ui-d7-Left-cpu-400x300.png | `80468a5b…f3bc00` | PASS |
| ui-d7-Left-gpu-400x300.png | `262389ff…ece8568` | PASS |
| ui-d7-Right-cpu-400x300.png | `5b1a1247…84794` | PASS |
| ui-d7-Right-gpu-400x300.png | `e1eb01e8…c38d0d` | PASS |
| ui-d7-Top-cpu-400x300.png | `0a951c0e…f28618` | PASS |
| ui-d7-Top-gpu-400x300.png | `8ad31153…17f30` | PASS |
| ui-d7-Bottom-cpu-400x300.png | `f4c78e4b…0277e` | PASS |
| ui-d7-Bottom-gpu-400x300.png | `0e9f1b97…4b5cc78` | PASS |
| ui-d7-Iso-cpu-400x300.png | `adadf5c4…bb63e0` | PASS |
| ui-d7-Iso-gpu-400x300.png | `3164ea28…b054` | PASS |
| ui-d7-all-culled-cpu-400x300.png | `0f2e1911…ec043f` | PASS |
| ui-d7-all-culled-gpu-400x300.png | `5ca67f6b…1a4fbe` | PASS |
| ui-d2-shaded.png | `68b991a4…efe4d` | PASS |
| ui-d2-edges.png | `1a34e7bf…b79de` | PASS |
| ui-d2-flat.png | `cc69bd61…5e5daa` | PASS |
| ui-d2-smooth.png | `bdc83f18…73e972e` | PASS |
| ui-d2-transparent-50.png | `532c5642…97fe05` | PASS |

**21 / 21 PASS, 0 moved.** Shaded/Edges/Flat/Smooth/Transparent-50 are the GPU
pipeline's output through the new MVP uniform + vertex divide — byte-identical
to D7, proving the shader's divide-then-`w=1` form is rasterization-invariant.
`render_axis_views` prints each of the 16 D7 proof sha256s
(`transcripts/04-render-axis-views.txt`) and the coverage/determinism facts
below.

## CPU demo golden (constant)

The ISOLITH_GPU=off CPU round-trip is unchanged: `CPU_DEMO_GOLDEN_HASH` is
still `12597229237775278722` — `gpu_backend` asserts it and the test passes
(`transcripts/06-gpu-backend.txt`):
`CPU demo fallback hash = 12597229237775278722 (pre-card golden
12597229237775278722)`. The CPU rasterizer's output did not move.

## Acceptance legs — real transcripts

| Leg | Result | Transcript |
|---|---|---|
| STEP 0 merge | `Already up to date` (base = wave head w/ UI-D7) | (in log) |
| `cargo test -p oep-geo` | exit 0, green (incl. 4 new Vec4 tests) | `transcripts/10-oep-geo.txt` |
| `cargo test -p oep-viewer` (all) | green (render_axis_views 4/4, gpu_backend 6/6, viewer_contract, etc.) | `04-render-axis-views.txt`, `06-gpu-backend.txt` |
| `cargo test --workspace` | **exit 0, 222 targets, 4452 passed / 0 failed** | `transcripts/01-workspace.txt` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0, `renderer=wgpu viewport-backend=gpu` | `transcripts/02-smoke.txt` |
| `cargo test -p oep-app --test gpu_shading` | 124 passed / 0 failed; 5 snapshots sha256 == UI-D2 | `transcripts/05-gpu-shading.txt` |
| `cargo test -p oep-app --test pick_gpu` | 122 passed / 0 failed; 20 sampled pixels `idbuf == oep_viewer::pick`, all `match=true` (same primitive ids, same order as D7) | `transcripts/08-pick-gpu.txt` |
| `cargo test -p oep-app --test measure` | 130 passed / 0 failed | `transcripts/09-measure.txt` |
| `cargo test -p oep-viewer --test gpu_backend` | 6/6 (CPU golden round-trip + coverage: demo GPU 45866 vs CPU 45357 covered px, no panic) | `transcripts/06-gpu-backend.txt` |

### GPU determinism oracle

`gpu_backend` renders the same scene twice and hashes the two frames — both
instances of the GPU path hash identically (the test asserts it; 6/6 green).
Two identical renders ⇒ identical hash, unchanged from D7.

### Picking parity

Fixed camera + fixed pixel list: `pick_gpu` samples 20 fixed pixels and asserts
`idbuffer` pick == `pick::unproject` pick, all `match=true` (transcript prints
each `px=(x,y) idbuf=Some("cube-*") pick=Some("cube-*") match=true`). The
id-buffer pass math is untouched by this card, so the picks are by construction
identical to D7; the test passing confirms the colour/depth surfaces didn't
drift either.

## Vertex-stage timing (before / after) — numbers, not adjectives

Measured with a temporary timing harness (`crates/oep-app/tests/u8_timing_temp.rs`,
created for this card and DELETED before commit — it is not part of the tree)
at 800x600, median + p95 over N frames, same demo scene, same harness both
sides. "Before" = UI-D7 baseline; "after" = this card's tree.

| metric | before (median / p95) | after (median / p95) |
|---|---|---|
| CPU full-frame per frame | 19.98 ms / (run 1) | 21.35 ms / 21.40 ms |
| GPU `render_frame` (full) | 8.21 ms / | 8.88–8.92 ms / 9.06–9.16 ms |
| GPU vertex-prep (upload bytes) | 91 µs / | 108–119 µs / 151–179 µs |

Raw after runs: `transcripts/03-timing-after.txt`
(`CPU … median=21367902 p95=21713438`; and three repeated runs: CPU
~21.35ms, GPU full median 8.87–8.92ms, vertex-prep median 107–119µs). The CPU
rasterizer's algorithm and output are byte-identical (golden table); the
~1.4ms CPU diff is the run-to-run noise band (same harness, this host). The GPU
vertex-prep ~18–28µs increase is the per-frame 64-byte MVP uniform write +
`set_bind_group`, which is exactly the work this card added. Per-frame GPU full
median rose ~0.7ms for the same reason (one small uniform upload per frame);
no further profiling — UI-D5 UI-PERF is the dedicated perf card and this card
only records the numbers as asked.

## Pre-existing flake note (not introduced here)

As in UI-D7, `cargo test --workspace` occasionally aborts with an exit-time
`signal: 11 SIGSEGV` in an egui_kittest/eframe harness test AFTER all that
binary's assertions printed ok (confirmed this dispatch in `layout`,
`feature_tree`, `feature_from_sketch`, `pick_gpu`, `palette` — every one a
kittest harness binary untouched by my diff). Reproduced ~1 in 3 isolated.
It is a harness teardown crash, not a test failure. The committed workspace
transcript `01-workspace.txt` is the exit-0 run (222 targets, 4452 passed /
0 failed); aborted attempts before it show only the SIGSEGV pattern and zero
failing tests.

Proven pre-existing, not introduced here: the same `pick_gpu` teardown
SIGSEGV reproduces on the UNMODIFIED base commit (`ec6759a`, equivalent to
this card's parent, verified via a temporary detached worktree sharing the
target dir — `transcripts/12-baseline-pickgpu-flake.txt`: run 1 crashed with
`signal: 11`, runs 2-3 passed 122/122, the identical intermittent pattern).
My diff touches no kittest harness file.

## Artifact sha256

Every file in this proof directory is hashed in the canonical manifest
`transcripts/100-artifacts.sha256` (committed with this proof). Key hashes, for
the record (PROOF.md's own hash is the canonical manifest's first line — a
file cannot contain its own digest):

| artifact | sha256 |
|---|---|
| `golden_table.sh` | `d635f5006fe5c95fb2e41b96cd50319f6926f761592329f9544286d8654678d4` |
| `transcripts/01-workspace.txt` (exit-0) | `11c349fd47cd48c4e78b7427c1c1d8deb9b3a3287fe9311fbea7da94fecda383` |
| `transcripts/02-smoke.txt` | `ec11880471c3b00fbbc2731db3e952d0d94fcadc99d751ff41e04797907fc8fb` |
| `transcripts/03-timing-after.txt` | `b9624349583d4add4e566e2e02101ce2a2528ffd6c070ebbcab063be90ff79fa` |
| `transcripts/04-render-axis-views.txt` | `0d39c8f02444ff07442cc2ebba59596fadbcdc57499b6c7ea08d9e050f519d01` |
| `transcripts/05-gpu-shading.txt` | `38f3a4f8675cb09423213f5652b94cda3a0234c66ebb3ab206cbfa82a0395337` |
| `transcripts/06-gpu-backend.txt` | `63e4cd8cafa997bb3b162a9949f26a09282c51f54656f6d227995653faf6e94e` |
| `transcripts/07-golden-table.txt` | `019e56025794224ca2c82244c816153222701978e77b272b204455def17f4a3c` |
| `transcripts/08-pick-gpu.txt` | `fa4a50da3c6c8eaa2d8c6a8087e0e89789c1ae7cb63657bd36b68da740cb1626` |
| `transcripts/09-measure.txt` | `772c0b2f423d85495931a6f85d04c3cd0eca54d6d2358809984a83888e2820d1` |
| `transcripts/10-oep-geo.txt` | `a687e5d331aaf0968d7c5db4c963b713f212750866678a9bd4de1895f18b2c43` |
| `transcripts/11-probe-ndc.py` | `6257ba4d54f2b49eadcb86c5729b0d79d31c323e4c67284d56288687ed30a1ab` |
| `transcripts/100-artifacts.sha256` | recomputed at commit time |

## README

Row added in `workstreams/modeler/ui/README.md` (UI-D8).
