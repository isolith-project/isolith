# P12-T05 -- Release train: cut branch, freeze, staged canary rollout, patch policy

> Task: t_f23f86a1 (P12 release engineering). Status: implemented + verified
> live on the U1 run target 2026-10-18. Version 1.
> Repo: keelworks/keelworks @ NAS Gitea; branch task/p12t05-release-train-1018a.
> Policy record: P12/release-train.md v1.0.0 (Ratified).
> Gate classification: **PASS** -- one real dry-run train cut executed end to
> end on the program integration head; candidate delivered to the canary repo
> and read back; frozen-set manifest verified faithful to the git object
> database.

## 1. What this card delivers

The operating policy and the running machinery for how Keelworks ships,
implementing the P0-M5-T03 decision (semver + calendar versioning, e.g.
2027.1):

| Acceptance | Measured result (live) |
|---|---|
| release-train doc at P12/release-train.md | committed v1.0.0; contents asserted by tests/test_p12release_train.py |
| a release cut produces a tagged branch with a frozen set of commits | release/2027.1 @ e67b8fcc; frozen set = 99 commits (41 first-parent), tree a846b80; annotated tag v2027.1.0-rc1; manifest P12/freeze-manifests/2027.1.0-rc1.json |
| a canary repo receives the candidate before GA | keelworks/keelworks-canary created; canary/2027.1.0-rc1 + v2027.1.0-rc1 pushed and read back at the exact cut sha |
| patch policy states when a patch is cut vs the next train | release-train.md section 6: P0/P1 regression, data-integrity bug, security advisory, or release-blocking packaging defect on a shipped GA line -> `<train>.<patch+1>` patch train cut from the release branch (never main) via a fast-tracked canary soak; all other changes -> next quarterly train |
| verified by one dry-run train cut on the R1A/U1 run target | executed; measured proof at P12/proof/dry-run-cut-2027.1.0-rc1.{json,md} |

## 2. New surface

- **`P12/release-train.md`** -- the binding policy: quarterly calendar train
  (`YYYY.N`), branch model (`release/<train>` cut from the trunk, trunk =
  `main` in steady state, program integration head during wave-3), freeze
  manifest contract (`P12/freeze-manifests/<version>.json`,
  `keelworks.release-freeze.v1`), staged canary rollout
  (`keelworks/keelworks-canary`) with a defined soak/promotion gate, and the
  patch/hotfix trigger list.
- **`python/oep_tooling/release_train.py`** -- tooling: `validate`
  (version scheme), `freeze` (read-only manifest from real git facts), `verify`
  (recompute-and-compare fidelity), `release-cut` (manifest + release branch +
  annotated tag + canary push; idempotent; `--dry-run`; credentials never
  embedded in remote URLs).
- **`tests/test_p12release_train.py`** -- acceptance: version scheme matrix,
  policy-doc coverage, freeze/verify round-trip, tamper detection, and a
  hermetic end-to-end release-cut against a local bare canary repo.
- **`P12/release-trunk`** -- current cut point pointer (program integration
  head; flips to `main` on consolidation).
- **`P12/freeze-manifests/2027.1.0-rc1.json`** -- the dry-run candidate's
  frozen set.
- **`P12/proof/`** -- measured dry-run cut record.
- **`keelworks/keelworks-canary`** (new Gitea repo) -- staged-rollout channel,
  README-seeded, pre-release entry for the candidate.
- **docs/layout.md** -- P12 lane materialized.

## 3. Verification

- pytest 108 passed on the frozen tree (includes the new acceptance test).
- cargo test --workspace 0 failures.
- `release_train.py verify` passed against the committed manifest.
- Canary repo branch/tag read back via API: shas == cut sha.
- release/* branch protection enabled on keelworks/keelworks to enforce the
  feature freeze (no direct pushes, no force).

## Change log

- 1.0.0 (2026-10-18) -- delivered.
