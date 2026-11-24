# UI-D7 UI-RENDER-FIX — proof of the clip-w projection correction and empty-face safety

Wave D, in-wave fix card (the wave-B UI-B10 pattern). Branch `ui/d7-render-fix`,
commits `b7bf432` (correction + golden re-register) and `04a6fcf` (proof-image
emission), both DCO-signed. Full real transcripts under `transcripts/`.

## STEP 0 — brought nothing

`git fetch github && git merge github/wave/ui-d` → `Already up to date.`
(the wave head `513ce8f` — UI-D1+D2+D3 — was already the branch base).
See `transcripts/00-step0-merge.txt`.

## What was wrong (the UI-D1-deferred "D2+ correct + regolden" item)

The shared projection step computed the clip-space `w` as

```
w = row4(vp) · (view, 1)        # OLD: combined view-projection row applied to a
                                # VIEW-SPACE point -> view transform double-applied
```

`vp = P·V` expects a BODY/world point, so feeding it the already-view-space
point applied `V` a second time. For a right-handed camera where `row4(P) =
[0,0,-1,0]`, the true clip w of a view-space point is `-view.z`, and the buggy
formula degenerated to **exactly 0** at the demo solid's near (z=0) corners at
the exact Back view (yaw = π), so `push_polygon`'s `|w| < 1e-9` guard dropped
every face:
- CPU rendered 0 box pixels at Back;
- GPU hit the empty face buffer and panicked
  (`wgpu: "buffer slice can not be empty"`, ui/d4's BLOCKED-DIAGNOSIS).

## The correction

Both backends now apply the combined matrix to the point it owns:

```
w = row4(vp) · (world, 1)        # NEW: identical body-space operand as the clip xyz
```

Near-corner `w` at the Back view, 400x300 (from
`transcripts/01-render-axis-views.txt`, test `back_view_near_corner_w_before_and_after`):

```
clip-w formula change: row4(vp)·(view,1)  ->  row4(vp)·(world,1) = -view.z
corner              old_w             d7_w
              0.000000000    136.108800207
              0.000000000    136.108800207
              0.000000000    136.108800207
              0.000000000    136.108800207
Back view CPU covered-pixels after fix: 22810
```

## Covered-pixel table, six standard views + iso, 400x300, BOTH backends

From `all_standard_views_render_on_both_backends`
(`transcripts/01-render-axis-views.txt`):
every view > 0 on both backends (today Back = 0 on CPU / panic on GPU), and
CPU and GPU agree within 10% (worst drift 2.94%):

```
view        gpu_px     cpu_px   drift%
Front        22487      22827   0.0149
Back         22520      22810   0.0127
Left          5576       5745   0.0294
Right         5635       5471   0.0291
Top           7853       7817   0.0046
Bottom        7834       7813   0.0027
Iso          22161      21628   0.0241
```

## All-culled scene: empty frame, no panic, both backends

`all_culled_scene_renders_empty_frame_without_panic_on_both_backends`
(near plane pulled inside the model = pure face-list emptiness). Frames are
byte-count-identical to the overlays-only baseline (same bounds, no
primitives), i.e. zero model pixels and no panic on either backend:

```
all-culled scene: CPU faces=231 baseline=231; GPU faces=246 baseline=246; no panic
```

The empty-face guard the wave head already carries (UI-D2, gpu/mod.rs) is the
mechanism; this test pins it.

## Acceptance legs (real transcripts)

| Leg | Result | Transcript |
|---|---|---|
| `cargo test -p oep-viewer --test render_axis_views` | 4 tests, all ok | `transcripts/01-render-axis-views.txt` |
| `cargo test -p oep-viewer --test gpu_backend` | 6 tests, all ok (golden re-registered) | `transcripts/02-gpu-backend.txt` |
| `cargo test -p oep-viewer` (full suite) | all ok | `transcripts/03-oep-viewer.txt` |
| `cargo test --workspace` | exit 0, 4448 passed / 0 failed | `transcripts/04-workspace.txt` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 | `transcripts/05-smoke.txt` |
| `cargo test -p oep-app --test nav` | **NOT green on this tree — see diagnostics below** | merge-tree run captured in `transcripts/06-nav-diagnosis.txt` |

### `--test nav` status (important)

The wave head does NOT yet contain `crates/oep-app/tests/nav.rs` — that test is
UI-D4's and lands with the UI-D4 card (my child, currently `todo`, waiting on
this card). To prove my fix's share of that acceptance, I built a temporary
merge of the wave head + my `ui/d7-render-fix` + the `ui/d4-gpu-nav` delta
(additive files only) and ran the **unchanged** six-cube test.

Result: the renderer-caused panic is **gone** (the GPU no longer panics on an
empty face buffer, and Back renders 22k pixels), but the test now fails on a
DIFFERENT, PRE-EXISTING UI-D4 defect that the old panic had masked:
`crates/oep-app/src/panels/nav.rs` lays out the six cube-face buttons at
`FW=40, FH=20, GAP=4` (24px pitch) but the buttons grow to ~64px tall
(the `"Cube …"` label wraps inside the 40px min width), so the face buttons
overlap vertically and a click on one face resolves to a neighbouring face.
Measured on this merge tree via AccessKit (see `06-nav-diagnosis.txt`):

| clicked | label node rect | camera yaw after | hits |
|---|---|---|---|
| Cube Front | [852,88]-[892,152] | π (Back!) | overlapping Back row |
| Cube Back | [852,112]-[892,176] | 0, pitch -1.54 (Bottom!) | overlapping Bottom row |
| Cube Left | [808,88]-[848,137] | -π/2 (correct) | isolated |
| Cube Right | [896,88]-[936,152] | π/2 (correct) | isolated |
| Cube Top | [852,64]-[892,113] | 0,0 (Front!) | overlapping Front row |
| Cube Bottom | [852,136]-[892,200] | 0, pitch 1.54 (Top!) | overlapping Top row |

The same scramble reproduces on the **pristine `ui/d4-gpu-nav` branch** with
the CPU backend (`ISOLITH_GPU=off`), so it predates my card and is independent
of the clip-w correction. It is inside UI-D4's owned file, not in this card's
owned list; per the card rules ("touch no file outside your owned list") this
card does not fix `nav.rs`. The renderer share of the acceptance (no panic,
every standard view renders) is proven above; the `nav.rs` click-geometry
defect is tracked for the UI-D4 card, which lands after this one.

## Goldens that moved — old/new sha256

| File | old | new | why |
|---|---|---|---|
| `crates/oep-viewer/tests/gpu_backend.rs` `CPU_DEMO_GOLDEN_HASH` | `11436915591341135581` | `12597229237775278722` | The corrected projection moves the CPU demo frame (the UI-D1-deferred "correct + regolden" item, explicitly re-registered with a comment in the file; no test logic weakened) |

The launch-frame golden (`shot.sh` / `ui-a2/frame.png*`) is intentionally NOT
touched: its refresh belongs to the wave-D integrate card UI-D6.

## Proof images (all sha256 in `transcripts/01` run output and below)

16 deterministic PNGs under this directory (verified byte-identical across two
runs — CPU renderer deterministic, GPU determinism oracle intact):
six standard views × {cpu,gpu} at 400x300 plus the all-culled frame ×
{cpu,gpu}. sha256 for every image:

```
all-culled CPU 0f2e19116639692e6d20e9877c89d5033ddfb7e2fd6127173b9efaf333ec043f
all-culled GPU 5ca67f6b3d8621323818f590ba38b2e2f07454fb159080166d452da1b91a4fbe
Back      CPU 16557c16c8aa75ce0cbe4c9f6617f61ff4aa8509a3efbfd7b0975d517c78690d
Back      GPU 11e7b4f92ba4bfc17bbb3d389e421a35e0341470a4434355789afbc0d641b977
Bottom    CPU f4c78e4b052bbf7046ca40b670256cf23b0ecc74c8ac5ad2a27b395a1680277e
Bottom    GPU 0e9f1b972f79576b08d46ce68aa7dee62eaeae62f035014d6f36e113e4b5cc78
Front     CPU 721209ec77edb9215960b075f14e5e316af8baa5de8c5ac0da1ddcba6b35c35a
Front     GPU 90750d3fb61497589f06089de39b6610095b8c96e1c717ed009562b0f27e9898
Iso       CPU adadf5c4df5bd1699c997d023048c37ef0ee4d81d73c8b72f42f1b3b84bb63e0
Iso       GPU 3164ea28d7571e83a2bc79deee5f8b593756ab2d939046a87bcb8a5f034bb054
Left      CPU 80468a5baf39f18c3a98602ba25b4c7527981d4379023271be319167b6f3bc00
Left      GPU 262389ff0ce9650d4a801ac7eeb3c765b6bb9980c3e2fe9d69a2315bdcec8568
Right     CPU 5b1a1247721e1db2850d036c64b01dc17b18a48fab1024bff373d39115784794
Right     GPU e1eb01e8d4791d3cf8fa4e5487075756e5c52969f29a3936dfd5d812efc38d0d
Top       CPU 0a951c0e679627060c4816f9f6186050bcc2c7db152ac4853626c1c9d9f28618
Top       GPU 8ad31153c612d3a5257b594482d4ed22cceaa86629fef87665d4a5788f317f30
```

## Files owned and touched

- `crates/oep-viewer/src/render.rs` (clip-w + `to_px`/`draw_wire` world-space)
- `crates/oep-viewer/src/gpu/mod.rs` (same correction on the GPU path; the
  empty-face guard was already present from UI-D2 — verified, not re-added)
- `crates/oep-viewer/tests/render_axis_views.rs` (new acceptance)
- `crates/oep-viewer/tests/gpu_backend.rs` (golden constant re-registered only)
- `workstreams/modeler/ui/proof/UI-D7/` (this proof)
- `workstreams/modeler/ui/README.md` (this card's row)
