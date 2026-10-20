# P6-M2-T02: NURBS / B-spline Math Integration (oep-geo)

Status: implemented, GREEN on real material (Isolith wave 1, P6).
Card: P6-M2-T02  -  "NURBS/B-spline math integration (OCC bindings or Rust
NURBS crate); curve/surface evaluation tests."
Bump: none (first P6-M2-T02 record).

## What this is

A self-contained NURBS / B-spline math module for the OEP geometry core:
`crates/oep-geo/src/nurbs.rs`. It provides B-spline basis functions (Cox-de
Boor), a non-rational B-spline curve, a rational NURBS curve, and a rational
tensor-product NURBS surface, with evaluation and (curve) first-derivative
support. Everything runs on `f64`, std-only, and returns typed `NurbsError`s
instead of panicking.

## Decision: own vendored math, not OCC, not an external crate

The card offers "OCC bindings or Rust NURBS crate". This delivery implements
the Rust path as our own auditable, vendored module. Reasons:

1. The build host is offline from crates.io (verified: `GET
   https://crates.io/api/v1/crates/nalgebra` returns 403). An external Rust
   NURBS crate cannot be fetched, either here or in CI, without changing the
   whole dependency posture.
2. OCCT is not installed on U1 (no opencascade headers, no TKMath). The
   P6-M3-T01 ADR ratifies OCC as the B-Rep boolean engine but its binding
   (oep-engine-occ) is a dedicated FFI crate that does not exist yet, and the
   ADR explicitly says the boolean decision "does not force" the OCC lane on
   the math lane.
3. Both sibling math cards already chose vendored std-only: P6-M2-T01
   (`docs/geometry-math.md`  -  "zero new dependencies ... no crates.io
   fetch risk on CI (403s)") and P3-M2-T01 (`gregory.rs`  -  "own math,
   dependency-free"). Standing on that precedent keeps the workspace std-only
   and the CI risk zero.

Tradeoff acknowledged and accepted, matching the P6-M2-T01 note: a mature
external NURBS library (OCCT's Geom_BSplineCurve/Geom_BSplineSurface, or a
Rust crate) is more battle-tested than our kernels. The kernels here are the
standard NURBS Book algorithms (Piegl & Tiller, chapters 2, 3, 4), the
acceptance gate is two-sided (Rust + an independent stdlib-Python Cox-de Boor
implementation in `tests/test_oep_geo_nurbs_acceptance.py`), and the goldens
are exact rational constructions (a unit circle to ~2e-16, a cylinder) so
verification is against analytic truth, not self-reference.

## Conventions (NURBS Book, Piegl & Tiller)

- Clamped (grounded) knot vectors: `U = [u0 .. u0 (p+1 times), ...,
  um .. um (p+1 times)]`, length `n + p + 1` where `n` is the number of
  control points and `p` the degree. The usable domain is
  `[U[p], U[n]]`.
- Indexing: basis function `N_{i,p}` with `i in 0..n`; a knot span
  containing `u` is found by `find_span` (A2.1); nonzero basis values for a
  span are `N_{span-p+j, p}(u)`, `j in 0..=p` (A2.2); basis + derivative
  values by `ders_basis` (A2.3).
- Rational evaluation by homogeneous projection: `C(u) = A(u)/w(u)` with
  `A(u) = sum_i N_{i,p}(u) w_i P_i`, `w(u) = sum_i N_{i,p}(u) w_i`.
- Rational first derivative by the Leibniz/quotient rule over the homogeneous
  curve (A4.2): `C' = (A' w - A w') / w^2`, generalised to arbitrary order by
  recurrence in `NurbsCurve::derivative`.

## Public API

| Type | Members |
| --- | --- |
| `BsplineCurve` | `new(degree, knots, control_points)`, `point(u)`, `derivative(u, order)`, `basis_sum(u)`, `domain()` |
| `NurbsCurve` | `new(degree, knots, control_points, weights)`, `point(u)`, `derivative(u, order)` |
| `NurbsSurface` | `new(degree_u, degree_v, knots_u, knots_v, control_points[iv][iu], weights[iv][iu])`, `point(u, v)` |
| `NurbsError` | `NotEnoughControlPoints, KnotNotNonDecreasing, KnotCountMismatch, WeightCountMismatch, NonPositiveWeight, ParameterOutOfRange, ZeroDenominator, NonFiniteInput, DerivativeOrderTooHigh, SurfaceGridMismatch` |

All scalars are `f64` (P6-M1-T03 discipline, "double everywhere"). The public
point type is plain `[f64; 3]`  -  deliberately NOT named `Vec3` so it cannot
collide with the parallel P6-M2-T01 `math::Vec3` when both PRs merge; the two
are reconciled at the P6-M4-T02 importer hub.

## Real material and verification

Committed goldens (`crates/oep-geo/tests/golden/`, our own documented
`OEP-NURBS-GOLDEN` v1 whitespace format so both sides parse without deps):

- `nurbs-circle.oepgeo.txt`: the classic degree-2 rational B-spline unit
  circle (9 control points at the tangential corners, weights `1/sqrt(2)` at
  the odd points, clamped knots `{0,0,0,1/4,1/4,1/2,1/2,3/4,3/4,1,1,1}`).
  Verified: radius exactly 1 up to 2e-16 across a dense sweep; spot at
  u = 0.125 is the 45-degree point `(sqrt2/2, sqrt2/2, 0)`; first derivative
  is orthogonal to the radius at every sampled parameter (central-difference
  check in Python, analytic in Rust); clamped quarter marks interpolate the
  on-circle control points.
- `nurbs-cylinder.oepgeo.txt`: a ruled rational cylinder, degree `(2,1)`,
  control rows at z = 0 and z = H = 2, same circle law. Verified: radius 1 at
  every sampled (u,v); z == H*v exactly (ruling linear in v).

Two-sided gate:
- Rust: `crates/oep-geo/tests/nurbs_eval.rs`  -  9 tests (golden spot match,
  radius == 1, tangent orthogonal to radius, partition of unity, linear
  polygon reproduction, cubic Bezier == Bernstein, derivative vs finite
  difference, surface spot match, surface radius/z).
- Python: `tests/test_oep_geo_nurbs_acceptance.py`  -  its own Cox-de Boor in
  stdlib (no numpy, no Rust), re-derives the analytic truth from the golden
  structure and cross-checks the committed spots, radius, z-ruling, marks and
  partition of unity. This gate runs without any Rust binary.

## Non-goals (scope boundary)

- Second derivatives and arbitrary-order surface partials: currently only the
  curve's first derivative and surface point evaluation are exposed. Higher
  derivatives are the natural extension for P6-M2-T03 (geometric predicates)
  and P6-M4 exporters.
- Knot insertion / refinement / degree elevation: not needed for the card;
  the importer hub (P6-M4-T02) and tessellation (P3-M2-T02) can add them on
  top of this basis.
- Conversion to/from `math::Vec3` and integration with the oep-geo model
  schema: deferred to P6-M4-T02 per layout convention.

## Change log

- (2026-10-16): Initial implementation for P6-M2-T02. RED commit = acceptance
  tests + goldens; GREEN commit = `nurbs.rs` + docs. PR left open for review
  per worktree-isolation (unmerged).
