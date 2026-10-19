# P12-T07 -- release notes / changelog automation: dry-run proof

Generator: git-cliff 2.14.2 (single static binary, sha512-pinned in CI)
Config:   cliff.toml (conventional-commit category mapping, repo root)
Driver:   scripts/release/changelog.sh

Run: 2026-10-18 (UTC-07:00)

## Command

    bash scripts/release/changelog.sh 2027.1.0-rc1 \
      workstreams/P12/proof/changelog-v2027.1.0-rc1.md

## Material

The changelog was generated from the ACTUAL commit history of the dry-run
release tag v2027.1.0-rc1 (P12-T05 release-train dry run), which sits on the
release branch base release/2027.1 at commit e67b8fc (main tip at the cut).
git-cliff walked the full history the tag sees: 100 commits, merge commits
excluded by the template, categorized by conventional commit type.

## Results

- Output: workstreams/P12/proof/changelog-v2027.1.0-rc1.md
- SHA-256: f139b86314fd0627ab3989fbd6cece31c0ced9818b157f0d950ca6bd5e1c272a
- Reproduction: re-running git-cliff --tag 2027.1.0-rc1 on the same commit
  yields the identical SHA-256 (deterministic, checksum-verified).

Category counts (from the generated changelog):

| category | entries |
|----------|---------|
| Features               | 37 |
| Tests                  | 20 |
| Documentation          | 9  |
| Bug Fixes              | 3  |
| Chores                 | 3  |
| Continuous Integration | 1  |
| Other                  | 1  |

Breaking changes: none in this history (pre-release tree, P0 versioning).

## CI path

- .gitea/workflows/release-changelog.yaml fires on the release cut: any tag
  push matching v20xx.* (calendar trains, P0-M5-T03) and on workflow_dispatch
  for a version + ref. It checks out the full history with tags, installs the
  pinned git-cliff (sha512-verified), runs scripts/release/changelog.sh, and
  attaches the markdown as Gitea release notes plus a versioned asset.
- .gitea/workflows/p12-t07-changelog.yaml runs the acceptance test
  (tests/test_release_changelog_acceptance.py) on every push/PR.

Verification rows:

| leg | id | status | detail |
|-----|----|--------|--------|
| GEN | config | PASS | cliff.toml parses; conventional_commits + categorized sections; calendar tag_pattern |
| GEN | driver | PASS | changelog.sh resolves the tag, renders, writes .sha256, prints counts |
| GEN | history | PASS | generated from tag v2027.1.0-rc1 commit history on release/2027.1 base |
| REPRO | regenerate | PASS | acceptance test regenerates over real history; sha256 identical (f139b863...) |
| CI | release-cut | PASS | workflow triggers on v20xx.* tags + workflow_dispatch, full-history fetch, attaches notes |
| CI | lane | PASS | per-delivery lane runs the acceptance test, CI-safe degrade without a binary |

Conclusion: P12-T07 acceptance criteria met (generator in CI on release cut,
categorized changelog from the real release history, dry-run train emits the
artifact, verification regenerates and matches).
