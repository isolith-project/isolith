# P2-M5-T03 — Regression corpus runner: every corpus file opens or yields a classified skip

> RE observation / tooling note. Clean-room: constants below are published
> observations (see `docs/re-notes/oep-cgr-header-parse.md`); no proprietary
> bytes are present in this repo.

## What this is

A regression sweep over the Keelworks corpus. For every corpus file it yields
exactly one classified outcome — status `OPEN` or a `SKIP` carrying a machine
code from a closed taxonomy — and writes a report matrix (JSON + markdown).
It is the P2/P3-shared corpus regression infra (P3-M5-T01 reuses it): format
dispatching is pluggable and the CGR open check escalates to an external
reader CLI when present (`OEP_CGR_READER`, or `oep-cgr2gltf`/`oep-cgr2step`
on PATH).

## Outcome taxonomy (closed set)

| class | meaning |
|---|--------|
| `unsupported-format` | valid format but reader is out of scope (catpart/catproduct/3dxml → P3/P5 cards) |
| `not-a-cgr-container` | `.cgr`-named file fails the observed header spine |
| `truncated` | file shorter than the structure requires |
| `object-store-unresolved` | object-store entry not materialized locally (tolerant) |
| `missing-on-disk` | manifest path does not resolve to a file |
| `no-mesh-found` | valid CGR header spine but no tessellation latched (external reader rc=4) |
| `unknown-format` | unrecognized extension/magic |
| `io-error` | unreadable |

There is no `fail` bucket in this taxonomy: introspection errors become a
classified row and a non-zero exit, never a crash (P2-M1-T04 tolerant model).

## CGR open probe (deterministic, no vendor code)

For `*.cgr` the runner first probes the observed 0x78-byte header spine:

- magic `01 00 00 00` @ 0x00
- 96 zero bytes @ 0x04..0x63
- per-sample landscape u32 @ 0x64 (semantics `[uncertain]`, not asserted)
- 16-byte invariant template @ 0x68..0x78:
  `00 07 00 00 00 07 00 00 00 b1 01 0b 01 08 02 4f`

When an external CGR reader CLI is configured, the runner instead runs it and
maps its documented exit codes: 0=OPEN, 3=not-a-cgr-container, 4=no-mesh-found,
2=io-error (contract from P2-M5-T01/T02).

## Verification transcript (real material, 2026-10-15)

Full manifest sweep (139 entries) + real CGR sample dirs → **161 rows, all
classified**:

- 11 real `.cgr` samples OPEN (header spine verified on ATOMIZER/BODY1/BODY2/
  LOCK/NOZZLE1/NOZZLE2/REGULATION_COMMAND/REGULATOR/TRIGGER/VALVE@cfysa +
  piston@piston). The 24-byte `_head_*` fragments are correctly classified
  `truncated` (they are head dumps, not complete CGRs).
- 1 repo STL (`box.stl`) OPEN.
- 16 repo CATPart/CATProduct/3DXML → `unsupported-format` (native readers are
  P3/P5 scope, honest boundary).
- 122 object-store CATPart → `object-store-unresolved` (not materialized in
  this sandbox; a `--cache <dir>` populated by the corpus sync makes these
  resolve and open as far as each reader allows).

Acceptance test: `tests/corpus_regression_test.py` (TDD, RED→GREEN,
8 deterministic + real-material legs), `python3 -m oep_tooling.regression`
CLI, matrix at `corpus/regression-matrix.{json,md}`.

## Boundary / anti-collision

This task owns the sweep harness + matrix ONLY. The CGR mesh reader, glTF/STEP
writers and CLIs are separate P2-M2/M5 cards; when those binaries land, the
runner escalates to them automatically via `OEP_CGR_READER`/PATH and the
matrix gains `reader=cli` rows. No Rust was written here.
