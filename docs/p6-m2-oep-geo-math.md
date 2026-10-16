# P6-M2-T01: Vector / Matrix / Quaternion Math Core for `oep-geo`

Status: implemented and property-tested (Keelworks wave 1, P6).

## Scope

A self-contained, vendored, std-only `f64` math core for the OEP geometry
workstream: `Vec3`, `Mat3`, `Mat4`, `Quat` (unit quaternion), plus a typed
non-panicking error model (`MathError`). This is the foundation P6-M2-T02
(NURBS evaluation), P6-M2-T03 (geometric predicates) and the P6-M4 exporters
build on.

## Decision: vendored, not `nalgebra`

The card allows "`nalgebra` or vendored". Vendored was chosen:

- Zero new dependencies. `nalgebra` drags a wide tree (`approx`, `num-traits`,
  `wide`, ...); `proptest` for property tests adds another. Keeping the core
  std-only means no `Cargo.toml`/`Cargo.lock` churn, which simultaneously
  (a) avoids a merge collision with the concurrently-running P6-M1-T01 card
  (which adds `serde`/`serde_json` to this same crate) and (b) removes the
  crates.io fetch risk on CI that the sibling P6-M1 plan flagged (403s).
- Clean-room posture (P0-M2/M3 doctrine): the most central numeric code is our
  own, auditable line-by-line, and integrates directly with the P6-M1-T01
  instance `transform: [f64;16]` contract.
- Property tests are written with a deterministic seeded PRNG (xorshift64*)
  inside the integration test — real property testing over thousands of random
  cases without a `proptest` dependency. Reproducible (fixed seeds) and std-only.
- Tradeoff acknowledged: `nalgebra` is more extensively battle-tested; our
  kernels are small, standard formulas (Rodrigues, Hamilton product, cofactor
  inverse) and the property suite + golden files + independent Python
  verification cover the surface.

## Layout convention (binding for P6 consumers)

- `Mat4` stored **column-major** `[f64;16]`, entry `(r,c)` at `c*4+r`;
  translation in the last column (`[12], [13], [14]`), matching the P6-M1-T01
  instance `transform: [f64;16]` contract exactly (`to_array16`/`from_array16`
  are the interchange points).
- `Mat3` stored **column-major** `[f64;9]`, entry `(r,c)` at `c*3+r`.
- `Quat` components `(w, x, y, z)`.
- All scalars `f64` (P6-M1-T03: double everywhere). No `f32` in this core.
- Right-hand rule for axis-angle rotations (`from_axis_angle(axis, angle)`).
  The axis is normalized internally; a zero axis is a typed error.
- P2's `oep-cgr` keeps its own f32 row-major `Transform`; cross-crate
  conversion is deliberately out of scope here and belongs to the P6-M4-T02
  importer hub (layout divergence documented, not reconciled, per `docs/layout.md`).

## API

| Type | Members |
| --- | --- |
| `Vec3` | `new`, `zero`, `add`, `sub`, `neg`, `mul`, `div`, `dot`, `cross`, `norm`, `norm_squared`, `normalize`, `distance_to`, `lerp` |
| `Mat3` | `identity`, `zero`, `from_cols`, `get`, `transpose`, `det`, `mul`, `mul_vec3`, `inverse`, `from_axis_angle` |
| `Mat4` | `identity`, `zero`, `from_translation`, `from_rotation`, `from_scale`, `from_axis_angle`, `get`, `mul`, `transform_point`, `transform_vector`, `transpose`, `det`, `inverse`, `is_finite`, `to_array16`, `from_array16` |
| `Quat` | `identity`, `new`, `from_axis_angle`, `from_rotation_matrix` (Shepperd), `mul` (Hamilton), `conjugate`, `norm`, `normalize`, `inverse`, `rotate_vector`, `to_matrix3`, `to_axis_angle`, `dot`, `slerp` (shortest path) |
| `MathError` | `InsufficientNorm`, `DegenerateAxis`, `NonFiniteInput`, `SingularMatrix` |

All fallible operations return `Result<_, MathError>` — nothing panics on
degenerate input. The crate keeps `#![deny(missing_docs)]`.

## Verification

- **Two-sided golden:** `crates/oep-geo/tests/golden/math-golden.json` is
  produced by the crate itself (`examples/gen_math_golden.rs`), then verified
  twice independently:
  1. `crates/oep-geo/tests/math_contract.rs` asserts closed-form expectations
     (90 deg rotation about Z maps (1,0,0)->(0,1,0); translation composition
     T(1,2,3) then T(4,5,6) of the origin -> (5,7,9); column-major array16
     orthonormality and zero translation).
  2. `tests/test_oep_geo_math_acceptance.py` re-computes the same figures in
     Python stdlib (Rodrigues rotation, orthonormal columns, translation
     composition) and cross-checks within 1e-9. Python does not share the
     implementation, so a self-consistent-but-wrong crate cannot pass.
- **Property tests** (`math_contract.rs`, deterministic xorshift64* seeds,
  thousands of iterations each): rotation length preservation (Mat4 + Quat),
  quaternion inverse round-trip, quaternion multiplication = composition
  (group homomorphism), quaternion row-major vs matrix equivalence,
  Mat3 associativity, Mat4 inverse recovers identity, array16 format
  round-trip on random affine matrices, translation composition, Vec3 dot/cross
  identities, normalize unit-length. Fixed seeds keep every run reproducible.
- The property suite already caught two real bugs during development: a zero
  axis returning the wrong error variant and the Mat4 inverse transposing the
  cofactor matrix (adjugate bug). Both were fixed; the tests that caught them
  are part of the committed suite.

## File map

- `crates/oep-geo/src/math.rs` — implementation (also holds small unit tests)
- `crates/oep-geo/src/lib.rs` — adds `pub mod math;`
- `crates/oep-geo/tests/math_contract.rs` — acceptance contract + property tests
- `crates/oep-geo/tests/golden/math-golden.json` — committed golden artifact
- `crates/oep-geo/examples/gen_math_golden.rs` — golden generator (crate-authoritative)
- `tests/test_oep_geo_math_acceptance.py` — independent Python verification
- this file

## Linkage

- P6-M1-T03 (units/accuracy double-everywhere): this core is the reference f64
  implementation for that discipline.
- P6-M2-T02 NURBS: curve/surface evaluation will consume `Vec3`/`Mat4` and the
  error model.
- P6-M2-T03 predicates: on/off tolerance, distance, containment build directly
  on `Vec3` (+ future `Plane`/`Ray` helpers in the same module).
- P6-M4-T02 importer hub: converts P2 f32 row-major / external transform
  records into this core's `Mat4` column-major format.
