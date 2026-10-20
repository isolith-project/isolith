# P6-M2-T03: Geometric Predicates (On/Off Tolerance, Distance, Containment) for `oep-geo`

Status: implemented and fuzz-verified (Keelworks wave 1, P6).

## Scope

A self-contained, std-only, `f64` geometric-predicate module for `oep-geo`
(`crates/oep-geo/src/predicates.rs`):

- **On/off tolerance** — is a point "on" a plane, line, segment, or sphere shell
  within an explicit tolerance (`on_plane`, `on_line`, `on_segment`, `on_sphere`).
- **Distance** — point-to-point, point-to-line, point-to-segment (clamped
  projection), point-to-plane (signed and unsigned).
- **Containment** — point in an axis-aligned box, point in a sphere, point in a
  planar triangle (with a coplanarity gate so a point floating above a face is
  not "in" it).

## Decision: `[f64;3]` points, not `math::Vec3`

The P6-M2-T01 math branch (which owns `Vec3`) is unmerged. Consuming `Vec3` here
would make this card depend on that branch's merge; instead this module is fully
self-contained with its own tiny vector helpers, exactly like the sibling
P6-M2-T02 NURBS card did with `[f64;3]`. The two point types are reconciled at
the P6-M4-T02 importer hub (documented in `docs/layout.md` conventions). A later
card may re-express the public API in terms of `Vec3` without changing math.

## Decision: std-only, explicit tolerances, total functions

- No external crate, no `Cargo.toml`/`Cargo.lock` change. This removes the
  crates.io fetch risk on the offline CI host and avoids a merge collision with
  the concurrently-running P6-M1-T01 card (which adds `serde` to this same crate).
- Every predicate takes an explicit `tol: f64` (no hidden global epsilon).
  Semantics are **closed**: "on"/"in" includes the boundary, `d <= tol`.
  A negative tolerance is treated as zero.
- All scalars `f64` (P6-M1-T03: double everywhere).
- Non-panicking, total: booleans return `false` and distances return `NaN` on
  non-finite input; degenerate primitives degrade to documented values
  (zero-length segment/line -> point distance; zero/near-zero plane normal ->
  `NaN` signed distance and `on_plane == false`; degenerate/collinear triangle
  -> `false`).

## API

| Function | Meaning |
| --- | --- |
| `distance_point_point(a, b)` | Euclidean distance |
| `distance_point_line(p, a, b)` | |cross(b-a, p-a)| / |b-a| (infinite line) |
| `distance_point_segment(p, a, b)` | clamped projection onto segment `[a,b]` |
| `distance_point_plane(p, q, n)` | \|signed distance to plane (q,n)\| |
| `signed_distance_point_plane(p, q, n)` | dot(p-q, n)/|n|; sign = side |
| `on_plane(p, q, n, tol)` | \|signed dist\| <= tol |
| `on_line(p, a, b, tol)` | distance to line <= tol |
| `on_segment(p, a, b, tol)` | distance to segment <= tol |
| `on_sphere(p, c, r, tol)` | \|dist(p,c) - r\| <= tol |
| `in_aabb(p, min, max, tol)` | every coord in [min_i - tol, max_i + tol] |
| `in_sphere(p, c, r, tol)` | dist(p,c) <= r + tol |
| `in_triangle_plane(p, a, b, c, tol)` | coplanarity gate + half-plane sign tests |

The point type is `pub type Point = [f64; 3];`.

## Verification

- **Two-sided golden.** `crates/oep-geo/tests/golden/predicates-golden.json`
  (25 cases) is produced by the crate itself
  (`examples/gen_predicates_golden.rs`), then verified twice independently:
  1. `crates/oep-geo/tests/predicates_contract.rs` asserts closed-form analytic
     expectations (exact 3-4-5 distances, `1/sqrt(3)` plane distance, tolerance
     bucket boundaries, containment of exact centroids / lifted points).
  2. `tests/test_oep_geo_predicates_acceptance.py` recomputes every case with an
     independent Python stdlib implementation using different formulas
     (cross-product area from vectors, clamped projection, half-plane sign
     tests) and cross-checks within `1e-6`.
- **Fuzz vs reference.** The Rust contract suite draws seeded random inputs
  (deterministic xorshift64*, fixed seeds, thousands of iterations) and compares
  the module against an independent reference implementation in the test file:
  - point-to-line and point-to-segment distances vs a **golden-section numeric
    minimizer** along the support line (a genuinely different algorithm, not the
    module's closed-form projection);
  - signed plane distance vs an explicit-normalization reference, plus
    `on_plane <=> (|signed| <= tol)` and `|signed| == unsigned` consistency;
  - `on_line` / `on_segment` vs the distance-threshold characterization;
  - `in_sphere <=> dist <= r+tol`, `on_sphere <=> |dist-r| <= tol`,
    `on_sphere => in_sphere`;
  - `in_aabb` vs the coordinate-bounds reference;
  - `in_triangle_plane` vs an independent half-plane-sign reference (different
    code path from the module) and constructed inside/outside convex-combination
    points.
  - point-point symmetry and the degenerate/non-finite never-panic contract.
- The property suite caught two real defects during development (both in the
  test harness, not the module): an inverted golden-section ratio (0.618 vs
  1.618) exploding the minimizer, and a self-bug where the "outside" probe
  landed on a triangle vertex. Both fixed; the maximized value was a deliberate
  guard against silently-passing fuzz tests.

## File map

- `crates/oep-geo/src/predicates.rs` — implementation (+ `pub mod predicates;`
  in `lib.rs`)
- `crates/oep-geo/tests/predicates_contract.rs` — acceptance contract + fuzz-vs-reference
- `crates/oep-geo/tests/golden/predicates-golden.json` — committed golden artifact
- `crates/oep-geo/examples/gen_predicates_golden.rs` — golden generator
- `tests/test_oep_geo_predicates_acceptance.py` — independent Python verification
- this file

## Linkage

- P6-M1-T03 (units/accuracy): f64 everywhere, tolerance documented per predicate.
- P6-M2-T01 math core: predicates currently stand on `[f64;3]`; reconciled at the
  P6-M4-T02 importer hub.
- P6-M2-T02 NURBS: curve/surface evaluation uses the same `[f64;3]` convention;
  these predicates (point-to-curve distance, containment) are the natural
  consumers.
- When the P6 B-Rep ops land (P6-M3), point-on-boundary / containment tests are
  the primitive used by ray casting and class-based solidity checks.
