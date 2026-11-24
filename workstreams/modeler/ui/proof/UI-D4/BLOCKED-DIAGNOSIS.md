# UI-D4 BLOCKED — diagnosis: exact Back standard view degenerates the renderer

Status: BLOCKED (needs_input). The card's six-standard-view acceptance cannot
be green on this wave head because of a pre-existing renderer defect that is
NOT in this card's owned file list.

## What the card contributed (done, tested, fmt+clippy clean)

- `crates/oep-viewer/src/views.rs` (new): `StandardView` (committed yaw/pitch
  per view: Front 0/0, Back PI/0, Left -PI/2/0, Right PI/2/0, Top 0/1.54,
  Bottom 0/-1.54, Iso PI/4/asin(1/sqrt3)), `CubeFace` hit test, `section_plane`
  presets, `section_triangle_count` (analytic) and `visible_triangle_count`
  (renderer-clip emulation). oep-viewer lib tests: 40 passed incl. 5 new.
- `crates/oep-app/src/panels/nav.rs` (new): orientation cube gizmo, quick
  Front/Top/Right/Iso buttons, Section slider publishing a `NavSnapshot`
  tuple; applies the camera through the shared backend seam.
- `crates/oep-app/src/panels/viewport.rs` (additive): `#[path]` inclusion +
  one `nav::update(...)` call.
- `crates/oep-app/tests/nav.rs` (new): 4 tests; 3 green (six-face numeric
  camera plus all of quick buttons, section counts, a11y names).
- Section analytics: the committed slider fractions 1.0 / 0.5 / 0.0 yield the
  committed visible-triangle counts 12 / 10 / 2 (analytic == measured),
  non-increasing, asserted through the live slider via the AccessKit
  `SetValue` action (reachable without a mouse).

## The blocker (reproducible on this host, adapter present)

At the exact **Back** standard view (yaw = PI) of the committed 80x40x10 demo
solid, the shared renderer's projection/clip-w step (`clip_w =
row4(vp)*(view,1)`, the UI-D1-documented "clip_w quirk" deferred to "D2+
correct+regolden") evaluates `w == 0` at the model's near-face (z=0) corners,
so `push_polygon`'s `|w| < 1e-9` guard drops EVERY face:

- CPU path (`crates/oep-viewer/src/render.rs::push_polygon`): renders 0 box
  pixels at Back (measured 5253 front / 0 back / 5166 left / 1600 right /
  4220 top / 7667 bottom / 5020 iso at 400x300, no overlays).
- GPU path (`crates/oep-viewer/src/gpu/mod.rs::render_frame`): with zero
  faces, `pass.set_vertex_buffer(0, face_buf.slice(..))` panics inside wgpu
  (`buffer slice can not be empty`, wgpu-30.0.2 api/buffer.rs:683) because the
  face branch lacks the empty-list guard the lines branch already has.

Exact repro:

```
cargo test -p oep-app --test nav six_cube_faces_land_exact_standard_views
  -> prints face=clicked "Cube Front" ... then
  panicked at .../wgpu-30.0.2/src/api/buffer.rs:683: buffer slice can not be empty
  stack: GpuBackend::render_frame -> set_camera -> nav::apply_view
```

Benign path check (throwaway probe, not committed): `GpuBackend::set_camera`
at Front/Left/Right/Top/Bottom/Iso all return frame hashes; only Back panics;
`CpuBackend` renders an empty box at Back too (no panic).

## Files needed (outside this card's owned list)

1. `crates/oep-viewer/src/gpu/mod.rs` — `render_frame`: guard the face branch
   like the lines branch (`if !faces.is_empty() { ... set_vertex_buffer ...
   draw ... }`) so an all-culled view renders an empty (safe) frame instead of
   panicking. Two-line, mirrors line 232.
2. `crates/oep-viewer/src/render.rs` + `crates/oep-viewer/src/gpu/mod.rs` —
   the shared `clip_w` projection correction (the UI-D1-deferred item, "D2+
   correct+regolden") so exact axis views actually render the model (currently
   the Back view also reads as an empty model on CPU).

Without (1) the acceptance's six-face test crashes on any GPU host; without
(2) the Back view is visually empty even once it stops crashing.

## Suggested resolution

Dispatch the renderer correction (D2 or a small follow-up card) to land
before/with this card, then unblock; the nav branch (ui/d4-gpu-nav) is ready
and the six-face test will be re-run as-is.
