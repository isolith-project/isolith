# P8-M5-T01 -- Sync client: folder-sync for CAD files (NAS + U1 + mobile view)

Status: implemented. Owner: P8 PDM lane. Version: 1 (2026-10-17).

## 1. Purpose

A Nextcloud-style working-set sync client for CAD assets moving between the
NAS (the authoritative remote), the U1 work host (the client mirror) and a
mobile/thin viewer (read-only). The contract, in the card's own words:

1. A working-set folder **syncs both directions** with **hash verification**.
2. **Conflicts are staged, not silently overwritten** -- the remote variant is
   parked for the P8-M5-T02 conflict-resolution milestone; neither side is
   clobbered.
3. A **mobile read-only view** works -- list + verified fetch only, no write
   surface.

Everything is content-addressed: a file's sha-256 is its address, matching the
P1-M4-T01 object-store mirror and P8-M1-T02 blobstore discipline already in
the program. The sync client never trusts a transfer; it re-hashes after every
copy and refuses to land bytes that do not match the recorded digest.

## 2. Where it lives

- Code: `crates/oep-pdm/src/sync/` (module `oep_pdm::sync`).
  - `manifest.rs` -- [`FolderManifest`]: scan a directory into
    relative-path -> [`FileEntry`] (size, sha-256, mtime); reserved
    `.oep-sync` control dir is never a payload; path-escape-rejecting
    `join_safe`.
  - `plan.rs` -- pure three-way reconcile (`local`, `remote`, `baseline`) ->
    [`SyncPlan`] of [`Action`]s and [`Conflict`]s. No IO, unit-testable.
  - `engine.rs` -- [`SyncEngine`]: executes the plan against two real roots
    with atomic write + hash verification, stages conflicts, persists the
    baseline, honours read-only mode.
  - `view.rs` -- [`ReadOnlyView`] (mobile lens): `list()` + `fetch()` with
    hash verification; no mutating methods.
- Tests: `crates/oep-pdm/tests/sync_acceptance.rs` (14 tests, real temp-dir
  fixtures with real sha-256 digests).
- This doc.

## 3. The model

    Side        = local (U1 mirror) | remote (NAS)
    FileEntry   = { size, sha256 (64 hex), mtime_unix }
    FolderManifest = { rel_path -> FileEntry }        (normalized 'a/b.ext')
    Baseline    = FolderManifest snapshot of the last successful sync
    Action      = Upload{rel} | Download{rel} | DeleteLocal{rel} | DeleteRemote{rel}
    Conflict    = { rel, local_sha256?, remote_sha256? }
    SyncPlan    = { actions, conflicts, in_sync }

Control directory (reserved, never a payload, at any depth):

    <local>/.oep-sync/state.json           last successful baseline
    <local>/.oep-sync/conflicts/           staged remote variants
                         *.<sha16>.conflict
    <local>/.oep-sync/conflicts.json       JSONL conflict ledger (T02 handoff)

## 4. Reconcile rules (three-way, no silent winner)

| local | remote | baseline | decision |
|-------|--------|----------|----------|
| A     | A      | any      | in sync |
| A     | B      | A        | Download (remote changed) |
| B     | A      | A        | Upload (local changed) |
| A     | B      | C        | Conflict (both changed) |
| -     | A      | A        | DeleteLocal (remote deleted, local unchanged) |
| -     | B      | A        | Conflict (remote changed while local deleted) |
| A     | -      | A        | DeleteRemote (local deleted, remote unchanged) |
| B     | -      | A        | Conflict (local changed while remote deleted) |

New paths with no baseline entry: present on one side = one-way add; present
on both sides = conflict only when content differs, otherwise in sync.

Deletions propagate only when the surviving side is unchanged, so an edit on
one host can never be silently cancelled by a delete on the other.

## 5. Hash verification (why a mismatch cannot land silently)

Every [`Action::Upload`]/[`Action::Download`] in the engine:

1. reads the source; computes its sha-256 (the **expected** address);
2. writes to a temp file **inside the destination tree**, then re-reads the
   temp and re-hashes it (the **actual** address);
3. only if `actual == expected` does it rename the temp into place; a
   mismatch deletes the temp and raises [`SyncError::HashMismatch`];

So a truncated write, a failing disk, or a racing corruption is caught before
it becomes the visible file. The mobile [`ReadOnlyView::fetch`] applies the
same check against a live scan before returning bytes to the caller.

## 6. Conflict staging (the P8-M5-T02 seam)

When the planner reports a `Conflict`, the engine:

- touches **neither** the local file nor the remote file;
- copies the **remote variant** into
  `<local>/.oep-sync/conflicts/<leaf>.<sha16>.conflict` (idempotent: repeated
  runs do not re-stage an identical variant);
- appends a JSONL line to `<local>/.oep-sync/conflicts.json`:

      {"rel":"critical.step","local_sha256":"<64 hex>",
       "remote_sha256":"<64 hex>","at_unix":...,"staged_file":"conflicts/<leaf>.<sha16>.conflict"}

- keeps the **old** baseline entry for the conflicted path, so a re-run still
  sees the divergence -- the client never pretends a conflict resolved itself.

P8-M5-T02 (offline mode + conflict-resolution UI) consumes
`conflicts.json` + the staged files to present per-path resolution.

## 7. Mobile read-only view

[`ReadOnlyView`] exposes exactly three operations: `new`, `list` (rel, size,
sha256, mtime -- enough to render a file list and thumbnail grid without
opening bytes) and `fetch` (hash-verified bytes). There is **no mutating
method on the type**, and the P8 review must gate on that boundary. The
engine's read-only mode is the complementary hard guard: a mobile client can
refresh its local mirror but any plan that would touch the remote (upload /
remote delete) is refused up front with [`SyncError::ReadOnly`].

## 8. Verified on real material

`cargo test --workspace` -- 14 integration tests, all green, real temp
directories and real sha-256 digests:

- initial pull mirrors the NAS working set, hashes agree;
- local edits / remote edits / simultaneous bidirectional changes converge
  and byte-match on both sides;
- same file edited on both sides -> staged, neither side overwritten, ledger
  written, re-run still a conflict;
- divergent same-name new files -> staged;
- deletion propagates only when the other side is unchanged; edit-vs-delete
  is a conflict;
- mobile read-only view lists and fetches with verification, rejects missing
  files, and has no write surface; read-only engine refuses to push;
- pure reconcile semantics + manifest scan edge cases (control dir skipped,
  path escape rejected).

Quality gates: `cargo fmt --check`, `clippy -- -D warnings`, precommit gate
all clean.
