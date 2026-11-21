# UI-A8 UI-WAVE-A-INTEGRATE proof

Card: UI-A8 — close wave A: merge the wave into main through one checkpoint
commit, run the full gate on the merged head, tag the wave, and attach the
three screenshots (shell, palette, viewport).

Status: DONE.

## STEP 0

```
git fetch github
git merge github/wave/ui-a
```

The remote is named `github` (not `origin`) in the Isolith worktrees. This
brought the wave head `33aaa68` (which already includes the UI-A10 gate fix)
into the integration tree. Wave-head-relative history and hashes are recorded
below.

## Ancestor audit (acceptance #1) — PASS

Real output in `transcripts/00-ancestors.txt`:

```
wave head: 33aaa689a15c9cc535e79fb02f539252282bb348
ANCESTOR ok : ui/a1-deps       (UI-A1)
ANCESTOR ok : ui/a2-harness    (UI-A2)
ANCESTOR ok : ui/a3-theme      (UI-A3)
ANCESTOR ok : ui/a4-layout     (UI-A4)
ANCESTOR ok : ui/a5-viewport   (UI-A5)
ANCESTOR ok : ui/a6-palette    (UI-A6)
ANCESTOR ok : ui/a7-open-save  (UI-A7)
ANCESTOR ok : ui/a9-contrast   (UI-A9)
ANCESTOR ok : ui/a10-gate-fix  (UI-A10)
```

Every wave-A card branch is an ancestor of the wave head; every card's proof
dir is present on the wave head (ui-a1..ui-a7, ui-a9, ui-a10).

## Licensing boundary (addendum, owned)

The addendum owns `docs/licensing-boundary.{json,md}` at this wave close:
UI-A3 added `crates/oep-ui-theme` without a boundary row, so the crate-boundary
leg failed on the pre-fix wave head and merged head. Attempt 1 of this card
added the `oep-ui-theme` row as Apache-2.0 (library crate under the
applications/libraries split) and carried the honest count to 34 workspace
crates (22 Apache-2.0 + 12 AGPL-3.0); that fix is already landed on the wave
branch as `merge-UI-A8` (`dd4725f`), so the merged head inherits it. Verified
here on the wave head `33aaa68` AND on the checkpoint merge:

```
licensing check: total=6 pass=6 fail=0 skip=0 | exit=0
  [PASS] crate-boundary: 34 crates match the boundary map
  [PASS] boundary-doc-sync: 34 rows in sync
```

See `transcripts/04-licensing.txt`.

## Checkpoint merge (acceptance #2)

One no-ff merge of `github/wave/ui-a` into `main`, exact message:

```
git merge --no-ff github/wave/ui-a \
  -m "checkpoint(wave-ui-a): walking skeleton -- cards UI-A1..UI-A9"
```

Checkpoint commit `0ddf3834c1081a86b54fe1fdeb9eadc4780a5ea1`, parents
`8760cfe` (main before the wave) and `33aaa68` (wave head). No fast-forward;
one checkpoint commit. Remote `github/main` updated to this commit and
annotated tag `wave-ui-a` pushed (see below). `docs/scope-register.json`
diff across the checkpoint is empty.

## Full gate on the merged head (acceptance #3) — PASS

Transcripts in `transcripts/`: `01-fmt.txt`, `02-clippy.txt`,
`03-test.txt`, `04-licensing.txt`, `05-precommit-gate.txt`,
`06-shot.txt`. Merged head `0ddf383` (parents `8760cfe` + `33aaa68`).

| # | Command | Exit | Notes |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | 0 | clean |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| 3 | `cargo test --workspace` | 0 | all targets pass |
| 4 | `python3 scripts/licensing-check.py` | 0 | 6/6 legs, 34 crates |
| 5 | `bash scripts/gate/precommit-gate.sh` | 0 | PASS: pytest 558 passed, 29 skipped; cargo test ok |
| 6 | `bash scripts/ui/shot.sh` | 0 | frame `e60b86dc…9a9b` byte-identical to UI-A9 golden |

The shot.sh drift guard passed against the UI-A9 golden on the merged head,
which is the ADDENDUM-2 requirement: a drift would have been a finding, not a
hash to overwrite. There was no drift.

## Screenshots (acceptance #5 + ADDENDUM 2) — PASS, separated panels shown

Captured from the real merged-head binary under `xvfb-run` via the XTEST
injector in this proof dir (`capture_ui.sh`, `xtest_inject.c`), the same
determinism discipline as shot.sh (two independent app launches, byte-identical
sha256 oracle):

| Shot | sha256 | Shows |
|---|---|---|
| `shell.png` | `e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4` | full shell, byte-identical to UI-A9 golden; five separated regions (raised fill + 1px hairline per region) |
| `palette.png` | `ed2634e1a6957a58f27d9ce6faf83a94dd1855b33f2c58ccd44ae1fb48eb9445` | Ctrl+K palette open over the separated shell; rows `load:…`, `save:…`, `assem:render | assem:render:PATH` highlighted, tooltip and filter box visible |
| `viewport.png` | `f7cc1e582b56f97f67d2cf968f0b31dead06c99bf712866595088d47183234e1` | model orbited in the viewport (camera moved by left-drag), separated panels intact |

Hashfiles `<shot>.png.sha256` committed alongside. The palette capture needed
a guarded `XSetInputFocus` in the injector under bare Xvfb (no WM -> winit
would not deliver Ctrl+K); see `transcripts/09-palette-focus-fix.txt`. All
three shots show the separated panels ADDENDUM 2 requires — verified by pixel
review (shell/palette/viewport regions with SURFACE_RAISED fill + BORDER
hairline) and by visual review.

## Wave record

`docs/status/waves/wave-ui-a.md` (new) records the wave: cards, wave head sha,
checkpoint sha, gate output, and what is not done. It is committed together
with this proof and the README row, and carried into `main` by the single
checkpoint merge, so main's checkpoint commit holds the complete wave record.

## Landing

Per the card's landing step the UI-A8 record landed on `wave/ui-a`
(`merge-t_39d71100`), then the checkpoint merged `origin/wave/ui-a` into main
in one no-ff commit. Acceptance command transcripts are in `transcripts/`.
