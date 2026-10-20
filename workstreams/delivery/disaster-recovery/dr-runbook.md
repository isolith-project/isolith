# P10-M3-T01 -- DR runbook: fresh-machine restore from backup, under 1 hour

Status: implemented + drill recorded on the real U1-class host (GB10, 2026-10-17).
Owner: Isolith operations (self-hosted PDM lane). Version: 1.

## 1. What this is

The disaster-recovery runbook for the Isolith self-hosted PDM platform
(oep-pdm vault: `store.json` metadata DB + content-addressed blobs + the
tamper-evident audit chain). The recovery-time objective is:

> restore the full vault to a fresh machine in under **60 minutes**, from the
> P10-M1-T02 backup, from a bare state, verified by health checks and a real
> vault read.

The run target is a **U1/VM-class host** (decision #9, amended x2): drills and
restores run on U1-class hosts / approved VMs / the garage PC wall. The NAS is
passive storage and repo host only; it is **never** a restore/drill target.

## 2. Cadence and record

- Frequency: **quarterly**. Next scheduled drill: 2027-01-17 (this run:
  2026-10-17 = Q4-2026).
- Trigger "off-cycle" drills whenever any of these changes:
  - the vault on-disk schema (`store.json` shape, blob layout) changes;
  - the backup bundle layout (P10-M1-T02) changes;
  - the restore procedure or the stack install flow changes;
  - a release train ships a new PDM binary.
- Every drill is timed and recorded. Evidence is committed under
  `workstreams/delivery/disaster-recovery/proof/dr-drill-<ts>.{json,md}` with:
  - measured `TDR` in seconds and pass/fail vs 60 minutes;
  - the health JSON, restored item count, and the verified blob sha256;
  - the audit verify result;
  - the backup target and fresh image used.
- A drill that exceeds 60 minutes is a FAIL and opens an incident: the runbook
  is revised (RTO remediation) and a follow-up drill is scheduled within 14 days.

## 3. The backup contract (produced by P10-M1-T02)

The bundle this runbook restores from lives on a **second disk / archive
target** on the U1/VM host, at `<BK_TARGET>/isolith-vault-backup/`:

    latest/                    # latest snapshot (rolled)
      store.json               # vault item/revision metadata DB
      blobs/<sha256>           # content-addressed object store
      audit/audit.log          # append-only, tamper-evident chain
      audit/audit.head
    manifest.sha256            # sha256sum over latest/ (relative paths)
    meta.json                  # created_at, source, counts, source_audit_head

The backup script is `scripts/dr/backup-vault.sh`. It snapshots the vault
blobs first, then the DB, then the audit chain, self-verifies every blob name
against its own sha256 (content determinism), writes the sha256 manifest and a
machine-readable `meta.json`, and only exits 0 after `sha256sum -c` passes.
Quiesce the live PDM write path (or accept the audit head as-of snapshot time)
before capturing, per the script header.

## 4. The drill procedure

1. Preflight: `scripts/dr/drill-restore.sh --check` (backup bundle present and
   manifest-verified, fresh image present, docker daemon up, PDM binary built).
2. Record the current time / date. `T0 = start of fresh-host provisioning`.
3. The script provisions a brand-new bare machine image, installs the PDM
   binary, and transfers the backup bundle **from the archive target only**.
4. Integrity gate: the manifest is re-verified inside the fresh host before any
   file is placed. A failure aborts the drill (no partial restore is left).
5. Restore layout: `store.json` + `blobs/` + `audit/` into
   `/var/lib/isolith/vault`; boot `oep-pdm --persist-dir ... --addr ...`.
6. Acceptance checks, all-or-nothing:
   - `GET /api/v1/health` returns `status: ok`;
   - `GET /api/v1/items` returns the vault's item set (non-empty);
   - `GET /api/v1/audit/verify` reports the tamper-evident chain ok;
   - a **real vault read**: download one item's latest revision file and its
     sha256 must equal the content-addressed blob name and match the entry in
     the backup manifest.
7. `T1 = time the real read is verified`. `TDR = T1 - T0`. PASS iff
   `TDR < 3600 s` and every check above passed. Tear down the fresh host.

Run it with:

    scripts/dr/backup-vault.sh            # refresh the backup (if needed)
    scripts/dr/drill-restore.sh           # timed drill -> workstreams/delivery/disaster-recovery/proof/dr-drill-<ts>.json

## 5. Scope boundaries / deliberate non-goals

- The offline / air-gap installer (signed ISO/USB) is P10-M2-T02; this drill
  installs the restored node from a prebuilt binary. RTO budget assumes a
  binary/offline-bundle is on hand.
- Multi-site replication and conflict semantics are P10-M4-T01, not a DR
  substitute: DR restores a single coherent backup rather than reconciling
  divergent sites.
- The NAS remains passive; nothing in this runbook writes backup or restore
  data to the NAS.

Version: 1 (2026-10-17). Next revision trigger: next drill after any P10-M1-T02
backup contract change.
