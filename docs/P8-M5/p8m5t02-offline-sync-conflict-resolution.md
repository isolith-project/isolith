# P8-M5-T02: Offline mode, sync-back on reconnect, conflict resolution UI

## Card (verbatim)

> Offline mode: local working set, sync back on reconnect, conflict resolution UI.
> Acceptance: edits made offline sync back on reconnect; divergent edits surface a
> 3-way-merge UI (ours vs theirs vs base); resolution outcome verified by hash
> after commit.

## Scope

This milestone delivers the second half of P8-M5 (Sync/hybrid). It is
downstream of P8-M1 (service skeleton + vault store + working-set protocol) and
does not change the P8 REST API v1 items/revisions contract inherited from
P7-M2-T02. It adds a *sync surface* to the v1 contract plus the client-side
offline working set. Predecessor P8-M1-T03 (working-set lock protocol) notes that
"concurrent editing of working copies is deliberately allowed across sessions;
conflicts surface at publish time and are handled by P8-M5-T02" -- this design
note is that handler.

## Design decisions

### D1: Divergence is detected by the server, by content hash, at push time

The server is the single authority on revision state. A sync push carries the
editor's *base* identity (base revision number + base blob sha-256). The server
compares it against the item's current head:

- head == base -> clean sync: the offline edit becomes the next revision.
- head != base -> divergent: no revision is created; a *pending merge* object is
  recorded and a conflict descriptor is returned.

Comparing the blob hash (not just the revision number) makes the check content-
true and immune to revision-number resets.

### D2: "Ours" is the offline edit; "theirs" is the head at conflict time

A conflict captures three immutable snapshots so the resolution UI can show a
true three-way merge:

- base   -- the revision the offline edit was based on (unchanged in the vault);
- ours   -- the offline edit the client pushed (held pending, not committed);
- theirs -- the vault head at conflict time (what the server accepted meanwhile).

All three are content-addressed in the vault blob store; "theirs" stays a normal
immutable revision even after resolution, so the audit trail is complete.

### D3: Resolution is a new revision, content-addressed, hash-verifiable

The client picks ours / theirs / a client-merged document. Resolution commits a
*new revision* of the item whose blob sha-256 is the hash of the resolved bytes.
The acceptance "resolution outcome verified by hash after commit" is therefore
checked exactly: after POST resolve, the client verifies that the returned
revision's `files[0].blob_sha256` equals `sha256(resolved_bytes)` -- the server
never stores a blob under a hash it did not compute from the received bytes, so a
byte-mismatch is a protocol failure, not a UI nicety.

### D4: The offline working set is a plain local folder (Nextcloud-style, minus the watcher)

`OfflineWorkingSet` is a directory containing a content-addressed `blobs/` tree
plus a JSON index of pending edits. Recording an edit needs no server. Sync
replays the pending index through the sync-push endpoint; a transport failure
means "still offline" and leaves edits pending for the next sync. The local blob
hash written at record time is compared to the hash committed by the server, so a
local corruption cannot silently diverge from what the server accepted.

### D5: The 3-way-merge UI is served by the PDM service as a self-contained page

`GET /ui/sync/conflicts/{id}` renders base/ours/theirs (markers + hashes + a text
preview for text content, binary note otherwise) and a resolution form
(keep ours / keep theirs / submit merged text). The UI is a plain HTML form that
POSTs to the same resolve endpoint the API uses; no client-side framework. This
is deliberately a *minimal* web surface -- the full P8-M4 item/browser UI is a
separate milestone -- but it is real and testable over a socket.

## Contract surface (additions to P8 REST API v1)

All under the existing contract root `/api/v1`; additions are additive (new
paths + one new error code), nothing on the v1 items/revisions contract changes.

### POST /api/v1/sync/push
Body:

    {
      "item_id": "...",
      "base_revision": "1",        // revision the offline edit was based on
      "base_blob_sha256": "...",   // hash of that revision's file (content truth)
      "state": "working",          // optional, default working
      "comment": "...",            // optional
      "created_by": "...",         // optional, default pdm
      "file": { FileUpload }       // the offline edit (ours)
    }

Responses:

- 201 `{"status":"synced","revision":{...}}` -- clean sync, revision created.
- 409 `{"status":"conflict","sync_id":"...","base_revision":"1",...,
  "theirs_revision":"2","theirs_blob_sha256":"..."}` -- divergent; no revision
  created; use the conflict id to view/resolve via the endpoints below.
- 404/400 contract envelopes as usual (unknown item, bad base64, ...).

The 409 body is not the `{"error":...}` envelope because a conflict is an
expected business outcome with a payload, not a malformed call.

### GET /api/v1/sync/conflicts/{sync_id}
Returns `SyncConflictView`:

    {
      "sync_id": "...", "item_id": "...",
      "base":   { "revision":"1", "filename":"...","format":"...","content_type":"...","size":N,"blob_sha256":"..." },
      "ours":   { "revision":null, ... },
      "theirs": { "revision":"2", ... }
    }

`revision` is null for "ours" (it is not yet a server revision).

### GET /api/v1/sync/conflicts/{sync_id}/file/{base|ours|theirs}
Raw bytes of one side, with `X-Blob-Sha256` (and `X-Oep-Filename`,
`X-Oep-Format`) response headers -- the byte-exact, hash-verifiable access used
by tests and clients.

### POST /api/v1/sync/conflicts/{sync_id}/resolve
Body:

    { "choice": "ours" | "theirs" | "merged", "merged": { FileUpload }? }

`merged` is required iff `choice == "merged"`. Resolves by committing a new
revision of the item whose file is the chosen content; returns 201 `{Revision}`
whose `files[0].blob_sha256` is the hash of the resolved bytes. The conflict is
then removed (GET of the same sync_id -> 404).

### GET /ui/sync/conflicts/{sync_id}
The 3-way merge UI page. POST the resolution form to
`/ui/sync/conflicts/{sync_id}/resolve` (form-encoded, same semantics; the merged
field is raw text, base64-encoded server-side).

## Error code

Adds `sync_conflict` (used by 409 bodies on push/resolve edge cases that must
still speak the envelope, e.g. resolving an already-resolved conflict).

## Out of scope (future milestones)

- File-system watcher/daemon (P8-M5-T01 folder sync).
- P2P/hybrid cloud relay (P8-M5-T03).
- Rich P8-M4 web UI (item explorer, BOM tree, viewer embed).
- Automatic hunk-level three-way text merge: resolution here is
  ours/theirs/client-merged-document. A diff3 column view is a natural workstreams/P8-M4/T05
  follow-up; the data model (three immutable, hash-verifiable snapshots) already
  supports it.

## Hash verification (acceptance criterion spelling)

1. Offline record: `ours_sha256 = sha256(ours_bytes)` is stored in the working
   set index at record time (no server).
2. Clean sync / conflict push: the server computes and stores the blob under
   `sha256(ours_bytes)` and reports it.
3. After resolution commit: `revision.files[0].blob_sha256` must equal
   `sha256(resolved_bytes)`; clients that require proof re-open the latest file
   and compare `X-Blob-Sha256` against the locally computed digest.
