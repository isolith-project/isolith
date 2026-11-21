# Wave ui-a status record

Wave A of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`, section 5): the walking skeleton. Branch `wave/ui-a` cut from main at `0ccef03`.

Written by card UI-A8 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-A1 | UI-DEPS-BASELINE: eframe/egui 0.37, `crates/oep-app` | done |
| UI-A2 | UI-HARNESS: headless UI test and deterministic screenshot guard | done |
| UI-A3 | UI-THEME: `oep-ui-theme` semantic tokens, light default | done |
| UI-A4 | UI-LAYOUT: five-region shell, status bar, panel seam | done |
| UI-A5 | UI-VIEWPORT-CPU: backend seam and live frame | done |
| UI-A6 | UI-COMMAND-PALETTE: searchable command surface | done |
| UI-A7 | UI-OPEN-SAVE: open, save, recent files | done |
| UI-A9 | UI-SHELL-CONTRAST: separated panels (raised fill + hairline) | done |
| UI-A8 | UI-WAVE-A-INTEGRATE: merge, gate, checkpoint, screenshots | blocked |

Every wave-A card branch is an ancestor of the wave head, and every card's proof
dir is present on the wave head (audit in `proof/ui-a8/`).

## Head sha

- Wave head: `dd4725f` (`merge-UI-A8`; contains the UI-A8 licensing fix)
- Checkpoint merge onto local main: `3588429` (`checkpoint(wave-ui-a): walking skeleton -- cards UI-A1..UI-A9`), parents `8760cfe` (main) and `dd4725f` (wave head). **Local only - remote `github/main` is untouched** because the full gate is not green.
- Remote `github/main` still `8760cfe`; no annotated tag `wave-ui-a` has been pushed.

## Licensing fix (addendum)

UI-A3 added `crates/oep-ui-theme` to the workspace without a boundary row, so the
crate-boundary leg failed. UI-A8 added the `oep-ui-theme` row as Apache-2.0
(library under the applications/libraries split) to `docs/licensing-boundary.{json,md}`
and carried the crate count honestly: **34 workspace crates** (UI-A1 already
carried `oep-app` to 33; `oep-ui-theme` is the 34th). `license_map_total`
Apache-2.0 21 -> 22, AGPL-3.0 12. `docs/proof/licensing-check.json` regenerated.
All 6 legs PASS, 34 crates in sync. Landed on the wave as `merge-UI-A8` (`dd4725f`).

## Full gate on the merged head 3588429

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - incl. oep-app doc_io 10, harness 7, layout 8, palette 9, viewport 7, each with `contrast_test`; oep-ui-theme 4+4 |
| `python3 scripts/licensing-check.py` | PASS (6/6, 34 crates) |
| `bash scripts/gate/precommit-gate.sh` | **FAIL** - pytest `2 failed, 554 passed, 29 skipped` |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `e60b86dc…9a9b` byte-identical to UI-A9 golden |

The failing leg, in detail:

1. `tests/test_contributor_rules_decision.py::test_contributor_rules_decision_acceptance`
   fails on the merged head with `CODEOWNERS missing row for workspace member
   crates/oep-ui-theme (want 'crates/oep-ui-theme/.*')`. Root cause: UI-A3 added
   the crate without the CODEOWNERS row (P0-M1-T04). Reproduces on a clean
   `git archive` export, so CI fails it too. This is a **wave-A defect caught at
   the gate**. The fix is two lines beside the oep-app rows, but **CODEOWNERS is
   not in UI-A8's FILES YOU OWN list**, so this card blocks instead of editing it.
2. `tests/test_brand_identity.py::test_retired_codename_absent_outside_allowed_paths`
   fails only in the main worktree because of untracked on-disk sibling
   `.worktrees/` dirs not in `SKIP_DIRS`; it PASSES on a clean export of the same
   head. Environmental, not a wave defect, CI unaffected.

## Screenshots (shell / palette / viewport)

All three captured from the real merged-head binary under xvfb-run via an XTEST
injector (double-captured byte-identical; sha256 in `proof/ui-a8/`). They show
the separated panels (raised fill + hairline per region) as ADDENDUM 2 requires:

- shell: `e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4` (== UI-A9 golden)
- palette (Ctrl+K open): `9d63ef12ac0d81a3cef52791c0c7e69ce6fd3d653f01b9c23fcf61423f9a7d7c`
- viewport (model orbited): `f7cc1e582b56f97f67d2cf968f0b31dead06c99bf712866595088d47183234e1`

## What is NOT done / why the wave is not merged

- Remote `main` is untouched: the full gate is not green, so UI-A8 did not push
  the checkpoint, tag `wave-ui-a`, or merge a partial wave (card rules).
- **One wave-A defect remains**: `crates/oep-ui-theme` has no CODEOWNERS row.
  Ownership decision needed (needs_input): reopen UI-A3 / a small follow-up card
  to add it, or explicitly grant `CODEOWNERS` to this card so UI-A8 adds the two
  lines and re-runs the gate.
- When that is resolved, re-run the full gate on the merged head, push `main`
  and tag `wave-ui-a`, and carry the three screenshots to the human gate (GATE A).

## Proof

Real command output and sha256 for every image under `workstreams/modeler/ui/proof/ui-a8/`
(transcripts 00-09, PROOF.md, shell/palette/viewport png + sha, injector source).
