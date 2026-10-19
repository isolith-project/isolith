# P4-M5-T01: MBD validator service (rule engine + honest report)

Task t_17247e17, Keelworks wave-2 P4 MBD lane. Reference implementation:
`python/oep_tooling/mbd_validate.py` (pure stdlib; Python is the designated
tooling location per docs/layout.md). Product surfacing happens in the P4-M5-T02
card (CLI + exit codes); this card owns the rule engine, the service API, the
corpus fixture checks and the honest report contract that T02/T03 consume.

## 1. What the service does

Checks a *supplied model/PMI bundle* against a documented rule catalog and
emits a deterministic, honest report (JSON + Markdown). The catalog has four
layers, mapping to the master plan (`missing PMI`, `dangling datums`, `unit
errors`):

| layer     | rules | scope |
|-----------|-------|-------|
| STRUCTURAL | s01..s09 | oep-pmi schema invariants, 1:1 parity with `crates/oep-pmi/src/validate.rs` (bad-format, unsupported-version, duplicate-id/label/ref, unresolved-datum/frame, invalid/duplicate-precedence, non-finite-value, invalid-position) |
| Y14.41 MBD | y01..y05 | PMI present (missing-PMI), datum features anchored, tolerances anchored, no dangling datums, no orphan frames |
| UNITS      | u01..u02 | document units statement recognized; PMI/model unit consistency |
| CROSS-MODEL| c01       | datum/tolerance/note feature+body refs resolve in the supplied model inventory |

Rule sets (each rule lives in at least `y4141`):

- `y4141` - ASME Y14.41 Digital Product Definition expectations.
- `prime` - the defense-prime requirements bundle: the same rules with the
  warnings upward-flagged to release-blocking `error` severity (floating
  annotations, dangling datums, orphan frames, unit mismatch/unrecognized,
  unresolved model references).

Effective severity = default severity, overridden per set (see
`RULE_SETS[*]["overrides"]`). One finding per rule per input, deterministic
catalog order.

## 2. Statuses and honesty doctrine

Each finding is exactly one of:

- **PASS** - the supplied data demonstrated the requirement satisfied.
- **FAIL** - the supplied data demonstrated a violation.
- **WARN** - a non-passing condition that needs a ruling (e.g. unrecognized
  unit symbol).
- **UNVERIFIED** - the rule could not run on the supplied data (missing PMI,
  missing model side, unknown extraction status). UNVERIFIED is **never** a
  PASS and **never** a fabricated FAIL.

The two hard honesty rules, enforced by unit tests:

1. `y01-pmi-present` FAILs **only** when the source declares a complete PMI
   pass (`pmi_extracted: true`) and the document carries zero annotations.
   With `pmi_extracted: null` (unknown) the same bytes are UNVERIFIED - the
   service cannot distinguish "the part has no PMI" from "PMI wasn't
   extractable" and says so.
2. The report summary arithmetic is computed from the granular findings, never
   hand-typed; tests verify rollup == sum of finding counts and that the
   generated report cannot drift from a live scan.

Every generated report carries a **honesty scope** block: the report is not a
certificate of conformance to ASME Y14.41 or to any single prime requirement
for a manufactured part; it certifies only the supplied data.

## 3. Input bundle schema (v1) - contract for T02/T03 and importer adapters

```json
{
  "schema_version": 1,
  "source": {"origin": "...", "file": "...", "provenance": "..."},
  "pmi": { "format": "oep-pmi", "version": 1, "units": "mm",
           "datums": [], "frames": [], "tolerances": [], "notes": [] },
  "pmi_extracted": true,
  "model": {"units": "mm", "feature_ids": [...], "body_ids": [...]},
  "model_present": true
}
```

- `pmi` is an oep-pmi schema-v1 document (P6-M1-T02). Rule checks are
  importer-agnostic: whatever P4-M2-T01 or a future AP242/QIF importer emits
  as oep-pmi is fed here directly.
- `pmi_extracted`: `true` = the source ran a complete semantic-PMI pass;
  `null`/absent = extraction status unknown. Drives the honest missing-PMI
  verdict (section 2).
- `model` is the `oep-model-facts` summary (units + feature/body id
  inventory) an importer would fill from an `oep_geo::Document`. It is
  optional; when absent, cross-model rules (c01) and unit consistency (u02)
  report UNVERIFIED, never PASS.
- A full bundle JSON file, a bare oep-pmi file, or a manifest entry all load
  through the same path. `load_input()` raises the typed `MbdInputError` on
  missing/malformed files; the runner records an error row per fixture rather
  than crashing (tolerant-failure doctrine).

## 4. Report format (v1) - contract for T02/T03

```json
{
  "report_format": "oep-mbd-validate",
  "report_version": 1,
  "generated": "<ISO-8601 UTC, set at write time so build_report stays deterministic>",
  "tool": {"name": "mbd_validate", "version": "0.1.0"},
  "task": "P4-M5-T01",
  "rule_sets": ["y4141", "prime"],
  "honesty": { "scope": [...], "limits": [...] },
  "summary": {"inputs": 16, "findings": 272, "pass": ..., "fail": ...,
              "warn": ..., "unverified": ...},
  "inputs": [
    {"id": "bracket-complete", "rule_set": "y4141", "status": "checked",
     "source": {...}, "summary": {...},
     "findings": [{"rule": "s01-pmi-format", "status": "pass",
                   "severity": "error", "evidence": "...", "detail": null}]}
  ]
}
```

`render_markdown()` renders the same data as human-readable Markdown (the
status legend and honesty block always render the PASS/FAIL/WARN/UNVERIFIED
vocabulary so a partial slice of the report stays self-explanatory).
`write_report()` stamps the generation timestamp and emits
`mbd-validate-report.json` + `.md`.

## 5. Corpus fixtures

`corpus/mbd-fixtures/` grows the MBD sub-corpus from zero (mirrors the
P1-M4-T02 surface-corpus pattern). `bracket-complete.oeppmi.json` is grounded
verbatim on the committed real-material golden
`crates/oep-pmi/tests/golden/example-bracket.oeppmi.json` (P6-M1-T02
real-material gate); the others are intentional defect variants, one per rule
category. `corpus/mbd-fixture-manifest.json` registers them with the expected
outcome only for the acceptance test and human review - the engine never reads
the expect blocks.

| fixture | defect exercised | expected |
|---|---|---|
| bracket-complete | none (golden) | all rules pass, 0 unverified |
| bracket-no-pmi | zero annotations, extraction confirmed | y01 FAIL (missing PMI) |
| bracket-no-pmi-unknown | same bytes, extraction unknown | y01 UNVERIFIED (honest) |
| bracket-dangling-datum | datum_A defined, never in a frame | y04 FAIL |
| bracket-floating-frame | datum and tolerance with no refs | y02/y03 FAIL |
| bracket-unit-mismatch | PMI mm on model facts inches | u02 FAIL |
| bracket-unknown-units | units "parsecs" | u01 WARN |
| bracket-unresolved-refs | frame -> unknown datum; tolerance -> unknown frame | s06/s07 FAIL |

## 6. Demo (defense-prime walkthrough)

    cd keelworks          # repo root
    PYTHONPATH=python python3 -m oep_tooling.mbd_validate \
        --manifest corpus/mbd-fixture-manifest.json --out corpus/

Prints a one-line rollup and the two report paths; the committed outputs
(`corpus/mbd-validate-report.json/.md`) are produced by exactly this command.
For a supplied third-party part: convert/import its PMI to an oep-pmi document
(P4-M2-T01 path when landed), add a manifest entry (or pass a bundle file),
re-run. Rule catalog and honest statuses make the compliance posture
defensible in front of a prime.

## 7. Acceptance mapping

- Rule engine + service check corpus fixtures: `check_input`/`run_manifest`
  over `corpus/mbd-fixture-manifest.json` - committed report rows == fixtures
  x rule sets, no drops.
- Each rule unit-tested: one test function per rule id (tests
  `test_mbd_validate_acceptance.py`), exercising pass and fail/warn/unverified
  branches.
- Report output honest: UNVERIFIED never counts as PASS, missing-PMI honesty
  (section 2), rollup == granular counts, integrity block, deterministic
  build_report.
- Designed to be demoable to defense primes: committed report + one documented
  command (section 6).
- Anti-collision: new files only (python/oep_tooling module, tests, corpus/
  fixtures+manifest+report, this note); no Rust crates touched, no edits to
  files owned by other PR branches.
