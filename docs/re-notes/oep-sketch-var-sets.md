# Sketcher + parametric variable sets in the chosen shell (P7-M4-T01)

Version 1.0, 2026-10-16. Keelworks / Open Engineering Platform (OEP), P7
modeler lane. Chosen shell: our own thin Rust shell around Open CASCADE
(P7-M1-T01). FreeCAD is the behavioral reference, NOT the shell: this card
implements sketcher and variable-set semantics in the chosen shell's own
model, behaviorally matching FreeCAD v1 variable-set semantics without
importing FreeCAD (clean-room invariant).

Reference implementation: `crates/oep-sketch` (Rust, serde). Solver decision:
`workstreams/P7-M4/sketch-solver-adr.md` (adopt an OSS 2D GCS behind a seam; do not
rebuild, do not import FreeCAD). Real-material gate:
`crates/oep-sketch/tests/golden/bracket-sketch-drive.oepgeo.json`, byte-locked
by `crates/oep-sketch/tests/p7m4_sketch_acceptance.rs` and independently
re-parsed by `tests/test_p7m4t01_sketch_acceptance.py`.

## 1. The acceptance (this card's bar)

"a sketch parametrically drives a model dimension; repeats for the chosen
shell's semantics." Concretely, driven on real solver output:

1. A constrained 2D sketch (rectangle: Fixed + Horizontal + Vertical +
   Length constraints) converges from a sloppy start (residual < 1e-9, DOF 0).
2. A dimension constraint references a VARIABLE-SET ENTRY BY NAME (`width`),
   not a literal: solved line length == variable value.
3. Editing the variable (width 80 -> 120) re-solves and the solved dimension
   follows to 120. The sketch is parametrically driven by the variable set.
4. The solved dimension drives a MODEL dimension: an `Extrude`-style model
   feature parameter and the solid body bbox take the solved value, and follow
   the edit (body width 80 -> 120).
5. The sketch + variable set round-trip as `oep-geo` FEATURES (kinds `Sketch`
   / `VariableSet`) in a canonical document: validate() clean, parse-back
   identical, byte-identical serialize. This is the chosen shell's semantics.
6. No FreeCAD anywhere: the public surface is our own types; the OSS solver is
   an implementation detail of `crate::solve` (mirror-tested).

## 2. The model

- `Sketch` (crate::sketch): points, line segments, and constraints
  (Fixed/Horizontal/Vertical/Coincident + dimensional Length/Distance). A
  dimensional constraint's target is a `DimExpr`: `Literal(f64)` or
  `Variable(name)`  -  the FreeCAD-v1-variable-set semantics.
- `VariableSet` (crate::varset): an ordered `name -> f64` collection, edited
  at the model level; solve resolves `DimExpr::Variable(name)` against it.
- `solve_sketch` (crate::solve): the ADOPTED OSS 2D constraint solver behind
  the seam. Fresh numeric model per solve; returns `SolveOutcome` with solved
  points, solved line lengths, and `dimensions` keyed by the VARIABLE name
  (so a driven model dimension is addressed by `"width"`, not a line id).
- `drive` (crate::drive): `drive_sketch_to_document` solves + materializes a
  canonical `oep-geo` Document  -  a part, a driven solid body whose width
  equals the solved dimension, and the Sketch + VariableSet features in the
  part's history. `body_width(doc, part_id)` reads the driven model dimension.
- `features` (crate::features): sketch <-> oep-geo Feature and VariableSet <->
  oep-geo Feature, deterministic (kinds `Sketch` / `VariableSet`).

## 3. Key decisions

- ADOPT an OSS solver, do not rebuild, no FreeCAD module source copied:
  `workstreams/P7-M4/sketch-solver-adr.md` (solver = `vcad-kernel-constraints` v0.10.0,
  MIT, behind the seam; escape hatches and revisit gates documented).
- The dimension is the variable: `SolveOutcome.dimensions` is keyed by
  variable name so downstream consumers (P7-M4-T02 rules/checks sandbox) read
  `"width": 80.0`, never solver-internal state.

## 4. Fidelity / honesty notes

- Solver output is measured and converged (residual ~1e-13 on the acceptance
  rectangle), but the LM solver keeps ~1e-13-scale noise (the golden body
  width serializes as 80.0000000000002). The tolerance model (P6-M1-T03) is
  the authority for when a "match" counts; this card asserts within 1e-6.
- The sketcher surface is deliberately small: one closure shape (points +
  lines), five constraint behaviors under the seam, two dimensional kinds.
  arcs/circles/tangent and the full FreeCAD constraint vocabulary are later
  sketcher cards; this milestone proves the parametric-variable-set machinery
  on a real material gate, not a feature-for-feature port (no CATIA/FreeCAD
  parity as a goal  -  program invariant).

## 5. Run it

- `cargo test -p oep-sketch`  -  unit + acceptance (solver-backed, byte-locked
  golden).
- `python3 tests/test_p7m4t01_sketch_acceptance.py`  -  independent stdlib
  re-parse of the committed golden.
- `python3 tests/test_p7m4t01_solver_adr.py`  -  the solver ADR decision gate.
- Regenerate the golden deliberately:
  `cargo run -p oep-sketch --example gen_sketch_golden`
