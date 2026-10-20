# P10-M1-T03 RE note: V5-era folder-tree migration tooling

Task: P10-M1-T03 (t_bf3397e5). Delivered as isolith PR on branch
task/p10m1t03-migration-1017a (base task/p8m1t02-vault-model-1016a). Version 1
(2026-10-17).

## What landed

`oep-vault-migrate` -- a Rust CLI in `crates/oep-pdm` that imports a V5-era
folder tree into an `oep-pdm` vault with metadata mapping, consuming the
P8-M1-T02 schema exactly as that card anticipated (section 1.6: `Origin`,
folder-tree -> context tree, the idempotent `imports` ledger and
`not_yet_imported` delta):

- `discover` -- deterministic legacy-file discovery (kind/format by extension).
- `plan` -- DRY-RUN: prints the item/revision/file/context mapping for the
  delta; writes NOTHING (the vault directory is not even created). `--vault`
  may point at an existing vault so the plan reports already-imported skips.
- `import` -- REAL import: content-addressed blobs + `vault.json` under
  `--vault`, merged into any existing vault, validated before writing
  (refuses to write an invalid document), one append-only `imports`-ledger
  row per run, then byte-verifies every stored blob against its source file.
  Idempotent: a no-op re-run leaves `vault.json` byte-identical; new files in
  the tree import as the delta exactly once.
- `verify` -- re-hashes every blob, byte-compares every source file to its
  vault blob and cross-checks `Origin.sha256`; a tampered source file OR a
  tampered stored blob exits non-zero.
- Three new tested building blocks in `legacy.rs`: `plan_delta` (delta filter),
  `merge_vault` (id-stable merge, ledger append-only), and a fix so the folder
  tree re-materializes with CORRECT deep nesting (parents resolved top-down,
  empty dirs included) -- previously a nested folder whose parent sorted after
  it landed under the root.

## Fidelity decision (this card owns it, per the schema handoff)

Tree-shape **metadata** mapping only, observe-don't-copy. Extension-based
classification, no proprietary byte parsing, and no synthesized `composed_of`
BOM edges -- component resolution needs the CATProduct reader lane (P5/P6).
Full rationale + limits in `workstreams/delivery/deploy/migration-tooling.md`.

## Measured result (real material, live on U1)

`workstreams/delivery/deploy/proof/migration-proof.json`: 19 legs, 19 PASS, 0 FAIL, exit 0.
Representative tree (fake CATPart/CATProduct/txt/pdf bytes + an empty Archive
folder): 6 files -> 7 Contexts (root + nested folders, incl. the empty
folder), 6 items/revisions/files, every `blob_sha256` recomputed here equals
the source sha-256, vault blob bytes == source bytes, no-op re-run
byte-identical, delta imported once, tamper detected in both directions.

## Verification

- `crates/oep-pdm/tests/vault_migration_cli.rs`: 6 Rust tests against the real
  binary via `CARGO_BIN_EXE` (RED da40310 -> GREEN 8de20b0).
- `tests/test_p10_m1_t03_migration.py`: 5 pytest tests, independent stdlib
  re-parse + hashlib recompute.
- Workspace: `cargo fmt --check`, `clippy -D warnings`, `cargo test
  --workspace` (oep-pdm 44: 15 golden + 23 blob/migration + 6 CLI), 19/19
  pytest, pre-commit quality gate PASS.

## Handoff

- P10-M1-T01 (docker-compose stack): migrated vaults are directly serviceable
  (same `vault.json` + `blobs/` layout the live `oep-pdm` serves).
- P9: migrated files are content-addressed and verifiable, so they can feed
  the job spec's `File.blob_sha256` refs as-is.
- NAS mirror (`python/oep_tooling/vault_blobs.py`) reuses the same
  content-address-as-object-name rule; the written blobs mirror verbatim.
- Follow-on: P5/P6 CATProduct resolution may enrich the same items with
  `composed_of` edges without re-importing.
