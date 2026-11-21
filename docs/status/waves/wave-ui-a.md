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
| UI-A10 | UI-GATE-FIX: CODEOWNERS row for `oep-ui-theme` + brand-scan `.worktrees` skip | done |
| UI-A8 | UI-WAVE-A-INTEGRATE: merge, gate, checkpoint, screenshots | done |

Every wave-A card branch is an ancestor of the wave head, and every card's proof
dir is present on the wave head (audit in `proof/ui-a8/transcripts/00-ancestors.txt`).

## Head sha

- Wave head: `33aaa68` (`merge-UI-A10`, the gate fix; contains all wave-A cards
  including the UI-A8 licensing fix landed earlier as `merge-UI-A8` `dd4725f`).
- Checkpoint merge onto main: one no-ff commit, `checkpoint(wave-ui-a): walking
  skeleton -- cards UI-A1..UI-A9`, parents `8760cfe` (main) and the wave head.
  The UI-A8 record (this file, the README row, `proof/ui-a8/`) lands on the wave
  branch and is carried into main by that single checkpoint commit.
- Annotated tag `wave-ui-a` pushed on github/main.

## Licensing boundary (wave close, UI-A8 addendum)

UI-A3 added `crates/oep-ui-theme` without a boundary row, so the crate-boundary
leg failed. UI-A8 owns the wave-close fix: `oep-ui-theme` row added as
Apache-2.0 (library crate under the applications/libraries split) to
`docs/licensing-boundary.{json,md}`, crate count carried honestly: **34
workspace crates** (22 Apache-2.0 + 12 AGPL-3.0). `docs/proof/licensing-check.json`
regenerated (its own check report). All 6 legs PASS on the wave head `33aaa68`
and on the merged head; 34 crates in the boundary map. (The row itself was
authoritative; the earlier `merge-UI-A8` `dd4725f` landed `a66cf3e` on the wave.)

## Full gate on the merged head

Transcripts in `workstreams/modeler/ui/proof/ui-a8/transcripts/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - incl. oep-app doc_io / harness / layout / palette / viewport, each with `contrast_test`; oep-ui-theme both targets |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps allow-listed, NOTICE in sync |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green; no UI files staged |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `e60b86dc…9a9b` byte-identical to the UI-A9 golden (no drift) |

The `shot.sh` drift guard passing against the UI-A9 golden is the ADDENDUM-2
requirement: a drift on the merged head would have been a finding, not a hash
to overwrite. There was none.

## Screenshots (acceptance #5, ADDENDUM 2)

Three screenshots of the real merged-head binary (captured under xvfb-run via
the XTEST injector, two independent launches byte-identical per shot):

| Shot | sha256 | Shows |
|---|---|---|
| shell | `e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4` | five separated regions (SURFACE_RAISED fill + BORDER hairline) |
| palette | `ed2634e1a6957a58f27d9ce6faf83a94dd1855b33f2c58ccd44ae1fb48eb9445` | Ctrl+K palette open over the separated shell |
| viewport | `f7cc1e582b56f97f67d2cf968f0b31dead06c99bf712866595088d47183234e1` | model orbited in the viewport, separated panels intact |

All three show the separated panels UI-A9 added. Shell is byte-identical to the
UI-A9 golden `e60b86dc…9a9b`.

## What is not done

- Wave A is a walking skeleton: modeler workflows beyond the five shell regions
  (open/save/recent, command palette routing, live CPU viewport) are the seeds
  wave B will grow; nothing beyond them is claimed.
- The `wave/ui-a` branch is left in place (not deleted) after the checkpoint,
  so the wave history and record are preserved.
- No UI tests were weakened, skipped or deleted to reach green; the sole wave-A
  defect found at the gate (missing `oep-ui-theme` CODEOWNERS row) went to its
  own fix card UI-A10, which is on this wave.
