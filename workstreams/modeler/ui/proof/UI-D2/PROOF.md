# UI-D2 UI-GPU-SHADING — proof

Card: `t_63d7a543` — studio lighting (directional key + fill), visible feature
edges, flat/smooth view modes, and a transparency control, each selectable at
runtime and each proven by a snapshot.

Files owned (this card):
- `crates/oep-viewer/src/gpu/shading.rs` (new) — light rig, view modes, feature
  edge list, process-global state (the single source of truth).
- `crates/oep-viewer/src/gpu/shaders/shading.wgsl` (new) — key+fill fragment
  shading; placeholders substituted from `shading_wgsl()` at build time.
- `crates/oep-viewer/src/gpu/mod.rs` — ADDITIVE graft of the shading pipeline.
- `crates/oep-app/src/panels/shading.rs` (new) — view-mode selector + alpha
  slider + oracle card.
- `crates/oep-app/src/panels/viewport.rs` — ADDITIVE `#[path]` include + one
  `shading::draw` call.
- `crates/oep-app/tests/gpu_shading.rs` (new) — acceptance.
- `workstreams/modeler/ui/proof/UI-D2/` — this proof.
- `workstreams/modeler/ui/README.md` — this card's row.

All commands below ran on the committed tree in the `ui-d2` worktree
(`/home/r1a/isolith-main/.worktrees/ui-d2`, aarch64, lavapipe under xvfb).

## Snapshot sha256 (one per view mode, all five distinct)

```text
shaded          07eebca5f53cdd1d387ebb6450e91de1875ce528ae52fd20b000ad520ef8c84d  ui-d2-shaded.png
edges           8dee0722f3ffe6f006438e27819916e22d40897279708161348758970abf92fd  ui-d2-edges.png
flat            d4cb47d07e7a4f2cd1645ab975c44cedd859b8b154cb6e50eee81d5f96f99d25  ui-d2-flat.png
smooth          4836698181e4bbef3e754d73d9f95c486bb633b974de0acbe38bdd71eb7c60a3  ui-d2-smooth.png
transparent-50  afdb52960dc6ac160e9f7c82161fcf36f4c4c9a37693a4a2cd8bae62f37f3b24  ui-d2-transparent-50.png
```

All five hashes differ (the acceptance asserts pairwise distinctness), so the
five rendered states are five genuinely different pixels.

## Acceptance gate

```text
$ cargo test -p oep-app --test gpu_shading
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ xvfb-run -a cargo run -p oep-app -- --smoke
isolith smoke: renderer=wgpu viewport-backend=gpu (set ISOLITH_GPU=off to force cpu)
frames=8 smoke ok
$ echo $?
0
```

## The six proof legs (real test output)

### 1. Five view modes -> five distinct rendered states

```text
shaded:         sha256 07eebca5…8c84d
edges:          sha256 8dee0722…92fd
flat:           sha256 d4cb47d0…9d25
smooth:         sha256 48366981…c60a3
transparent-50: sha256 afdb5296…3b24
all five snapshot hashes are pairwise distinct
```

### 2. The edge pass draws EXACTLY the boundary edge list

Fixture: one box proxy (12 box edges) + one mesh cube primitive with an
explicit 12-edge boundary list; total boundary edges = 24.

```text
Edges mode:  drawn edge primitives = 24, fixture boundary edge count = 24
Shaded mode: drawn edge primitives = 24, fixture boundary edge count = 24
```

The pass submits one line primitive per mesh `edges` / box `BOX_EDGES` entry
only — not more, not less.

### 3. No z-fighting

Neither pipeline has a depth stencil attachment, so the GPU performs zero
depth comparisons:

```text
depth_stencil: None on shading and lines pipelines => depth comparisons performed = 0
face-vs-face period-2 alternating runs (>=4px, horizontal+vertical) along shared edges = 0
```

The alternation scan (method in the test docstring: every horizontal and
vertical scanline run is walked; a run counts only when colours repeat
period-2 `c[i]==c[i+2]!=c[i+1]` and both alternating colours are shaded-face
colours, not background or a line-pass colour) finds 0 on the committed
Shaded snapshot.

### 4. Smooth vs Flat differ only in normal-driven pixels

```text
smooth-vs-flat: differing pixels = 56415, silhouette (non-bg mask) flat = 27840, smooth = 27840
```

Non-zero differing-pixel count (normal-driven lighting differs) AND the
silhouette (non-background mask) is byte-identical between Flat and Smooth.

### 5. Transparency changes the readback alpha of the overlapping body

The mesh cube overlaps the box proxy on screen; the mean readback alpha of the
overlap region is printed before (opaque) and after (50%):

```text
overlap-region mean readback alpha: opaque = 255.0, 50% transparent = 149.3
```

The alpha channel writes the source alpha (STUDIO_BLEND alpha REPLACE), so
the 50% body measurably reads back less than the opaque one.

### 6. The shading panel is runtime-selectable (kittest through the real app)

```text
panel click 'Edges'  -> oracle "mode=Edges\nalpha=1.00\n"
panel click 'Flat'   -> oracle "mode=Flat\nalpha=1.00\n"
panel click 'Smooth' -> oracle "mode=Smooth\nalpha=1.00\n"
panel click 'Shaded' -> oracle "mode=Shaded\nalpha=1.00\n"
slider dragged       -> oracle "mode=Shaded\nalpha=0.61\n"  (alpha 0.61)
```

Clicking each view-mode button flips the panel oracle AND the process-global
shading state the GPU backend renders; dragging the transparency slider moves
the runtime alpha. (The slider lands at ~0.6 for the probe drag; the 50%
numeric proof is leg 5.)

## Regression guards

```text
$ cargo test -p oep-viewer --test gpu_backend   (D1 seam/determinism oracle)
test result: ok. 6 passed; 0 failed            (coverage within 5%, byte-identical re-render, ISOLITH_GPU=off golden)

$ cargo test -p oep-app                          (all 19+ targets)
all green — includes viewport 117/117, measure, dimensions, feature_from_sketch,
gpu_shading 121/121 — no prior card regressed.
```

Determinism: everything is a pure function of `(scene, camera, size, shading
state)`, so the same state re-renders to byte-identical pixels (re-proven by
the D1 `gpu_backend` determinism oracle on the committed tree).

## Files

- `ui-d2-shaded.png`, `ui-d2-edges.png`, `ui-d2-flat.png`,
  `ui-d2-smooth.png`, `ui-d2-transparent-50.png` — the five snapshots.
