# Interactive editing surface (the mouse-grade editor)

**Task:** INTERACTIVE-SHELL (editor spine, decision #19): turn oep-shell +
oep-viewer into a drivable, mouse-grade editor so an engineer can do their
main job with the program.
**Lane:** workstreams/modeler/editor (oep-shell editor session + oep-viewer
viewport + oep-sketch solver/variable set + oep-geo tessellation seam).
**Status:** landed on wave/editor, gates green.

## What the acceptance asked

On a clean U1 (plus a second machine via the P10/P12 rehearsal pattern) a
user sketches a rectangle, constrains it (fixed, horizontal, vertical, length
and distance via the variable set), solves it, extrudes (and revolves) it,
changes a dimension in the variable set, SEES the solid update, and exports
STEP. A headless smoke test drives the same path.

1. Viewport: orbit/pan/zoom, selection with highlighting, visual fidelity for
   solids and edges from the real model.
2. Sketch in the viewport: points, line segments, the oep-sketch constraint
   set driven by the variable set.
3. Feature creation from sketch: extrude and revolve; direct dimension entry
   through the variable set; live recompute on edit so the solid follows the
   parameter (drive lane semantics).
4. Canonical model is real: every shown state is the actual oep-geo document,
   never a fake; undo/redo affordance over the feature graph.
5. The CLI/agent lane and the keyboard-first accessibility contract do not
   regress: the CLI gains an interactive sibling sharing the same command
   registry (commands.rs single source, help index drift-guarded).

## What exists now

The editor surface sits on four existing pillars: the oep-geo canonical model
+ kernel seam (KERNEL-BASE), the oep-sketch solver + variable set (P7-M4),
the oep-viewer camera/render/pick (P6-M5), and the oep-shell command surface
(P7-M2/M5). This card joins them:

- **Deterministic sweep tessellation** (`crates/oep-geo/src/tessellate.rs`):
  `extrude_profile` and `revolve_profile` turn a closed planar profile and a
  sweep amount into a watertight triangle mesh with listed boundary edges,
  using an ear-clipping polygon filler (std-only, no new deps). The mesh is
  EXACTLY the solid swept by the polygonal profile: `mesh_volume` equals the
  analytic value (e.g. an 80x40 rectangle by 10 -> 32000, an L shape -> its
  exact area x depth), and a full-turn revolved ring equals the faceted
  annular volume. This is the same profile + parameters the OCCT-backed
  kernel seam ([`oep_geo::kernel::solid_extrude` /
  `solid_revolve`]) consumes, so the shown solid cross-checks the kernel's
  closed-form goldens (the "canonical model is real" contract).

- **Real-solid viewport** (oep-viewer, additive): `Primitive` gained an
  optional `MeshData` (world-space triangle mesh + boundary edges) and a
  `highlighted` flag. The renderer draws mesh triangles through the same
  deterministic painter pipeline as box proxies, overlays edge wires, and
  tints the selected primitive with the theme highlight color. The box-proxy
  path and every pre-existing viewer contract test are untouched. Camera
  gained `pan` alongside the existing `orbit`/`zoom` and `Camera::framed`.

- **EditorSession** (`crates/oep-shell/src/editor.rs`): the interactive
  surface. It owns the canonical oep-geo document (part-1 / body-1 / Sketch
  feature / VariableSet feature / Extrude or Revolve feature), the derived
  solid mesh, the orbit camera, the selection, and a bounded real undo/redo
  history over document snapshots:
  - sketch: `point`, `line`, `constrain` (fixed, horizontal, vertical,
    coincident, length, distance; a dimension value that is not a number
    references the variable set by name), `solve` (writes solved coordinates
    back into the canonical sketch);
  - variable set: `set_var` triggers live recompute when a solid feature
    exists; the report carries the volume before and after;
  - features: `extrude` / `revolve` (a dimension value or a variable name;
    when a variable name is used, the feature stores the reference and
    re-resolves it on recompute, so the swept depth/angle follows the
    variable), `recompute` re-solves + re-derives;
  - viewport: `orbit`, `zoom`, `pan`, `frame`, `select_at` (screen-space
    pick through the camera), `render` to PPM or PNG with a self-contained
    SHA-256 (two renders prove the model moved);
  - export: `export_step` writes the derived solid as an AP214 faceted STEP
    through the oep-geo exporter; the exported bytes re-import through the
    importer hub (locked in the acceptance test);
  - history: `undo` / `redo` restore the prior/next canonical document and
    re-derive the solid over the real feature graph.

- **CLI/agent lane unchanged in spirit, grown in coverage.** The `workbench`
  example dispatches the full editor surface (`editor:open`, `sk:point`,
  `sk:line`, `sk:constrain`, `sk:solve`, `sk:render`, `sk:props`, `vars:list`,
  `vars:set`, `feat:extrude`, `feat:revolve`, `feat:recompute`, `view:frame`,
  `view:orbit`, `view:zoom`, `view:pan`, `view:pick`, `export:step`, `props`,
  `undo`, `redo`). Every command is registered in the single-source command
  metadata (`crates/oep-shell/src/commands.rs`, version 1.2.0), the committed
  help index is regenerated from it, and both drift-guard tests (Rust +
  Python) cover every command. Accessibility checklist widened to WB-A11Y-10.

## Honesty contract

- Every on-screen and exported solid is derived deterministically from the
  actual oep-geo model by `oep_geo::tessellate`, never fabricated: the same
  profile + parameters the kernel consumes, with the same closed-form
  results. The acceptance test drives the whole path against the real binary
  and asserts volume 32000 -> 48000 on the variable edit, bbox updates, and
  byte-different renders.
- A failing sketch solve, a broken loop, a non-finite dimension, or an export
  of a not-yet-derived solid is a typed error, never a panic and never an
  invented value.
- Undo/redo restore the actual prior document; the session re-derives the
  solid from it rather than replaying a pretend pose.

## Verification

- Rust: `crates/oep-geo/src/tessellate.rs` (10 unit tests incl. concave L),
  `crates/oep-viewer/tests/mesh_scene.rs` (5), `crates/oep-shell/tests/
  editor_surface.rs` (5, the full acceptance), plus the existing suites
  (oep-shell, oep-viewer, oep-geo, oep-sketch) all green.
- Python: `tests/test_interactive_shell_acceptance.py` drives the compiled
  workbench CLI (the headless smoke) and cross-checks the committed demo;
  `tests/test_p7m5t02_a11y_help.py` regenerated to the 1.2.0 command set.
- Commit demo: `demos/editor-sketch-extrude/` (trace.txt, manifest.json,
  edpart.step) reproduces the analytics live (frames/ regenerated).
- Two-machine rehearsal: `scripts/editor-headless-smoke.sh` drives machine A
  (U1) and, when reachable, machine B (default keelwb2 on U1;
  MACHINE_B_HOST/PORT override) per the P10/P12 pattern; `--selfcheck` is the
  CI-safe mode.
