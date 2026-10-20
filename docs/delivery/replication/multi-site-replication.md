# P10-M4-T01: Multi-site replication — conflict semantics (documented, honest)

Version: 1.0.0 — 2026-10-17
Owner: P10 hybrid-cloud / multi-site mesh delivery lane
Baseline: workstreams/pdm/sync/hybrid-cloud-role-adr.md v1.0.0 (airgap default, hybrid opt-in)
Run targets (decision #9): U1 (LAN node) + garage PC (remote node, WoL), NOT NAS

## Card (verbatim)

> Multi-site replication demo: a LAN node and a remote node replicating the
> vault, conflict semantics documented and honest.
>
> Acceptance: a vault edit on one node appears on the other with conflict
> semantics documented; concurrent-edit conflict resolves deterministically;
> verified live across the two approved hosts.

## Topology

The demo replicates one Keelworks vault between two nodes inside a single
trust boundary (decision #10: everything customer-hosted, LAN default):

- **LAN node (node A)** — the authoritative vault: one `oep-pdm` service
  (axum, AGPL-3.0) speaking the P8 REST API v1 contract at `workstreams/pdm/api/openapi-v1
  .yaml`. Runs under the P8-M5-T03 default `DeploymentRole::Airgap`:
  loopback/LAN only, telemetry forced off, no relay, no internet egress.
- **Remote node (node B)** — a sync client replica: the generated
  `python/oep_pdm_client.py` (`PdmClient`), which holds a local working copy,
  records offline edits against an item's base identity, pulls the vault head
  to see edits made on node A, and syncs its own edits back via
  `POST /api/v1/sync/push`.

The two nodes address the same vault: there is ONE revision authority (the
service; the server serializes revision numbers). "Replication" is the vault's
state appearing on the other node's view — the LAN node commits a revision, the
remote node pulls it; the remote node records an offline edit, syncs back, and
the LAN node reads the committed result.

## Conflict semantics (the honest rules)

The P8-M5-T02 machinery gives the PDM lane its conflict surface. What is
deterministic, in wire terms:

### D1 — Divergence is detected by the server, by content hash, at push time

A sync push carries the editor's base identity: `base_revision` **and**
`base_blob_sha256` (the content address of that revision's file). The server
compares it against the item's current head:

- head == base  → clean sync: the offline edit becomes the item's next
  revision (201 `status:"synced"`).
- head != base  → divergent: **no revision is created**; a pending merge is
  recorded and a conflict descriptor (409 `status:"conflict"` with a
  server-minted `sync_id`) is returned.

The content hash (not just the revision number) makes the check content-true
and immune to revision-number resets. This part is deterministic: given the
same head and the same pushed base identity, the server makes the same
accept/conflict decision every time.

### D2 — A conflict freezes three immutable snapshots (base / ours / theirs)

`GET /api/v1/sync/conflicts/{sync_id}` returns base, ours (the offline edit,
not yet a server revision) and theirs (the vault head at conflict time), each
with its filename, format and `blob_sha256`. "Theirs" stays a normal immutable
revision even after resolution, so the audit trail is complete. `GET
/api/v1/sync/conflicts/{sync_id}/file/{base|ours|theirs}` serves the raw bytes
content-addressed (X-Blob-Sha256 header) — byte-exact, hash-verifiable.

### D3 — Resolution is a new revision, content-addressed, hash-verified

The client picks `ours` / `theirs` / a client-merged document (`merged`).
Resolution commits a **new revision** of the item whose file is the chosen
content; the returned revision's `files[0].blob_sha256` is the sha-256 of the
resolved bytes. The acceptance "resolution outcome verified by hash after
commit" is therefore checked exactly: the server never stores a blob under a
hash it did not compute from the received bytes. Deterministic: identical
inputs (same base/ours/theirs + same choice) produce the identical committed
revision. The resolved conflict is then removed (GET → 404).

### D4 — What is NOT automatic (honesty boundary)

Resolution is **ours / theirs / client-merged-document** — it is NOT an
automatic hunk-level text merge, and there is no diff3 column view in this
milestone. When the client picks `merged`, it supplies the merged document
itself; the server commits what it is given, hash-verified. A diff3 column view
is a natural workstreams/pdm/web/T05 follow-up; the data model (three immutable,
hash-verifiable snapshots) already supports it. The demo's "deterministic
resolution" is: divergence detection is deterministic (D1), the three-way
snapshots are deterministic and hash-frozen (D2), and a resolution commits a
revision whose content address equals the chosen bytes (D3) — not a claim that
two divergent text files are auto-merged without a human decision.

## Vector: what the demo proves

1. **Replicate** — a vault edit on one node appears on the other, both ways:
   LAN node commits → remote node pulls and reads it back (hash-verified);
   remote node records an offline edit → syncs back → LAN node reads the
   committed revision (hash-verified). Row ids `lan-edit-seen-on-remote`,
   `remote-edit-seen-on-lan`.
2. **Conflict** — concurrent edits diverge and resolve deterministically:
   node B records an offline edit on rev N while node A advances the head;
   the push is refused (409, no overlay); the pending merge exposes
   base/ours/theirs with hashes; resolution commits the chosen bytes as the
   next revision, hash-verified; the conflict 404s afterward. Row ids
   `concurrent-edit-divergence`, `conflict-three-way-snapshots`,
   `resolution-hash-verified`, `conflict-consumed`.
3. **Airgap** — the whole demo runs inside the LAN boundary under the
   P8-M5-T03 default `Airgap` role: the demo's clients are instrumented so no
   request leaves the bound LAN/loopback host (no relay, no telemetry, no
   internet). Row id `airgap-lan-boundary`.

## Where the live proof lives

- CI-safe acceptance (one host, real service over real sockets):
  `tests/test_p10m4t01_replication_demo.py` →
  `python/oep_tooling/replication_demo.py`.
- Live two-host proof (UI LAN node + garage PC remote node, decision #9):
  `scripts/replication-proof.sh` →
  `workstreams/delivery/replication/proof/replication.json` / `.md`. The garage PC is woken
  via WoL (see the garage-pc-power-wake skill) and runs the remote-node sync
  client against the U1 vault; the LAN node edits appear on the garage PC and
  a garage-PC offline edit syncs back to U1, plus a concurrent-edit conflict
  resolved deterministically — all hash-verified on both hosts.

## Guardrails

- The vault stays the single revision authority; the server, never a client,
  decides whether an edit is clean or divergent (D1). No silent overlay, ever.
- Hash verification is required after every transfer (pull, push, resolution) —
  a node that cannot reproduce a content address fails visibly, not silently.
- The demo runs in the `Airgap` role by default (P8-M5-T03). A `hybrid`
  deployment (relay for NAT'd remote) is a later opt-in and is out of scope
  here; decision #10 keeps remote access a customer-network choice, not a
  Keelworks product feature.
- Nothing here runs on the NAS (decision #9: NAS is passive storage + repo host
  only).

## Change log

- 1.0.0 (2026-10-17): ratified with the P10-M4-T01 demo; deterministic rules
  D1-D3 + the honest no-auto-text-merge boundary (D4) documented; acceptance
  and live-proof vectors defined.
