# P7-M3-T01: Solver decision primary-source evidence

Sources checked live on 2026-10-16 for the original adoption evidence and on
2026-10-18 for the SOLVER-REPLACE evaluation. This file backs
`solver-adr.md`.

## SolveSpace (the originally adopted engine), GPL-3.0

- Repo: https://github.com/solvespace/solvespace (master, 4.2k stars)
- License: GPL-3.0 (repo `COPYING.txt`)
- Command-line/library notes and the explicit GPL note:
  https://solvespace.github.io/solvespace-web/library.html ("SolveSpace is
  distributed under the GPLv3 ... for commercial support and licensing,
  please contact us")
- The solver library API (`slvs.h`) and its constraint catalog:
  https://github.com/solvespace/solvespace/blob/master/include/slvs.h and
  the pinned vendored copy that card SOLVER-REPLACE removed
- Adopted pin: upstream commit `8ef723e` on master (fetched 2026-10-16);
  source-identical vendored tree under
  `crates/oep-assem/third_party/solvespace-solver/` (upstream `slvs-solver`
  translation units + `src/slvs/lib.cpp`, plus the Eigen headers those units
  include, plus `include/slvs.h` and the license/attribution text). This tree
  is removed from the integrated and released tree per program decision #16
  and remains readable only in the unmerged branch history for provenance.

Adopter precedent: FreeCAD's Assembly3 workbench embeds the SolveSpace
solver for 3D assembly constraint solving (secondary evidence of the engine
being embedded headless at scale).

## License disposition (checked 2026-10-16 and 2026-10-18)

- SolveSpace upstream license page
  (https://solvespace.github.io/solvespace-web/library.html) still states the
  engine is distributed under the GPLv3 and invites commercial inquiries.
  The author's public position is that the current revision cannot be
  relicensed because the project dropped its contributor CLA and the codebase
  carries many contributors' changes. The commercial license is a possible
  negotiation, not a dependable path, per the v1.1.0 resolution.
- Program decision #16 (Don, 2026-10-18): replace the engine through the
  seam with a permissive engine. Card SOLVER-REPLACE implements it.

## solverang (primary permissive candidate, evaluated 2026-10-18)

- crates.io: https://crates.io/crates/solverang, version 0.1.0 (published
  2026-03-02), license Apache-2.0, description "Domain-agnostic numerical
  solver for nonlinear systems and least-squares problems". Download stats at
  evaluation: 28 total, 7 recent.
- Source inspected from the published crate: modules `assembly` (
  `RigidBody`, `UnitQuaternion`, `Mate`, `CoaxialAssembly`, `Insert`, `Gear`),
  `sketch2d`, `sketch3d` (`Point3D`, `LineSegment3D`, `Plane`, `Axis3D`;
  constraints `Distance3D`, `Coincident3D`, `Fixed3D`, `PointOnPlane`,
  `Coplanar`, `Parallel3D`, `Perpendicular3D`, `Coaxial`), plus a generic
  solver stack (`Solver`, `LMSolver`, `RobustSolver`, `SparseSolver`,
  `AutoSolver`).
- Evaluation against the required mate set (fixed, coincident, on-plane,
  plane-distance, parallel): the `assembly` module provides point coincidence
  (`Mate`) but no on-plane, plane-distance or parallel constraint; those
  exist in `sketch3d` but only for sketch entities (points, lines, planes),
  not rigid-body poses. Satisfying the fixture's vocabulary would require
  authoring the plane / plane-distance / parallel residuals and Jacobians as
  custom `Constraint` impls on top of solverang regardless.
- Default features pull nalgebra 0.34, a Levenberg-Marquardt crate, with
  optional sparse (faer), parallel (rayon) and JIT (cranelift) features.
- Verdict recorded in ADR 5.3: not integrated. The crate does not ship the
  required mate vocabulary and is immature (v0.1.0, single release, 28
  downloads); the card's "integrate only if it solves the fixture; maturity
  is the known open risk" therefore rules it out for now. Tracked as a
  revisit trigger in ADR section 9.

## KittyCAD solve-rs (rejected, dead)

- Historical repo https://github.com/KittyCAD/solve-rs returns HTTP 404
  today (verified via GitHub API `GET /repos/KittyCAD/solve-rs` -> "Not
  Found", 2026-10-16). The project was superseded by the 2D solver ezpz.
- No usable published Rust package: the only crates.io crate named `solve`
  is 0.0.1 from 2021 and is unrelated (a rustyline/clap demo wrapper around
  `solve_lib`).

## KittyCAD ezpz (rejected for assembly, watched for the sketcher)

- Repo: https://github.com/KittyCAD/ezpz, license MIT, last push 2026-08-05
- Self-description: "This is a 2D constraint solver" (README); crates:
  `ezpz`, `kcl-ezpz`, `ezpz-cli`, `ezpz-wasm`. 2D only, so it cannot host
  3D rigid placement solving. It is a strong candidate for the P7-M4
  sketcher lane (pure Rust, MIT, maintained).

## PlaneGCS (FreeCAD sketcher solver, rejected for assembly)

- In-tree: https://github.com/FreeCAD/FreeCAD/blob/main/src/Mod/Sketcher/App/planegcs with LGPL-2.1-or-later headers
- Standalone extractions: https://github.com/CadQuery/PlaneGCS (LGPL-2.1) and
  https://github.com/marcuswu/libplanegcs
- 2D sketcher solver; cannot solve 3D rigid placements. Candidate for the
  P7-M4 sketcher lane. P7-M1 already flagged it as adoptable standalone.

## rust_slvs `slvs` crate (rejected as the integration route)

- Repo: https://github.com/thekakkun/rust_slvs, GPL-3.0, last push 2023-07-05
- crates.io: `slvs` 0.6.0 (2023-06-20), bindgen build-dependency
- Verified failure on the program toolchain (2026-10-16): bindgen 0.64 under
  libclang 18 silently drops the Slvs_* typedefs, yielding empty bindings
  (`E0432 unresolved imports Slvs_hEntity/Slvs_hGroup`), so the crate does
  not compile unmodified.

## The own bounded solver (the implemented route)

- Lane: `crates/oep-kinematics/src/solver.rs` (P7-M3-T02), a dependency-free
  damped Gauss-Newton least-squares solver (finite-difference Jacobian,
  normal equations, Gaussian elimination, Levenberg damping, rank-based DOF),
  verified against analytic ground truth by the P7-M3-T02 demo.
- oep-assem builds a bounded pose system over moving instances (position +
  unit quaternion) and delegates the numerical solve to this lane through the
  `adopted` seam. No third-party solver dependency is added.
