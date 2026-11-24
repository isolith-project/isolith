# UI-D3 UI-PICK: id-buffer picking, hover pre-highlight, box select

Selection comes from what is actually drawn: an id buffer rasterized on the
GPU and read back, with hover pre-highlight inside one frame, shift-click
multi-select, and box select returning exactly the enclosed set.

## STEP 0 (run first)

`git fetch github` then `git merge github/wave/ui-d` brought the UI-D1 wave
head onto this card's branch (`43ebe18`, `82d9302`), so the base is the wave
head `ca9aa1b`; transcript `transcripts/00-step0-merge.txt` shows the branch
is now at the wave head (re-merge says "Already up to date"). No rebase, no
reset.

## What shipped

- `crates/oep-viewer/src/gpu/idbuffer.rs` (new): the id pass. Rasterizes the
  same face set (box proxies via `box_faces`, meshes via `triangles`) in the
  same back-to-front painter order into an offscreen `R32Uint` target where
  each pixel is `primitive_index + 1` (`0` = background), reads the grid back
  once per view change, and answers [`IdPass::pick_at`] (pixel -> primitive
  index) and [`IdPass::pick_rect`] (normalized, clamped pixel rect -> the
  exact enclosed index set, sorted, deduped).
- `crates/oep-viewer/src/gpu/mod.rs` (ADDITIVE): `pub mod idbuffer`.
- `crates/oep-app/src/panels/pick_gpu.rs` (new): the pick surface - hover
  pre-highlight (same-frame id lookup), shift+click accumulate multi-select,
  Alt+left-drag box select with rubber band, plain click select-or-clear
  (empty background clears and picks nothing). Publishes a `String` oracle
  (`pre=`, `sel=`, `box=`, `pick=`) every frame.
- `crates/oep-app/src/panels/viewport.rs` (ADDITIVE): `#[path]` include of
  `pick_gpu`, one call `pick_gpu::update(ui, rect, scale, in_sketch)` after
  the backend block, and the orbit lane excludes `alt` so Alt+left-drag
  belongs to box select instead of fighting navigation.
- `crates/oep-app/tests/pick_gpu.rs` (new): the acceptance harness.

Interaction model (documented in the panel): hover shows the pre-highlight in
the same frame; shift+click accumulates the exact id list; Alt+drag commits
the enclosed set; a plain click on empty background clears the selection and
picks nothing. All grounded in the id buffer, never a hand-rolled re-project.

## The projection decision (important finding)

The card's binding acceptance is "id-buffer pick equals the analytic ray pick
from `oep_viewer::pick` for the same camera". The shipped colour pass has the
pre-existing, D1-documented `clip_w = row4(vp)*view` quirk: its drawn pixels
drift from the analytic projection off the optical axis (D1's handoff said
"CPU goldens use clip_w=row4(mvp)*(view*p); D2+ correct+regolden"). Under
that quirk the drawn frame and `oep_viewer::pick` genuinely disagree off-axis,
so an id buffer that mirrors the quirk can NOT equal the analytic oracle.
Per the CAD contract (the oracle is authoritative and D2 owns the colour
"correct + regolden"), the id pass therefore projects with the standard
perspective divide (`nx = clip.x / -view.z`, the convention
`oep_viewer::pick` unprojects): at every pixel the id buffer reports exactly
what the analytic pick resolves, and it coincides with the drawn frame once
D2 fixes the colour projection. Picking is analytic-truth on this tree; the
visual frame's off-axis drift is D2's documented item. Both kittest click
positions (`--test pick_gpu`) and the panel's overlay projection (`panel_of`)
use the same standard divide, so the pre-highlight / selection outlines hug
the id-buffer (analytic) positions.

This is the only behavioural deviation from the colour pass, it is loud in
code comments and module docs, and it keeps the card's numbers honest.

## Acceptance (numbers)

GATE 1 - `cargo test -p oep-app --test pick_gpu` -> **green, 122 passed /
0 failed, exit 0** (transcript `06-final-acceptance-gate.txt`; full run
`01-pick-gpu-acceptance.txt`):

- **20 sampled screen points, id-buffer == analytic** (`oep_viewer::pick`
  for the same camera): all 20 comparisons printed, `match=true` each, spread
  across all three fixture cubes
  (e.g. `px=(243,244) idbuf=cube-a pick=cube-a match=true`).
- **Hover pre-highlight within ONE frame**: kittest moves the pointer over
  cube A and asserts `pre=cube-a` after exactly one `harness.step()`; a second
  step is asserted not to change it (no second frame needed). Before any
  hover the oracle reads `pre=none`.
- **Shift-click accumulates the exact multi-select set**: shift+click A then
  B then C yields `sel=cube-a` -> `cube-a,cube-b` -> `cube-a,cube-b,cube-c`
  (exact id list asserted at each step).
- **Box select returns exactly the enclosed set**: a committed viewport rect
  enclosing cubes A+B (screen bbox of their world corners) - expected set from
  the analytic grid (`oep_viewer::pick` inside the rect):
  `["cube-a", "cube-b"]`; the id buffer returned `["cube-a", "cube-b"]`;
  printed and matched exactly. The rect stops ~60 world units short of cube C.
- **Click empty background clears and picks nothing**: after selecting A, a
  plain click at a genuinely-empty point (verified empty by the analytic
  pick) gives `sel=0`, `pre=none`, `pick=clear`.

GATE 2 - `xvfb-run -a cargo run -p oep-app -- --smoke` -> **exit 0**,
`isolith smoke: renderer=wgpu viewport-backend=gpu`, `frames=8 smoke ok`
(transcript `06-final-acceptance-gate.txt`).

Also green on the committed tree: `cargo fmt --all -- --check` (0), clippy
`-D warnings` for `oep-viewer` + `oep-app` (0), full `cargo test -p oep-viewer`
(0 failed), full `cargo test -p oep-app` (23 targets, 3066 passed, 0 failed)
(transcripts `03-`..`05-`).

## Images (real binary under xvfb, XTEST-injected, byte-identical x2)

- `hover.png` - pointer parked over the demo solid: the hover PRE-HIGHLIGHT
  (accent outline) surrounds the solid.
- `box.png` - Alt+left-drag across the demo solid, captured while held: the
  box-select rubber band (translucent accent fill + accent stroke) encloses
  the solid.

Hashes in `images.sha256` and per-file `<name>.png.sha256`:

```
cdf1c5eecdcb7525216882311fba794a6c0b3afb3379681e19e44c5f0cca7cca  hover.png
4b9f8c5159c423e0b6ff5fad291336eed0d0f56cb75949a8ac8a5186a0e75cbb  box.png
```

## Transcripts

| file | what |
|---|---|
| `00-step0-merge.txt` | STEP 0: fetch + merge of the wave head |
| `01-pick-gpu-acceptance.txt` | acceptance test, full --nocapture output |
| `02-smoke.txt` | smoke run |
| `03-fmt-clippy.txt` | fmt --check + clippy -D warnings |
| `04-oep-viewer-suite.txt` | full oep-viewer test suite |
| `05-oep-app-suite.txt` | full oep-app suite summary (23 targets, 3066 passed) |
| `06-final-acceptance-gate.txt` | the two acceptance commands with the printed numbers |

Requires a GPU adapter (lavapipe on this host): the id pass and the GPU
viewport run on it, exactly as the wave-D contract expects.
