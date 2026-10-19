# P7-M2-T02 -- Open-from-PDM / save-revisions-to-PDM: integration points

Status: implemented (stub first). Owner: P7 modeler lane. Version: 1 (2026-10-16).

## 1. Purpose

The OEP workbench (P7-M2-T01) exposes a PDM-facing surface: the user browses
the vault catalogue, opens a file from PDM into the modeler, edits, and saves
revisions back to PDM. This card delivered the stub-first version of that
surface: a real, runnable P8 REST API v1 stub plus the client that the browser
will call, with the integration seam documented so the real P8 service comes
online without rewriting the client.

This document is the "integration points documented so P8 can come online
without rewrite" acceptance item. The wire contract lives in the OpenAPI spec
`workstreams/P8-M6/openapi-v1.yaml`.

## 2. The two verbs

Everything the browser does is two verbs plus a small browse/list set:

### 2.1 Open-from-PDM

    let item = c.get_item(id)?;                  // or: list_items() to browse
    let file = c.open_latest_file(&item.id)?;    // latest revision's bytes
    // or, to open a specific revision:
    let file = c.open_revision_file(&item.id, "1")?;

`file` is a `FilePayload { item_id, rev, filename, format, content_type,
blob_sha256, bytes }`. The workbench routes `bytes` by `format` into the
importer hub (P6-M4-T02, `oep-import`): `step`/`3dxml`/`cgr`/own-format ->
`oep_geo::Document`. `blob_sha256` lets the workbench verify the download
locally before trusting it.

### 2.2 Save-revisions-to-PDM

    let rev = c.save_revision(
        &item.id,
        SaveRevisionInput {
            filename: "part.step".to_string(),
            format: "step".to_string(),
            content_type: None,
            bytes: <current document serialized>,
            state: Some("working".to_string()),
            comment: Some("geometry change".to_string()),
            created_by: Some("p7-modeler".to_string()),
        },
    )?;

The workbench serializes the current `oep_geo::Document` to a format (STEP for
geometry today; own `.oep` for round-trip later) and hands the bytes over. The
client base64-encodes them per the contract, the server assigns the next
revision number and stores the file content-addressed. The returned `Revision`
carries the new number, state, and `files[0].blob_sha256` for audit display.

## 3. Where things live

    workstreams/P8-M6/openapi-v1.yaml                       wire contract (OpenAPI v1 spec)
    crates/oep-pdm/src/api.rs                   Rust projection of the contract
    crates/oep-pdm/src/store.rs                 vault object model subset + blobs
    crates/oep-pdm/src/stub.rs                  stub HTTP/1.1 server (binary oep-pdm-stub)
    crates/oep-pdm/src/client.rs                PdmClient (the browser face)
    crates/oep-pdm/examples/browser-open-save-demo.rs   runnable demo of both verbs

## 4. Configuration seam

The browser learns the PDM endpoint once, as a base URL:

    PdmClient::new("http://127.0.0.1:8787/api/v1")

Accepted forms: `http://host:port` (client appends `/api/v1`) or the fully
anchored `http://host:port/api/v1`. Only `http://` is accepted. The default
stub address is `127.0.0.1:8787`; the P7 workbench should surface this as a
single preference (e.g. `OEP_PDM_URL`), shared by the stub today and the real
P8 service tomorrow.

## 5. The P8 swap contract (the no-rewrite guarantee)

P8-M6-T01 ("REST API v1 + python client; OpenAPI spec as the contract")
inherits `workstreams/P8-M6/openapi-v1.yaml` as the single source of truth for what the
real service MUST implement. To come online without a client rewrite, the real
service must satisfy only:

  * Serve the same contract at `/api/v1` on the configured base URL: same
    paths, verbs, JSON shapes, status codes, error envelope, and the
    `X-Oep-*` / `X-Blob-Sha256` download headers.
  * Keep file content-addressed: `blob_sha256` remains the lowercase hex
    sha-256 of the file bytes.
  * Keep revision numbers as strings, newest-last in ascending order (
    list returns newest first).

That is it. The client (and therefore the browser) needs no changes. P8 owns
what the stub deliberately does not: the P8-M1-T02 real vault storage,
P8-M2-T01 revision state machine (the stub only carries the four states and
never mutates them after create), P8-M2-T03 relationship/where-used graph,
P8-M3 RBAC/identity/audit, and scale. P8-M6-T01 may also add a streaming
upload endpoint for large files as a NEW endpoint; existing clients keep using
the base64 JSON body.

### What the stub guarantees today (verified)

  * Byte-exact open round-trip over a real socket on a committed STEP fixture
    (Rust integration test + independent Python client).
  * Revision isolation: opening revision 1 still returns the original bytes
    after revision 2 is saved.
  * Content addressing: identical bytes share one blob/sha; blobs persist
    content-addressed on disk (`--persist-dir` => `blobs/<sha256>` +
    `store.json`), surviving a server restart (tested).
  * Contract error surface: 400 / 404 / 409 / 415 / 500 with
    `{"error":{code,message}}`.

### Stub maturity / known limits (by design)

  * Single-user, in-memory (with optional snapshot persistence); no locks,
    no concurrent-edit protection (that is P8-M1-T03 working-set protocol).
  * The four revision states are stored verbatim; transitions and maturity
    are P8-M2-T01 work.
  * Upload is base64-in-JSON only (fine for the sizes this lane moves today).
  * No auth — LAN/dev only; never point it at shared infrastructure.

## 6. Run and verify

    # stub server (contract root = http://127.0.0.1:8787/api/v1):
    cargo run -p oep-pdm --bin oep-pdm-stub -- --addr 127.0.0.1:8787
    # with durable vault:
    ... --persist-dir /tmp/pdm-vault

    # demo of the two verbs:
    cargo run -p oep-pdm --example browser-open-save-demo

    # acceptance (Rust, real sockets):
    cargo test -p oep-pdm
    # acceptance (independent python client):
    python3 -m pytest tests/test_p8_pdm_stub_contract.py

See also `crates/oep-pdm/tests/pdm_contract_acceptance.rs` (what "speaks the
contract" means, item by item) and `workstreams/P8-M6/openapi-v1.yaml` (the contract).
