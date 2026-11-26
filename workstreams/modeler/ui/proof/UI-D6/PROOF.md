# UI-D6 UI-WAVE-D-INTEGRATE: merge, gate, checkpoint, screenshots, tag (proof)

Card: `t_62060655`. Branch: `ui/d6-integrate`. Owned files (exactly):

- `docs/status/waves/wave-ui-d.md` (new wave record)
- `workstreams/modeler/ui/README.md` (UI-D6 row)
- `workstreams/modeler/ui/proof/UI-D6/` (this dir: proof + gate/landing transcripts + screenshots)
- `workstreams/modeler/ui/proof/ui-a2/frame.png*` (only if the golden legitimately
  drifted - it did not on this run; `shot.sh` passed byte-identical to the
  UI-D12-refreshed golden `56afced6...`, no refresh needed)

No crate was edited and no file outside the owned list was touched (the one
side-effect of a gate leg - the `docs/proof/licensing-check.json` timestamp
rewrite from `licensing-check.py` - is outside this card's owned list and was
reverted, exactly as UI-D12 did).

## History: attempt 1 blocked, UI-D12 unblocked

Attempt 1 of this card (2026-11-26) recorded a hard block: the then-current
merged wave head `1097f76` failed four of six gate legs, every red root cause in
files this card does not own (D3/D5 fmt + clippy, D1-D10 em-dash text guards,
MEDIA-RENDERER present PNG drift, and shot golden drift with no delegating card).
The full blocked analysis lives in this directory's git history on
`ui/d6-integrate` (`e7530d9`). Instead of faking green, the orchestrator cut
UI-D12 (UI-GATE-FIX, `t_633acec0`), which landed the mechanical fixes on
`wave/ui-d` as `91a07a5`. This run re-runs the whole close on the new wave head.

## STEP 0

`git fetch github && git merge github/wave/ui-d` from the card worktree. NOT
empty: brought the UI-D12 landings (card `e6bcf13` + `merge-UI-D12` `91a07a5`,
40 files incl. the UI-D12 proof and the refreshed golden) and merged as
`39425ee` on `ui/d6-integrate`. Transcript: `transcripts/00-step0-merge.txt`.

Every wave-D card branch IS an ancestor of `github/wave/ui-d` (audit
`transcripts/08-ancestors.txt`, 10/10): `ui/d1-gpu-backend`,
`ui/d2-gpu-shading`, `ui/d3-gpu-pick`, `ui/d4-gpu-nav`, `ui/d5-perf`,
`ui/d7-render-fix`, `ui/d8-vec4`, `ui/d9-perf-profile`,
`ui/d10-resident-scene`, plus the gate-fix branch `ui/d12-gate-fix`.
`ui/d11-depth-sort` (UI-D11, deferred per the orchestrator addendum) has no
remote branch at all and is correctly NOT an ancestor; it is recorded in the
wave record with the deferral reason.

## Checkpoint merged head

- `git switch -c checkpoint/wave-ui-d-91a07a5 github/main` then
  `git merge --no-ff github/wave/ui-d` -> checkpoint commit `881405a`,
  parents `bed3d50` (github/main) + `91a07a5` (wave/ui-d head).
- `main` is NOT an ancestor of the wave head: main carries two post-wave-C docs
  PRs (readme-refresh `abb8206`, brand-avatar `ea4dddf`) that never rode the
  wave. The checkpoint tree therefore differs from the wave head only by those
  two docs commits; `git diff --stat HEAD github/wave/ui-d` shows exactly
  `README.md`, `assets/brand/*`, `assets/readme/*`, `docs-site/index.html` -
  no crate, no test, no script, no committed artifact touched. None of the six
  gate legs is affected by them (proven below).

## Full gate on the checkpoint merged head `881405a`

Transcripts in `transcripts/checkpoint-merged-head/` (real command output):

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0, `Finished dev profile in 51.74s`, all targets incl. tests) |
| `cargo test --workspace` | PASS (exit 0) - 225 suites, aggregate 4608 passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (exit 0) - 6/6; 34 crates match the boundary map; 535 deps in the permissive allow-list; NOTICE 535 + 1 native |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped, 0 failed; `cargo test` green; gate PASS |
| `bash scripts/ui/shot.sh` | PASS (exit 0) - frame `56afced6473393b20a00388e1a1057032b2eeb0d6721fe01caebc43b23fd0f45`, byte-identical to the UI-D12-refreshed golden across two launches, NO drift |

`scope_register_check` (supplementary leg, same as wave C and attempt 1):
23 pass / 0 fail with `docs/scope-register.json` and `.md` byte-identical
(no wave-D test proves a register row). Transcript:
`transcripts/checkpoint-merged-head/07-scope-register.txt`.

### Precommit gate: one environment fix + the documented kittest flake

The precommit gate exited 1 on its first three runs for two reasons that are NOT
tree defects (the full workspace gate and every other leg were already green):

1. Attempt 1: the gate's python resolution picked the Hermes venv `python3`
   (no pytest): `[FAIL] pytest - No module named pytest`. Re-run with
   `VIRTUAL_ENV=/tmp/isol-gate-venv` (a throwaway `python3.12` venv with
   `pytest 9.2.0`), which gave a clean `pytest 558 passed`.
2. Attempts 1-3: precommit's `cargo test -q` leg tripped the documented
   egui_kittest exit-time SIGSEGV flake (`pick_gpu`, `feature_tree` - the
   UI-D12/D4/D8-recorded class; all assertions pass, the harness process dies at
   teardown under the full parallel suite). `cargo test --workspace` on this
   very tree passed clean 4608/0. Attempt 4 completed both legs clean:
   `gate PASS`.

All four attempts are preserved verbatim:
`05-precommit-flake1.txt` (the combined pytest-venv miss + feature_tree SIGSEGV
of the very first run), `05-precommit-flake2.txt`, `05-precommit-flake3.txt`
(the re-run flakes), and `05-precommit.txt` (attempt 4, the clean PASS). The
retry loop and venv creation are environmental, not repo changes; no test was
weakened, skipped or deleted.

## Screenshots (acceptance: real-binary frames of the merged head)

Three real-binary frames captured from the merged-head `isolith` binary
(`target/debug/isolith`, built from the gated tree) under `xvfb-run`
1280x800x24 via the XTEST injector (`ui_d6_inject.c`, `capture.sh`) - the same
pattern as `proof/ui-c7` and `proof/UI-D3`. Two independent launches per mode
must produce byte-identical PNGs (the script hard-fails on nondeterminism); the
`.sha256` is the oracle.

| Shot | sha256 | Shows (verified visually and by pixel diff) |
|---|---|---|
| shaded | `7fdf02ec1e1c60112b9aa400eaebbeb0aa54f04166f20e3363718f13311a3d41` | default studio launch: the GPU-shaded demo solid (UI-D10 resident painter path), the UI-D2 Shading card top-right with **Shaded** selected, model tree + properties sidebars, status bar |
| view-cube | `29e8fab759071574c2c6b5f98993fc72fef877e04fca34716e05b47308ff1eb3` | the UI-D4 Orientation card (six cube faces in the cross/diamond arrangement + Front/Top/Right/Iso quick buttons + Section + `Triangles: 12`) after clicking **Iso** (center 809,688, from UI-D4's layout probe); pixel diff vs `shaded` = 122,210 changed pixels across the whole viewport, i.e. the Iso click landed the Iso standard view |
| picking | `5678b384338a70b9b8c9549ccd11a0abac6164be37c1c979f60b4ce8a569c84e` | UI-D3 GPU box-select captured MID Alt+left-drag (360,300)->(880,520): the rubber band frames exactly the drag rect (diff vs `shaded` confined to a 522x222 box at (359,299)-(881,521), the D3 gesture region) over the shaded solid |

No image was fabricated; each frame is a real launch of the merged-head binary,
driven through XTEST, and each is deterministic (byte-identical x2).

## Ancestors

`transcripts/08-ancestors.txt` - see STEP 0 above (10/10).

## Folding into the checkpoint

This record, the gate transcripts, the screenshots and the wave record
(`docs/status/waves/wave-ui-d.md`) are folded into the single no-ff checkpoint
commit `881405a` by amending it (`--amend --no-edit`: same parents, code tree
untouched, docs only). The amended commit is the one pushed, PR-merged and
tagged. The landing evidence (PR, main sha, tag, Gitea mirror, `wave/ui-e`
advance) is recorded under `transcripts/landing/` after the landing, on the
wave branch, per the wave-close convention.
