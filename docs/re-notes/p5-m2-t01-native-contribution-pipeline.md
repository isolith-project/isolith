# P5-M2-T01: Grow native CATPart corpus via contribution pipeline (dev-only, license-noted)

Version: 1.0 (2026-10-16)
Owner: P5 native lane (P5-M2), contribution pipeline
Status: DONE — the native corpus grew from one writer era (V5R29SP0HF0, 2018)
to four, via a repeatable submission registry, sweep-gated and mirrored.

## Why this matters

P5-M1-T01's schema-drift pass on the full 131-row native corpus reported the
R2026-vs-older drift leg as **blocked_on_sample**: every native file in the
corpus was written by a single writer generation (V5R29SP0HF0,
CATBuildLevel 10-11-2018.20.00, MinimalVersionToRead=CATIAV5R29). Without
samples from other writer eras, schema-drift measurement across CATIA versions
is impossible, and P5-M2-T02's save-modify-save observation tooling has no
pair material from different eras.

## What was contributed

19 real native CATParts from the NIST MBE PMI Validation dataset (US Government
work, NIST open license / public domain):

- **STC PMI v4 (5 files, R2023-era write):** `nist_stc_06..10` — CATIA
  V5-6R2023 native saves; writer markers `V5R32SP0HF0` + `V5R33SP0HF0`,
  `CATIAV5R33`, build 10-13-2022.18.00.
- **FTC/CTC PMI v2 (14 files, R2019 + R2021-era writes):** `nist_ftc_06..11`
  and `nist_ctc_01..05` — V5-6R2019 (`ct5290`, `V5R29SP3HF0`) and V5-6R2021
  (`ct5315`, `V5R31SP5HF0` / `V5R31SP6HF0`) native saves.

All 19 are PMI-bearing (ASME Y14.5 / Y14.41 conformance test cases), which also
gives the P4 MBD lane native material to cross-check against, all `V5_CFV2`
magic-verified per file, and all prism solid parts (geometry class `prism`).

Fingerprint/era evidence was byte-observed on real material
(`writer_markers`, `catiav5r`, `build_dates`), recorded per entry in the
submission registry and cross-checked against the P5-M1-T01 schema-drift
methodology.

## Pipeline (the deliverable, not just the batch)

`corpus/native-samples.json` is the contribution/submission registry. Any
future native sample enters through it and is consumed by
`corpus/tools/build_manifest.py` (which re-verifies size + sha256 on the real
bytes when a local copy is reachable), then mirrored to the NAS object store
by `corpus/tools/mirror_object_store.py`. Entry criteria (per corpus README):

1. Magic-verified native bytes with measured size + sha256 + era markers.
2. License-noted per entry: `license`, `license_usage`
   ("dev-only reference material; never shipped"), `dev_only: true`.
3. **P1-M1-T06 privacy/ITAR sweep before any byte enters the shared repo** —
   see the sweep evidence below.
4. Acceptance proved by `tests/manifest_test.py` (checksum + geometry class +
   license note per entry) and the live mirror/regression acceptance.

Don's posture (2026-10-16) applied unchanged: corpus is dev-only reference
material, never shipped; do NOT over-filter on redistributability (permissive
samples that cannot be redistributed are fine for development); the
privacy/ITAR sweep stays the hard line.

## Verification (measured, not guessed)

1. **Checksums:** all 19 sha256 + sizes re-verified against the real on-disk
   bytes before registration (mismatches: 0).
2. **Manifest growth:** 139 -> 158 entries (object-store 122 -> 141, repo 17).
   Every native entry carries `sha256`, `geometry_class=prism`, `license`,
   `license_usage` and `dev_only=true`.
3. **Mirror:** all 19 new blobs mirrored to `don/keelworks-corpus` v1,
   content-addressed by sha256; live GET-back verified 158/158 (0 BAD, 0
   pending; 19/19 NIST blobs re-hash to the manifest).
4. **Regression:** `python3 -m oep_tooling.regression --fetch` on the full
   manifest -> 158/158 classified, 0 object-store-unresolved; native entries
   classify as real `unsupported-format` SKIP ("V5_CFV2 native container;
   native reader is P5 scope"), never an unresolved-object-store leak.
5. **Sweep (hard line):** native contributions swept on real bytes -> 18/19
   CLEAN; the single HIGH (an EMAIL-pattern hit `uz@5o.wH` at byte offset
   1656961, binary tessellation noise) was audited and dispositioned in the
   evidence; gate PASS.
   `P1-M1/privacy-itar-sweep-report-p5m2-native-2026-10-16.json`.
6. **Determinism:** `build_manifest.py --check` byte-identical.

## Files changed

- corpus/native-samples.json (new; P5-M2-T01 contribution/submission registry)
- corpus/manifest.json, corpus/inventory.json, corpus/inventory.md (regen, 158)
- corpus/object-store-index.json (158 uploaded + 11 reference-only)
- corpus/regression-matrix.json/.md (regen, 158 rows)
- corpus/tools/build_manifest.py (native pigeon, dev_only fields, POSTURE)
- corpus/tools/mirror_object_store.py (expected-count now derived from manifest)
- python/oep_tooling/regression.py (matrix task label, P1-M4-T01 / P5-M2-T01)
- corpus/README.md, corpus/manifest-README.md (contribution pipeline docs)
- P1-M1/privacy-itar-sweep.md + P1-M1/privacy-itar-sweep-report-p5m2-native-2026-10-16.json
- tests/manifest_test.py (native contribution coverage; counts 139/122 -> 158/141)
- tests/corpus_object_mirror_test.py (counts 139/122 -> 158/141)

## Follow-ups (do not do here)

- P5-M1-T01 can now re-open its R2026-vs-older drift leg against the R2019 /
  R2021 / R2023-era samples (the `blocked_on_sample` gap is closed).
- Whether the 19 NIST CATParts (or a subset) also belong in `corpus/data/`
  repo-backed fixtures for P4 MBD hermetic tests is a P4-lane decision.
- Wiring the sweep into CI as a hard precommit gate remains P11-T05.
