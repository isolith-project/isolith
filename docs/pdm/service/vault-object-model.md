# oep-vault: the P8 vault object model (P8-M1-T02)

Version 1.0, 2026-10-17. Milestone: P8-M1-T02 (vault object model: *item*,
*revision*, *file*, *relationship*, *context*; content-addressed blobs;
migration path from V5-era folder trees anticipated). Program: Keelworks /
Open Engineering Platform (OEP), P8 self-hosted PDM (ENOVIA-lite).
Clean-room, our own format — nothing here is a Dassault or vendor schema.

Reference implementation: `crates/oep-pdm` (Rust, serde), modules
`model` / `validate` / `blobstore` / `legacy`.
Real-material gate: `crates/oep-pdm/tests/golden/example-vault.oepvault.json`,
byte-locked by `crates/oep-pdm/tests/vault_model_golden.rs` and independently
re-parsed by `tests/test_p8_vault_object_model.py` (stdlib Python), which also
recomputes every `blob_sha256` from the committed fixture blobs under
`crates/oep-pdm/tests/fixtures/legacy/`. Blob mirror + verification on NAS:
`python/oep_tooling/vault_blobs.py`, acceptance `tests/test_p8_vault_blob_nas.py`.

---

## 1. The five model classes

A vault metadata document (`VaultDoc`) holds five geometry-free classes,
stored **flat** (one id key per entry) and linked by id, never by ownership
embedding:

| class          | module  | role |
|----------------|---------|------|
| Context        | `model` | an organizational tree node (project / product / folder) |
| Item           | `model` | a persistent engineering identity (the browse key) |
| Revision       | `model` | a versioned snapshot of an item, holding files |
| File           | `model` | a named payload whose bytes are content-addressed |
| Relationship   | `model` | a typed directed edge between items and/or revisions |

A sixth, non-model collection — `imports` — is the **migration ledger**: one
row per completed legacy-tree import, so re-runs are idempotent (P10-M1-T03).

### 1.1 Context

| field       | type      | notes |
|-------------|-----------|-------|
| `id`        | string    | unique |
| `name`      | string    | human-readable (a directory name for folders) |
| `kind`      | string    | closed: `project` \| `product` \| `folder` |
| `parent_id` | string?   | parent context id; `None` = root |
| `note`      | string?   | optional | 

`kind` is a committed three-value enum. Folders (the V5-era folder-tree shape)
**must** nest — a folder without a parent is invalid (`root-folder`) — because
a migrated directory tree is a tree, not a forest.

### 1.2 Item

| field         | type      | notes |
|---------------|-----------|-------|
| `id`          | string    | unique |
| `name`        | string    | the browse key |
| `kind`        | string    | closed: `part` \| `assembly` \| `document` (the P7-M2-T02 contract vocabulary) |
| `context_id`  | string?   | home context; must resolve to a Context |
| `part_number` | string?   | optional PN for BOM/PDM interop |
| `description` | string?   | optional |
| `created_at`  | string    | RFC 3339 UTC |
| `origin`      | object?   | provenance ([1.6](#16-origin--the-migration-anticipation-hook)) |

### 1.3 Revision

| field        | type        | notes |
|--------------|-------------|-------|
| `id`         | string      | unique |
| `item_id`    | string      | owning item; must resolve |
| `number`     | string      | `"1"`, `"2"`, ... assigned by the store |
| `state`      | string      | closed: `working` \| `under_review` \| `released` \| `obsolete` (transition rules are P8-M2-T01; the model only bounds values) |
| `comment`    | string?     | optional change comment |
| `created_at` | string      | RFC 3339 UTC |
| `created_by` | string      | acting principal |
| `file_ids`   | string[]    | the files that made up this revision |
| `origin`     | object?     | per-revision provenance (`pdm-save` \| `v5-folder-tree`) |

Revision `file_ids` is the **authority** on what a revision contains; files do
not store a back-reference, so there is no duplicated state to drift
(`validate` walks the ids one way, exactly like `oep-geo` part→body).

### 1.4 File

| field           | type    | notes |
|-----------------|---------|-------|
| `id`            | string  | unique |
| `filename`      | string  | original file name as stored |
| `format`        | string  | routing tag (`catpart`, `catproduct`, `step`, `txt`, ...) |
| `content_type`  | string  | stored media type (usually `application/octet-stream`) |
| `size`          | number  | decoded blob size in bytes |
| `blob_sha256`   | string  | the 64-hex sha-256 content address of the bytes |

**The bytes are never part of the document.** `blob_sha256` is the address
into the object store/sidecar blob store by which the bytes are found *and*
verified (section 3). The novel promise: identical bytes always share one
address (dedup), and a corrupted or swapped blob is detected by re-hashing,
never silently trusted.

### 1.5 Relationship

| field   | type               | notes |
|---------|--------------------|-------|
| `id`    | string             | unique |
| `kind`  | string             | closed (below) |
| `from`  | `{class, id}`      | source endpoint: `item` or `revision` |
| `to`    | `{class, id}`      | target endpoint: `item` or `revision` |
| `note`  | string?            | optional |

Kinds and their endpoint contracts (validated — never ambiguous):

| kind           | from          | to            | meaning |
|----------------|---------------|---------------|---------|
| `composed_of`  | item          | item          | BOM: assembly → component (where-used reverses this) |
| `depends_on`   | item          | item          | general dependency |
| `references`   | item          | item          | document → artifact citation |
| `supersedes`   | revision      | revision      | later revision → earlier revision of the **same** item |
| `derived_from` | item/revision | item/revision | lineage: this was copied from that |

The where-used / impact query layer (P8-M2-T03) is a built on this store;
`VaultDoc::where_used` / `components_of` ship the reverse/composition
primitives today.

### 1.6 Origin — the migration anticipation hook

The schema anticipates the V5-era folder-tree migration (P10-M1-T03) in three
places, so the migration card **consumes the schema** instead of renegotiating
it:

1. **Item `origin`** records ground truth of a captured file:

```json
{ "source": "v5-folder-tree",
  "legacy_path": "W:\\Data\\Baja\\Chassis\\Frame\\RearFrame.CATPart",
  "collected_at": "2026-10-17T00:00:00Z",
  "sha256": "4641...cfa6e",
  "importer": "oep-vault-migrate/0.1" }
```

   `source` is open-ended (ODB, spreadsheets, ... later) — tolerant. The
   `sha256` is the hash of the *original legacy bytes*, decoupling provenance
   from what later became the vault blob.

2. **Folder trees re-materialize as Contexts**: every directory becomes a
   `folder` Context nested under its parent directory's Context; the tree root
   becomes a `product` Context. A tree is therefore browsable in the vault
   *as it was on disk*.

3. **The `imports` ledger** records, per run, the tree root, the importer
   version, the counts, and one idempotency key per imported file (sha-256 of
   the normalized legacy path —
   `legacy::ledger_key_for`). Re-running a migration skips every path whose
   key is already in the ledger (`legacy::not_yet_imported` reports the delta).

Legacy file → vault mapping (this card):

| legacy file                   | becomes                    |
|-------------------------------|----------------------------|
| `.CATPart`                    | `part` item, format `catpart` |
| `.CATProduct`                 | `assembly` item, format `catproduct` |
| `.CATDrawing`, `.txt/.doc*/.pdf/.md` | `document` item |
| anything else                 | `document` item, format `bin` |

Deliberately **not** decided here: whether a `.CATProduct`'s component
references become `composed_of` edges. That needs the CATProduct reader lane
(P5/P6) to resolve components faithfully; P10-M1-T03 picks the fidelity level.
This card maps only the *tree shape* — the stable contract the migration card
extends.

---

## 2. Ids and the reference model

- Ids are non-empty strings; one vault = one id space across **all** five
  collections (`duplicate-id` rejects reuse).
- **Why flat store + id refs instead of nested ownership:** a file can be
  shared by several revisions, a revision belongs to an item, an item appears
  in many assemblies, an item spans contexts over time — id refs keep that
  possible and serialization acyclic.
- References are resolved by `VaultDoc::validate()`, never during
  deserialization, so a partially-linked vault still loads (tolerant-failure
  doctrine, P2-M1-T04).

---

## 3. Content-addressed blobs, verified by hash

### 3.1 Addressing

The content address is **`sha256(bytes)` as 64 lowercase hex characters** —
identical to the P1-M4-T01 corpus mirror's object-naming rule
(`python/oep_tooling/objectstore.py`: content address in the object name,
verification by GET-back re-hash). A `File.blob_sha256` must be such an
address (`invalid-blob-address` otherwise).

### 3.2 The BlobStore (`crates/oep-pdm/src/blobstore.rs`)

`BlobStore` keeps bytes on disk at `<root>/blobs/<sha256>` and enforces, in
code, the three verify-by-hash guarantees:

| operation  | guarantee |
|------------|-----------|
| `put`      | idempotent dedup — identical bytes store once; a hash/name collision is an error, never a silent overwrite; write is temp-file + atomic rename |
| `get`      | re-hashes before returning; a tampered/truncated blob is `Corrupt`, never silently trusted |
| `verify`   | re-hashes every blob; `verify_doc_files(doc)` proves every content address a vault references is present **and** hash-correct |

An absent blob is `Missing` (callers classify) — the store is tolerant when
data isn't there, and loud when it's wrong.

### 3.3 NAS mirror (`python/oep_tooling/vault_blobs.py`)

The vault blob directory mirrors to the NAS object store exactly like
P1-M4-T01 rerode the corpus: `PUT /api/packages/{owner}/generic/keelworks-vault/v1/{sha256}`,
object named by the content address, 409-safe idempotent upload (a conflict on
a content-addressed name races an identical prior upload and is re-verified),
and a `verify_mirror` that GETs every blob back and re-hashes it against its
address. NAS down ⇒ `probe()` False ⇒ acceptance classifies SKIP (never a
crash). The committed mirror index is the proof of what actually landed.

---

## 4. Serialization format (our own)

### 4.1 Header

```json
{ "format": "oep-vault", "version": 1 }
```

`format` disambiguates from other JSON; a reader rejects versions it does not
understand (`unsupported-version`).

### 4.2 Orientation and determinism

- JSON (RFC 8259), UTF-8; canonical pretty (2-space indent, trailing newline)
  or compact — writer SHOULD emit pretty, reader MUST accept both.
- Output is deterministic: key order = struct declaration order; every list
  keeps insertion order; **no `HashMap`** in serialized types, so the same
  logical vault always serializes to the same bytes (golden-byte-lock
  meaningful, VCS-friendly diffs).
- Round-trip: `serialize -> deserialize` is value-`==`; optional fields are
  `#[serde(default)]`/`skip_serializing_if`, so new optional fields do not
  break old files within version 1 (tolerant).

---

## 5. Validation rules (`VaultDoc::validate()`)

`validate()` returns every violation as `{code, message}`, never panics, and
rejects nothing at load time.

| code                          | condition |
|-------------------------------|-----------|
| `bad-format` / `unsupported-version` | header is not `oep-vault` version 1 |
| `duplicate-id`                | an id appears twice in the vault id space |
| `duplicate-ref`               | a file-id list contains the same id twice |
| `invalid-context-kind` / `invalid-item-kind` / `invalid-revision-state` / `invalid-relationship-kind` | closed vocabulary out of bounds |
| `invalid-blob-address`        | `blob_sha256` is not exactly 64 lowercase hex |
| `unresolved-context` / `unresolved-item` / `unresolved-file` | a reference id does not resolve |
| `root-folder`                 | a folder Context has no parent |
| `context-cycle`               | the context parent chain is (transitively) cyclic |
| `bad-relationship`            | a relationship endpoint does not resolve in its declared class |
| `relationship-kind-mismatch`  | an edge kind is used with disallowed endpoints (`supersedes` is revision↔revision of the same item) |

---

## 6. The canonical example (real material)

`crates/oep-pdm/tests/golden/example-vault.oepvault.json` is a hand-authored
vault of a small product imported from the V5-era tree `W:\Data\Baja`
(5 contexts including the re-materialized folder tree, 5 items with `Origin`
records, 6 revisions, 6 files whose `blob_sha256` are the **real** sha-256 of
the committed fixture blobs in `tests/fixtures/legacy/`, 5 relationships
exercising every kind, and one import-ledger row). Held to exact bytes by the
Rust snapshot test, independently re-parsed and re-hashed by the Python
acceptance test.

## 7. Handoff notes

- P8-M2-T01 (revision state machine): the four states are bounded here, not
  transitioned; the state machine mutates the same `Revision.state` field.
- P8-M2-T03 (where-used / impact): builds on `composed_of` edges;
  `where_used` / `components_of` are the seed primitives.
- P10-M1-T03 (V5 folder-tree migration): consumes section 1.6 exactly —
  `legacy::discover` / `plan_import` / `not_yet_imported` are the tested
  building blocks.
- P8-M6-T01 (REST service): the OpenAPI v1 contract keeps the P7-M2-T02
  `Item`/`Revision`/`FileRef` shapes; this model is the storage-side schema the
  service maps onto (blob addresses already match the contract's `blob_sha256`).

See also `docs/layout.md` (crate map) and `workstreams/modeler/interchange/pdm-integration.md` (the P8
swap contract).
