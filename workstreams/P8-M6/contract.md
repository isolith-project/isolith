# P8-M6-T01: REST API v1 -- the OpenAPI spec is the contract

Milestone: P8-M6 (REST API v1 + python client). Card: t_9c500573.
This document is the human-readable frame for the machine contract. The
machine contract itself is ONE file, and everything else is derived from it:
`workstreams/P8-M6/openapi-v1.yaml` (OpenAPI 3.0.3, base path `/api/v1`).

`P8-M6-T01` is the lane that owns this contract going forward. It inherited
the stub-first surface from `P7-M2-T02` (keelworks card t_c46b5c21): the
P7 modeler lane implemented open-from-PDM / save-revisions-to-PDM against this
exact spec, and the retained `oep-pdm-stub` binary (P8-M1-T01) is the reference
implementation that stays at the same base URL speaking the same contract.
P8-M6-T01 must stay backward-compatible with that stub surface: same paths,
same verbs, same JSON shapes, same status codes, same error envelope, same
`X-Oep-*` / `X-Blob-Sha256` headers, same base64 upload bodies.

## Single source of truth

```
workstreams/P8-M6/openapi-v1.yaml      THE CONTRACT (edit this)
```

Everything below is DERIVED, never hand-maintained:

```
python/oep_pdm_client.py   generated python client  (scripts/gen_pdm_client.py)
```

If the contract changes, you regenerate the client:

```
python3 scripts/gen_pdm_client.py
python3 scripts/gen_pdm_client.py --check   # drift gate (exit 1 on mismatch)
```

Changing the spec without regenerating is a hard failure: the drift test
`tests/test_p8_m6_spec_contract.py` asserts the committed client's embedded
`SPEC_SHA256` equals the sha-256 of the committed spec bytes, and (when PyYAML
is available) re-runs the full generator and diffs byte-for-byte.

## Contract surface (v1)

| Verb | Path | operationId | Purpose |
|------|------|-------------|---------|
| GET  | /health | health | liveness + contract identity |
| GET  | /items | listItems | browse catalogue (prefix/limit/offset) |
| POST | /items | createItem | create a vault item |
| GET  | /items/{item_id} | getItem | one item + latest revision |
| GET  | /items/{item_id}/file/latest | openLatestFile | open-from-PDM (bytes) |
| GET  | /items/{item_id}/revisions | listRevisions | revision list |
| POST | /items/{item_id}/revisions | createRevision | save-revision-to-PDM |
| GET  | /items/{item_id}/revisions/{rev} | getRevision | revision metadata |
| GET  | /items/{item_id}/revisions/{rev}/file | openRevisionFile | specific bytes |

Conventions that are part of the contract (also declared in the spec):

- Uploads are base64 inside the JSON body (single-shot atomic revision create).
- Downloads are raw bytes (`application/octet-stream` or the stored
  content_type) with metadata in `X-Oep-Item`, `X-Oep-Rev`, `X-Oep-Filename`,
  `X-Oep-Format`, and the content address in `X-Blob-Sha256`.
- Errors are the envelope `{"error":{"code","message"}}` with the listed
  status. Codes: bad_request, not_found, no_file, duplicate_name,
  invalid_state, method_not_allowed, unsupported_media, internal_error.
- Files are content-addressed: every file ref carries `blob_sha256`
  (lowercase hex sha-256 of the decoded bytes).

## Generated python client

`python/oep_pdm_client.py` is produced by `scripts/gen_pdm_client.py` from the
spec, stdlib-only (urllib). It mirrors the durable Rust `PdmClient`
(`crates/oep-pdm/src/client.rs`) so the two languages agree on the same
contract: same verbs, same path building, same base-url normalization (accepts
a host root or the anchored `/api/v1`), same error envelope handling.

Example:

```python
from oep_pdm_client import PdmClient

c = PdmClient("http://127.0.0.1:8787/api/v1")
status, item = c.create_item({"name": "OpenBox", "kind": "part"})
status, rev = c.save_revision(item["id"], "box.step", "step", open("box.step","rb").read())
payload = c.open_latest_file(item["id"])
assert c.verify_blob(payload)          # sha-256 verified against the wire
```

## The stub is now a contract test

The P7-M2-T02 stub is not a throwaway. Its compat binary `oep-pdm-stub`
(retained by P8-M1-T01) is the standing reference implementation for the
contract, and two suites keep it honest over real sockets:

- `tests/test_p8_pdm_stub_contract.py` (P7-M2-T02) -- independent urllib
  client, exercises the exact stub surface, persistence across restart.
- `tests/test_p8_m6_client_contract.py` (P8-M6-T01) -- the GENERATED client
  drives the same binary through every verb, error code, header, and the
  content-address verification. No hand-rolled HTTP here: the deliverable
  client is what is exercised.

When the real P8 storage/state-machine/RBAC/audit layers come online (P8-M1 /
P8-M2 / P8-M3), they serve this same contract at the same base URL and these
tests stay green unchanged -- that is the no-rewrite swap contract from
`workstreams/P7-M2/pdm-integration.md`.
