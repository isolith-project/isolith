# P12-T05 -- Release train: dry-run train cut (candidate 2027.1.0-rc1)

> Task: t_f23f86a1 (P12 release engineering, wave 3). Status: implemented +
> dry-run verified on the U1 run target (promaxgb10-83f1, 192.168.0.67)
> 2026-10-18. Policy: `workstreams/release-engineering/release-train.md` (version 1.0.0, ratified).
> Repo: keelworks/keelworks @ NAS Gitea; branch task/p12t05-release-train-1018a.

## 1. What this card delivers

The Keelworks release train, exercised end to end on the current program head:

| Acceptance (card) | Measured result (live) |
|---|---|
| release-train doc lives at workstreams/release-engineering/release-train.md | committed, version 1.0.0, and asserted by `tests/test_p12release_train.py` |
| a release cut produces a tagged branch with a frozen set of commits | `release/2027.1` cut at e67b8fcc; **frozen set = 99 commits (41 first-parent), tree a846b80**, recorded in `workstreams/release-engineering/freeze-manifests/2027.1.0-rc1.json`; annotated tag `v2027.1.0-rc1` |
| a canary repo receives the candidate before GA | `keelworks/keelworks-canary` created; received `canary/2027.1.0-rc1` @ e67b8fcc and `v2027.1.0-rc1` (read back via API, shas match the cut) |
| patch policy states when a patch is cut vs the next train | `workstreams/release-engineering/release-train.md` section 6: P0/P1 regression, data-integrity, security, or release-blocking packaging defect on a shipped GA line -> patch `<train>.<patch+1>` cut from the release branch via canary; everything else -> next quarterly train |
| run one dry-run train cut on the R1A/U1 run target | done here; full record in `workstreams/release-engineering/proof/dry-run-cut-2027.1.0-rc1.json` |

## 2. The cut, verbatim

```
python/oep_tooling/release_train.py release-cut \
  --version 2027.1.0-rc1 --trunk task/p10m5t02-ga-gate-1018a \
  --canary keelworks/keelworks-canary
```

- **Trunk rationale**: `main` still carries only the P0 bootstrap (5 merged
  PRs; 148 open) pending wave-3 consolidation, so the first cut is taken from
  the program integration head (the stacked tip). Trunk pointer kept at
  `workstreams/release-engineering/release-trunk`; the policy resolves to `main` in steady state
  (release-train.md section 7).
- The tool writes the freeze manifest first (immutable git facts: sha, tree,
  commit counts), creates the release branch and annotated tag idempotently
  (refusing to rewrite an existing ref at a different sha), and pushes the
  candidate to the canary remote -- credentials never embedded in the remote
  URL, host resolved from git-credentials at runtime.
- `verify` recomputes the manifest from the recorded sha and passed (the
  frozen set is faithful to the object database).

## 3. Verify-readback (live Gitea state)

- canary branches: `main` (0af3f9b8), `canary/2027.1.0-rc1` (e67b8fcc)
- canary tags: `v2027.1.0-rc1` (e67b8fcc28ed)
- both shas equal the freeze manifest's `cut_from.sha` -- the canary holds the
  exact frozen object.

## 4. Health of the frozen tree (run targets)

- pytest: 108 passed (includes the new `tests/test_p12release_train.py`)
- cargo test --workspace: 0 failures
- Precommit quality gate and the org CI workflows run on the task branch
  before merge (see PR).

## 5. What is deliberately NOT done

This dry run is not a product release. No GA channel was touched; the rc tag
lives only in the canary repo (per policy, nothing publishes off the staged
path). Promotion to GA (`v2027.1.0`, release notes) is a separate, gated step
(release-train.md sections 5 and 8). P12-T04 binary packaging and P12-T07
changelog automation are separate cards; the canary pre-release entry records
the source candidate today and will carry artifacts once packaging lands.

## Change log

- 1.0.0 (2026-10-18) -- dry-run cut 2027.1.0-rc1 performed and recorded.
