---
title: Keelworks Sketcher Constraint-Solver Decision (adopt an OSS 2D GCS behind a seam; do not rebuild, do not import FreeCAD)
decision: adopt_oss_2d_sketch_constraint_solver_behind_the_solver_seam
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks program governance, P7 modeler lane (P7-M4-T01)
applies_to: the P7-M4 sketcher + parametric variable sets work (P7-M4-T01), the oep-sketch crate, and the P7-M4-T02 rules/checks sandbox which consumes solved sketch dimensions
supersedes: nothing (first P7 sketcher solver record; reads the P7-M1 open modeler shell decision as its parent)
---

# Keelworks Sketcher Constraint-Solver Decision

The P7-M1 open-modeler-shell decision binds this card twice: the shell is our
own thin Rust shell around Open CASCADE (FreeCAD was rejected as the shell),
and the sketcher "stores its sketch and variable sets as oep-geo features; the
adopted solver solves them; behaviorally they match FreeCAD v1 variable-set
semantics without importing FreeCAD." This record picks the sketcher's 2D
geometric constraint solver: ADOPT an existing OSS library behind a clean seam,
do NOT rebuild a solver, and do NOT copy FreeCAD's PlaneGCS source. The
P7-M3-T01 assembly-lane solver decision is a sibling record owned by that card;
this record covers the 2D sketch solver only and is agnostic to the 3D choice.

## 1. Decision

`oep-sketch` ADOPTS the pure-Rust OSS 2D geometric constraint solver crate
`vcad-kernel-constraints` v0.10.0 (MIT) as its solver backend, reached ONLY
through `crate::solve` (the solver seam). Concretely:

1. The solver is a hidden implementation detail. `crate::solve::solve_sketch`
   and `SolveOutcome` are our own types; no `vcad_*` type, entity id, or
   constraint object appears on the `oep-sketch` public surface. This mirrors
   how `oep-step` wraps the OSS STEP library behind a vendor-neutral API.
2. The sketcher model (`crate::sketch::Sketch`, constraints, `DimExpr`) and
   variable sets (`crate::varset::VariableSet`) are our OWN declarative types.
   A length constraint references a variable-set entry by name
   (`DimExpr::Variable("width")`), the FreeCAD-v1 variable-set semantics: the
   dimension is the variable, editing the variable re-solves, and the solved
   value drives the model dimension. The solver is only asked "satisfy these
   residuals with these target values"; all parametric intent lives in our
   model, not the library.
3. Adoption discipline: version-pinned (`=0.10.0`) per P0-M3-T02; license
   posture per the P7-M1 shell record (permissive library linked behind the
   seam with notice; no FreeCAD-module source copied  -  clean-room invariant).
4. Escape hatch: if the solver misses a future sketcher gate (new constraint
   family, large-fidelity requirement, or a license reassessment), the seam
   lets us swap the backend (e.g. the standalone PlaneGCS line) without
   touching the shell surface.

## 2. Why this decision

Five facts drive it.

1. The program pre-decided "adopt an OSS solver lib, do not rebuild"
   (P7-M3-T01 wording, restated in the P7-M1 shell record section 1.3). A 2D
   GCS is a mature problem class with maintained OSS solutions; rebuilding it
   spends agent-hours we can already name (P7-M1 accounting of the sketcher UX
   cost) and duplicates tested code.
2. FreeCAD is the behavioral reference, not the engine. The card wording ("like
   FreeCAD v1 variable sets") is a semantics target; FreeCAD's own PlaneGCS is
   LGPL C++ and lives inside the FreeCAD module tree, so linking or adapting it
   would either add a C++ build to the Rust-first core or run a "read
   FreeCAD's GCS and rewrite it" path. Both are worse than adopting a
   standalone permissive Rust GCS; the clean-room invariant also forbids making
   our solver a copy of PlaneGCS.
3. The adopted library is a real, converging 2D GCS with the constraint
   families this milestone needs: points, lines, arcs/circles, and the
   fixed/coincident/horizontal/vertical/parallel/perpendicular/equal/tangent
   geometric family plus the distance/length/radius/angle dimensional family,
   solved by Levenberg-Marquardt with DOF reporting. Verified on this card's
   real material: an 80x40 rectangle with a sloppy start converges to
   residual ~1e-13, and a variable edit (width 80 -> 120) re-converges to
   120 (`cargo test -p oep-sketch`).
4. It is pure Rust and modest in size. Its dependency stack is entirely
   permissive (MIT / MIT-OR-Apache / Zlib): the vcad-kernel-sketch family,
   tang, tang-expr, tang-la, robust, slotmap, thiserror. No C++ toolchain step
   enters the workspace, keeping the shell build single-language and the
   supply-chain posture clean.
5. The seam is cheap to keep honest. Because the solver is one module behind
   our own types, the "no FreeCAD, no OCC types on the shell surface" program
   rule is structurally enforced (mirror test:
   `oss_solver_stays_behind_the_seam_no_freecad`), and a backend swap is a
   one-module change if this library proves insufficient at a later gate.

## 3. Context

- The P7-M1 shell decision section 5.4 pre-decides solver ADOPTION and names
  both "the PlaneGCS-style standalone LGPL line" and "a permissive Rust solver
  chosen at P7-M3-T01/M4-T01" as the two acceptable families; this record is
  the P7-M4-T01 half of that choice (the 2D sketch solver). The P7-M3-T01
  assembly card owns the 3D/assembly solver decision separately; if that card
  later picks the same library the ecosystem only gets stronger, and the two
  records remain independent.
- The importer hub and workbench already set the wrapping pattern this card
  follows: `oep-step` (P4-M1 ADR) wraps an OSS library behind our own API and
  maps onto the one canonical oep-geo model. The sketcher is the P7 mirror:
  our own sketch/varset model mapped onto oep-geo features (kinds `Sketch` /
  `VariableSet`), solved by the adopted library behind the seam.
- The oep-geo model keeps its dependency-free status unchanged; `oep-sketch`
  depends on `oep-geo` (for the feature mapping), never the reverse.

## 4. Evidence

- Adopted solver: `vcad-kernel-constraints` v0.10.0 (crates.io), MIT; pure
  Rust 2D GCS (points/lines/arcs/circles; geometric + dimensional constraint
  families; Levenberg-Marquardt with DOF/over-constraint reporting). Checked
  live 2026-10-16 (crates.io metadata + local registry source).
- Dependency licenses (local registry, checked 2026-10-16): vcad-kernel-sketch
  / -geom / -math / -topo / -primitives MIT, tang/tang-expr/tang-la MIT,
  robust MIT-OR-Apache-2.0, slotmap Zlib, thiserror MIT. All permissive; no
  GPL/AGPL component enters the library build.
- Real material: `crates/oep-sketch/tests/p7m4_sketch_acceptance.rs` drives an
  80x40 rectangle from a sloppy start to residual ~1e-13 (DOF 0), re-solves a
  variable edit to 120, and drives an oep-geo model body width from the solved
  dimension. Measured, not asserted-without-running.
- Program records: P7-M1 open modeler shell decision (solver adoption
  pre-decided, seam rule, license posture), P4-M1 STEP library ADR (the OSS
  wrapping pattern this record follows), P0-M3-T02 toolchain pinning, P6-M1
  oep-geo schema/our-own-format.

## 5. Options considered

1. ADOPT `vcad-kernel-constraints` (permissive Rust GCS) behind the seam -
   ADOPTED, for the reasons in section 2.
2. ADOPT the standalone PlaneGCS line (LGPL-2.1-or-later C++/Python) - not
   today. It is the historically strongest open GCS and FreeCAD's own, but it
   is C++ (breaks the Rust-first core build for the shoelace of a 2D solver),
   not publish-ready as a pure Rust target, and its license, while acceptable
   behind the seam, is heavier than the MIT stack we adopt. Kept as the
   documented escape hatch if a future constraint family exceeds the adopted
   library.
3. WRITE our own 2D GCS - REJECTED. Violates the pre-decided "adopt, do not
   rebuild" (P7-M3-T01), re-implements a tested numerical problem, and spends
   the exact agent-hours the P7-M1 record says belong to the sketcher UX
   ergonomics instead.
4. Adopt a different pure-Rust GCS crate (e.g. arael-sketch-solver, also MIT,
   with expression support) - deferred. The card's semantics target (variable
   sets) is satisfied in our model layer without a library-side expression
   engine; the adopted crate's smaller surface and constraint coverage matched
   this milestone's needs. Revisit at a later sketcher gate if expression
   dimensions must run inside the solver.

## 6. Enforcement in the plan

- Every sketcher constraint added later maps through `crate::solve`; the seam
  denial is a `#![deny(missing_docs)]`-adjacent structural rule enforced by
  the seam mirror test (no solver type on the public surface).
- The `vcad-kernel-constraints` pin is `=0.10.0`; any bump is a deliberate
  toolchain-PR with a re-run of the acceptance test on real material.
- The P7-M4-T02 rules/checks sandbox reads solved sketch `dimensions` (by
  variable name) from `SolveOutcome`, never from the solver or from FreeCAD.
- This record is revisited at each P7 milestone review (per P7-M1 section 11);
  a material change of solver is a version bump with a change-log line.

## 7. Change log

- 1.0.0 (2026-10-16): initial ratification for P7-M4-T01. Adopt the pure-Rust
  OSS 2D constraint solver `vcad-kernel-constraints` v0.10.0 (MIT) behind the
  `crate::solve` seam for the oep-sketch sketcher, storing sketches and
  variable sets as oep-geo features, matching FreeCAD v1 variable-set
  semantics without importing FreeCAD; document the escape hatches and revisit
  gates. Verified live on an 80x40 rectangle + variable edit (P7-M4-T01
  acceptance test).
