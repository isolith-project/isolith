# Retention / Archive Policy (P10-M3-T02)

Status: delivered (wave-2, P10-M3-T02). Applies to `oep-pdm` vault content
and to the export-all interop archives written by `oep-archive`.

## One-line doctrine

**Keelworks never deletes data automatically. Every retention value below is a
review horizon, not an auto-delete timer; the only delete path is a manual,
audit-logged admin action that additionally requires the content to already
exist in a verified export-all archive.**

This mirrors the program's sovereignty principle (decision #10 and the master
plan's "no telemetry / no paywall / customer owns their data" posture): the
platform is a vault, not a janitor.

## Scope

The policy governs two things that travel together:

1. Vault revision retention: how long revisions in each lifecycle state stay
   in the PDM vault before an admin should review them for archival.
2. Archive retention: how the export-all interop archive (the escape hatch) is
   kept as the durable, format-neutral record.

## Defaults (the machine-readable source of truth)

`oep-archive retention-policy --write <dir>` emits `retention-policy.json`;
the in-code defaults in `oep_archive::retention::RetentionPolicy::defaults()`
are identical. `oep-archive export-all` embeds a snapshot of the policy into
every archive (`manifest.retention`, plus `retention-policy.json` at the
archive root), so the retention context travels with the data.

| State        | Review horizon | Keep forever? | Behavior |
|--------------|----------------|---------------|----------|
| working      | 180 days       | no            | stale in-progress work flagged for archival review |
| under_review | 365 days       | no            | review collateral retained a year past last activity |
| released     | —              | **yes**       | released revisions are immutable gold, retained indefinitely |
| obsolete     | 365 days       | no            | review for archival after one year; purge legal only after archiving + a 90-day quarantine |

Deletion guard rails (all states):

- `automatic_deletion_enabled = false` — hard default; flipping it is a
  deliberate site-admin decision, never a product default.
- `purge_requires_archive_export = true` — a revision may only be purged when
  a verified export-all archive already contains it (your data always has an
  escape copy before it can be removed).
- `quarantine_after_archive_days = 90` — after archival, purged content stays
  in a quarantine bucket 90 days before final deletion (a second chance).
- `audit_logged_only = true` — every quarantine entry and every purge is
  written to the tamper-evident audit log (`oep_pdm::audit`, P8-M3-T03).
  Deletion is a visible, verifiable event or it does not happen.

An admin override is a normal file (e.g. `retention-policy.json` in the vault
or site config); overrides may tighten or relax horizons but never switch on
automatic deletion accidentally, because the audit + archive-required gates
are structural.

## Archive retention

The export-all archive is the permanent interop record and is retained
independently of vault pruning:

- Keep at least **two generations** of archives (the new one and the previous
  one) unless the new archive's `verify-import` report (`import-report.json`)
  confirms a clean re-import.
- An archive generation is only superseded, never edited; archives are
  immutable snapshots (the `checksums.sha256` manifest guards integrity).
- DR drills (P10-M3-T01) restore **from** the most recent verified archive /
  vault backup; P10-M1-T02 backs the vault DB + blobs to second storage.

## What makes content safe to purge

1. It is present in at least one verified export-all archive
   (`verify-import` exit 0).
2. The admin review has passed (past the state's review horizon).
3. It has sat in quarantine (90 days) or the admin explicitly waived it.
4. The purge is recorded in the audit log with actor + target + timestamp.

## Interaction with export-all

`oep-archive export-all` is the gate that makes any later pruning safe: it
writes neutral geometry (STEP / glTF / STL / 3DXML), the oep-geo own-format
model, the JSON vault metadata, and a sha-256 integrity manifest. See
`docs/delivery/disaster-recovery/export-all.md` for the archive layout.

## Open questions / honest limits

- oep-geo own-format documents carry no tessellation in the current schema
  (P6-M2); native-model-only vault files export to the archive as raw JSON
  (open + documented format) with derived geometry deferred until a
  tessellation carrier lands. The archive is still fully portable because the
  raw documents are open JSON and the metadata is complete.
- Retention enforcement (a scheduled review/expiry sweep) is deliberately
  out of scope for P10-M3-T02: the policy is the contract, the CLI hands each
  admin the same defaults, and no deletion machinery exists that could fire
  on its own.
