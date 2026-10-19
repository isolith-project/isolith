# P4-M5-T01 MBD Validator Report

- report_format: oep-mbd-validate v1
- generated: 2026-10-16T20:01:56.874750+00:00
- task: P4-M5-T01
- inputs: 16  findings: 272  PASS 246 | FAIL 18 | WARN 2 | UNVERIFIED 6

## Honesty scope

- This report is NOT a certificate of conformance to ASME Y14.41 or to any single defense-prime requirement for a manufactured part.
- It certifies only what the supplied PMI/model data demonstrates. Gaps in the supplied data are reported UNVERIFIED, never silently as PASS.
- UNVERIFIED findings are NOT PASS findings; they must be resolved by the requester (more data or a confirmed extraction status) before this report can support acceptance.

## Status legend

- **PASS** - the supplied data demonstrated the requirement satisfied.
- **FAIL** - the supplied data demonstrated a violation.
- **WARN** - the requirement is not met exactly but needs a ruling (e.g. unrecognized unit symbol).
- **UNVERIFIED** - the rule could not run on the supplied data; this is not a PASS.

## bracket-complete  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (8 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-complete  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (8 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-dangling-datum  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (3 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | FAIL | error | dangling datum(s) defined but referenced by no frame: datum_A |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-dangling-datum  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (3 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | FAIL | warning | dangling datum(s) defined but referenced by no frame: datum_A |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-floating-frame  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | FAIL | error | datum feature(s) with no designated feature/body: datum_F |
| y03-tolerance-anchored | FAIL | error | tolerance(s) with no toleranced feature/body: gt_float |
| y04-datum-used | FAIL | error | dangling datum(s) defined but referenced by no frame: datum_F |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-floating-frame  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | FAIL | warning | datum feature(s) with no designated feature/body: datum_F |
| y03-tolerance-anchored | FAIL | warning | tolerance(s) with no toleranced feature/body: gt_float |
| y04-datum-used | FAIL | warning | dangling datum(s) defined but referenced by no frame: datum_F |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-no-pmi  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | FAIL | error | model carries no PMI (0 datums/tolerances/notes) after a source-confirmed complete PMI pass: missing PMI |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-no-pmi  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | FAIL | error | model carries no PMI (0 datums/tolerances/notes) after a source-confirmed complete PMI pass: missing PMI |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-no-pmi-unknown  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | UNVERIFIED | error | model carries no PMI in the supplied data, but the source did not confirm a complete PMI pass; cannot distinguish 'no PMI on part' from 'PMI not extractable' - UNVERIFIED, not a finding of missing PMI |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-no-pmi-unknown  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | UNVERIFIED | error | model carries no PMI in the supplied data, but the source did not confirm a complete PMI pass; cannot distinguish 'no PMI on part' from 'PMI not extractable' - UNVERIFIED, not a finding of missing PMI |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-unit-mismatch  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (3 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | FAIL | error | PMI units 'mm' (mm) disagree with model units 'in' (in) |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-unit-mismatch  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (3 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | FAIL | warning | PMI units 'mm' (mm) disagree with model units 'in' (in) |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-unknown-units  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | PASS | error | every datum reference frame is used by a tolerance |
| u01-units-declared | WARN | error | units symbol 'parsecs' is not a recognized length unit (known: cm, feet, foot, ft, in, inch, inches, m, mm, um, µm, μm) |
| u02-units-consistent | UNVERIFIED | error | no model units supplied to compare against; unit consistency is UNVERIFIED, not assumed |
| c01-model-ref-resolution | UNVERIFIED | error | no model facts supplied; resolution of PMI refs into the model is UNVERIFIED, not assumed |

## bracket-unknown-units  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | PASS | error | all frame datum refs resolve with valid precedence |
| s07-tolerance-frame-resolution | PASS | error | all tolerance frame refs resolve |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | PASS | warning | every datum reference frame is used by a tolerance |
| u01-units-declared | WARN | warning | units symbol 'parsecs' is not a recognized length unit (known: cm, feet, foot, ft, in, inch, inches, m, mm, um, µm, μm) |
| u02-units-consistent | UNVERIFIED | warning | no model units supplied to compare against; unit consistency is UNVERIFIED, not assumed |
| c01-model-ref-resolution | UNVERIFIED | warning | no model facts supplied; resolution of PMI refs into the model is UNVERIFIED, not assumed |

## bracket-unresolved-refs  (prime)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | FAIL | error | frame drf_z references unknown datum 'datum_Z' |
| s07-tolerance-frame-resolution | FAIL | error | tolerance gt_x references unknown frame 'drf_missing' |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | error | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | error | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | error | every datum participates in a datum reference frame |
| y05-frame-used | FAIL | error | orphan frame(s) defined but referenced by no tolerance: drf_z |
| u01-units-declared | PASS | error | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | error | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | error | all PMI refs resolve in model inventory (2 features, 0 bodies) |

## bracket-unresolved-refs  (y4141)

| rule | status | severity | evidence |
|---|---|---|---|
| s01-pmi-format | PASS | error | format tag is 'oep-pmi' |
| s02-pmi-version | PASS | error | schema version is 1 |
| s03-unique-ids | PASS | error | all ids unique across the PMI document |
| s04-unique-labels | PASS | error | datum labels unique and non-blank |
| s05-refs-unique | PASS | error | no duplicate ids in any single reference list |
| s06-datum-frame-resolution | FAIL | error | frame drf_z references unknown datum 'datum_Z' |
| s07-tolerance-frame-resolution | FAIL | error | tolerance gt_x references unknown frame 'drf_missing' |
| s08-tolerance-values | PASS | error | tolerance values finite and non-negative |
| s09-note-position | PASS | error | note positions well-formed |
| y01-pmi-present | PASS | error | model carries PMI (2 datum/tolerance/note annotation(s)) |
| y02-datum-anchored | PASS | warning | every datum designates a feature or body |
| y03-tolerance-anchored | PASS | warning | every tolerance attaches to a feature or body |
| y04-datum-used | PASS | warning | every datum participates in a datum reference frame |
| y05-frame-used | FAIL | warning | orphan frame(s) defined but referenced by no tolerance: drf_z |
| u01-units-declared | PASS | warning | document units 'mm' recognized (mm unit) |
| u02-units-consistent | PASS | warning | PMI units 'mm' and model units 'mm' agree |
| c01-model-ref-resolution | PASS | warning | all PMI refs resolve in model inventory (2 features, 0 bodies) |
