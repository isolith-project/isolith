# Isolith wave checkpoint: wave-4

Schema: isolith.wave-checkpoint.v1
Branch: `wave/editor` -> `main`
Base sha: `75c0027a10cd330f8f3dd00d39f0e858bed099c4`
Tip sha:  `8f0d9753e58e7f6ffbe18e0cf1b8e4f4663964f5`
Emitted: 2026-10-22 (U1, promaxgb10-83f1), seal card t_0e2a6fa7

## Result

Wave 4 (the editor spine wave) closes with this checkpoint. At seal time
`wave/editor` was 1 commit ahead of `main` (INTERACTIVE-SHELL, 8f0d975); the
other eight editor-spine cards were already on `main` via their own merges.
`wave/editor` is merged to `main` as one fast-forward to the commit that adds
this checkpoint, and is tagged `wave-4`. All nine editor-spine cards are on
`main`; no editor work remains on a branch or in an open PR. `main` is pushed
and remote == local (verified).

Audit findings at seal, all addressed:

- Two stale worktrees held a staged rewind with zero unique content.
  `.worktrees/main` had an index rewound to the 77dab6b tree (a staged undo
  of KERNEL-REPLAY); `.worktrees/editor` had an index rewound to the 2fe7f1b
  tree (a staged undo of MASS-PROP + KERNEL-REPLAY + INTERACTIVE-SHELL). Both
  targets are ancestors of `main`, so discarding them loses nothing.
- Twelve open PRs remained from before the wave integration policy (P11, P12
  and P13 era, base `wave/1-3` or `main`). Every head was audited against
  `main`: eleven are ancestors of `main` (fully merged) and one (PR #170,
  kernel-seam ADR v1.1) is superseded by the landed ADR v1.2.0 / v1.3.0 and the
  OCCT engine on the seam. None held work missing from `main`. All twelve were
  closed as already satisfied / superseded with the seal note (policy section
  9), and their remote branches deleted.

## What shipped

The nine editor-spine cards, all on main:

| card | sha on main | PR | summary |
|---|---|---|---|
| DECISION-19 editor spine | 9c2fb65 | direct | decision record + 2027.1 retitle to interop/PDM foundation |
| FEATURE-BASELINE | 6229781 | direct | CATIA/SolidWorks parity baseline in the scope register |
| SURFACING-CLASS-A | 3c15781 | direct | two-track Class-A surfacing ADR, P15 decomposition, honest cost |
| EDITOR-ASSEM-EDIT | f37c648 | direct | assembly mate editing, live re-solve, PPM viewport, mechanism motion |
| KERNEL-BASE | 584084f via 894aaca | #181 | oep-engine-occ OCCT 8.0.1 seam, capability matrix 16/16 |
| EDITOR-2D-DRAFTING | 2fe7f1b via 894aaca | direct | oep-drafting workbench, projected views, SVG/PDF export |
| EDITOR-MASS-PROP | 77dab6b | direct | oep-analysis mass properties, clash, exploded views |
| KERNEL-REPLAY | 75c0027 | #182 | feature-graph replay produces real OCCT solids, goldens locked |
| INTERACTIVE-SHELL | 8f0d975 (sealed here) | direct | mouse-grade EditorSession on oep-shell + oep-viewer |

## Gate results

Measured on the merged code head `8f0d975` (the sealed delta; this
checkpoint adds documentation only, which no test consumes), from a clean
build (cargo clean first, per the wave's authoritative-green methodology):

| check | result |
|---|---|
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets -- -D warnings | PASS, no warnings |
| cargo test --workspace | PASS, 185 suites, 1156 tests, 0 failed |
| full pytest tests/ | PASS, 530 passed / 28 skipped, 0 failed |
| wave_checkpoint.py verify (wave-4.json) | PASS |
| tests/test_editor_spine_decision.py | PASS (in the pytest run) |
| tests/test_wave_integration_policy.py | PASS (in the pytest run) |

Re-verified after the merge on `main` == the checkpoint commit: identical
result plus `wave_checkpoint.py repos` PASS (two active repos, one archived
legacy).

## Deferrals and waivers (with reasons)

- Machine B leg of the editor headless smoke rehearsal (keelwb2 VM) was
  recorded SKIP by INTERACTIVE-SHELL: the VM was unreachable over SSH when the
  card ran. The rehearsal script (scripts/editor-headless-smoke.sh) implements
  and re-runs the leg when a second host is up. Machine A (U1) PASS is
  recorded in the committed proof. Not a functional deferral.
- No editor-spine card deferrals functionality. 2027.1 stays the interop/PDM
  foundation release, not the editor GA (decision #19); a follow-up train will
  cut when the editor surface is the product bar.
- Single Gitea runner backlog: the merged `wave/editor` and `main` refs queue
  CI jobs behind the serial runner. Every card replicated its exact CI gate
  locally against the real OCCT 8.0.1 during the wave; this checkpoint's gate
  evidence is the local run above.

## Decisions taken

- Program decision #19 (2026-10-22): the editor is the product spine;
  2027.1 is the interop/PDM foundation release; no reader-only release claims
  to be the editor. Supersedes decision #13 default-exclude for Class-A.
- SURFACING-CLASS-A (ADR v1.0.0): modeling-grade surfacing on the OCCT seam in
  the near-term editor train (honestly labeled not Class-A), plus an own
  permissive Class-A surface core behind the same seam as the P15
  strategy-gated long program. No GPL, no commercial licensing.
- Kernel seam ADR v1.1.0 -> v1.2.0 (KERNEL-BASE) -> v1.3.0 (KERNEL-REPLAY):
  OCCT 8.0.1 behind the oep-geo Evaluator seam; MiniBox demoted to a
  documented test-only reference; goldens byte-locked.
- Accessible CLI lane: single-source commands.rs registry (help index 1.2.0)
  with three drift-guard surfaces updated in lockstep across all editor cards.

## Repo hygiene at seal

- Repositories: exactly `isolith` + `isolith-canary` active, plus the
  grandfathered archived `oep-meta` (policy section 6 allows the one legacy
  repo and the tooling reports it as a warning). Verified via Gitea API and
  `wave_checkpoint.py repos`.
- Open PRs at seal: 12, all pre-wave (P11/P12/P13 era). All audited and
  closed as in-main (11) or superseded (1, PR #170).
- Remote branches deleted: task/editor-mass-prop-1022a,
  task/interactive-shell-1022a, task/p6m3t04-kernel-seam-adr-v11.
- Local branches deleted: task/editor-assem-edit-1022a,
  task/editor-mass-prop-1022a, task/interactive-shell-1022a,
  task/kernel-base-occ-1022a, task/kernel-base-occ-1022b,
  task/p6m3t04-kernel-seam-adr-v11, task/surfacing-class-a-1022a,
  editor-mass-prop-rebase, p11t03-remote-head.
- Stale worktrees pruned: .worktrees/editor, .worktrees/editor-mass-prop-1022a,
  .worktrees/interactive-shell-1022a, .worktrees/surfacing-class-a-1022a,
  .worktrees/wave-editor. Kept: .worktrees/main and the status/nightly
  checkout at /home/r1a/isolith-nightly-status (pushed on the remote).
- Canary untouched: isolith-canary was only read at seal. No new train, no
  release, no push to canary. This was a wave close, not a release.

## Cards and uncarded commits

Cards: 0 card()-formatted commits (the wave landed as direct merges and
wave/editor fast-forwards, not per-card squash PRs). Commits: 1 (the sealed
delta). Uncarded: 1.

## Uncarded commits (fixups and merge plumbing)

- `8f0d9753e` feat(editor): mouse-grade interactive editing surface on oep-shell + oep-viewer (INTERACTIVE-SHELL)
