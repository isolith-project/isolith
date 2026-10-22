# Assembly editing in the editor surface (live solver)

**Task:** EDITOR-ASSEM-EDIT (editor spine, decision #19): make the assembly
solver interactive and part of the editor surface.
**Lane:** workstreams/modeler/assembly (oep-shell assembly surface + oep-assem
solver seam + oep-kinematics motion lane).
**Status:** landed on wave/editor, gates green.

## What the acceptance asked

1. Create, edit, delete mates in the viewport: fixed, coincident, on-plane,
   plane-distance, parallel, per the oep-assem Mate vocabulary.
2. Live solve feedback: a mate edit re-solves through the vendor-neutral seam
   and the instances follow immediately; diverging solves return a typed
   error, never a frozen or fabricated pose.
3. Mechanism motion through the kinematics lane (P7-M3-T02 damped
   Gauss-Newton): animate a simple mechanism from its mate set.

Acceptance: a 3-part assembly with mates solves interactively in the surface,
accepts a mate edit, and the mechanism motion plays deterministically for a
simple crank or slider (committed fixture).

## What exists now

The assembly solver was already delivered (oep-assem workbench over the
oep-kinematics damped Gauss-Newton lane, SOLVER-REPLACE decision #16). This
card wires it into the editor surface:

- **Committed 3-part fixture** (base plate, cap, peg):
  `crates/oep-assem/tests/golden/three-part-fixture.oepgeo.json` plus its mate
  set `three-part-fixture.constraints.json`. The fixture opens deliberately
  misplaced and tilted; the solver must place cap at (36, 18, 10) and peg at
  (18, 22, 10), both upright, with the base fixed. Matches the proven 2-part
  fixture pattern at one more part of complexity.

- **AssemblySession** (`crates/oep-shell/src/assem.rs`): the surface holds the
  current oep-geo document + oep-assem constraint set and journals every edit
  and solve. Public commands cover the full mate vocabulary and are driven by
  both the CLI (agent/automation lane) and Rust integration tests:
  `assem:open`, `assem:instances`, `assem:mates`, `assem:mate-add`,
  `assem:mate-edit`, `assem:mate-del`, `assem:solve`, `assem:render`,
  `assem:animate`. All commands are registered in the generated help surface
  (P7-M5-T02) and drift-guarded.

- **Live solve semantics.** Every mate create/edit/delete re-solves through the
  vendor-neutral seam and the report prints immediately. A convergent solve
  writes the placements back; a diverging solve returns a typed error
  (`solver-failed` mapped from the workbench error) and leaves the document's
  placements byte-identical to the last converged solve. The bad mate is kept
  in the set so an engineer can fix or delete it (standard CAD behavior), and
  the typed error is never a fabricated or stale pose. Malformed edits
  (unknown anchor, out-of-range index) are typed rejections that do not
  mutate the set.

- **Viewport.** A deterministic CPU raster renders the current pose as body
  bounding-box proxies to PPM (`assem:render`); each render is content-hashed
  with a self-contained SHA-256 (`crates/oep-shell/src/sha256.rs`, locked to
  FIPS 180-4 vectors), so "the instances follow" is provable from two
  consecutive renders. GPU-less and std-only: the interactive acceptance runs
  on the CI container.

- **Mechanism motion.** `assem:animate:FRAMES:OUT_DIR` drives the kinematics
  lane (P7-M3-T02 damped Gauss-Newton) through a full crank turn of the
  committed crank-slider mechanism built from its joint set, writing
  `trace.json` (per-frame poses), `manifest.json` and rendered PPM frames.
  Determinism: two recordings produce byte-identical traces. Physical gate:
  piston stroke 60 mm (2 x crank radius), piston on the groove line, one free
  DOF undriven, closed-loop residual and analytic error machine-tight.

## Verification

- Cargo: `cargo test -p oep-assem --test three_part_fixture`,
  `cargo test -p oep-shell --test assembly_surface`, help-index drift guard.
- Python: `tests/test_editor_assem_edit.py` drives the workbench CLI and
  re-verifies every acceptance line independently (stdlib only).
- The fixture is byte-locked: the shell embeds the same golden files the
  oep-assem crate tests, and a test re-checks the path wiring.

## Known limits (honest)

- Rendering is bounding-box proxy + PPM, not a polished interactive viewport;
  the mouse-grade editor surface is the INTERACTIVE-SHELL lane.
- The solver-only mates (fixed, coincident, on-plane, plane-distance,
  parallel) are static-position constraints; revolute/prismatic motion lives
  on the kinematics lane and drives the mechanism animation.
