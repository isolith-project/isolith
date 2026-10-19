# INTEGRATION-T02: Port-forward record (2027.1 base)

Generated on U1, 2026-10-18, on the integrated branch `wave/1-3` off current main (`d22800b`).
Every (b)/(c) PR from the INTEGRATION-T01 worklist has a record in `integration/port-log.json`;
this file is the human-readable summary.

## Counts

| Class | Count | Porter status |
|---|---|---|
| (b) GENUINELY MISSING | 84 | 84 covered (83 ported, 1 waived) |
| (c) SUPERSEDED/CONFLICTING | 12 | 12 covered (decision recorded each) |
| Total | 96 | 95 ported/covered, 1 waived |

## Waived items (deliberately NOT ported)

- **PR 115 / P7-M3-T01 oep-assem** (GPL-3.0): per program decision #16 (2026-10-18) the vendored
  GPL-3.0 SolveSpace solver tree is NOT carried into the integrated tree. The solver slot is filled
  by card SOLVER-REPLACE (t_4cd7d9b3), which gates WAVE-1-3-CHECKPOINT. oep-assem deferred until
  the permissive replacement lands behind the vendor-neutral seam.

## Items covered transitively (stacked branches / existing main content)

- **PR 77 importer hub** -> oep-import adapter crate arrived via PR 101/113 (importer_hub test green).
- **PR 84 accuracy harness** -> arrived via PR 92 (test_accuracy_harness_acceptance green).
- **PR 86 modern-CGR latch** -> arrived via PR 113 (locate.rs + cgr_modern_reference green).
- **PR 118 RBAC** -> carried by P11-T03 (rbac/ byte-identical to p8m3t02-rbac-1017a) plus the authz
  enforcement layer; NOT double-ported. rbac/ and authz/ present exactly once (verified).
- **PR 122 vault model** -> arrived via PR 148 migration port (blobstore/legacy/model/validate).
- **PR 131 web UI** -> card already implemented in main oep-web; proof + doc + acceptance ported and
  verified PASS against main.

## Key keep/replace decisions

- **PR 126 sync client**: main already has the T02 offline-sync engine (`sync.rs`). The T01
  folder-sync client is distinct content; landed as `crates/oep-pdm/src/foldersync/` (folder sync
  with 3-way reconcile + hash verify) with `foldersync_acceptance` green. Main's T02 engine kept.
- **PR 129 admin UI**: branch's `store.rs` (P8-M2 revision store) diverged from main's `VaultStore`
  (P8-M1). Resolved by landing the branch Store/RevisionStore as `revstore/` module; admin.rs and
  its tests re-pointed. Main's VaultStore untouched.
- **PR 74 f64 audit**: the P6-M1-T03 "no f32/f16 anywhere" audit was scoped to exclude the
  format-bound `export/` adapter dir (glTF/STL/OBJ/PDF are float32 formats; documented in
  docs/p6-m4-oep-geo-exporters.md).

## New crates registered in the workspace

oep-pmi, oep-step-pmi, oep-qif, oep-import, oep-shell, oep-kinematics, oep-rules, oep-sketch,
oep-archive.

## Verification summary

- `cargo test` on every affected crate green (oep-geo, oep-cgr, oep-3dxml, oep-pmi, oep-step-pmi,
  oep-qif, oep-step incl. jt_golden, oep-import, oep-shell, oep-kinematics, oep-rules, oep-sketch,
  oep-pdm incl. auth_boundary + foldersync, oep-archive, oep-viewer).
- Workspace pytest: 426 passed, 22 skipped (corpus/NAS-real-material legs skip with a documented
  reason when the mirror is not provisioned; NAS stays repo/storage only per the integration pin).
- Pre-commit quality gate passes on every commit (python syntax + full pytest + cargo test).
- Verified on U1. NAS untouched (repo/storage only).
