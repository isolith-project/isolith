# P7-M3-T02: Mates / Kinematics Demo (own DMU-like semantics)

Card: P7-M3-T02, Wave-2 core, P7 modeler lane.
Status: delivered, recorded demo + clean-room invariant verified.

## What this is

A clean-room implementation and **recorded demo** of DMU-like assembly
semantics on top of the `oep-geo` canonical model. It is an **own
implementation with no vendor libraries**: the constraint model, the numeric
solver, the kinematics driver and the renderer are all written for this card.

Demarcation (important): per the ratified P7-M1-T01 decision record, the
**production** assembly constraint solver is ADOPTED as OSS by P7-M3-T01
("adopt, do not rebuild"). P7-M3-T02 is scoped differently by its card: a
**semantics wedge and recorded demo** whose code must contain zero vendor
bytes. The own solver here is a dependency-free reference that makes the mate
semantics executable, testable and recordable now, behind the same
`oep-kinematics` seam P7-M3-T01's adopted solver will later swap in.

## Fixture

An **inline crank-slider** (crank + connecting rod + piston) mounting a real
`oep-geo` corpus model (the committed golden bracket) as its fixed base:

- pivot O = (50, 50, 8) mm, crank length R = 30 mm, rod length L = 70 mm
- joints: revolute (crank->ground, driven command), revolute (rod->crank),
  revolute (piston->rod), prismatic (piston->ground groove along X at y=50)
- a closed kinematic loop; 1 DOF when the crank is free, 0 when driven
- analytic (closed-form) solution exists and is used as ground truth

## Semantics implemented (`crates/oep-kinematics`)

- `mate.rs`   -- Revolute, Prismatic, Spherical, Rigid joints with residual
                 evaluation (DMU-like vocabulary, our own formulation).
- `solver.rs` -- own damped Gauss-Newton least-squares solver
                 (finite-difference Jacobian, normal equations, Gaussian
                 elimination, Levenberg damping, rank-based free-DOF report).
- `mechanism.rs` -- Mechanism over `oep-geo` instance ids + transforms,
                 driver overrides (a commanded crank pose), verify_residual.
- `pose.rs`   -- Rigid pose + Rodrigues rotation-vector math, oep-geo
                 Transform interop.
- `raster.rs` -- own dependency-free software renderer (z-buffer, PPM P6)
                 for the recorded animation.
- `demo.rs`   -- fixture builder, analytic ground truth, per-frame run,
                 trace writer, scene builder.

## Results (verified, recorded)

| gate | value | criterion |
| --- | --- | --- |
| frames recorded | 90 (one full crank turn) | 90 PPM + trace.json + GIF/MP4 |
| worst closed-loop residual | 3.5e-11 | < 1e-6 |
| worst translation vs analytic | 7.4e-11 mm | < 1e-4 mm |
| worst orientation vs analytic | 1.1e-12 rad | < 1e-6 rad |
| piston stroke | 60.0 mm = 2R | exact |
| groove constraint (y,z const) | yes, 1e-12 | hold |
| rod length (closed loop) | 70.0 mm | < 1e-3 |
| mechanism free DOF (undriven) | 1 | planar 1-DOF loop |
| clean-room invariant | holds | see below |

The demo output lives at `demos/p7-m3-t02/` in the PR branch:
`trace.json`, `manifest.json`, `crank-slider-demo.mp4` (264k),
`crank-slider-demo.gif` (226k), and `frames/*.ppm` (generated, git-ignored).
Reproduce with `scripts/p7-m3-t02-record.sh`.

## Clean-room invariant (no vendor bytes)

Enforced by an executable test (`crates/oep-kinematics/tests/cleanroom.rs` and
the pytest mirror `tests/test_p7_m3_t02_acceptance.py`):

- runtime deps of `oep-kinematics` are exactly `oep-geo` (path dep); the only
  third-party crates anywhere in its graph are the `serde` family already used
  by `oep-geo` (general serialization, not a CAD/kinematics stack)
- no source line (code, not docs) names OCCT/FreeCAD/CGAL/nalgebra etc.
- no prebuilt artifacts (DLL/SO/DYLIB/LIB/OBJ/A/O/PDB/RLIB) and no vendored
  third-party directory inside the crate
- the demo animation is rendered by our own rasterizer (std-only); ffmpeg is
  used only on the recording host to package PPM frames

## Real material

The demo instantiates the committed real-corpus golden oep-geo model
(`example-bracket.oepgeo.json`: bracket part + fastener set) as the fixed base,
embedded byte-identical into `crates/oep-kinematics/tests/golden/` (a test
asserts the sha256 matches the oep-geo golden). Instances, transforms and the
document id space all validate against `Document::validate`.

## Files

- `crates/oep-kinematics/*` -- the crate (lib + bin + tests)
- `tests/test_p7_m3_t02_acceptance.py` -- independent pytest acceptance
- `docs/p7-m3-t02-mates-kinematics-demo.md` -- this file
- `P7-M3/mates-kinematics-demo-memo.md` -- decision-style memo (front matter)
- `scripts/p7-m3-t02-record.sh` -- reproduce the recording
- `demos/p7-m3-t02/{trace.json,manifest.json,crank-slider-demo.{mp4,gif}}` -- recorded demo

## Notes

Display meshes are visualization-only approximations from body bounding boxes
(oep-geo bodies carry only a bbox at this program wave); the kinematics act
purely on instance transforms, so rendering never affects mechanics.
