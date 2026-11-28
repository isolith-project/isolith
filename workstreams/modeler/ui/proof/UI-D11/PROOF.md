# UI-D11 UI-DEPTH-SORT: reverse-Z depth buffer replacing the CPU painter sort (ADR Option B, Stage 3b)

Wave F boundary, render-correctness fix. Branch `ui/d11-depth-sort`, based on
`github/wave/ui-f` (head `8d263a5`, F1..F4 merged; wave D and E closed on
main at `a68505d`). Every commit DCO-signed by the commit-msg hook
(`Signed-off-by: Don Allen <don@redonealpha.com>`). Raw transcripts under
`transcripts/` (the six gate legs and the perf harness both tee their real
output there).

## STEP 0

`git fetch github && git merge github/wave/ui-f` -> `Already up to date.` The
wave-F head `8d263a5` was already in this branch's ancestry (the resume
snapshot `4c213c0` was cut on top of it after the wave-D merge `a8f3ae0`), so
the merge brought nothing. Nothing extra was merged; the card landed on top of
the committed WIP snapshot exactly as the resume addendum directed.

## What this card is

A correctness fix with measured proof, not a perf card. UI-D10 already met the
16.6 ms bar at p95 6.366 ms with the painter sort retained. The mean-depth
painter sort is wrong on real CAD geometry (interpenetrating and concave
parts), and this card replaces it with per-pixel hidden-surface resolution: a
reverse-Z D32Float depth attachment, adopting the prior-art brief's settled
answer (opaque first with the depth buffer ordering them pixel-exactly, then a
back-to-front entity-sorted transparent set with depth test on and depth write
off, `proof/UI-D9/prior-art.md` sections 1.3, 1.4, 1.5, 2.3 3b).

## Stage 1: prove the defect (committed, 28f1502)

Constructed scenes (fixed coordinates, no RNG, no corpus) rendered through the
EXISTING pre-fix pipeline (GpuBackend colour frame + IdPass, both painter
sorted) and compared per pixel against an independent analytic camera-ray /
nearest-triangle oracle in f64, with a disjoint-box control validating the
oracle. Evidence and images under `stage1/`:

- interpenetrating walls, yaw=1.90 pitch=0.55: wrong-surface px 18003 / 55192
  compared (32.62%)
- concave L crossing a wall, yaw=0.60 pitch=0.30: wrong-surface px 8189 /
  35443 (23.10%)
- control, three disjoint boxes: wrong-surface px 384 / 39845 (0.96%)

Verdict quoted in the Stage-1 commit and `stage1/numbers.txt`:
MISORDER CONFIRMED. The mean-depth painter paints the wrong surface on roughly
a third of the pixels where two parts interpenetrate, and on the concave L
face where its near and far ends fall on opposite sides of the crossing wall.
Interpenetration and concavity are ordinary CAD geometry (wave E shipped
assemblies), so this is a visible defect in the shipped renderer, not a corner
case.

## Stage 2: the fix

### Depth attachment (reverse-Z D32Float)

`crates/oep-viewer/src/gpu/target.rs` grows a `Depth32Float` reverse-Z depth
texture (1600x1000, render-attachment only, never read back) with the card's
semantics: near plane maps to depth 1.0, far to 0.0, compare GreaterEqual,
clear 0.0 (far). The vertex stage (`shaders/shading.wgsl`) negates the NDC z
to reverse the range; x and y are untouched, so raster coverage is bit-for-bit
the forward-Z position and the CPU/GPU coverage band is unchanged (re-measured
below). The id pass (`gpu/idbuffer.rs`) attaches the same D32Float format and
GreaterEqual semantics with its own vertices carrying their NDC z, so the
topmost id equals the topmost colour fragment (I1).

### Pass structure

The packed face stream is split into an opaque prefix and a transparent
suffix (`gpu/resident.rs` `face_opaque_bytes`, part of the same frame key as
the bytes, so the resident fast path reuses the split verbatim). The opaque
set draws first with depth test on and depth write on, in ASSEMBLY order: the
CPU painter sort is gone from the GPU path. The transparent set (vertex alpha
below 255, i.e. the whole scene while the global slider is below 1.0) draws
after, with depth test on and depth write off, entity-sorted back to front
(per primitive, mean depth per entity, stable). The line overlay keeps depth
compare Always and write off, so it stays on top exactly like the pre-depth
pipeline. `assemble.sort` now measures only the transparent entity sort (0 for
an all-opaque frame).

### One convergence this run made

The committed WIP used Greater on the colour pass and GreaterEqual on the id
pass. On an exact-equal tie (coplanar coincident faces) the colour pass then
let the first-drawn fragment win while the id pass let the last-drawn win, so
the topmost id could disagree with the topmost colour fragment. Both passes
now use GreaterEqual (the card's spec: compare GreaterEqual), which also
matches the wgsl/target/gpu_shading documentation and lets the later-drawn
fragment win an equal tie exactly as the pre-depth painter let the last draw
overwrite.

## Invariants (all quoted real output; full transcripts under transcripts/)

- I1 id-buffer parity: `cargo test -p oep-app --test pick_gpu` -> 170 passed /
  0 failed; `id_buffer_pick_equals_analytic_ray_pick_for_20_sampled_points`,
  hover, multi-select, box-select and empty-click all green under reverse-Z.
- I3 CPU determinism: `CPU demo fallback hash = 12597229237775278722 (pre-card
  golden 12597229237775278722)` in gpu_backend, match=true (unchanged).
- I4 shared device and queue: untouched (device.rs not modified).
- I5 transparency: gpu_shading prints `overlap-region mean readback alpha:
  opaque = 255.0, 50% transparent = 146.7` and the assertion is green
  (`half < opaque - 60` and `|half - 128| < 40`): a true source-over
  composite with the source-alpha readback intact. The z-fight scan counts 0
  alternating face-colour runs on the committed snapshot.

## The constructed-scene regression gate stays

`crates/oep-viewer/tests/depth_sort_defect.rs` now runs the SAME constructed
scenes and 8 fixed poses against the fixed pipeline and asserts each appears at
or below the control silhouette band (< 2% of compared pixels), so a future
change that reintroduces CPU mean-depth ordering fails loudly. Real output on
this tree:

```
[interpenetrating-walls] chosen pose yaw=2.40 pitch=0.35: wrong-surface px = 91/53205 compared (0.17%)
[concave-l-cross]        chosen pose yaw=0.35 pitch=0.25: wrong-surface px =  0/33801 compared (0.00%)
[control-disjoint-boxes] chosen pose yaw=0.35 pitch=0.25: wrong-surface px =  0/38159 compared (0.00%)
```

91 wrong pixels out of 53205 compared on the worst interpenetrating pose sits
at 0.17%, inside the same thin silhouette band as the always-correct control.
Stage-1's numbers (32.62% / 23.10%, control 0.96%) remain the documented
before picture under `stage1/`. Per-pose numbers are in
`stage2/numbers.txt`, regenerated on this tree.

## Section-plane coverage band re-measure (render_axis_views)

Section planes still clip in CPU NDC space; the CPU/GPU coverage relation is a
band within a few percent by design, never byte equality. Re-measured on this
tree (400x300, six standard views + iso, all views > 0 on both backends; the
drift column is |gpu-cpu|/cpu):

| view | gpu_px | cpu_px | drift |
|---|---|---|---|
| Front | 22487 | 22827 | 0.0149 |
| Back | 22520 | 22810 | 0.0127 |
| Left | 5576 | 5745 | 0.0294 |
| Right | 5635 | 5471 | 0.0291 |
| Top | 7853 | 7817 | 0.0046 |
| Bottom | 7834 | 7813 | 0.0027 |
| Iso | 22161 | 21628 | 0.0241 |

All within the documented band; `cargo test -p oep-viewer --test
render_axis_views` -> 4 passed / 0 failed. The gpu_backend coverage band at
800x600 is unchanged: demo GPU 45866 / CPU 45357, bracket GPU 53534 / CPU
52783, both within 5% (`gpu_geometry_coverage_within_5pct_of_cpu` green,
6/6). The only golden the corrected clip-mapping touches is the launch-frame
golden (below).

## Regolden: every superseded frame preserved, diffs counted

Per-pixel depth ordering legitimately changes pixels versus a mean-depth
painter: wherever two faces overlap in screen space and the painter's mean-
depth key picks a non-nearest surface, the depth buffer now draws the nearest.
That is the whole point of the card, so the committed GPU goldens that show a
concave or interpenetrating surface moved. No golden was silently refreshed;
every superseded original is preserved byte-for-byte under `superseded/`
(recovered from the pre-D11 wave base `github/wave/ui-f` after the resume
snapshot had mistakenly written the new pixels over the old files; sidecar
sha256 files verify every preserved original), and every moved frame has its
pixel-diff count below.

Why per-pixel ordering is the more correct answer, in one line per frame:
each pixel now shows the analytically-nearest surface (validated in Stage 1
against an independent f64 camera-ray oracle, which the disjoint-box control
proved); the painter's per-face mean depth has no such guarantee and was
measured wrong on 23-33% of overlapping pixels. The CPU backend, its
determinism golden and the CPU proof frames are untouched, so the painter
pictures remain visible and comparable in the CPU columns of UI-D7.

| frame | moved px | % of frame | superseded sha256 (verified sidecar) |
|---|---|---|---|
| UI-D2 flat | 50026 / 480000 | 10.42 | cc69bd61be43c6020d7a4f87157111a877ee69ce145a1899f758f83b0b5e5daa |
| UI-D2 shaded | 53864 / 480000 | 11.22 | 68b991a446677afdc0dcd03344328d86bc313ed7864c2fd61fdda464451efe4d |
| UI-D2 smooth | 55795 / 480000 | 11.62 | bdc83f188647f7245670094e7e4d250192b2d3e30c41bf794f769b62473e972e |
| UI-D2 transparent-50 | 38776 / 480000 | 8.08 | 532c564261d71d94262b50859709415bc590b19b71fc00233c0fe7dd5d97fe05 |
| UI-D7 Front gpu | 22065 / 120000 | 18.39 | 90750d3fb61497589f06089de39b6610095b8c96e1c717ed009562b0f27e9898 |
| UI-D7 Left gpu | 4636 / 120000 | 3.86 | 262389ff0ce9650d4a801ac7eeb3c765b6bb9980c3e2fe9d69a2315bdcec8568 |
| UI-D7 Right gpu | 4829 / 120000 | 4.02 | e1eb01e8d4791d3cf8fa4e5487075756e5c52969f29a3936dfd5d812efc38d0d |
| UI-D7 Top gpu | 6546 / 120000 | 5.46 | 8ad31153c612d3a5257b594482d4ed22cceaa86629fef87665d4a5788f317f30 |
| UI-D7 Bottom gpu | 6680 / 120000 | 5.57 | 0e9f1b972f79576b08d46ce68aa7dee62eaeae62f035014d6f36e113e4b5cc78 |
| UI-D7 Iso gpu | 16098 / 120000 | 13.42 | 3164ea28d7571e83a2bc79deee5f8b593756ab2d939046a87bcb8a5f034bb054 |
| ui-a2 launch frame | 122784 / 1024000 | 11.99 | 6f81c1c5573b07a95340815bf5f094dd1838d94f8de22ed9a0134a1cada6da84 |

UI-D2 `ui-d2-edges` and UI-D7 `Back` and `all-culled` re-render byte-identical
under the depth pipeline (no depth-induced pixel change at those pixels: the
edges frame is lines only, and the Back/all-culled projections resolve to the
same surfaces the painter drew), so no discovery there. The two small-diff
views (Left 3.9%, Right 4.0%) are limb views where the bracket is near edge-on
and the corrected interior is a thin screen band; Front/Iso expose the bore
and ledges, so more of the image moves. All differ by surface selection only:
coverage (non-background pixel count) is unchanged (the D11 pipeline draws
the same polygons at the same x/y; gpu_backend coverage is byte-identical to
D10), and the colour change is the lit colour of the now-correctly-front face
versus the painter's wrong face (Stage 1 proved the painter's selection was
the error, and Stage 2 validates the new selection against the f64 oracle).

The launch-frame golden: `scripts/ui/shot.sh` reports deterministic byte drift
under the new pipeline (`current 7fcf893f... vs golden 6f81c1c5...`, two
independent captures identical), which is legitimate: the launch viewport
renders the bracket fixture, whose concave interior now draws depth-correct.
Old golden preserved as `superseded/ui-a2-golden-before-ui-d11.png`
(6f81c1c5573b07a95340815bf5f094dd1838d94f8de22ed9a0134a1cada6da84), refreshed
through shot.sh, and shot.sh is green on the refreshed golden (transcript
`transcripts/05-shot.txt`).

## Perf: budget met with the depth attachment in

`bash scripts/ui/perf.sh` exit 0, all three legs green. Real numbers from the
committed report `proof/UI-D5/perf.json` on the real NVIDIA GB10
(Vulkan, driver 580.178.04):

- p95 6.491841 ms (bar 16.6 ms), p50 6.323697, p99 6.530321, max 6.562096
- min fps 152.4, mean fps 158.3; 120 measured frames after 20 warm-up
- 1,002,528 triangles at 1600x1000; the 6.4 MB D32Float attachment adds about
  0.08 ms to the D10 p95 (6.408 ms) and stays far under the bar.
- CPU determinism oracle unchanged: cpu_demo_frame_hash ==
  cpu_demo_golden_hash == 12597229237775278722.

## Full gate (all legs quoted in transcripts/)

- fmt: `cargo fmt --check` exit 0 (`transcripts/01-fmt.txt`)
- clippy: `cargo clippy --workspace --all-targets --keep-going -- -D warnings`
  exit 0 (`transcripts/02-clippy.txt`)
- workspace: `cargo test --workspace` exit 0, 235 test-result blocks / 7229
  passed / 0 failed (`transcripts/03-workspace.txt`)
- viewer GPU legs: gpu_backend 6/6, resident_scene 6/6, depth_sort_defect 1/1,
  render_axis_views 4/4 (`transcripts/04-viewer-gpu.txt`)
- app GPU legs: gpu_shading 172/0, pick_gpu 170/0 (`transcripts/04-viewer-gpu.txt`)
- smoke: `xvfb-run -a cargo run -p oep-app -- --smoke` -> `frames=8 smoke ok`
  exit 0 (perf.sh leg 3; re-verified in `transcripts/04-viewer-gpu.txt`)
- shot.sh: green on the refreshed launch golden (`transcripts/05-shot.txt`)
- perf.sh: exit 0, all three legs, budget met (`transcripts/06-perf.txt` and
  `proof/UI-D5/perf.json`)
- em-dash text guard: the card-owned viewer GPU sources/tests and oep-app GPU
  tests contain zero U+2014 (scrubbed in the working commits; repo-wide legacy
  em dashes in untouched crates are grandfathered and out of this card's
  surface)

## Files changed (this card's surface)

`crates/oep-viewer/src/gpu/{mod,target,idbuffer,resident}.rs`,
`crates/oep-viewer/src/gpu/shaders/shading.wgsl`,
`crates/oep-viewer/tests/depth_sort_defect.rs`,
`crates/oep-app/tests/{gpu_shading,pick_gpu}.rs` (em-dash scrub on pick_gpu only),
`workstreams/modeler/ui/proof/UI-D11/**` (stage1, stage2, superseded, this proof,
transcripts), `workstreams/modeler/ui/proof/UI-D2/ui-d2-{flat,shaded,smooth,transparent-50}.png`,
`workstreams/modeler/ui/proof/UI-D7/ui-d7-{Top,Bottom,Front,Right,Left,Iso}-gpu-400x300.png`,
`workstreams/modeler/ui/proof/ui-a2/{frame.png,frame.png.sha256}`,
`workstreams/modeler/ui/README.md` (UI-D11 row),
`docs/status/waves/wave-ui-d.md` (correct the deferred line), and the wave-F git
history. No `crates/oep-app/src/**` change was required. No test was weakened,
skipped or deleted; the depth_sort_defect regression is new and permanent.

## Landing

Per the card: pushed to `github`, merged no-ff onto `github/wave/ui-f` from
`merge-UI-D11`, remote read-back verified. Recorded in `transcripts/07-landing.txt`.
