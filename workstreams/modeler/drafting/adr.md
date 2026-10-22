# Drafting workbench: ADR / decisions (EDITOR-2D-DRAFTING)

Status: ACCEPTED, 2026-10-22. Task EDITOR-2D-DRAFTING (t_49eeb2e7).
Program: Isolith / Open Engineering Platform. Editor spine (decision #19).

## 1. The drawing record is a Class 2 (model-extend) `oep-geo` extension

The scope register listed drafting as cost class 1 (additive, new crates that
do not touch the canonical model); decision #19 pulled drafting into the
editor spine and the task instructed "versioned oep-geo extension (Class 2)
if the model schema is extended; bump the schema version with migration per
the P6-M1-T03 discipline." The drawing record (sheets / views / dimensions /
title block) becomes a **sixth class** in the canonical `Document`, because a
drawing's dimensions must reference the model's `oep-sketch` VariableSet by
name inside ONE file (FreeCAD-v1 behavior, PDM-safe). Schema bumps
`oep-geo` v1 -> v2; migration is tolerant and forward (v1 loads, promotes on
parse via `Document::migrate`; committed v1 goldens migrated through the
canonical serializer). Rejected alternative: a standalone drawing format in a
new crate — it would split a drawing from the parameter set that drives it
and duplicate PDM/round-trip plumbing for no benefit.

## 2. Every view is computed from the model — no fabricated output

The drafting workbench replays the `oep-geo` feature graph into a solid mesh
(`oep-drafting::solid::SolidEvaluator`, the mini feature-tree vocabulary) and
projects the mesh's TRUE edges onto each standard view plane
(`oep-drafting::view`). Projecting triangle boundaries would leak per-face
triangulation diagonals (not model edges), so the mesh carries its true edge
list; a generic future mesh without one falls back to triangle boundaries.
No view geometry is stored in the drawing record, and a non-converged sketch,
a missing variable or an unknown feature kind returns a typed error rather
than fabricating output.

## 3. Dimensions are driven by the SAME parameter that drives the model

A `Dimension` stores a reference to a variable-set entry BY NAME. At render
time the drawn value resolves from the model's VariableSet feature (the
FreeCAD-v1 semantics) and is cross-checked against the measured projected
extent; an edit to the variable re-solves, re-drives the model, and the
redrawn dimension follows (acceptance: width 80 -> 120). The variable-set
entry, not the solver's estimate, is authoritative for the model dimension —
the parameter IS the dimension, and the geometry cross-check keeps the two
honest.

## 4. First- and third-angle are a sheet arrangement, not a projection change

The projected CONTENT of a view is identical under both conventions; the
convention decides how the views are ARRANGED on the sheet (third-angle: top
above front, right to the right; first-angle: folded the other way). The
`Sheet.projection` field is the per-document setting.

## 5. Export is vector documents; STEP carries the 3D model, never a sheet

PDF **or equivalent vector output** is the task's export bar: SVG + PDF over
one deterministic primitive model, with a shared Helvetica metric table so
text centres identically in both. STEP has no 2D sheet concept; the STEP lane
exports/imports the 3D model (faceted B-Rep, honest `StepOpts`). DXF (the
traditional fab-shop 2D interchange) is a documented future lane behind the
`export::Prim` seam. Honest story: `docs/drafting-step-2d-story.md`.

## Open items (out of scope for this card)

- Hidden-line removal (drawings currently project all true edges).
- Section views, auxiliary/iso views, more title-block templates.
- DXF writer on the `export::Prim` seam.
- SDNF/variant note: none.
