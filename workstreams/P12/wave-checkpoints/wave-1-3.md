# Keelworks wave checkpoint: wave-1-3

Schema: keelworks.wave-checkpoint.v1
Branch: `wave/1-3` -> `main`
Base sha: `d22800bf2d345533fe07caf66f45d189122b2e08`
Tip sha:  `0acde1e6ab450cd78c7496c51e45c201fb9ab03c`
Emitted: 2026-10-18 (U1, promaxgb10-83f1)

## Result

The waves 1-3 retrofit closes with this checkpoint. `wave/1-3` (15 commits,
96 ported (b)/(c) items per `integration/port-log.json`) is merged to `main`
as ONE non-fast-forward checkpoint commit, tagged `wave-1-3`. Union
verification (INTEGRATION-T03): 14/14 checks PASS, 0 FAIL, 0 SKIP, plus 1
honestly-classified open finding (P2T-04-AZ, below). All GA gates were measured
against the single integrated tree, not each workstream's own branch lineage.

## What shipped

The retrofit carried the genuinely missing waves 1-3 work onto an integrated
main. Every (b)/(c) PR from the INTEGRATION-T01 worklist has a record in
`integration/port-log.json` (96 records: 84 (b) genuinely missing + 12 (c)
superseded/conflicting). Result: 95 ported/covered + 1 waived-resolved.

- P0/P1/P2: docs, tools, corpus waves (PR 31 P1 report + P1-M3 sources; PRs 3,
  11, 13, 19-21, 25-27, 29, 30, 33-37, 51, 58, 79, 85).
- P4: PMI/QIF/MBD lane (PRs 71, 87-94, 98).
- P5: research lane + JT reader (PRs 96, 97, 100-102, 105, 106, 108).
- P6: oep-geo math foundation (PRs 72-76, 80).
- P7: modeler crates (PRs 103, 104, 110, 111, 113, 114), workbench + sketcher
  (PRs 107, 112).
- P8: PDM + P10 lane (PRs 116, 118, 123, 125, 129, 148, 149), folder-sync
  client (P8-M5-T01).
- oep-cgr reader modules (PRs 32, 38, 41-46, 48-50, 52-54); oep-3dxml
  reader/writer (PRs 56, 57, 59-62, 66, 69, 70).
- New crates registered in the workspace: oep-pmi, oep-step-pmi, oep-qif,
  oep-import, oep-shell, oep-kinematics, oep-rules, oep-sketch, oep-archive.
- P12-T06 release install rehearsal ported onto the union (d97c7e9).
- SOLVER-REPLACE (201d016): GPL-3.0 SolveSpace tree REMOVED; own bounded
  damped-Gauss-Newton solver behind the unchanged oep-assem seam. Program
  decision #16, ADR `workstreams/P7-M3/solver-adr.md` v1.2.0. This resolves the waived
  PR 115 (oep-assem GPL-3.0).

## Deferrals and waivers (with reasons)

- PR 115 / P7-M3-T01 oep-assem (GPL-3.0 SolveSpace): originally WAIVED from
  the port (decision #16) because vendoring GPL-3.0 into the Apache-2.0
  ITAR-clean product fails defense procurement review and the upstream author
  cannot relicense. RESOLVED, not carried: SOLVER-REPLACE replaces it with a
  permissive own solver behind the vendor-neutral seam. Verified: license
  cleanroom gate 3/3 green; no solvespace/slvs/GPL engine in the tree or in
  Cargo.lock.
- Object-store corpus entries classify SKIP (not materialized on U1; NAS stays
  repo/storage only) - the documented integration pin, not a failure.
- Real CGR material (piston/stc06/ATOMIZER set) is dev-only reference, never
  committed; P2/P9 golden rows SKIP honestly rather than fabricating PASS.
- 44 in-main redundant PRs were closed as already satisfied by INTEGRATION-T01.

## Gate results (all against the single integrated tree)

| check | result |
|---|---|
| rustfmt (CI lint) | PASS |
| clippy -D warnings | PASS |
| cargo build + test --workspace | PASS |
| precommit quality gate | PASS (pytest 431/22, cargo test) |
| full pytest suite | PASS (431 passed / 22 skipped) |
| golden-file corpus harness | PASS |
| P2 (CGR load + golden) | PASS (2 rows, 0 FAIL) |
| P3 (3DXML) | PASS (141-row sweep, 7 real OPEN, 134 SKIP, 0 FAIL) |
| P4 (STEP fidelity + MBD) | PASS (14/14, MBD report 22 inputs / 374 findings) |
| P8 (multi-user PDM) | PASS (10/10: locks, 409, immutability, audit) |
| P9 (100-job wave) | PASS (100/100 hash-verified) |
| P10 (fresh install) | PASS (CI-safe 2 PASS/4 SKIP; live proof 6/6) |
| P11 (auth-boundary) | PASS (auth_boundary 3/3, authz 19/19, rbac 3/3) |
| P12 (install rehearsal) | PASS (acceptance 5/5) |
| P10/P12 live two-machine install | PASS (U1 + keelwb2: 4/4, exit 0) |

Evidence: `integration/union-gate.{json,md}` at the wave/1-3 tip; fresh
`workstreams/P12/proof/release-rehearsal-0.1.0.{json,md}`.

## Known risks and open findings

- P2T-04-AZ (HIGH, owner P11 lane continuation): authz is default-open on the
  shipped binary; the P11-T03 boundary is opt-in. BLOCKS the release drill
  until fixed or explicitly waived. Remediation spec (secure by default +
  `--no-auth` opt-out + attack-matrix tests vs shipped default) is recorded in
  kanban comment 146 for the owning lane. Descendant cards SEC-AUTHZ-DEFAULT
  (secure-by-default) and P11-T06 were delivered after this union head; they
  are not in this checkpoint and follow the next integration.
- P3: 7 committed 3DXML files are real OPEN (unparameterized) - classified
  honestly, not failures.
- Integration drill CI jobs (wave-3 PRs #163-#177) queue behind the saturated
  serial runner; exact job steps are green in a CI-sim. This checkpoint's own
  gate evidence is local on U1 + keelwb2, recorded in `integration/union-gate.*`.

## Decisions taken

- Program decision #16 (2026-10-18): replace GPL-3.0 SolveSpace through the
  vendor-neutral seam with a permissive own bounded solver; solverang was
  evaluated and rejected with evidence. ADR v1.2.0 supersedes v1.1.0.
- Worktrees are ephemeral: stale worktrees pruned in this step (policy section
  6). Card branches remain as refs for provenance.
- Repository bound (policy section 6): exactly keelworks + keelworks-canary
  (+ grandfathered archived oep-meta). `wave_checkpoint.py repos` confirms no
  new repositories.

## Cards and uncarded commits

Cards: 0 card()-formatted commits (the retrofit used INTEGRATION-T0x port
commits, not per-card squash commits). Commits: 15. Uncarded: 15.

## Uncarded commits (the port and verification commits)

- `301f5a73b` INTEGRATION-T02: port oep-geo P6 math foundation (PRs 72,73,74,75,76,80)
- `805fe4598` INTEGRATION-T02: port oep-cgr reader modules (PRs 32,38,41-46,48-50,52-54)
- `3c1a20a1c` INTEGRATION-T02: port oep-3dxml reader/writer modules (PRs 56,57,59,60,61,62,66,69,70)
- `2557c0a10` INTEGRATION-T02: port P4 PMI/QIF/MBD lane (PRs 71,87-94,98)
- `44f17d57a` INTEGRATION-T02: port P0/P1/P2 docs, tools, corpus waves (PRs 3,11,13,19-21,25-27,29,30,33-37,51,58,79,85)
- `6db841cf0` INTEGRATION-T02: port P5 research lane + JT reader (PRs 96,97,100,101,102,105,106,108)
- `5cb69575e` INTEGRATION-T02: port P7 modeler crates (PRs 103,104,110,111,113,114)
- `23b6ca124` INTEGRATION-T02: port P7-M2-T01 workbench + P7-M4-T01 sketcher (PRs 107,112)
- `978934f3a` INTEGRATION-T02: port P8 PDM + P10 lane (PRs 116,118,123,125,129,148,149)
- `a00d2b3b1` INTEGRATION-T02: (c)-item dispositions + P8-M5-T01 folder-sync client
- `d57768923` INTEGRATION-T02: complete port log + record; P1 report (PR 31) + P1-M3 sources
- `d97c7e9e6` feat(P12-T06): release install rehearsal (fresh clean machine per release)
- `790db6a9a` INTEGRATION-T03: fix union gate defects + rustfmt normalize
- `201d01601` SOLVER-REPLACE: permissive bounded solver behind the unchanged oep-assem seam
- `0acde1e6a` INTEGRATION-T03: union verification proof - all gates green on integrated main
