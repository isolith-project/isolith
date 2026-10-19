# P10-M1-T03: V5-era folder-tree migration tooling

Version 1.0, 2026-10-17. Milestone: P10-M1-T03 (migration tooling: import
V5-era folder-based file trees into the P8 vault with metadata mapping).
Program: Keelworks / Open Engineering Platform (OEP), P10 delivery lane.

Tool: `oep-vault-migrate` (Rust binary in `crates/oep-pdm`, consuming the
P8-M1-T02 vault object model). This is the card P8-M1-T02 anticipated: section
1.6 (`Origin`, folder-tree -> context tree, the idempotent `imports` ledger and
`not_yet_imported` delta) exists so this tooling **consumes the schema** instead
of renegotiating it.

---

## 1. What it does

A V5-era deployment keeps engineering data as **folder trees** on a shared
drive: a root directory of `CATPart`/`CATProduct`/`CATDrawing` and aux files
organized into nested folders, with no PDM metadata. `oep-vault-migrate`
ingests such a tree into an `oep-pdm` vault **with metadata mapping, not a dumb
file copy**:

| legacy                      | vault object                                                          |
|-----------------------------|-----------------------------------------------------------------------|
| the tree root directory     | a `product` [`Context`]                                               |
| every directory under root  | a `folder` [`Context`] **nested under its on-disk parent's Context**  |
| a `.CATPart` file           | a `part` [`Item`], format `catpart`                                   |
| a `.CATProduct` file        | an `assembly` [`Item`], format `catproduct`                           |
| `.CATDrawing`/docs/other    | a `document` [`Item`] (format by extension, else `bin`)               |
| each file                   | one [`Revision`] (`"1"`, `working`) holding one [`File`]              |
| file bytes                  | one content-addressed blob (`<vault>/blobs/<sha256>`), sha-256 of the raw legacy bytes |
| provenance                  | [`Origin`] on the item: `source=v5-folder-tree`, absolute `legacy_path`, `collected_at`, `sha256` of the original legacy bytes, `importer` |
| a completed run             | one append-only [`ImportLedger`] row (idempotency keys = sha-256 of the normalized relative path of every imported file) |

The mapping is implemented as the tested building blocks in
`crates/oep-pdm/src/legacy.rs`:

- `legacy::discover` -- recursive, deterministic (sorted) file discovery;
- `legacy::plan_import` -- tree shape -> vault document + content addresses
  (no store touched);
- `legacy::not_yet_imported` / `legacy::plan_delta` -- the re-run/delta filter;
- `legacy::merge_vault` -- id-stable merge into an existing vault document
  (one vault = one id space; `duplicate-id` can never happen on re-run).

## 2. Fidelity decision (P10-M1-T03 owns this)

The vault-object-model handoff (docs/P8-M1/vault-object-model.md, section 1.6)
delegates to this card: *"whether a `.CATProduct`'s component references become
`composed_of` edges needs the CATProduct reader lane (P5/P6) to resolve
components faithfully; P10-M1-T03 picks the fidelity level."*

**Decision: tree-shape metadata mapping only.** This tool maps paths, names and
extensions -- metadata, observe-don't-copy. It never decodes proprietary file
bytes and it does NOT emit `composed_of` BOM edges: those require the CATProduct
reader lane, and claiming them here would misstate assembly structure. Folder
hierarchy *is* faithfully preserved (roots and every nested folder re-materialize
as Contexts, with parentage exactly as on disk, including empty folders), which
is precisely the portion of V5-era layout that survives without proprietary
semantics. When P5/P6 land, a follow-on card can enrich the same items with
`composed_of` edges without re-importing.

## 3. CLI

```text
oep-vault-migrate discover <root> [..]| list legacy files (no writes)
oep-vault-migrate plan <root> [--vault <dir>] [--importer <id>] [--collected-at <RFC3339>]
                 | DRY-RUN: print the mapping for the delta, write NOTHING
oep-vault-migrate import <root> --vault <dir> [--importer <id>] [--collected-at <RFC3339>]
                 | REAL import: write blobs + vault.json, append a ledger row
oep-vault-migrate verify <root> --vault <dir>
                 | checksums vs source; re-hash every blob
oep-vault-migrate --version
```

Exit codes: `0` success (including a no-op idempotent re-run); `1` failure
(usage error, unreadable tree, invalid result, verification failure); `2` usage.

`--vault <dir>` layout written by `import`:

```text
<dir>/vault.json          the VaultDoc (oep-vault v1, canonical pretty JSON)
<dir>/blobs/<sha256>      one content-addressed blob per distinct file
```

### Dry-run before real import (required)

`plan` computes the same plan an `import` would apply, prints the mapping rows
and counts, and writes **nothing** -- it does not even create `--vault`'s
directory. Re-running the exact `import` afterwards therefore reproduces,
deterministically, what `plan` previewed. `--vault` may point at an existing
vault so the plan reports which files are already imported (skipped) and which
form the delta.

### Idempotency

`import` records one `imports` ledger row per completed run whose `keys` are
the idempotency keys (sha-256 of the normalized relative path, portable across
Windows/POSIX). On a re-run, `not_yet_imported` filters those files out and the
snapshot is a no-op: the vault document is byte-identical and no blob is
rewritten. New files that appeared in the tree since the last run are imported
as the delta; the first run's ledger row and content are untouched. Deterministic
ids (derived from the same keys) guarantee a merged re-plan never reintroduces a
context/item/revision/file that already exists.

### Checksums verified against source

Three independent checks, all run by `import` and all re-runnable by `verify`:

1. every `File.blob_sha256` is the **real sha-256 of the source bytes** read
   by `plan_import` (content addressing by construction);
2. every stored blob is re-hashed on read (`BlobStore::get`); `verify_doc_files`
   proves every address the final vault references is present and hash-correct;
3. `verify` byte-compares each source file to its vault blob and cross-checks
   `Origin.sha256` -- a tampered source file **or** a tampered stored blob fails
   with a non-zero exit.

### Refusal to write an invalid vault

Before anything is written, the merged document passes `VaultDoc::validate()`
(duplicate ids, unresolved refs, context cycles/root-folders, relationship
contracts, blob address form). A violation aborts the run before the vault
directory is touched -- a migration never produces an unreadable vault. If
`plan` previews an invalid result, the dry-run reports the violations and fails
rather than hiding them.

## 4. Notes and limits (honest boundaries)

- **One tree per vault lane in this fidelity level.** Ids and ledger keys are
  derived from relative paths (per the P8-M1-T02 schema). Two distinct legacy
  roots with identical relative layouts share id space, so this tool is
  documented for one tree root per `--vault` dir. Point separate trees at
  separate vaults.
- **No proprietary parsing**: formats are classified by extension only
  (`legacy::classify`), clean-room observe-don't-copy. No vendor semantics are
  read, copied, or recreated.
- **No composition/relationships are synthesized** (fidelity decision, section
  2) -- `relationships` stays empty from this tool by design, and the schema
  validates that as perfectly legal.
- Imported revisions enter as `working` state `"1"`; lifecycle transitions are
  P8-M2-T01's surface (the live service), not the importer's.
- `verify` locates an item by its `Origin.legacy_path` ending with the entry's
  relative path, so verification works from any machine (absolute paths may
  differ), bounded by the relative-path id space noted above.

## 5. Verification/acceptance (real material)

- `crates/oep-pdm/tests/vault_migration_cli.rs` -- 6 Rust tests driving the real
  binary via `CARGO_BIN_EXE` against a representative on-disk V5-era tree:
  dry-run writes nothing; import preserves schema + checksums vs source;
  re-import idempotent + byte-identical + delta; verify detects tamper in both
  directions; plus focused unit coverage of `merge_vault`/`plan_delta`.
- `tests/test_p10_m1_t03_migration.py` -- independent stdlib-only pytest suite
  that rebuilds the source hashes with `hashlib` and re-parses the vault JSON,
  binding metadata to actual bytes (verify-by-hash), including an **empty**
  folder that must still re-materialize as a Context.
- `scripts/p10-m1-t03-proof.sh` -> `P10-M1/proof/migration-proof.json` -- live
  proof run captured on U1, every leg PASS.
- Workspace: `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`
  (44 oep-pdm: 15 golden + 23 blob/migration + 6 CLI), 19/19 pytest,
  precommit quality gate PASS.

## 6. Handoff notes

- **P10-M1-T01 (docker-compose stack)** mounts a vault dir for the live
  `oep-pdm` service; this tooling writes exactly that layout
  (`vault.json` + content-addressed `blobs/`), so a migrated vault is directly
  serviceable -- no conversion step.
- **P9** (P2P compute) consumes `File.blob_sha256` refs for job inputs/outputs;
  a migrated tree's files are already content-addressed and verifiable, so they
  can be handed to the job spec as-is.
- **NAS mirror** (`python/oep_tooling/vault_blobs.py`, P8-M1-T02) uses the same
  content-address-as-object-name rule; the blobs `oep-vault-migrate` writes can
  be mirrored verbatim (the address IS the object name).
- **Follow-on**: CATProduct component resolution (P5/P6) may add `composed_of`
  edges to these items; API automation (`oep_pdm_client.py`) is the path to push
  richer relationships later. Not in this card.

See also `docs/P8-M1/vault-object-model.md` (the schema being consumed),
`docs/layout.md` (crate map).
