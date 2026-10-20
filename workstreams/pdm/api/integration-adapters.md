# P8-M6-T02 -- Integration adapters: OEP shell + tools talking to the PDM API

Status: implemented. Owner: P8 PDM lane. Version: 1 (2026-10-17).

## 1. Purpose

The OEP shell (P7 workbench) and any adopted tools need to talk to the PDM
API. This card delivers the thin **integration adapter layer**
(`oep_pdm::adapter`) that sits between the shell/tool and the raw REST verbs
of the P8 API v1 contract, and it replaces the P7-M2-T02 stub path: the
adapter is constructed against the *live* PDM service (P8-M1-T01 axum
`PdmService`) and its acceptance tests drive it over real sockets.

The layer owns the things a raw `PdmClient` caller would otherwise repeat on
every integration:

- **Item identity mapping.** The shell works in a human item space (`BOX-01`);
  the server assigns opaque ids (`item-3-ab12...`). The adapter resolves a
  shell atom to a server id (id or name hit) and presents the shell atom back
  in results.
- **Auto-create on first save.** Publishing a brand-new part needs no
  pre-created item: the adapter creates it the first time `save-revisions`
  is called for an unknown atom.
- **The two workbench verbs.** `open_from_pdm` / `open_from_pdm_at` and
  `save_revisions_to_pdm` are exactly the verbs the P7 workbench's
  `OpenFromPdm` / `SaveToPdm` buttons need (`workstreams/modeler/interchange/pdm-integration.md` 2.1 /
  2.2), typed and non-panicking.
- **Unit instrumentation.** An `AdapterInstrument` records every verb call and
  byte totals, so integration behavior is observable and assertable from tests
  (and ready for a metrics sink later). "Adapter layer unit-instrumented" is an
  acceptance item of this card and is exercised by
  `tests/adapter_live_api.rs`.

## 2. The seam (what a tool imports)

```rust
let mut adapter = PdmAdapter::new("http://127.0.0.1:8787/api/v1")?;

// Save-revisions-to-PDM: bytes in, next revision out (item auto-created).
let saved = adapter.save_revisions_to_pdm(
    "BOX-01",             // shell atom; resolved/created server-side
    "box-reference.step",
    "step",
    bytes,
    "initial geometry",   // becomes the revision change comment
)?;
assert_eq!(&saved.revision, "1");
assert_eq!(&saved.blob_sha256, &hex_sha256(&bytes));

// Open-from-PDM: latest or at a specific revision, byte-exact.
let latest = adapter.open_from_pdm("BOX-01")?;          // rev 1
let at_rev  = adapter.open_from_pdm_at("BOX-01", "1")?; // revision isolation

// Browse + revision metadata (typed, non-panicking).
let catalogue = adapter.catalogue(None)?;
let revisions = adapter.revisions("BOX-01")?;
```

The workbench routes `OpenedFile.bytes` by `OpenedFile.format` into the
importer hub (oep-import) on the P6/P7 lanes; that geometry wiring is out of
scope here. This card is the PDM side of the seam, owned by the P8 lane.

## 3. The model

    PdmAdapter { client: PdmClient, instr: AdapterInstrument }
    OpenedFile { item_id, rev, filename, format, content_type,
                 blob_sha256, bytes }
    SavedRevision { item_id, revision, state, blob_sha256 }
    AdapterInstrument { health_checks, saves, items_created, opens,
                        catalogue_queries, revision_queries,
                        bytes_uploaded, bytes_downloaded, errors }

Rules the adapter enforces:

- **Live service only.** `PdmAdapter::new` binds the REST client to the
  configured base URL (the axum `PdmService`); there is no in-process
  shortcut, no mock transport, no local vault. The P7-M2-T02 stub path is
  replaced by the real PDM. (The retained `StubServer` symbol still exists for
  the P7 contract tests, but the adapter never uses it.)
- **Non-panicking (tolerant-failure doctrine).** Every method returns a typed
  `Result`; an unknown item atom is `AdapterError::ItemNotFound`, never a
  panic on any input path.
- **Byte-exact + content-addressed.** Download `blob_sha256` must match the
  bytes (the server stores content-addressed; the acceptance test re-checks
  locally). Revision isolation: opening revision 1 still returns the original
  bytes after revision 2 is saved.
- **Instrumented.** Every success/failure path updates `AdapterInstrument`
  exactly once per verb, including deliberate failures (errors are counted not
  hidden).

## 4. Clean-room stance (acceptance item)

The adapter is **our own typed projection of the P8 REST API v1 contract**.
No vendor PDM client (ENOVIA/3DEXPERIENCE, Teamcenter, or any other vendor)
API surface, object model, COM/OML semantics, session/login choreography, or
function names are copied, re-exported, or referenced anywhere in this module
or its tests. The shell-facing verbs are the OEP ones defined in
`workstreams/modeler/interchange/pdm-integration.md`. This is consistent with the program invariant
(clean-room only: no vendor binaries, no copied source).

## 5. Acceptance mapping

- **oep-shell open/save against live API tested** -- `tests/adapter_live_api.rs`
  boots the real `PdmService` (axum, ephemeral port) and drives both verbs
  through the adapter over real sockets: byte-exact round trip, revision
  isolation, catalogue/revisions surfaces, and typed failure for a missing
  item. This replaces the P7-M2-T02 stub path with real PDM.
- **Adapter layer unit-instrumented** -- `AdapterInstrument` is asserted in
  `adapter_layer_is_unit_instrumented` (health=1, saves=2, opens=2,
  catalogue=1, revisions=1, uploaded/downloaded byte totals, errors=1).
- **No vendor PDM client semantics copied** -- section 4; enforced by
  construction (Rust projection of our own `api` contract types only).

## 6. Integration points

    workstreams/pdm/api/openapi-v1.yaml            wire contract (shared, P8-M6-T01 owns)
    crates/oep-pdm/src/adapter.rs    this adapter layer
    crates/oep-pdm/tests/adapter_live_api.rs   live-API acceptance + instrumentation
    crates/oep-pdm/src/client.rs     the REST client the adapter wraps
    crates/oep-pdm/src/service.rs    the live axum service the tests boot
