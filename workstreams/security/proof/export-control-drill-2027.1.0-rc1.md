# Keelworks export-control release drill proof -- 2027.1.0-rc1

Card: P11-T05 | Date: 2026-10-18 | Host: promaxgb10-83f1

The export-control checkbox wired into the release pipeline, verified by a
release drill in both directions: with the classification file present the
pipeline passes; with the file removed (or its content flagged) the pipeline
fails and the release is blocked.

Gate: `python/oep_tooling/export_control_gate.py` via
`scripts/export-control/verify-classification.sh`.
Basis memo: P0-M2-T04 (workstreams/governance/export-and-legal/export-control-itar-eccn.md, v1.0).

## Drill results

| leg | expectation | exit | result |
|-----|-------------|------|--------|
| drill-present  | PASS - release cleared to proceed | 0 | PASS |
| drill-removed  | FAIL - absence blocks the release | 1 | FAIL (expected) |
| drill-flagged  | FAIL - restricted content flagged | 1 | FAIL (expected) |

## What each leg proved

1. drill-present -- `verify --release 2027.1.0-rc1` with
   `workstreams/security/export-control/2027.1.0-rc1.classification.toml` committed:
   `UNCLASSIFIED, data-locality asserted, no ITAR/ECCN-restricted content`,
   RESULT PASS, exit 0. A release with the classification file in place is
   cleared to proceed.
2. drill-removed -- the same command with the classification file deleted:
   `FAIL ... does not exist -- release BLOCKED (export-control checkbox
   unchecked)`, RESULT FAIL, exit 1. This is the literal "absence fails the
   release" acceptance: the pipeline may not proceed without the file.
3. drill-flagged -- the same command with
   `restricted_content.contains_itar_technical_data = true`:
   `FAIL ... restricted content FLAGGED`, RESULT FAIL, exit 1. The checkbox is
   not just existence, it is the honest assertion; flagged content blocks the
   release until resolved.

Committed artifact under test after the drill (restored clean):
workstreams/security/export-control/2027.1.0-rc1.classification.toml (UNCLASSIFIED,
contains_itar_technical_data = false, data_locality asserted).
Machine record: workstreams/security/proof/export-control-drill-2027.1.0-rc1.json.
