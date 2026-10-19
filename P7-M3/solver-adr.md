---
version: 1.2.0
status: accepted (supersedes the v1.1.0 replacement decision record)
date: 2026-10-18
owner: keelworks-default
decision: replace-solvespace-through-the-seam-with-a-permissive-engine (implemented)
applies_to: P7-M3-T01 (assembly workbench + constraint solver); card SOLVER-REPLACE
supersedes: version 1.1.0 of this record (replace-solvespace-through-the-seam)
---

# P7-M3-T01: Assembly constraint solver ADR

## 1. Summary

The OEP assembly workbench uses the program's **own bounded constraint
solver** behind a vendor-neutral seam in the `oep-assem` crate. The engine is
the damped Gauss-Newton least-squares lane written in `oep-kinematics`
(P7-M3-T02), the ADR-designated fallback route, exercised end to end by a
real 2-part fixture that assembles and constrains through the workbench,
verified against real oep-geo models.

There is no third-party solver library in the crate: no SolveSpace, no GPL-3.0
tree, no non-permissive dependency. Card SOLVER-REPLACE removed the vendored
engine and put the own bounded solver behind the unchanged seam.

Decision key: `replace-solvespace-through-the-seam-with-a-permissive-engine`.

## 2. Context

`oep-geo` (P6-M1-T01) models assemblies as instances of parts, each carrying a
4x4 transform plus id references to bodies with bounding boxes. There is no
face geometry yet (B-Rep faces land in P5). P7-M3-T01 requires: an assembly
workbench built from that model, a constraint solver decision that adopts an
OSS solver library (do not rebuild, per the then-current P7-M1 card
constraint), an ADR on the solver choice, and a real 2-part fixture that
assembles and constrains through the workbench.

Assembly constraint solving here is the nonlinear parameter problem of finding
placements (the moving instances' rigid poses) that satisfy the mate set.

## 3. Candidates and evidence

| Candidate | License | Language / route | 3D rigid assembly | Verdict |
| -- | -- | -- | -- | -- |
| SolveSpace engine | GPL-3.0 | C++, `slvs.h` C lib (vendored) | yes | ADOPT v1.0.0, REMOVED by v1.1.0+ |
| own bounded solver (oep-kinematics GN lane) | OEP-owned | Rust | yes (bounded pose) | ADOPT v1.2.0 |
| solverang (crates.io 0.1.0) | Apache-2.0 | Rust | partial (assembly module) | REJECT for now; see 5.3 |
| KittyCAD solve-rs | MIT/Apache dual | Rust port of SolveSpace | yes (ported) | REJECT: repo deleted 2026 (HTTP 404) |
| KittyCAD ezpz | MIT | Rust, maintained | no (2D only) | REJECT for assembly; WATCH for the P7-M4 sketcher |
| PlaneGCS (FreeCAD sketcher GCS) | LGPL-2.1+ | C++ | no (2D sketcher) | REJECT for assembly |
| rust_slvs `slvs` crate | GPL-3.0 | Rust binding via bindgen | yes | REJECT: bindgen fragility, stale, GPL |

Full evidence: `P7-M3/solver-sources.md`.

## 4. Decision history

- **1.0.0 (2026-10-16)**: adopt the SolveSpace geometric constraint solver,
  vendored source-unmodified at `crates/oep-assem/third_party/solvespace-solver/`
  (pin `8ef723e`), hand-written FFI, vendor-neutral seam (`adopted.rs`). The
  "do not rebuild" rule came from the P7-M1-T01 card context.
- **1.1.0 (2026-10-18, program decision #16)**: replace the engine through the
  same seam. The upstream commercial license is not reliably available; a
  separate-GPL-module split does not survive binary linking; GPL-3.0 in a
  delivered artifact fails defense procurement review on an ITAR-clean product.
  The vendored tree must not be ported into the integrated tree.
- **1.2.0 (this record, card SOLVER-REPLACE)**: the replacement is implemented.
  `solverang` was evaluated first per the v1.1.0 plan and not integrated; the
  own bounded solver (the v1.1.0 fallback, reusing the oep-kinematics damped
  Gauss-Newton lane) is what sits behind the seam today.

## 5. License consequence, resolution and implementation

### 5.1 Why GPL-3.0 was unacceptable

The vendored SolveSpace engine is GPL-3.0. Any distributed artifact that links
the `oep-assem` solver integration must be GPL-3.0, conflicting with the
program's default Apache-2.0-for-applications posture, and GPL-3.0 in a
delivered artifact routinely fails defense procurement legal review on an
ITAR-clean product.

### 5.2 v1.1.0 resolution

REPLACE the engine through the same vendor-neutral seam. The vendored tree
stays readable in the unmerged branch's history for provenance; it is removed
from the integrated and released tree. Records why the upstream commercial
license is not a dependable path and why a distinct-GPL-module split is weak.

### 5.3 solverang evaluation (2026-10-18, live crates.io + source inspection)

- `solverang` 0.1.0 exists on crates.io (published 2026-03-02), license
  Apache-2.0, self-described as "domain-agnostic numerical solver for
  nonlinear systems and least-squares". V1.1.0 named it the primary permissive
  candidate for the required mate set; its `assembly` module provides
  `RigidBody` (position + quaternion), `UnitQuaternion`, `Mate` (point
  coincidence), `CoaxialAssembly`, `Insert`, `Gear`.
- Gap against the required mate set (fixed, coincident, on-plane,
  plane-distance, parallel): the `assembly` module has NO on-plane,
  plane-distance or parallel constraint. The `sketch3d` module has
  `PointOnPlane`, `Parallel3D`, `Distance3D`, but over sketch entities
  (points, lines, planes), not rigid bodies, so it cannot host rigid pose
  solving. Satisfying the fixture's vocabulary would mean authoring the
  plane / plane-distance / parallel residuals and their Jacobians as custom
  `Constraint` impls on top of solverang regardless.
- Maturity: v0.1.0, single release, 28 downloads, 7 recent at evaluation; its
  default dependency surface pulls nalgebra and a Levenberg-Marquardt crate,
  with optional sparse / parallel / JIT features. The card's rule is
  "integrate only if it solves the fixture; maturity is the known open risk."
- Verdict: not integrated. It does not ship the required mate vocabulary, so
  the bespoke constraint code would be ours anyway, and tying the fixture's
  behavior and DOF semantics to a 0.1.0 crate introduces exactly the open
  risk the card flags. Recorded here so the evaluation is a tested fact.

### 5.4 Implemented engine (v1.2.0)

The fallback route from v1.1.0 is what ships:

- `crates/oep-assem` builds a bounded least-squares system over the moving
  instances' rigid poses: per moving instance, unknowns `[x, y, z, qw, qx,
  qy, qz]` (position + unit quaternion). Mates become residual rows
  (coincident: 3 rows point difference; on-plane / plane-distance: 1 row
  signed plane equation; parallel: 3 rows cross product). A unit-quaternion
  row per moving instance keeps the pose bounded.
- The seam (`crates/oep-assem/src/adopted.rs`) delegates the numerical solve
  to `oep-kinematics::solver` (P7-M3-T02): damped Gauss-Newton, finite-
  difference Jacobian, normal equations, Levenberg damping, rank-based free
  DOF. This is the program's own lane, dependency-free except oep-geo, so the
  assembly crate adds no third-party solver dependency.
- `ffi.rs`, `build.rs`, `alloc/` and the vendored `third_party/` tree are
  gone. `crates/oep-assem/Cargo.toml` depends only on `oep-geo`,
  `oep-kinematics`, `serde`, `serde_json`.
- A license clean-room gate (`crates/oep-assem/tests/license_cleanroom.rs`)
  asserts: the dependency set is exactly the allowed list, no workspace tree
  names the removed engines or a `third_party/` directory, and `Cargo.lock`
  contains no GPL engine and no GPL marker. This is the acceptance's "a
  license check asserts the absence" gate and it runs as a normal cargo test.

## 6. Verified behavior of the replaced engine (measured 2026-10-18)

- 2-part fixture: cap starts mis-placed (28, 14, 20, tilted 25 deg about X);
  the solve drives it to (36, 18, 10) upright, every measured mate residual
  below 1e-9, the written-back document canonical, serialization returns a
  valid oep-geo document.
- Partially constrained solve (z + x fixed, y free) succeeds with free DOF
  >= 1, x and z exact, measured distance residuals below 1e-9.
- Real oep-geo bracket model: structure reads through the workbench; a real
  coincidence constraint on real fasteners solves with measured residual
  below 1e-9; sub-assembly instances surface the documented limitation.
- Solver status semantics keep the workbench contract: a converged solve with
  redundant residual rows reports RedundantOkay; both Okay and RedundantOkay
  count as success, as the workbench has always treated them.
- `cargo test -p oep-assem`: 16 unit + 4 assembly-fixture + 4 real-bracket
  + 3 license-gate green (the P7-M3-T01 acceptance passes unchanged in
  behavior). `cargo fmt` and `cargo clippy` clean; the program surface rule
  (no em dashes or curly quotes) is respected in the crate.

## 7. Workbench model and honest v1 limitations

- Moving instances are modeled by their rigid POSE: a pose point (the anchor
  at the part origin) and a pose normal (a +Z-axis anchor). Mates constrain
  that pose; the part stays rigid because its pose is the only thing solved.
- Rigid offset anchors (mate references a moving anchor whose origin is not
  the part origin) and planes on moving parts are NOT in v1. The workbench
  rejects them with explicit errors; they land with real B-Rep faces in P5.
- Orientation mates require Z-axis anchors (a documented convention).
- Sub-assembly instances are rejected with an explicit error; nested product
  structure lands with the P5 product-structure work.

## 8. Acceptance evidence

- ADR + `solver-sources.md` (this file and its evidence).
- `crates/oep-assem`: own bounded solver behind the seam (`adopted.rs`), no
  FFI, no vendored tree, dependency set gated by the license clean-room test.
- 2-part fixture: `tests/golden/two-part-fixture.oepgeo.json` (a canonical
  real oep-geo document) + `two-part-fixture.constraints.json`. Solving it
  through the workbench returns success with 1 free DOF (roll), places the
  cap at (36, 18, 10) with every measured mate residual below 1e-9, and
  writes back a valid oep-geo document.
- Verified against real oep-geo models: the P6 golden `example-bracket`
  document loads through the workbench; instances, derived anchors and real
  placements read back exactly; a real coincidence constraint on real
  fasteners solves with measured residual below 1e-9 and the document stays
  canonical.
- License: `cargo test -p oep-assem --test license_cleanroom` passes; no
  GPL-3.0 tree exists in the integrated tree and no GPL engine exists in
  `Cargo.lock`.

## 9. Revisit triggers and escape hatches

- If a permissive (MIT/Apache) maintained 3D solver reaches assembly-grade
  capability with the required mate vocabulary, re-evaluate the license and
  integration cost before the first public release. The seam is unchanged, so
  such a swap touches only `adopted.rs` and the residual construction in
  `workbench.rs`.
- If solverang's assembly module grows on-plane / plane-distance / parallel
  constraints and its maturity improves, re-run the fixture suite through it
  behind a feature flag before committing the crate.
- If the angle/perpendicular semantics needed for `Perpendicular` land in the
  geometry engine pass, extend the mate vocabulary and re-run the fixture
  suite; the ADR version bumps per the living-document rule.

## 10. References

- `P7-M3/solver-sources.md` (live sources, checked 2026-10-16 and
  2026-10-18)
- `crates/oep-assem/src/adopted.rs` (seam), `src/workbench.rs` (solve path)
- `crates/oep-kinematics/src/solver.rs` (damped Gauss-Newton lane, P7-M3-T02)
- P7-M1 `open-modeler-shell-eval-memo.md`
- `crates/oep-assem/tests/assembly_fixture.rs`, `tests/real_bracket.rs`,
  `tests/license_cleanroom.rs`

## Change log

- 1.0.0 (2026-10-16) initial ADR; SolveSpace adoption recorded on card
  t_e258572a.
- 1.1.0 (2026-10-18) solver disposition resolved: replace the SolveSpace
  engine through the seam (program decision #16). Names `solverang`
  (Apache-2.0) as the primary permissive candidate with an own bounded solver
  as fallback; bars the vendored GPL-3.0 tree from the integrated and
  released tree.
- 1.2.0 (2026-10-18) card SOLVER-REPLACE implemented: records the solverang
  evaluation verdict (not integrated), the own bounded solver behind the seam
  (oep-kinematics damped Gauss-Newton lane), the removed `ffi.rs`/vendored
  tree, and the license clean-room gate. Update while this record is the
  card's own test.
