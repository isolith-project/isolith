# P6-M1-T03: Units and Accuracy Discipline for `oep-geo`

Status: implemented and verified (Isolith wave 1, P6).

## Scope

This card establishes the **units and accuracy discipline** for the geometry
core: one documented answer to "what units are numbers in, and how close counts
as equal?" — the two questions every later geometry card (P6-M2-T03 predicates,
P6-M2-T02 NURBS, P6-M4 exporters, P6-M3 B-Rep ops) must answer consistently.

Delivered in two small std-only modules:

- `crates/oep-geo/src/units.rs` — `LengthUnit` / `AngleUnit` against canonical
  bases;
- `crates/oep-geo/src/tolerance.rs` — `Tolerance`, the documented epsilon
  policy.

## Double everywhere (accuracy discipline)

**All geometric scalars are `f64`**, in every module of `oep-geo`. Rationale:

- CAD geometry spans many magnitudes (nanometre features to kilometre
  assemblies). `f64` gives ~15-16 significant decimal digits of dynamic range;
  `f32` (~7) is insufficient for downstream B-Rep ops, feature-history replay
  (P6-M3-T03) and STEP AP242 round-trips (P4).
- STEP/AP242 and CATIA work natively in 64-bit doubles; narrowing at the
  internal boundary would silently corrupt fidelity for the P4/AP242 path and
  P6-M3-T03 feature replay.

Consequences (enforced by the acceptance source audit
`tests/test_oep_geo_units_acceptance.py`):

- No `f32` or `f16` token anywhere under `crates/oep-geo/src/`.
- Integer types (`u32`, `usize`, ...) are reserved for **counts, indices,
  ids and enums** — never for coordinate or scalar values.
- `f64::NAN` / `±Inf` are possible values of the type but are treated as "not
  comparable" by the tolerance predicates (see below), never as numbers.

## Units model

Canonical bases (the zero-point of the model):

| Kind  | Canonical base | Symbol |
|-------|----------------|--------|
| Length| **millimetre** | `mm`  |
| Angle | **radian**     | `rad` |

Length units and their **exact rational factors** against the millimetre base:

| Unit        | Symbol | Factor to mm |
|-------------|--------|--------------|
| Millimetre  | `mm`   | 1.0          |
| Micrometre  | `um`   | 0.001        |
| Centimetre  | `cm`   | 10.0         |
| Metre       | `m`    | 1000.0       |
| Inch        | `in`   | 25.4         |
| Foot        | `ft`   | 304.8        |

Rules:

- **Route through the canonical base.** `LengthUnit::convert` goes
  unit → mm → unit. A single-unit conversion (in → mm) is one exact factor
  multiply; a composed conversion (ft → in = 304.8/25.4) carries at most one
  `f64` rounding step. Callers needing bit-exact round-trips for composed
  conversions should store in a single unit (the model's recommendation) or
  test within the tolerance model, not with exact equality.
- `LengthUnit::parse` is total and case-insensitive (`mm`, `MM`, `inch`, `µm`,
  `μm`, ...); unknown symbols → `None`, never panic.
- The P6-M1-T01 document `units` field is the nominal model unit string; the
  tolerance model's `in_units` re-expresses `linear_abs` into that unit.

## Tolerance model

A comparison `a ≈ b` under `Tolerance { linear_abs, relative, angular_rad }`
holds when:

```
|a - b| <= linear_abs + relative · max(|a|, |b|)
```

| Field        | Default | Meaning                                            |
|--------------|---------|----------------------------------------------------|
| `linear_abs` | 1e-9    | absolute floor, in the model's length units        |
| `relative`   | 1e-9    | scale-invariant bound (dimensionless)              |
| `angular_rad`| 1e-9    | reserved angular bound (radians) for later cards   |

Why the two-term form:

- **Absolute term** catches near-zero and exact coincidence: `0.0` vs `1e-10`
  compares equal regardless of scale.
- **Relative term** carries scale: at magnitude `1e6` a drift of `1e-3` is
  compared against `1e-9·1e6 = 1e-3`, correctly rejecting large-magnitude
  drift that a fixed epsilon would silently accept.
- Both sit far above `f64` ULP (~2.2e-16 at 1.0), so **floating-point rounding,
  not the algorithm, sets the comparison bound** — the discipline invoked by the
  P6-M2 sister cards.

### Total predicates (no panics)

`approx_eq`, `approx_zero`, `points_eq` are **total**: any non-finite input
(`NaN`, `±Inf`) yields `false`, never a panic, never `true`. Rationale:
geometry code reading untrusted files (P2/P3 corpus) must degrade to "not
equal" on bad input; a panic there is a security/robustness bug, not a
mathematical one.

### `in_units`

`Tolerance::in_units(unit)` re-expresses `linear_abs` from canonical millimetres
into the working unit (`1e-9` mm → `1e-12` m, → `1e-9/25.4` in, → `1e-6` um),
so a tolerance travels with a model's declared units. `relative` and
`angular_rad` are unit-independent and pass through unchanged.

## How later cards pick epsilons

- Defaults here are **analytical/ep comparison defaults** (1e-9). Cards that
  compare against real tessellated geometry with known input quality may loosen
  `linear_abs` with an explicit, commented rationale (e.g. a mesh sag budget),
  but must never loosen `relative` below ~1e-12 without justification.
- Cards that need *tighter* bounds (e.g. unit round-trip in a single unit) should
  test with exact equality or an explicit tighter `Tolerance`, never assume the
  default.
- Angular comparisons (P6-M2-T03) use `angular_rad`; a conversion helper for
  degree tolerance is available via `AngleUnit`.

## Verification

- `crates/oep-geo/tests/units_tolerance.rs` — acceptance contract (was RED
  against the skeleton, then GREEN), byte-locks the golden header, checks exact
  factor table, bounded composed conversions, tolerance boundaries, and the
  non-finite total rule.
- `tests/test_oep_geo_units_acceptance.py` — stdlib-Python re-derivation of
  every figure + the **double-everywhere source audit** (no `f32`/`f16`).
- `crates/oep-geo/tests/golden/units-tolerance-golden.json` — committed golden
  generated by `examples/gen_units_tolerance_golden.rs`, re-verified by Python.

See also: `docs/geometry-model-schema.md` (P6-M1-T01; the `units` field),
`docs/geometry-math.md` (P6-M2-T01; first consumer of this discipline).
