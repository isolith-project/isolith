# Isolith renderer: prior art and a design brief for GPU-resident, large-scene rendering

Scope: the `oep-viewer` GPU backend (`crates/oep-viewer/src/gpu/mod.rs`, wave D / UI-D8).
Host measured on: NVIDIA GB10 (integrated GPU, unified memory, Vulkan, driver 580.178.04, aarch64, 20 cores), wgpu 30.0.2, egui 0.37.
Measured problem (committed card data, not this brief's invention): at 1,002,528 triangles, 1600x1000, 120 frames after 20 warm-up — p50 429.8 ms, p95 434.9 ms, mean 2.33 fps. Card bar: **p95 <= 16.6 ms** (i.e. >= 60 fps). The UI-D8 vertex-stage transform move changed p95 from 438.6 -> 434.9 ms (~nothing), so the cost is per-frame CPU `assemble()` plus the every-frame ~120 MB re-upload, not the transform.

Summary of the current hot path (read from the code):

- `render_frame()` calls `assemble()` every frame; `assemble()`:
  - copies every mesh's `[f32;3]` positions into a fresh `Vec<Vec3>` (f64) per primitive;
  - recomputes **smooth normals every frame** in `Smooth`/`Shaded` modes;
  - projects/culls every face on the CPU through `render::project_ndc`, builds one `GpuFace` (with two heap-backed `Vec<[f64;3]>` per triangle) per triangle = **~1,002,528 heap allocations per frame**, then a full f64 `sort_by` painter sort of the whole face list;
  - returns `Vec<ShadedVertex>` (40 B each: world pos + view normal + RGBA);
- `render_frame()` then `pack_shaded()`es every vertex into a fresh `Vec<u8>` and calls `device.create_buffer_init()` for a **new vertex buffer every frame** (~120 MB), draws a single triangle list, then the line pass does the same; there is no depth attachment.
- The offscreen `Rgba8Unorm` target is read back to a host `RgbaImage` every frame (`target::read_back`); the id pass (`gpu/idbuffer.rs`, `R32Uint`) is re-read only on view change.

Lower bound arithmetic (all figures derived here unless the source gives a number):

- 1,002,528 tris x 3 verts x 40 B = 3,007,584 x 40 B = 120,303,360 B = **120.3 MB of GPU upload per frame**, every frame.
- CPU transient garbage per frame (estimate): ~3M substituted f64 positions/normals VBOs plus ~1M per-face `Vec` headers (24 B = 24 MB) + per-face f64 position/normal heaps (~72 B = 72 MB) + per-frame packed byte vecs (120 MB) — on the order of 200-300 MB allocated and freed per frame. This is the measured wall time, not the upload.
- 1600x1000 target: Rgba8Unorm = 6.4 MB read back every frame; the R32Uint id grid = 6.4 MB read **only on view change**.

A complete index of sources cited below appears in ["7. Source list"](#7-source-list).

---

## 1. Prior art (real citations)

### 1.1 Retained-mode scene graphs and GPU-resident geometry

- **Coin3D / Open Inventor (the model FreeCAD is built on) made "retained mode" and GPU-resident display data first-class.** Coin3D is "a C++ object-oriented retained mode 3D graphics API used to provide a higher layer of programming for OpenGL" [S11]. The retained part that matters here is the *display cache*: `SoSeparator` nodes build **render caches (display lists/VBOs)** around their subtrees so that re-rendering does not re-transmit geometry; Open Inventor's own optimisation guide states "SoSeparator nodes build multiple render caches (by default, a maximum of two)" and that caching is chosen "more often when rendering is remote", with `SoRenderManager` decoupling scene mutation from the actual redraw [S12, S15]. The design lesson for Isolith: the *scene* (document, topology) and the *render representation* (GPU buffers) are two layers; you mutate one and the other is refreshed on invalidation, not rebuilt every frame.
- **OCCT (Open CASCADE) AIS is the same two-layer idea**: a presentable object (AIS_InteractiveObject) computes a *presentation* exactly once per display request into a `Graphic3d_Structure`, and the viewer manipulates that structure, not the B-Rep — "when you zoom, translate or rotate a view, the viewer operates on the graphic structure created by the presentable object and not on the data model" [S17]. Geometry stays in GPU VBOs through `Graphic3d_ArrayOfPrimitives`/`Graphic3d_Buffer` ("compatible with VBO... interleaved attributes") [S18].
- **Matching precedent in the engine world (bevy)**: bevy's render world extracts meshes into `GpuMesh` — "the transformation is done at the extraction stage" — and a 2025 change (pcwalton, PR #16385) explicitly "moves the front end of the rendering pipeline to a retained model when GPU preprocessing is in use", keeping `RenderMeshInstance`/uniform data resident across frames instead of re-uploading per frame [S19, S20]. The trend is unambiguous: **upload geometry once, re-upload only what changed**.
- Lesson transferable to wgpu directly: all of these are API-agnostic statements about staging your geometry into device-resident buffers once and referencing them; this is exactly what wgpu persists for you if you stop calling `create_buffer_init` per frame.

### 1.2 How OCCT/AIS and FreeCAD/Coin3D handle large assemblies

- **Draw-call reduction via merging, not per-part draw calls.** OCCT's official answer to "10,000+ identical objects": "AIS_ConnectedInteractive... does not reduce the number of draw calls... The first optimization level would be to group small objects into larger AIS objects... different geometry could be simply merged into one `Graphic3d_ArrayOfPrimitives` (mapped into VBO in OpenGL). This would reduce the number of Draw Calls, improve framerate and reduce memory footprint". Native hardware instancing exists in OCCT only via a custom `OpenGl_Element` drawer (`glDrawArraysInstanced` + a custom instance VBO filled with transform matrices) [S6]. So even the CAD reference implementation treats **per-draw-call overhead as the enemy** and reaches for batching/merging/instancing; Isolith's single-triangle-list-per-pass shape is exactly the OCCT "merge" recommendation carried further.
- **Frustum/occlusion culling is BVH-driven and CPU-side.** "All three types of OCCT selection are implemented as a single concept, based on the search for overlap between frustum and sensitive entity through 3-level BVH tree traversal"; the view redraw keeps a coarse culling scene (assembly/AIS nodes) as a BVH and only sends visible structures to the GPU [S16, S17]. FreeCAD's blocker thread and Coin3D's "AS_COMPACT/retained-IR" Vulkan work confirm the community view that Coin3D's OpenGL-era caching is being made false-modern for exactly the large-assembly case [S13, S14].
- **OCCT's own practice on camera moves**: performance counters in `V3d_View`; "never redraw the viewer after modification of an individual AIS object — redraw content only after all objects have been updated" [S7, S5]; and OCCT 7.x introduced "ray-tracing with BVH" but the standard path stays raster.

### 1.3 Modern engines: batching, instancing, vertex pulling, indirect draws, culling, LOD, depth prepass, dirty-range re-uploads

- **GPU-driven rendering (the canonical talk).** Ulrich Haar & Sebastian Aaltonen, *GPU-Driven Rendering Pipelines*, Advances in Real-Time Rendering SIGGRAPH 2015 (Assassin's Creed Unity + RedLynx): per-material instance batching instead of per-mesh batching; "render... without CPU intervention, using just a handful of indirect draw calls... hundreds of thousands of independent objects with unique meshes... at 60 FPS on current console hardware"; culling moved to the GPU (indirect dispatch + multidraw) [S1]. The transferable core for Isolith is *decoupling draw submission from geometry*, i.e. the GPU holds an instance/hit-table and draws are generated without per-object CPU work.
- **Write-an-efficient-Vulkan-renderer (zeux.io, Arseny Kapoulkine, GPU Zen 2 / 2020) — the practical memory bible:**
  - `DEVICE_LOCAL_BIT` memory is "fastest to access from the GPU and this is the memory you should be using to store... **all static resources such as textures and geometry buffers**"; per-frame writers (uniform buffers, dynamic morphology) go in host-visible memory.
  - "On integrated GPUs, there is no distinction between GPU and CPU memory — these devices generally expose `DEVICE_LOCAL_BIT | HOST_VISIBLE_BIT` that you can allocate **all static resources through as well**" [S2].
  - Drivers only require ~4096 allocations; you suballocate from 16-256 MB blocks; frequent per-resource driver allocations are wasteful [S2].
- **Vulkan Guide — compute culling + indirect draws (vkguide.dev).** The canonical little-engine recipe: one `instanceBuffer` of `ObjectID + BatchID`; a compute shader frustum-culls spheres, atomically appends survivors into `Draw` commands' `instanceCount`; "frustum culling will easily cut half the objects"; occlusion culling via a previous-frame depth pyramid adds a 1-frame latency [S8]. Crucially for us, it also records the transparency interaction: "Sadly, doing culling and sorting at the same time is a really hard problem... The culling uses GPU atomics... no stability at all for sorting," so their GPU-driven path does not sort transparents [S8].
- **Indirect/multi-draw on the command processor (AMD AGS MDI).** "The draw calls are issued on the GPU's command processor (CP), potentially saving the significant CPU overheads incurred by submitting the equivalent draw calls" [S9].
- **Batching/instancing keeps draw calls small** (SaschaWillems' `instancing`/`computecullandlod`/`indirectdraw` Vulkan examples are the canonical worked demos of instanced draws, compute-cull + LOD, and indirect draws respectively) [S10]; the Arm GPU best-practices doc calls out that the *CPU dispatch* cost of many small draws is what you pay even when the GPU work is trivial [S21].
- **Retained buffers + double buffering.** `Queue::write_buffer` per frame routes through wgpu's internal staging pool ("probably it can create new allocations") [S23]; the right shape is persistent mapped/write buffers recycled across frames ("multiple buffers in flight and update one that is not in use by the GPU", gpuweb discussion) [S24]; a real regression in wgpu 27 -> later shows staging-allocation strategy moves upload cost measurably (~0.03 ms -> 0.12-0.23 ms in one remeshing case) [S25].
- **AIS-style static geometry + dirty-range re-upload** = upload once on load, re-upload only the bytes for meshes whose `(transform, highlight, geometry)` changed; nothing in the cited sources re-uploads static geometry per frame.

### 1.4 Alpha transparency ordering when a depth buffer is used

- **The standard is: opaque pass first (depth buffer resolves ordering pixel-exactly), then transparent pass sorted back-to-front with depth test on and depth **write** off.** "To make blending work for multiple objects we have to draw the most distant object first and the closest object last. The normal non-blended objects can still be drawn as normal using the depth buffer so they don't have to be sorted... 1. Draw all opaque objects first. 2. Sort all the transparent objects. 3. Draw all the transparent objects in sorted order" (LearnOpenGL Blending chapter — the benchmark teaching answer) [S27]. three.js does exactly this (opaque first nearest-to-farthest "and the depth buffer sorts them out pixel by pixel"; transparents rendered back-to-front, after o cadas the sorted set, with depth-write disabled) [S26, S29].
- **Why you sort at all**: blending is order-dependent at the fragment level, so the depth buffer alone cannot order blended fragments; per-pixel ordering (Order-Independent Transparency) is "significantly more expensive" (vkguide) [S8] and is only justified when object-level ordering is insufficient [S28].
- **Cost implication for Isolith**: switching to a depth buffer does *not* eliminate sorting of the transparent set; it *shrinks* the sorted set to transparents only (in Isolith that is "all primitives while the global alpha < 1") and lets the opaque majority be unordered.

### 1.5 Depth precision (large CAD near/far ranges)

- **Depth is nonlinear (1/z)**: "the GPU hardware depth buffer stores 1/z... precision is concentrated near the near plane; pulling in the near plane makes the d range skyrocket toward the asymptote" (NVIDIA blog, Nathan Reed) [S30].
- **Reverse-Z + float depth gives zero comparison errors in the cited simulation,** and "erases the distinctions between precomposed versus separate view/projection matrices, and finite versus infinite far planes. In any perspective projection situation, just use a floating-point depth buffer with reversed-Z" [S30]. For CAD-scale near/far ratios this is the difference between z-fighting thin faces and not [S30, S31].

---

## 2. Recommended architecture for Isolith, in independently shippable stages

Design skeleton (what stays CPU, what becomes GPU-resident, the dirty tracking, depth vs painter, picking).

### 2.0 The split, up front

- **Stays on the CPU**: document/B-Rep and scene semantics (`Scene::primitives`, their `id`s, `highlighted`, per-primitive base colour, `transform`), view-mode state, and the painter sort for whatever stays painter-sorted (see 2.4). This is the seam's "what actually is drawn" authority, so the CPU backend and the tests keep working untouched.
- **Becomes GPU-resident, uploaded once**: per-mesh topology. For each `Primitive` with a `mesh`, upload once per *version* of the mesh: positions (`[f32;3]`), and normals — flat normals computed once at upload (or on the fly per face in the shader via `dpdx`/`dpdy`, which costs no storage at all), smooth normals computed once at upload when Smooth/Shaded mode needs them. Per-vertex data shrinks from 40 B to 12-24 B because colour and (in flat mode) normals no longer live per vertex.
- **Per-primitive, not per-vertex**: base RGB colour, `highlighted` flag, and instance/transform live in a small per-primitive upload table (a `storage`/instance buffer), not copied into 3 million vertices. Then moving the global alpha slider costs **zero bytes** and **zero sorting** (alpha becomes a uniform read in the fragment shader).
- **Camera state**: the 64-byte MVP uniform (already there) is the only per-frame transform write.

### 2.1 Stage 1 — Retain geometry on the GPU; drop the every-frame 120 MB re-upload (no pixel change)

- Keep `assemble()`'s output contract exactly (painter-sorted face list, same projection, same shading). The *only* change is transport: uploaded meshes live in device-local vertex/index buffers keyed by mesh; each frame builds a `Vec<u32>` index range / instance list that references resident buffers instead of re-serializing vertices.
- Buffers: one vertex buffer per distinct mesh (or one big arena buffer with suballocated ranges), `usage: VERTEX | COPY_DST`; create them once in `GpuBackend::new` (or lazily on first sight of a mesh version), reuse forever. Indexed draws via a `[u32;3]` index buffer cut the vertex count substantially if meshes share vertices; if the tessellator emits unique positions per triangle (~unchanged), skip indices at first — they can come later for free.
- **Expected effect (estimate, arithmetic below)**: the measured ~435 ms collapses to roughly the `assemble()` CPU cost (per-frame allocations + sort) plus the ~6.4 MB readback and a tiny draw. Stage 1 already removes 120.3 MB/frame of packing + allocation + staging + copy.
- **Testable invariants this stage must keep**: identical pixels (determinism oracle), same coverage band vs CPU, same edge-primitive counts, same id-buffer parity. No golden change (CPU `ISOLITH_GPU=off` path byte-untouched).
- Memory: see 2.7.

### 2.2 Stage 2 — CPU cull + dirty-tracking; shrink the per-frame CPU face list

- Add a per-primitive **version stamp** in `Scene`-adjacent state (not the doc): bump whenever `prim.mesh` bytes / `transform` / `highlighted` / base colour change. `MeshData` already derives `PartialEq`, so "dirty" can be `cached_mesh != prim.mesh`; that is the whole dirty key. Store `(cached_ptr/hash, uploaded_range, last_version)` per primitive.
- **CP the CPU worker**: do a cheap **frustum cull on the per-primitive world AABB** (`prim.world`) before assembling faces. This is OCCT's BVH-cull essence applied at the primitive level ([S16], [S8]: "frustum culling will easily cut half the objects"); in an isometric-ish framed CAD view this typically culls meaningful fractions of a 1M-triassembly's primitives.
- Restrict the painter sort to the visible subset; when *only the camera moved* and no mesh is dirty, reuse the previous sorted order where the sort key order cannot change across a view change is not guaranteed, so still re-sort the visible subset — but **reuse allocation** (preallocated face arena, no per-frame `Vec` churn) so the cost is `O(visible)` instead of `O(all) * log(all)` plus 1M heap allocations.
- Optionally skip re-assemble entirely for **pure camera moves with the same visible-dirty set** is *not* correct (projection changes culling/order), so keep the reduction honest: per-frame work becomes "cull AABBs, project only survivors, sort survivors".
- Testable: a new `faces_submitted / total_triangles` assert (e.g. >= 50% culled in the framed test view), and upload-bytes-per-frame `<= 1 MB` at steady state.

### 2.3 Stage 3 — depth buffer for the opaque pass; transparent pass stays sorted

This is the stage that changes pixels, so it gates on re-verified GPU goldens / id parity (see 5). Two sub-options:

- **3a (recommended default): keep the CPU painter sort for *all* faces (i.e. do Stages 1-2 and stop).** Cost: the sort is still O(visible · log visible) per frame, but with resident geometry, culling and no allocations that is hundreds of microseconds, not hundreds of milliseconds. Keeps the byte-level CPU-parity story trivially intact and transparency working exactly as today. This alone meets the card bar at 1M tris for typical interactivity if `assemble()` ends up < ~8 ms (it is measurement, not assertion — verify).
- **3b: depth buffer + unordered opaque pass** (the published general-purpose answer [S27]).
  - `depth_stencil_attachment` = D32Float (reverse-Z, [S30]) or D24S8; opaque triangles drawn in any order with depth write/test on; this is `O(n)` GPU-side per-pixel ordering that is strictly more correct than per-face mean-depth painter sorting in self-intersecting/concave cases.
  - **What it costs for transparency**: the transparent set (alpha < 1.0 — with the global-alpha control that is all faces in the frame) now needs an *entity-level back-to-front* sort instead of the current per-face sort — an operation over `O(visible)` primitives, and per-transparent-object depth test **on**, depth write **off** ([S27], [S29]). That is *substantially cheaper* than today's per-face sort because blending correctness only needs primitive order, not face order, and opaque ordering is handled by depth.
  - **What it costs for the CPU-golden invariant**: the CPU rasterizer path (`CpuBackend`, `ISOLITH_GPU=off`, `CPU_DEMO_GOLDEN_HASH`, 21 sha256 proof frames, CPU determinism oracle) stays byte-identical — none of that is touched. The *GPU* surface changes to depth-resolved output; the existing contract is already "coverage within 5%, pixels NOT byte-equal, determinism re-proven on the GPU path" ([S33] / `tests/gpu_backend.rs` docs). So 3b requires a documented regolden of the GPU coverage numbers + re-running id-parity, not a CPU golden change.
  - Reverse-Z + `Depth32Float` is mandatory-ish at CAD depth ranges (1/z precision implosion, [S30]); pick `Vulkan/Depth32Float` and a reversed clip range; the projection change lives in the single `render::project_ndc`-adjacent formula (keep it in one place, UI-D8 discipline).
  - A depth prepass (render depth-only first) is optional here and pays off only when fragment cost is high; with flat-shaded single-pass CAD lit fragments it is low value at 1M tris. Revisit when shading gets expensive.
- Stage-gating note: 3a and 3b both *need* Stages 1-2; 3b additionally needs the transparency + id tests green (5.2).

### 2.4 Keep the CPU painter sort or move to a depth buffer — decision table

| Concern | Painter (current/3a) | Depth buffer (3b) |
|---|---|---|
| Correctness for intersecting/concave faces | approximate (per-face mean depth) | exact per-pixel |
| Transparent faces | trivial (global order is the sort) | opaque unordered; transparents sorted back-to-front, depthWrite off, depthTest on |
| CPU golden invariant (`ISOLITH_GPU=off`) | untouched | untouched (CPU path not modified) |
| GPU-vs-CPU pixel parity | strongest (same painter order) | looser (depth-resolved ≠ mean-depth painter; GPU covered-count band already allows this) |
| Every-frame CPU cost | O(visible·log visible) | O(visible) + tiny transparents sort |
| Id-buffer parity with `oep_viewer::pick` | keeps today's mapping | needs re-verification (topmost-fragment id matches analytic pick — expected equal, but assert it) |
| Main risk | CPU remains the bottleneck as tris grow | depth precision, z-fighting on thin CAD faces (mitigate reverse-Z), transparency ordering |

Recommendation: land 1→2→3a first (painter preserved, bytes/allocations/culling fixed, goldens untouched), and treat 3b as its own shippable card because it changes the GPU output contract and needs the transparency/id re-verification.

### 2.5 Picking and hover stay correct

- The id pass (`gpu/idbuffer.rs`, R32Uint) rasterizes *the same face set/order the colour pass draws* and reads back only on view change; `pick_at`/`pick_rect` derive from the grid [S34].
- **Under Stages 1-2** (no pixel change): nothing about picking changes.
- **Under 3b** (depth buffer): attach the same depth buffer to the id pass (id pass also writes depth), so the topmost-fragment primitive id equals what the colour pass resolved; re-run the `id pick == oep_viewer::pick` parity test at the 1600x1000 resolution, across reversed-Z, and for box-select rects. Hover re-queries the in-memory grid for free regardless; keep the readback-on-view-change policy (that policy is already the expensive-part avoidance) [S34].

### 2.6 Draw calls

- Today: 1 face draw + 1 line draw per pass. After the change the face pass can stay 1-2 draws for the whole scene (one resident index buffer + one instance table) — this is the OCCT "merge small objects into larger arrays" recipe applied wholesale [S6], and it makes the draw-call cost irrelevant at 1M tris. Use `MultiDrawIndirect`-style batching (wgpu `draw_indexed_indirect`) only when a future wave adds per-primitive culling/LOD on the GPU; at this scale CPU *submission* of ~1-2 dozen draws is trivially cheap [S9, S21]. Do not spawn one draw per `Primitive` on the CPU — that reintroduces the dispatch overhead the CAD references warn about [S6, S21].

### 2.7 Resident memory at 1M triangles (estimates with arithmetic; all <= cited limits)

- Per-vertex, unindexed, pos+normal f32x3x2 = 24 B: 3,007,584 verts x 24 B = **72.2 MB**.
- Per-vertex, unindexed, pos-only f32x3 = 12 B (flat normals derived in FS or once at upload): **36.1 MB**.
- Index buffer (if meshes share vertices): `u32` x 3 x tris = 12.0 MB, optional now.
- Per-primitive instance/colour/table: ~64-96 B x #primitives (negligible; e.g. 20k prims ~ 2 MB).
- Depth (1600x1000, D32Float): 6.4 MB; colour target 6.4 MB + readback; id target 6.4 MB (view-change only).
- **Total GPU-resident: ~90-100 MB.** That comfortably fits:
  - wgpu 30 default `Limits::default()`: `max_buffer_size` = **256 MiB** (one 72 MB buffer is fine; a single 120 MB one also was), `max_storage_buffer_binding_size` = **128 MiB** (a 72 MB storage buffer fits a single binding; split position/normal or split per-mesh across bindings if a single mesh ever exceeds it), `max_uniform_buffer_binding_size` = 64 KiB (the 64 B MVP is not close), `max_vertex_buffers` = **8** (the seam uses 1-2) [S22].
  - The device is created with `wgpu::DeviceDescriptor::default()` -> wgpu 30 `Limits::default()` applies (`device.rs`), so the numbers above are the enforced defaults, not wishes.
- Current design is roughly **120 MB uploaded/frame + ~200-300 MB CPU transient/frame**; target is ~90-100 MB resident total and near-zero steady-state upload.

### 2.8 Dirty-range re-uploads (keyed to mesh identity/version)

- Key = `(prim.id, version)` where version bumps on any of: `mesh` bytes changed, `transform` changed, `highlighted` flipped, base colour (part theme) changed. Upload **range = the changed primitive's buffer range only** via `queue.write_buffer(buffer, offset, bytes)`; everything else stays untouched. This is the standard retained/resident pattern in every cited engine [S2, S20, S23, S24].
- Colour/alpha: move base RGB+flags to the instance/colour table (re-upload only that primitive's row), keep alpha as a per-pass uniform -> the transparency slider moves with **zero re-upload and zero re-sort**.

---

## 3. Pitfalls the current design will hit, with evidence

1. **Depth precision implosion at CAD near/far ranges.** Depth is 1/z in the hardware ([S30]); a painter-sorted design avoids the question entirely, but the moment you add a depth buffer you must use reverse-Z + floating depth or thin CAD faces z-fight. Evidence: NVIDIA "Visualizing Depth Precision" [S30]; DLR comparison paper on depth buffer techniques for large scenes [S31].
2. **Translucent faces are order-dependent.** Blending needs back-to-front drawing even *with* a depth buffer; a depth buffer does not order blended fragments [S27, S28]; GPU-driven culling/sorting via atomics "has no stability at all for sorting" so GPU-sorted transparents are not done in the reference implementations [S8]. Current CPU painter sort *is* the transparency mechanism — any change must keep a correct transparent ordering (2.3) or it regresses the 50%-alpha behavior that is a committed acceptance number.
3. **Wireframe/edge-overlay z-fighting.** Edges drawn as separate line primitives in the same world space as the filled faces fight the faces when a depth buffer is added; published remedy is polygon offset / depth bias on the edges (or draw edges on top with depth test `LEQUAL` + small offset) [S35, S36]; OCCT renders immediate/highlighted content in a top Z layer *sharing the same depth buffer state* to avoid exactly this [S16].
4. **Per-frame buffer creation ~ 120 MB every frame.** `create_buffer_init` per frame allocates a fresh device buffer (and wgpu routes the upload through internal staging [S23]; real deployments have measured the staging path as a measurable chunk of frame time [S25]). The fix (persistent buffers + double buffering) is the headline of Section 2.
5. **wgpu default limits.** `max_buffer_size` 256 MiB, `max_vertex_buffers` 8, `max_storage_buffer_binding_size` 128 MiB, `max_uniform_buffer_binding_size` 64 KiB (defaults, active here) [S22]. A single >128 MiB storage binding or >8 vertex streams would trip them; the resident plan (2.7) stays inside them, but a naive "one giant interleaved full-attribute buffer > 128 MiB as one storage binding" would not.
6. **Unified-memory upload on an integrated GPU.** There is no discrete-PCIe staging bounce on an iGPU — shared RAM is `DEVICE_LOCAL|HOST_VISIBLE` on iGPUs [S2] — but that only removes the PCIe bounce; the CPU still spends DRAM/UNC bandwidth on a 120 MB memcpy plus creates staging allocations, and the measurements show the whole per-frame cost anyway. The point of Stage 1 is that a "fast copy" of 120 MB/frame is still 120 MB/frame of work that simply must not exist; a steady-state upload of a few KB (MVP + dirty rows) costs nothing.
7. **The offscreen readback itself.** `target::read_back` copies the whole 1600x1000 target + repacks into `RgbaImage` every frame (~6.4 MB + row padding) and then egui re-uploads it as a texture (another copy). Not in the original problem statement, but it is a recurring per-frame cost that survives every stage; the seam's end-to-end gating number should include it (measure it — `vertex_prep_time` does not capture the readback).
8. **Recomputing smooth normals and re-sorting the full set every frame** is O(all·log all + 3M f64 ops + ~1M allocations) — the measured 435 ms. Any design that keeps "walk all faces every frame" on the CPU (stage 0-1 without culling) fails the 16.6 ms bar; 2.2's culling + retained arena is what makes the CPU part survivable.
9. **Per-vertex colour is a 40% bandwidth/storage waste.** 16 of the 40 B per vertex encode something that varies per *primitive*; CAD flat-shading needs per-primitive, not per-vertex colour. Removing it is Stage 1's single biggest byte win (120 -> 72/36 MB).

---

## 4. What published practice says is NOT worth doing at this scale (avoid over-engineering)

- **Full GPU-driven meshlet pipeline / task+mesh shaders.** At 1M tris with flat-shaded instance-like parts, object/primitive-level culling ("frustum culling... easily cut half the objects" [S8]; OCCT merges instead [S6]) captures the win; meshlets (64-128 tri clusters, [S3]) and mesh/task shaders are the 10M-100M-triangle answer and carry real engineering + portability cost (wgpu mesh-shader support is new/limited). Keep meshlets in the back pocket for a "100M-tris" wave; do not build them now.
- **Order-Independent Transparency (per-pixel linked lists / weighted blending).** "significantly more expensive" [S8]; only needed when object-level back-to-front (2.3) is insufficient. Isolith has one global alpha and per-part flat shading; 2.3's sorted transparent pass is the published standard and is enough.
- **Occlusion-culling with a depth pyramid / Hi-Z** as a first step. Frustum culling at the primitive AABB already removes most invisible work [S8]; a depth pyramid adds a frame of latency and a 2nd render target for sub-mesh culling value that 1M-tris CAD does not earn yet. Defer to the same future wave as meshlets.
- **Ray-tracing pick, SSAO/deferred G-buffers, virtual texturing/streaming.** None are needed for flat-shaded CAD at this scale; the id-buffer + analytic `pick` parity path is cheaper and already correct.
- **A Coin3D-grade recursive retained scene graph with per-node caches.** Isolith's assembly is essentially a flat list of leaf instances; an instance/colour table keyed by `prim.id` + version (2.2) gives the retained benefits without a scene-traversal/render-cache system [S11, S12] that FreeCAD is itself trying to shed [S13, S14].

---

## 5. Verification plan

### 5.1 Must stay green in every stage (existing tests)

- CPU path byte-parity: `ISOLITH_GPU=off` renders the demo to `CPU_DEMO_GOLDEN_HASH = 12597229237775278722` and the 21 sha256'd proof frames; `CpuBackend` must remain literally untouched so its determinism oracle and goldens cannot drift. (`CpuBackend` edits are forbidden on these cards unless explicitly re-goldened — the brief assumes none.)
- GPU determinism oracle: two identical `(scene,camera,size)` renders hash identically (`tests/gpu_backend.rs`).
- GPU coverage vs CPU within `COVERAGE_TOLERANCE` (5%) at 800x600, demo + bracket fixtures (a documented band, not byte equality — see the test module docs).
- Id-buffer parity: `IdPass::pick_at`/`pick_rect` == analytic `oep_viewer::pick` for the same camera, at the current and expanded resolutions; ids map via `scene.primitives[i].id`.
- Transparency: the shaded pass's source-alpha write keeps the 50%-alpha body reading back alpha ~= 128 with rgb a true source-over composite; the UI-D2 acceptance prints before/after — must still print ~128/128 after every stage.
- Edge-primitive counts (`last_edge_primitives`) still equal the fixture boundary-edge list in Shaded/Edges modes.
- All CPU-backend render tests and the `oep_geo` golden bracket render keep passing.

### 5.2 New measurements that prove the improvement (add to the test/bench harness; commit thresholds per stage)

Each measurement should be a real counter exposed by `GpuBackend` beside the existing `vertex_prep_time()`:

1. **Upload bytes/frame**: sum of `create_buffer_init`/`write_buffer` sizes this frame. Before: ~120.3 MB. Stage 1 target: <= a few MB on camera move, `<= 64 KB` steady-state idle; Stage 2+: `~= 0 at steady state` (only MVP uniform + dirty rows).
2. **Frame time**: same 1,002,528-tri, 1600x1000, 20 warm-up / 120-frame protocol (so numbers are comparable to the committed 429.8/434.9 ms). Stage targets, all p95 <= 16.6 ms bar at stage 3a/3b; intermediate stages state their own reduced p95 (Stage 1 removes the biggest cost, so expect the largest single jump — verify by instrumentation, not assumption).
3. **CPU assemble wall time** and **faces-submitted / total-triangles ratio** (cull effectiveness): framed view must submit <= ~50% of tris (assert a floor), and assemble must be measured separately from upload so the 435 ms attribution is proven, not assumed.
4. **Draw-call count per frame**: stays tiny (<= ~10 in the new shape for a 20k-primitive scene, + indirect if added); assert it.
5. **Resident buffers**: after load, device-local bytes stable (allocation count constant across frames — no per-frame device-buffer allocations; this catches regressions back to `create_buffer_init`-per-frame).
6. **Id-buffer parity re-run** at the new resolution and, if Stage 3b lands, under reverse-Z depth (same ids as analytic pick).
7. **Transparency re-check** after Stage 3b specifically (sorted transparent pass + depthWrite off): 50% alpha readback still ~= 128, and no new bleed-through regression vs the CPU reference in the coverage band.
8. **Depth(s)**: if 3b lands, log near/far used and confirm reverse-Z D32Float; assert no z-fighting on the thinnest feature in the demo fixture at the farthest standard orbit.

Re-goldening rule: the CPU goldens never change in any stage; GPU coverage/alpha numbers are re-measured and any movement must be documented in the card's acceptance (they are bands, not byte hashes, already [S33]).

---

## 6. Numbers used (with derivation, so nothing looks harvested)

- 120.3 MB/frame upload: 1,002,528 * 3 * 40 B = 120,303,360 B. **Derived from code** (`ShadedVertex` = 40 B, unpacked & re-packable; `pack_shaded` capacity = len*40). 
- 12/24/40 B/vertex formats, 36.1/72.2/120.3 MB totals: **derived** (12 B = f32x3, 24 B = f32x3x2, 40 B = pos+normal+color).
- 6.4 MB targets at 1600x1000: cx*yz*B. **Derived.**
- 16.6 ms card bar, 429.8/434.9 ms measured, 1,002,528-tris fixture: **given in the card context** (this brief did not re-run the benchmark).
- wgpu limit defaults (256 MiB / 128 MiB / 64 KiB / 8 vertex buffers): **from wgpu 30.0.2 `Limits` doc** [S22] and confirmed the device is created with the default limits in `device.rs`.
- "~200-300 MB CPU transient per frame": **estimate** — 3M substituted f64 vertex copies (positions + normals, 24 B = ~72 MB) + 1M per-face `Vec`s + 1M per-face f64 position/normal heaps (~72 MB) + 120 MB packed bytes, on top of wgpu staging; not directly measured, labeled estimate.
- "p50/p95 of the whole stage" statements of the form "expect/`'estimate'`": **estimates**, to be replaced by the 5.2 measurements.

---

## 7. Source list

- **[S1]** Haar & Aaltonen, *GPU-Driven Rendering Pipelines* (Assassin's Creed Unity + RedLynx), Advances in Real-Time Rendering, SIGGRAPH 2015 — "a handful of indirect draw calls... hundreds of thousands of independent objects... at 60 FPS"; per-material instance batching; GPU culling. https://advances.realtimerendering.com/s2015/index.html (slides PDF: aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf)
- **[S2]** A. Kapoulkine (zeux.io), *Writing an efficient Vulkan renderer* (GPU Zen 2, free edition) — memory heaps: `DEVICE_LOCAL` for static geometry; iGPUs expose `DEVICE_LOCAL|HOST_VISIBLE` for static resources; suballocation, ~4096 allocation limit. https://zeux.io/2020/02/27/writing-an-efficient-vulkan-renderer/
- **[S3]** NVIDIA, *Introduction to Turing Mesh Shaders* — mesh shaders/task shaders as the programmable geometric pipeline; meshlets are the sub-mesh culling unit (64-128 tri clusters per modern culling posts). https://developer.nvidia.com/blog/introduction-turing-mesh-shaders/
- **[S5]** Unlimited 3D, *AIS optimization tricks – short list* — merge presentations, batch updates, "never redraw... after modification of an individual AIS object — redraw content only after all objects have been updated". https://unlimited3d.wordpress.com/2022/05/29/ais-optimization-tricks-short-list/
- **[S6]** OCCT Forum (best-approach-rendering-10000-identical-objects-instancing) — `AIS_ConnectedInteractive` does not reduce draw calls; merge small objects into one `Graphic3d_ArrayOfPrimitives` (VBO) to cut draw calls; hardware instancing needs a custom `OpenGl_Element` with an instance VBO (transform matrices) + `glDrawArraysInstanced`; BVH bounding-box per custom object for culling. https://dev.opencascade.org/content/best-approach-rendering-10000-identical-objects-instancing
- **[S7]** OCCT Forum (how-improve-rendering-performance-when-redrawing-v3dview-many-elements) — use the view performance counters, redraw-after-all-updates advice. https://dev.opencascade.org/content/how-improve-rendering-performance-when-redrawing-v3dview-many-elements
- **[S8]** Vulkan Guide (vkguide.dev), *Compute based culling* — compute cull of sphere bounds + atomic instance-append into draw-indirect; "frustum culling will easily cut half the objects"; depth-pyramid occlusion culling (1-frame latency); *"doing culling and sorting [transparent] at the same time is a really hard problem... GPU atomics... no stability for sorting"; "order-independent transparency... significantly more expensive"*. https://vkguide.dev/docs/gpudriven/compute_culling/
- **[S9]** AMD GPU Services (AGS), *Multi Draw Indirect* — "draw calls are issued on the GPU's command processor (CP), potentially saving the significant CPU overheads incurred by submitting the equivalent draw calls". https://gpuopen-librariesandsdks.github.io/ags/group__mdi.html
- **[S10]** Sascha Willems, Vulkan samples — `instancing` (one draw, N instances), `computecullandlod` (compute frustum cull + LOD + indirect draws), `indirectdraw` (two input states, indirect). https://github.com/SaschaWillems/Vulkan/tree/master/examples
- **[S11]** Coin3D — "Coin3D, like Open Inventor, is a C++ object-oriented retained mode 3D graphics API". https://en.wikipedia.org/wiki/Coin3D
- **[S12]** Open Inventor Mentor, *Optimizing Rendering* — "SoSeparator nodes build multiple render caches (by default, a maximum of two)"; settable `render caching`; remote-rendering caching. https://developer.openinventor.com/UserGuides/2023.1/Inventor_Mentor/Optimizing_Applications/Optimizing_Rendering/Optimizing_Rendering.html
- **[S13]** Coin rendering cache context + QueueElement — `SoGLCacheContextElement` document (cache context ids). https://coin3d.github.io/Coin/html/classSoGLCacheContextElement.html
- **[S14]** FreeCAD FPA grant proposal #35 *Modernizing FreeCAD's Rendering and Selection Systems* — OpenGL/Coin3D is "obsolete", modern Vulkan/wgpu replacement discussion. https://github.com/FreeCAD/FPA-grant-proposals/issues/35 ; Coin Vulkan "retained-IR dedup/perf" PR. https://github.com/brianmk/coin/pull/3
- **[S15]** SoGLRenderAction — "Inventor's auto-render-caching algorithm will choose to cache more often when rendering is remote". https://www.openinventor.com/reference-manuals/NewRefMan1030/RefManJava/com/openinventor/inventor/actions/SoGLRenderAction.html
- **[S16]** OCCT Wiki *visualization* — presentation computed once and retained as `Graphic3d_Structure`; viewer operates on the structure, not the data model; selection via 3-level BVH frustum traversal; Z-layer semantics ("Top... For rendering Immediate content interleaved with Persistent content (sharing common Depth buffer state)" — from OCCT visualization blog/wiki). https://github.com/Open-Cascade-SAS/OCCT/wiki/visualization
- **[S17]** OCCT docs, *Visualization* (user guide overview) — the AIS presentable-object / Graphic3d_Structure retained model; "the viewer operates on the graphic structure... and not on the data model of the application". https://dev.opencascade.org/doc/overview/html/occt_user_guides__visualization.html
- **[S18]** OCCT Forum, *Performance issues rendering with AIS_triangulation* — "You may configure Graphic3d_Buffer in whatever manner you want, as long as it is compatible with VBO... interleaved attributes". https://dev.opencascade.org/content/performance-issues-rendering-aistriangulation
- **[S19]** bevy render walkthrough — "what is accessed by the render world is not a Mesh, but a GpuMesh; the transformation is done at the extraction stage". https://github.com/bevyengine/bevy/discussions/9897
- **[S20]** pcwalton, bevy PR #16385 *Retain RenderMeshInstance / MeshInputUniform frame-to-frame* — "moves the front end of the rendering pipeline to a retained model when GPU preprocessing is in use". https://github.com/bevyengine/bevy/pull/16385
- **[S21]** Arm GPU Best Practices Developer Guide — draw-call batching: CPU dispatch time of many small draws is the cost even when GPU work is small. https://documentation-service.arm.com/static/67a62b17091bfc3e0a947695
- **[S22]** wgpu 30.0.2 `Limits` (docs.rs) — defaults: `max_buffer_size` 256 MiB, `max_storage_buffer_binding_size` 128 MiB, `max_uniform_buffer_binding_size` 64 KiB, `max_vertex_buffers` 8, `max_binding_array_elements_per_shader_stage` 500,000 when supported; requesting better-than-adapter panics. https://docs.rs/wgpu/30.0.2/wgpu/struct.Limits.html
- **[S23]** gfx-rs/wgpu discussion #1438, *How to properly write to a buffer every frame* — `write_buffer` uses internal staging management, "probably it can create new allocations". https://github.com/gfx-rs/wgpu/discussions/1438
- **[S24]** gpuweb discussion #2525 / #1428 — frames-in-flight handled by writeBuffer; double/triple-buffering strategy; per-frame UBO is "totally fine". https://github.com/gpuweb/gpuweb/discussions/2525
- **[S25]** gfx-rs/wgpu issue #10484 — `MAP_WRITE` staging path change moved a remesh upload from ~0.03 ms to 0.12-0.23 ms; also issue #9123 (172 write_buffer calls/frame, ~8.3 MB in a bevy scene). https://github.com/gfx-rs/wgpu/issues/10484
- **[S26]** three.js manual, *How to Draw Transparent Objects* — transparent objects render back-to-front, after opacables, with depthWrite effectively disabled; opaque first nearest-to-farthest "and the depth buffer sorts them out pixel by pixel". https://threejs.org/manual/pages/transparency.html
- **[S27]** LearnOpenGL, *Blending* — "Draw all opaque objects first. Sort all the transparent objects. Draw all the transparent objects in sorted order"; blending is order-dependent, depth buffer cannot order blended fragments. https://learnopengl.com/Advanced-OpenGL/Blending
- **[S28]** Filament issue #2838 + InterplayOfLight OIT series — transparent usage needs near-correct ordering; per-pixel linked-list OIT as the expensive per-pixel alternative. https://github.com/google/filament/issues/2838 ; https://interplayoflight.wordpress.com/2022/06/25/order-independent-transparency-part-1/
- **[S29]** three.js docs, `Material.depthWrite` / transparency notes — transparent pass keeps depth test, toggles depth write. https://threejs.org/docs/#api/en/materials/Material.depthWrite
- **[S30]** NVIDIA Developer Blog (Nathan Reed), *Visualizing Depth Precision* — 1/z nonlinearity; reverse-Z + float depth yields zero comparison errors in the reproduced simulation and removes sensitivity to far plane / composed matrices. https://developer.nvidia.com/blog/visualizing-depth-precision/
- **[S31]** DLR, *Comparison of Depth Buffer Techniques for Large and Detailed 3D Scenes* — reversed projection balances depth precision across the clip range. https://elib.dlr.de/187280/1/Comparison%20of%20Depth%20Buffer%20Techniques%20for%20Large%20and%20Detailed%203D%20Scenes.pdf
- **[S32]** Krupitskas, *Modern rendering culling techniques* — meshlet-level (64-128 tri) culling overview. https://krupitskas.com/posts/modern_culling_techniques/
- **[S33]** `crates/oep-viewer/tests/gpu_backend.rs` — `CPU_DEMO_GOLDEN_HASH`, coverage-within-5% contract, GPU determinism oracle, GPU-vs-CPU "pixels NOT byte-equal" contract.
- **[S34]** `crates/oep-viewer/src/gpu/idbuffer.rs` — R32Uint grid read back only on view change; pick_at/pick_rect parity with `oep_viewer::pick`.
- **[S35]** Khronos forum, *Wireframe (line draw) and Z-fighting* + StackOverflow D3D11 `DepthBias` — polygon offset/depth bias is the standard edge-over-face fix. https://forums.khronos.org/showthread.php/66101-Wireframe-line-draw-and-Z-Fighting
- **[S36]** StackOverflow, *Drawing wireframe over filled geometry / gl.POLYGON_OFFSET* — "use polygon offset so that the edges hover slightly above the polygon faces"; `glDepthFunc(GL_LEQUAL)` + offset for consistent overlay. https://stackoverflow.com/questions/30567044 ; https://stackoverflow.com/questions/17199817
