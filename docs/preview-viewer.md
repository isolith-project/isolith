# oep-viewer: preview viewer (desktop app shell) for oep-geo models (P6-M5-T02)

Version 1.0, 2026-10-16. Milestone: P6-M5-T02 (preview viewer shows `oep-geo`
models; camera, selections, clipping). Program: Keelworks / Open Engineering
Platform (OEP), P6 geometry core, renderer backend lane.

Reference implementation: `crates/oep-viewer` (Rust, std-only, deterministic).
Real-material gate: `crates/oep-geo/tests/golden/example-bracket.oepgeo.json`,
rendered to `workstreams/geometry-core/viewer/proof/preview-full.png` and `preview-clipped.png`, and
verified by `tests/test_preview_viewer_acceptance.py`.

## 1. What this milestone delivers

A desktop-app-shell preview viewer that shows the five-class `oep-geo` model
schema (P6-M1-T01) with the three interactive behaviors named on the card:

- **Scene**: `oep_geo::Document` -> [`Scene`] -> [`Primitive`] leaves. Each
  body (P6-M1 bodies carry only an AABB; the geometry kernel slots land at
  P6-M2/P6-M3) becomes a **box proxy** placed by the full assembly /
  instance transform chain. The instance ids that placed the box are recorded,
  so a selection can name exactly which placement the user clicked.
- **Camera**: [`Camera`] is an orbit camera (target + yaw/pitch/distance)
  over `oep_geo::math`, producing a right-handed view matrix and a matching
  perspective projection. Fit-to-scene framing is a first-class constructor.
- **Selections**: [`pick`] unprojects a screen pixel into a world ray (via
  the inverse view-projection, with the perspective divide handled by hand)
  and returns the nearest primitive plus its full identity: instance chain,
  part id, body id, hit distance, world point.
- **Clipping**: [`ClipPlane`] is a world-space half-space; points and AABBs
  classify to a [`Side`], and convex polygons clip on the kept side
  (Sutherland-Hodgman).
- **Render**: [`render`] is the reference CPU software rasterizer behind the
  P6-M5-T01 render seam (its documented escape hatch, section 9). It draws
  every clipped, camera-visible face back-to-front (painter's algorithm) with
  a deterministic Lambert term. A minimal dependency-free PNG encoder turns a
  frame into proof images on any host.

Everything is std-only (no wgpu in the workspace crate), f64, and
deterministic, so the acceptance contract runs on the GPU-less CI container
and still proves camera, selection, clipping and rendering on the real golden
document. The interactive/offline WGSL/wgpu backend (P6-M5-T01) can slot in
behind the same seam without touching callers.

## 2. Scene building

`build_scene(doc)`:

1. Walks every **root assembly** depth-first (an assembly nested under
   another assembly's instance is only walked from its parent, so no double
   emit). Along each chain it composes instance transforms with
   `oep_geo::math::Mat4` (`parent * instance`).
2. Emits one `Primitive` per body of every instanced part that is `visible`,
   using the body's AABB corners transformed to world space (a conservative
   world AABB is kept for picking/clipping).
3. Also emits any **part never referenced by an instance** at the identity
   pose, so opening a bare part file still previews.

The document `Transform` (`[f64;16]`) is consumed by `Mat4::from_array16`
verbatim: the math contract places the placement translation at `data[12..15]`,
which is exactly where the golden placement arrays put it (verified: feeding
the raw golden F1 array through `from_array16` yields the expected (10,10,0)
placement). A general affine document transform would need an explicit
transpose before adoption.

## 3. Camera and picking

The camera is a right-handed orbit camera looking down the -Z view axis:

- `eye()` derives the eye from target/yaw/pitch/distance (yaw around +Y,
  pitch above the XZ plane).
- `view_matrix()` is an OpenGL-style look-at.
- `projection(aspect)` is the standard symmetric perspective frustum.
- `view_projection()` = `projection * view`.

Picking unprojects a screen point through the inverse view-projection by hand
(`Mat4` has no homogeneous multiply in P6-M2-T01): two clip points (near and
far) map to world, and the ray direction is their difference, normalized.
The ray origin is the camera eye; ray/AABB uses the slab method and returns
the nearest hit.

## 4. Clipping

A clip plane `n·p + d = 0` keeps the `<= 0` side. `classify_aabb` is
conservative over the 8 corners (`Inside` / `Outside` / `Intersecting`).
`clip_polygon` runs Sutherland-Hodgman against the plane and returns the kept
polygon; the renderer feeds every box face through the enabled clip planes
before drawing, so slicing the model genuinely removes (or cuts) the
geometry. The clipped proof (`preview-clipped.png`) keeps the x<=40 half of
the bracket assembly; the right-hand fasteners are fully removed.

## 5. Reference renderer

- Per part, a stable color (FNV hash -> HSV). Per clipped face, a
  deterministic Lambert factor from the Newell face normal dotted with a fixed
  light direction.
- Faces are painter-sorted by average view depth (farthest first); convex box
  proxies on a closed box need no z-buffer for a correct fill.
- Near/far frustum culling happens in NDC (`z in [-1,1]`) before rasterizing.
- Deterministic scanline triangle fill with barycentric point-in-triangle.

## 6. Files

```
crates/oep-viewer/
  Cargo.toml            workspace member, depends on oep-geo
  examples/render_bracket.rs   loads the golden doc, frames camera, writes proofs
  src/lib.rs            module surface + re-exports
  src/scene.rs          Scene / Primitive / Aabb / build_scene
  src/camera.rs         orbit Camera (view, projection, fit)
  src/pick.rs           Ray, Selection, unproject, ray_aabb, pick
  src/clip.rs           ClipPlane, Side, Sutherland-Hodgman polygon clip
  src/render.rs         reference rasterizer + dependency-free PNG encoder
  tests/viewer_contract.rs   contract test on the REAL golden document
tests/test_preview_viewer_acceptance.py   acceptance gate (TDD, written first)
workstreams/geometry-core/viewer/proof/preview-full.png              real-material render (full assembly)
workstreams/geometry-core/viewer/proof/preview-clipped.png           real-material render (x<=40 kept)
docs/preview-viewer.md              this record
```

## 7. Verification (real material)

On the reference host (U1): `cargo run -p oep-viewer --example render_bracket`
loads the committed golden `example-bracket.oepgeo.json`, builds a 5-leaf
scene (bracket + 4 fasteners), frames the orbit camera, and writes the two
proof PNGs. Independent Python decode confirms: geometry drawn (13,003 non-
background pixels in the full view, 8,065 after clipping), the clipped render
differs from the full render, and selection at the screen centre returns the
bracket with its instance chain `oep-inst-bracket`.

Green CI checks on push: pytest (33 passed incl. this acceptance test),
cargo fmt, clippy -D warnings, cargo test --workspace, precommit gate PASS.

## Change log

- 1.0.0 (2026-10-16): milestone record; scene/camera/selection/clipping/
  reference renderer + real-material proofs committed.
