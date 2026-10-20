# oep-meta archive note

The Gitea repository `keelworks/oep-meta` was created on 2026-10-15 for card
P0-M1-T03 ("set up NAS Gitea org + repos: oep-meta plus per-workstream repos")
and is ARCHIVED. This note records what it held and where each artifact now
lives, so archiving loses nothing.

## Why it was archived

Decision #15 (2026-10-18, the wave integration and checkpoint policy,
`workstreams/release-engineering/wave-integration-and-checkpoint-policy.md` section 6) bounds the program to
exactly two repositories: `keelworks/keelworks` (the single canonical
development repo) and `keelworks/keelworks-canary` (the one canary for staged
rollout). Per-workstream repositories are superseded: workstreams are
directories inside the canonical repo. `oep-meta` is grandfathered as legacy,
receives no new work, and is archived. The hygiene check
(`python3 python/oep_tooling/wave_checkpoint.py repos`) enforces the bound and
reports the legacy repo until it is archived, then reports it as resolved.

## What oep-meta held, and its disposition

| artifact | bytes | disposition |
|---|---|---|
| `README.md` | program meta index | already present in the canonical repo; no action |
| `docs/layout.md` | 1764 | superseded: the canonical repo's `docs/layout.md` is the maintained version (larger and current); the oep-meta copy was an earlier snapshot |
| `tests/gitea_layout_test.py` | 5363 | PRESERVED byte-identical at `tests/gitea_layout_test.py` in the canonical repo (sha256 2518e6a41ff54d80...) |

The preserved test is the P0-M1-T03 acceptance test that checked the live Gitea
instance layout (org plus `oep-meta` plus the code repo plus the retired
`don/keelworks` path). It is retained for provenance. Its assertions are
superseded by decision #15: the live-repo check now lives in
`python/oep_tooling/wave_checkpoint.py repos`, and the in-repo layout check
lives in `tests/monorepo_layout_test.py`.

## Unarchiving

If the history is ever needed live again, a Gitea admin can unarchive the repo
(PATCH `/repos/keelworks/oep-meta` with `{"archived": false}`); the repository
was archived, never deleted, so its single commit `bf302037` remains.
