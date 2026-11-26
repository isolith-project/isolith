---
title: Isolith Render-Seam Performance Decision (GPU-resident geometry, measured per-stage profile)
decision: gpu_resident_geometry_with_cpu_side_retention_for_the_render_seam
version: 1.0.0
date: 2026-11-25
status: Ratified
owner: Isolith program governance / modeler UI workstream (wave D)
applies_to: oep-viewer GPU backend (crates/oep-viewer/src/gpu, `render_frame` + `assemble`), the interactive seam and its 16.6 ms p95 budget (UI-D5), and the UI-D3 id-buffer / UI-D4 section-plane / UI-D2 transparency surfaces that ride on it
supersedes: nothing (first render-seam performance record; UI-D5 measured the seam's miss, UI-D9 attributes the cost per stage and chooses the fix from evidence)
---

# Isolith Render-Seam Performance Decision

This record answers the wave-D question "why is the interactive seam 26x over
budget and which fix do we build first?" with measured numbers, chosen once so
a wave is not built twice. UI-D5 measured the seam's p95 at 434.94 ms against
the committed 16.6 ms bar (1,002,528 triangles, 1600x1000, the real NVIDIA GB10
Vulkan adapter, driver 580.178.04) and ruled out the projection space (moving
the transform into the vertex stage, UI-D8, moved p95 by 0.8 percent). UI-D9
(this card) profiles the remaining seam stage by stage and records the decision,
with the prior-art brief (workstreams/modeler/ui/proof/UI-D9/prior-art.md, 36
cited sources) as a primary input per Don's 2026-11-25 directive. This card
implements nothing; the chosen direction lands as its own single-change card
after review (one behaviour per card, per Don's doctrine).

## 1. The measured seam

The interactive seam (`GpuBackend::render_frame`, crates/oep-viewer/src/gpu/mod.rs):

- `assemble()` runs every frame and, in the harness's default `Shaded` mode,
  converts 502,681 mesh positions to fresh `f64` `Vec3`s, recomputes smooth
  normals every frame, projects and culls every one of 1,002,528 faces through
  `render::project_ndc` (building one `GpuFace` with two heap-backed `Vec`
  each, about 2M heap allocations per frame), painter-sorts the whole face
  list, and expands it into about 3,007,584 `ShadedVertex` (40 B each with
  per-vertex colour).
- `render_frame` then re-serializes those vertices into a fresh `Vec<u8>`
  (`pack_shaded`, ~120.3 MB) and calls `device.create_buffer_init` for a new
  vertex buffer every frame (the line overlay does the same for its smaller
  list), submits, and readbacks the 1600x1000 target (6.4 MB) through a
  synchronous `device.poll`. The seam is synchronous, so the GPU raster time is
  inside the frame's wall clock.
- A fresh `frame_hash` (FNV-1a over the 6.4 MB frame, the determinism oracle)
  is computed every frame after the readback.

What already exists from earlier cards: a coarse `vertex_prep` timer and
`frame_time` (UI-D8's recording), the id-buffer picking path (UI-D3), the
section-plane CPU clip (UI-D4), and the painter-ordered draw (UI-D2).

## 2. Measured per-stage cost (this card's profile, real numbers)

Method: `perf record` and `perf report` are blocked on this host.
`kernel.perf_event_paranoid = 4` denies unprivileged sampling ("Access to
performance monitoring and observability operations is limited"), there is no
passwordless sudo and no `setcap` on `/usr/bin/perf`. Per the card's sanctioned
fallback, stage timers were added to `render_frame` and `assemble` behind
`ISOLITH_PERF_STAGES=1`, inert when the variable is unset (proven: the fast
smoke is green and the GPU frame hash is identical with the variable unset and
set, section 6). The profile runs the same UI-D5 heavy protocol: the
1,002,528-triangle scene, 1600x1000, 20 warm-up frames, then 120 measured
frames. The clean uninstrumented p95 (leg 3 of scripts/ui/perf-profile.sh) and
the instrumented run's 120 per-frame stage lines (leg 4) are the inputs; the
stage means below are over the 120 measured frames of the instrumented leg.

The numbers below are quoted from workstreams/modeler/ui/proof/UI-D9/profile.json
(this host, the committed run) and every stage sums to the frame total:
`assemble` equals its six sub-buckets (positions, normals, project+cull, sort,
expand, other), and the four seam stages (assemble + vertex-prep + submit +
readback+w) sum to `frame_time` within 0.004 ms.

Per-frame stage table (means over the 120 measured frames; one concrete median
frame's raw stage line is quoted after the table):

| Stage | mean ms | share of frame | what it is |
|---|---|---|---|
| assemble | 293.34 | 75.6 percent | CPU re-derivation and teardown (all CPU derivation below) |
| - assemble.positions | 0.31 |  | fresh f64 Vec3 copy of 502,681 mesh positions |
| - assemble.normals | 3.37 |  | smooth normals recomputed every frame |
| - assemble.project+cull | 85.51 |  | per-face projection/cull, 2 heap allocs per face |
| - assemble.sort | 50.14 |  | full-list f64 painter sort by view depth |
| - assemble.expand | 98.65 |  | build 3,007,584 ShadedVertex at 40 B |
| - assemble.other | 55.37 |  | unmetered residual of assemble (teardown of the intermediate face list plus misc) |
| vertex-prep | 92.49 | 23.8 percent | pack_shaded (120.3 MB) + lines pack + create_buffer_init x2 + draw record |
| submit | 0.07 | 0.0 percent | queue.submit wall time |
| readback+w | 2.26 | 0.6 percent | copy to buffer + map + synchronous device.poll (owns the GPU raster wait) |
| hash (post-frame) | 4.95 |  | FNV-1a over the 6.4 MB frame, AFTER frame_time is captured (outside the p95 bucket) |

Concrete median measured frame (raw stage line, frame 36):

```
ISOLITH_PERF_STAGES frame=36 total=387.326ms assemble.positions=0.299ms assemble.normals=3.338ms assemble.project+cull=85.472ms assemble.sort=52.246ms assemble.expand=97.693ms assemble=294.777ms assemble.other=55.730ms vertex-prep=90.213ms submit=0.069ms readback+w=2.263ms hash=4.948ms
```

The stage sum versus the measured p95: the four seam stages sum to 388.17 ms
(frame mean 388.17 ms, sum-to-mean gap 0.004 ms) against a clean p95 of
424.81 ms and an instrumented p95 of 408.97 ms on the same host and adapter.
The clean leg-3 runs this session varied 404.4 to 434.5 ms p95 (a few percent
run-to-run on the shared integrated GPU; the committed UI-D5 reference,
434.94 ms, sits at the top of that band). The gap from the stage sum to the
clean p95 is 36.65 ms: the p95 tail, the untimed bookkeeping (shading::state,
encoder creation, the gap between assemble's return and vertex-prep, begin-frame
work, the per-frame print), and inter-leg variance; the instrumented-to-stagesum
gap is 20.8 ms. The `hash` stage (~4.95 ms) is post-frame oracle work and is
deliberately excluded from the seam bucket because `frame_time` is captured
before it; end-to-end (seam + hash) is 393.12 ms.

Attribution conclusion: the CPU re-derivation (`assemble`) is the seam at
75.6 percent of the frame; within it the profile separates (a) building the
per-face list with per-face heap allocations and its teardown (project+cull
plus other, about 141 ms), (b) the painter sort (50.1 ms), (c) the expand into
3M shaded vertices (98.7 ms), and (d) the 120 MB re-serialization and
fresh-buffer upload (vertex-prep, 92.5 ms). The GPU-side cost (submit +
synchronous wait + readback) is under 3 ms, a rounding error at this scene
size.

## 3. The stage costs each option removes (evidence-based)

From the profile (section 2), the per-frame costs are:

- 120 MB re-upload artifact: `vertex-prep` 92.49 ms (pack + buffer create) plus
  the 98.65 ms expand that exists only to serialize 3M vertices for that
  upload. Together about 191 ms of the frame is an artifact of not keeping
  geometry on the GPU.
- Per-face CPU derivation and teardown: project+cull 85.51 ms with about 1M
  times 2 heap allocations, and assemble.other 55.37 ms (mostly the drop of the
  intermediate face list, 1M GpuFace with two heap Vectors each).
- Painter sort: 50.14 ms for the full 1M-face list.
- Normals and positions: 3.37 + 0.31 ms for the every-frame smooth normals and
  f64 position copy.
- Synchronous readback + GPU wait: 2.26 ms (includes the GPU raster wait; tiny
  at this fragment load).
- Determinism oracle: hash 4.95 ms post-frame (not in the seam bucket, but a
  third of the 16.6 ms target, so it gets its own line below).

## 4. Options

Every option states: the measured stage cost it removes (section 2), the ms it
must beat (p95 <= 16.6 ms at this scene, the committed UI-D5 gate), and which
of the seam's invariants it puts at risk. Invariants: I1 the UI-D3 id-buffer
picking path reuses the drawn face set and painter order; I2 the UI-D4 section
planes clip in CPU NDC space; I3 the CPU-path determinism goldens
(CPU_DEMO_GOLDEN_HASH = 12597229237775278722) must not move; I4 egui shares the
device and the queue; I5 transparency depends on back-to-front painter order.
The wgpu default limits that bound any design (Limits::default(), which
device.rs enforces): max_buffer_size 256 MiB, max_storage_buffer_binding_size
128 MiB, max_uniform_buffer_binding_size 64 KiB, max_vertex_buffers 8 [S22].

### Option A: GPU-resident geometry, retained per primitive, CPU side retained (the brief's Stage 1 + 2)

What it does (techniques: retained/display-cache rendering as in OCCT AIS
Graphic3d_Structure [S16][S17] and Coin3D render caches [S11][S12]; bevy's
retained RenderMeshInstance extraction [S19][S20]; the zeux efficient-Vulkan
rule of device-local buffers for static geometry [S2]): upload each mesh once
into a device-local vertex/index (or plain triangle) buffer keyed by mesh
identity; every frame builds a small index-range or instance list that
references the resident buffers instead of re-serializing vertices. Re-upload
only the dirty primitive's range when its mesh bytes, transform, highlight or
colour change. The whole painter-sorted face list still derives on the CPU
(same order, same projection, no pixel change). On top, the brief's Stage 2:
per-primitive world-AABB frustum cull before assembling faces (cheap, per part,
the OCCT BVH-cull essence at primitive level [S16][S8]) and a reused face arena
instead of 1M fresh allocations per frame, so a camera move's work is
O(visible) and allocation-free. This is the brief's Stage 1 + Stage 2, ADOPT.

Cost it removes (measured, section 2): the expand + vertex-prep upload artifact
entirely (98.65 + 92.49 = about 191 ms); normals 3.37 ms and positions 0.31 ms
become dirty-only (near zero at steady state); most of the 1M allocation share
of project+cull and its teardown; the sort 50.14 ms stays (painter kept). A
realistic immediate removal of about 191 ms means the frame drops from about
425 ms toward about 235 ms before culling savings, at zero pixel risk.

Must beat: p95 <= 16.6 ms at this scene. Keeping the CPU painter sort (3a),
the floor at 1,002,528 all-visible triangles is the survivors' projection plus
sort plus readback. Against the measured costs this is NOT reached at the full
visible 1M scene (honest floor and the scene size that does meet the bar are in
section 5).

Invariants at risk: none of I1 to I5. Painter order, drawn face set, seam
projection, transparency ordering and the id-buffer mapping are byte-identical;
CpuBackend is untouched. I4 (shared device/queue) is unchanged, the design uses
the same queue with persistent buffers.

### Option B: Option A plus a depth buffer replacing the CPU painter sort (the brief's Stage 3b)

What it does (techniques: the standard depth-buffer opaque pass plus a sorted
transparent pass, LearnOpenGL blending [S27] and three.js [S26][S29]; reverse-Z
plus float depth for CAD near/far ranges [S30][S31]): draw the opaque faces in
any order with depth test and write on (per-pixel ordering, strictly more
correct than mean-depth painter for self-intersecting or concave faces); the
transparent set (alpha < 1, driven by the global slider) keeps an entity-level
back-to-front sort with depth test on and depth write off. The full 1M-face
painter sort (50.14 ms) becomes an opaque GPU-side O(n) issue plus a tiny
transparent-table sort; the CPU per-face sort disappears.

Cost it removes: the sort 50.14 ms and all CPU ordering work; combined with
Option A it is the only in-scope route to a sub-16.6 ms floor at the full 1M
visible scene (section 5). The depth attachment (D32Float reverse-Z, 1600x1000
= 6.4 MB) is trivial and the id pass attaches the same depth buffer, so the
topmost id equals the colour pass's topmost fragment.

Invariants at risk: I5 (transparency: the committed 50%-alpha behaviour must
survive the sorted-transparent pass with depthWrite off; this is the brief's
stage-gating note), I1 (id-buffer topmost parity must be re-verified at the
packed resolution under reverse-Z; the same face set is drawn but ordering is
now depth-resolved), I2 (section planes still clip in CPU NDC space, unchanged,
but the GPU surface feeds are now depth-resolved so the CPU/GPU coverage band
must be re-measured), and a documented GPU regolden: the GPU coverage numbers
are a band, within 5 percent of the CPU path and deliberately not byte-equal
[S33], so this moves them legitimately and the coverage + id-parity tests must
be re-run and re-recorded. I3 (CPU determinism goldens) and I4 (shared
device/queue) are untouched.

### Option C: keep the every-frame CPU derivation, derive less often only (reuse, no GPU retention)

What it does (techniques: retained state with dirty-range update [S2][S20][S23]
and the OCCT redraw-after-all-updates discipline [S5][S7]): keep the existing
per-frame create_buffer_init upload but cache f64 positions and smooth normals
per mesh (recompute only on mesh or transform change) and reuse a preallocated
face arena so the 1M allocations and their teardown disappear. CPU faces are
still projected, sorted and expanded every frame and the 120 MB still
re-uploads every frame.

Cost it removes: positions 0.31 + normals 3.37 ms and most of project+cull's
allocation share + the 55.37 ms teardown. It does NOT remove the 120 MB expand
+ upload (about 191 ms) nor the sort 50.14 ms.

Must beat: p95 <= 16.6 ms at this scene. It does not: at 1M all-visible
triangles the remaining project-on-all-faces + sort + 120 MB upload sum well
above 16.6 ms from the measured numbers. It is a partial design that keeps the
upload-every-frame cost, and it is REFUTED as the headline (the brief's Stage 1
is strictly better and cheaper to land); it survives only inside Option A's
dirty tracking.

Invariants at risk: none (I1 to I5 preserved, byte-identical pixels). That is
exactly why it is attractive and exactly why it is insufficient.

### Option D: do nothing (the floor)

Keep the seam exactly as profiled. Measured floor: p95 424.81 ms at 1,002,528
triangles / 1600x1000 on this host (this committed run; the UI-D5 committed
reference is 434.94 ms, the top of the session's 404.4 to 434.5 ms band), about
26x the 16.6 ms bar. The seam meets the budget only as the scene scales down:
at approximately 1,002,528 x (16.6 / 424.81) = about 39,000 visible triangles
at this frame-cost profile (roughly linear: same driver, 120 MB/frame upload,
full-derive CPU path). No change means the wave-D interactive budget stays
missed at 1M triangles with nothing implemented.

Invariants at risk: none (nothing changes); the entire cost stays on the
critical path.

### Option E: full GPU-driven pipeline (indirect draws, compute culling, meshlets)

What it does (techniques: Haar and Aaltonen GPU-driven rendering [S1],
vkguide compute culling with indirect draws [S8], AMD AGS MDI [S9], mesh
shaders [S3]): move culling and draw generation onto the GPU, store instances
and transforms in buffers, batch via indirect or draw-indexed-indirect. At 1M
flat-shaded CAD triangles this is the over-engineering the prior-art brief
calls out and rejects for this scale (object-level culling + retained geometry
captures the win [S8][S6], and wgpu mesh-shader support is new). DEFER it to a
future 10M to 100M-triangle wave.

## 5. Recommendation

Land Option A (GPU-resident geometry + CPU-side retention), then decide 3a
versus 3b with the post-A numbers. The profile's evidence: assemble is 75.6
percent of the frame, and the 120 MB re-upload artifact (expand + vertex-prep,
about 191 ms) is the single biggest removal available with a zero-pixel-risk
change (I1 to I5 all preserved, CpuBackend byte-untouched). The prior-art
brief's stage order (retain geometry, then cull + dirty tracking, then depth
only after goldens re-verify) is ADOPTED wholesale, and its what-NOT-to-build
list (meshlets, order-independent transparency, Hi-Z occlusion, a Coin3D-grade
scene graph) is ADOPTED too.

The honest budget line, stated plainly: no CPU-side option reaches 16.6 ms at
1,002,528 all-visible triangles on this host. The measured floor after Option
A's cheap part is project+cull(survivors) + painter sort + readback, which
stays above the bar at the full visible 1M scene. The painter-preserving
A -> 3a target is reached at roughly 1,002,528 x (16.6 / 137.9) = about 121,000
visible triangles (arena project+cull + sort + readback at 16.6 ms, derived
from the measured stage costs). Meeting the committed 16.6 ms at the full 1M
visible scene requires Option B (the depth buffer removes the sort), which
changes the GPU output contract and needs the documented I1/I2/I5
re-verification plus a GPU regolden. So this record directs: land Option A
first (no pixel risk), and cut 3b as its own card once A's post-upload numbers
are measured, exactly as the brief recommends (land 1 then 2 then 3a first;
3b is its own shippable card because it changes the GPU output contract).

The single fact that picks A over C: A removes the about-191 ms upload artifact
that C cannot touch, at equal invariant risk (none). The fact that picks
A-then-3b over A-only is the floor arithmetic above.

## 6. What was measured and what could not be, on this host

Measured (quoted in section 2 and profile.json): the four seam stages and
assemble's six sub-buckets over 120 measured frames at 1,002,528 triangles and
1600x1000 on the real NVIDIA GB10 (Vulkan, driver 580.178.04); the clean
uninstrumented p95 (same protocol as UI-D5); and both determinism hashes. The
env-gated timers are provably inert: fast smoke green with the variable unset
and with it set, and the GPU frame hash identical both ways (9287941873092625150,
"gpu structural echo: frame_hash x -> x, identical = true" in transcripts
01-fast-smoke-unset.txt and 02-fast-smoke-set.txt); CPU determinism unchanged
(12597229237775278722 == committed golden, printed equal).

Could not be measured on this host: perf record/perf report CPU samples
(denied by kernel.perf_event_paranoid = 4, no passwordless sudo, no perf
capabilities; recorded in profile.json). The per-stage split is therefore
wall-clock, not sample-based. The pure GPU-raster wall time is not isolated: it
sits inside readback+w's synchronous device.poll, and it is small at this
fragment load. assemble.other (55.37 ms) is measured as part of assemble per
section 3 and is mostly the teardown of the intermediate face list; anything
else in it is per-primitive colour lookup, the line/grid/triad assembly and Vec
bookkeeping, and is not separately split on this host. Frame-to-frame and
run-to-run variance is a few percent on the shared integrated GPU (clean leg-3
p95 measured 404.4, 407.9, 434.5 and 424.8 ms this session; the committed UI-D5
reference, 434.94 ms, is the top of the band).

## 7. The prior-art brief: ADOPT or REFUTE, stage by stage

For each of the brief's three-stage plan and its don't-build list, this record
states the verdict explicitly, as the orchestrator's directive requires:

- Stage 1, resident GPU geometry with no per-frame re-upload: ADOPT. Retained
  geometry with static per-primitive vertex buffers and re-upload only on scene
  change is Option A; it removes the measured ~191 ms upload artifact at zero
  pixel risk [S2][S16][S17][S19][S20].
- Stage 2, per-primitive AABB frustum cull + mesh-version dirty tracking +
  allocation reuse: ADOPT. Primitive-level (not per-triangle) culling is the
  OCCT BVH-cull essence [S16][S8] and is cheap per part; dirty tracking keys on
  MeshData equality (PartialEq already derived). One honest limitation recorded
  in advance: in a fully framed view of the benchmark 1M grid, about zero
  primitives are culled. The cull is for real assemblies, not for the
  profile-only bench scene.
- Stage 3a, keep the CPU painter sort: ADOPT as the first-segment default. The
  50.14 ms sort stays until 3b's depth decision is carded; this preserves I1/I5
  and the byte-parity story.
- Stage 3b, reverse-Z D32Float depth buffer, opaque unordered, transparents
  sorted back-to-front with depthWrite off: DEFER as its own card. It is the
  only in-scope route to <= 16.6 ms at the full 1M visible scene (section 5),
  so it is not refuted on merit; it is sequenced after A because it changes the
  GPU output contract and needs I1/I2/I5 re-verification plus a GPU regolden
  [S27][S26][S30].
- Don't-build list (meshlets or task shaders, per-pixel order-independent
  transparency, Hi-Z occlusion, deferred, a Coin3D-grade recursive scene graph,
  ray-traced picking): ADOPT the rejection. At 1M flat-shaded CAD instances,
  object-level culling + merging captures the win [S8][S6][S1]; OIT is
  significantly more expensive [S8]; meshlets are the 10M to 100M answer [S3];
  Isolith's assembly is a flat list of leaves, so a scene graph would be
  re-introducing what FreeCAD and Coin are shedding [S13][S14].
- wgpu limits that bound the design [S22]: ADOPT as constraints. max_buffer_size
  256 MiB (the ~36 to 72 MB resident geometry fits one buffer),
  max_storage_buffer_binding_size 128 MiB (no single storage binding over that),
  max_uniform_buffer_binding_size 64 KiB (the 64 B MVP is far below),
  max_vertex_buffers 8 (the seam uses 1 to 2). A naive one-giant full-attribute
  storage buffer over 128 MiB would trip the cap; the brief's memory plan stays
  inside it [S2][S22].

No source in the brief contradicts the recommendation; the only tension is the
sequencing of 3b, which the brief itself resolves in favour of deferral.

## 8. Invariant contract (must stay green through the chosen change)

- CPU-path determinism goldens: CpuBackend is literally untouched;
  ISOLITH_GPU=off renders CPU_DEMO_GOLDEN_HASH = 12597229237775278722 unchanged;
  the 21 sha256'd UI-D7 proof frames and the 5 UI-D2 snapshots stay
  byte-identical (Option A). (I3)
- GPU determinism oracle: identical (scene, camera, size) renders hash
  identically. (I4)
- Id-buffer parity: pick_at and pick_rect equal analytic oep_viewer::pick;
  under Option B, re-verified at 1600x1000 under reverse-Z. (I1)
- Transparency: the 50%-alpha readback stays near alpha = 128 with rgb a true
  source-over composite. (I5)
- Section planes clip in CPU NDC space, unchanged. (I2)
- The harness's fast variant stays green and scripts/ui/perf-profile.sh stays
  reproducible (exit 0, writes proof/UI-D9/profile.json).
- wgpu limits: the resident plan stays inside Limits::default().

## 9. Cross-references

- UI-D5 (t_c24331a1): measured the miss and refuses to lower the bar.
- UI-D8: ruled projection space out (0.8 percent); this record's starting point.
- Prior-art brief: workstreams/modeler/ui/proof/UI-D9/prior-art.md.
- Next card to cut: "Stage 1: GPU-resident geometry with CPU-side retention"
  (Option A); then decide 3b on post-A numbers. The orchestrator cuts it; this
  card creates no cards by design.
- Don's doctrine (r1a-trl-evidence): one change at a time, decisions from
  measured evidence, no hedging in the record.

## Change log

- 1.0.0 (2026-11-25): first record. Per-stage profile on the real NVIDIA GB10
  at 1,002,528 triangles and 1600x1000 (clean p95 424.81 ms this committed run;
  UI-D5 reference 434.94 ms); assemble = 75.6 percent of the frame, the 120 MB
  upload artifact about 191 ms. Recommendation = Option A (GPU-resident
  geometry + CPU-side retention); 3b deferred as its own change; prior-art
  brief ADOPT/REFUTE recorded; the do-nothing floor (424.81 ms, about 26x the
  bar) and the budget-meeting scene sizes (~39k as-is, ~121k after Option A
  keeping the CPU sort) stated from the measured numbers.
