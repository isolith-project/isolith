# INTEGRATION-T03: Union verification — integrated main is a buildable, test-passing whole

Verified on U1 promaxgb10-83f1 (aarch64) + the keelwb2 U1/VM-class host (decision #9).
NAS stays repo/storage only. Branch `wave/1-3`, base `d22800b`, this proof at head
`790db6a` + the proof commit. Release tag: stack OEP_VERSION 0.1.0 (2027.1 train).

This is the first time the GA gates are measured against the UNION of the program
rather than each workstream's own branch lineage. Composition is now an explicit,
measured step, not an assumption.

## Result

    15 checks: PASS 14, FAIL 0, SKIP 0
    1 open finding: P2T-04-AZ (HIGH, authz default-open on the shipped binary), owner P11 lane — blocks the release drill until fixed or waived.

## Full quality gate, one pass on the union

| check | result | evidence |
|---|---|---|
| rustfmt (CI lint) | PASS | `cargo fmt --all -- --check` green (9 files normalized by T03, format-only) |
| clippy -D warnings | PASS | `cargo clippy --workspace --all-targets -- -D warnings` exit 0 |
| cargo build + test --workspace | PASS | `cargo test --workspace` green (exit 0) |
| precommit quality gate | PASS | vendored hook: pytest 431/22, cargo test, gate PASS |
| full pytest suite | PASS | 431 passed / 22 skipped (documented corpus/NAS legs) |
| golden-file corpus harness | PASS | corpus_harness_test PASS; golden rows SKIP honestly (real CGR dev-only, clean-room) |

## GA gates against the single integrated tree

| gate | result | evidence |
|---|---|---|
| P2 (CGR load + golden) | PASS | acceptance green after golden_leg corpus_root seam fix; runner 2 rows 0 FAIL exit 0 |
| P3 (3DXML) | PASS | acceptance green after CDATA probe fix; OPEN-leg sweep: 141 rows, 7 real OPEN (6 committed 3DXML + box.stl), 134 classified SKIP, 0 FAIL |
| P4 (STEP fidelity + MBD) | PASS | 14/14 real measured PASS, 0 FAIL; MBD report regenerated richer (22 inputs / 374 findings) |
| P8 (multi-user PDM) | PASS | real service, 3 concurrent clients: 10/10 PASS (locks, 409, immutability, audit) |
| P9 (100-job wave) | PASS | 100/100 ok, hash-verified 100, vault re-hash + audit re-verified, 0 FAIL |
| P10 (fresh install) | PASS | CI-safe 2 PASS/4 SKIP; committed live proof 6/6 (A+B + stack + airgap) |
| P11 (auth-boundary, wave-3) | PASS | auth_boundary 3/3, authz 19/19, rbac 3/3, acceptance PASS — see P2T-04-AZ |
| P12 (install rehearsal, wave-3) | PASS | acceptance 5/5; live TWO-MACHINE rehearsal below |

## P10/P12 live two-machine install rehearsal on the integrated stack (decision #9)

`scripts/install-rehearsal.sh` (ported to the union by this card) — fresh clean-state
one-command install of the INTEGRATED 0.1.0 stack on both approved hosts:

    RELEASE/version   PASS   install.sh + compose + .env.example pin OEP_VERSION=0.1.0
    DOCS/runbook      PASS   runbook names 3 health surfaces + 2 approved host classes
    INSTALL-A (U1)    PASS   pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present
    INSTALL-B (keelwb2) PASS  same on the second approved host

    total=4 PASS=4 FAIL=0 SKIP=0  exit_code 0
    proof: workstreams/release-engineering/proof/release-rehearsal-0.1.0.{json,md} (regenerated on the integrated head)

## Real integration defects fixed by this card (not waived)

1. **P12-T06 install rehearsal was missing from the union** — cherry-picked
   (d97c7e9); now part of the tree with ga-gate-p12.yaml wired.
2. **P2 GA gate CI job missing** from the union's ci.yaml — restored (the P2 gate
   never ran in CI on integrated main).
3. **golden_leg seam**: the escalated corpus runner dropped the gate's corpus
   scope, so the P2 acceptance could not detect a corrupted corpus. Fixed
   (`corpus_root` now flows through runner.run_spec/run_test + ga_gate).
4. **3DXML CDATA Manifest**: real Dassault 3DXML files wrap `<Root>` in CDATA; the
   clean-room probe rejected them. Fixed; the committed 3DXML corpus now OPENS.
5. **rustfmt red on the union** — normalized (format-only); the CI lint bar now
   holds on integrated main.

## Open findings (honest classification with owner)

- **P2T-04-AZ — HIGH — authz default-open on the shipped default binary**
  (owner: P11 lane, P11-T03 continuation). The P11-T03 authz boundary is OPT-IN:
  `oep-pdm` and `oep-pdm-stub` serve WITHOUT an authorizer unless the operator
  uses the in-process `start_with_authorizer` path, and the committed auth-boundary
  suite never asserts the shipped default binary denies the attack matrix. The
  pen-test tracked this to P11-T03, but on the union it is substantively open.
  This BLOCKS the release drill (drill acceptance requires pen-test findings
  resolved or waived). A remediation spec (secure by default + `--no-auth` opt-out +
  attack-matrix tests vs the shipped default) was observed generated transiently in
  this worktree by an untraced process and NOT adopted (see kanban comment 146);
  the owning lane has the spec recorded there.

## Decisions recorded

- Object-store corpus entries classify SKIP (not materialized on U1; NAS stays
  repo/storage only) — this is the documented integration pin, not a failure.
- Real CGR material (piston/stc06/ATOMIZER set) is dev-only reference, never
  committed; the P2/P9 golden rows SKIP honestly rather than fabricating PASS.
- Transient untraceable source edits (SEC-AUTHZ-DEFAULT) observed in this worktree
  were reverted and NOT committed; flagged in kanban comment 146 as a
  coordination hazard.

## Artifacts

- integration/union-gate.json (this record, machine-readable)
- integration/union-gate.md (this record)
- workstreams/release-engineering/proof/release-rehearsal-0.1.0.{json,md}
- corpus/ga-gate-p4-mbd-report.{json,md} (regenerated P4 MBD report on the union)
