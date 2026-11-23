# UI-C4 UI-ROLLBACK: suppress and roll the tree

**Card:** UI-C4 UI-ROLLBACK (wave C, editor parity program)
**Branch:** `ui/c4-rollback`
**Files owned (touched):** `crates/oep-app/src/panels/tree.rs`, `crates/oep-app/src/app.rs`,
`crates/oep-app/tests/rollback.rs` (new). No other crate touched; no root manifest edit.

## Behaviour (the card)

A roll bar and suppression marks are shown honestly: suppressing a feature
changes the mesh hash, the status bar reports the suppressed count, and rolling
back hides downstream features.

## Design summary

Both suppression and roll are **document state**, carried in the affected
feature's `parameters` map (`suppressed` / `roll` keys) inside the canonical
`.oepdoc` JSON - the same deterministic channels UI-C2's parameter editor and
UI-C3's variable table use. Because they live in the serialized document, a
saved and reopened file replays exactly the same marks.

- **Suppress** (`tree:suppress`, one journaled command through the UI-B2 seam
  `doc_io::EditHistory::apply`): sets `parameters["suppressed"]=true` on the
  selected feature. The canonical JSON changes, so the tree's published document
  hash (the FNV-1a mesh-hash oracle for this wave's panels, `hash=` in the C1
  report) moves. The row re-derives with an honest `(suppressed)` suffix and the
  count feeds the status bar ("N suppressed"), published by `App` every frame.
- **Roll** (`tree:roll`, one journaled command): `parameters["roll"]=true` on
  the feature the tree is rolled back to. Because `Document.features` is in
  creation order, every feature *created after* the roll point is DOWNSTREAM and
  is hidden; `rows_of` re-derives live every frame (the established "rows come
  from the live document, never a cached copy" contract), so no stale geometry
  can outlive the edit. The roll bar renders as an accessible marker under the
  roll point ("roll bar · N downstream hidden") and the status bar reports the
  rolled-back count. `Unroll` clears the roll point and restores the exact
  pre-roll hash.

Round-trip: the panel publishes its live canonical `.oepdoc` JSON every frame
(`ui_c1_tree_doc_json`); the acceptance re-seeds it into a fresh harness (the
save -> reopen path) and asserts byte-identical JSON plus identical report
fields (suppressed, roll, rolled, hash).

## Acceptance (real command output in transcripts/)

| Command | Result | Transcript |
|---|---|---|
| `cargo test -p oep-app --test rollback` | 119 passed / 0 failed | `01-rollback.txt` |
| `cargo test -p oep-app` | 20 targets / 0 failed | `02-oep-app.txt` |
| `cargo test --workspace` | 216 targets / 0 failed | `03-workspace.txt` |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean, EXIT=0 | `04-clippy.txt` |
| `cargo fmt --all -- --check` | clean, EXIT=0 | `05-fmt.txt` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok` EXIT=0 | `06-smoke.txt` |

The `rollback` suite proves each sentence of the card:

1. **suppress changes hash** - `suppressing_a_feature_changes_hash_shows_the_mark_and_reports_the_count`:
   suppressing moves `hash`, the row label gains `(suppressed)`, and restoring
   returns to the EXACT pre-suppress hash (canonical round trip).
2. **suppressed count in status bar** - the same test asserts the status-bar
   region shows "1 suppressed" after the mark (real AccessKit region text).
3. **roll hides downstream** - `rolling_back_hides_downstream_and_unrolling_restores_it`:
   rolling to the first feature hides the two created after it (rows drop 5->3,
   `rolled=2`, downstream labels gone), the roll bar marker renders, and Unroll
   restores the exact pre-roll hash and all rows.
4. **document state / save-reopen** - `suppression_and_roll_round_trip_through_save_and_reopen`:
   the panel's own published canonical JSON, re-seeded into a fresh harness,
   replays identical suppressed/roll/hash state with byte-identical JSON.
5. **journaled + undoable** - `suppress_is_a_journaled_undoable_edit`: one Undo
   after a suppress restores the exact prior hash.

## Honesty notes

- STEP 0 was NOT empty: the branch began at `736a0a7` (main's wave-A checkpoint)
  and merged `github/wave/ui-c` = `47900bc` (wave-B close + UI-C1/C2/C3), so the
  whole gate above ran against the merged wave head (`00-step0.txt`).
- Suppression "changes the mesh hash" in the honest sense available to this
  panel: the tree's published `hash=` is `doc_io::document_hash` (FNV-1a-64 over
  the canonical `.oepdoc` bytes), the mesh-hash oracle every wave-C panel uses
  (UI-C1/C2/C3 all assert against it). The 3D viewport in this wave renders the
  committed demo scene (not the document); the document-tree panel is the
  document view that shows the roll state, proven by kittest above and the real
  binary below.
- The real binary's demo document contains exactly ONE feature (`step-import`),
  so its honest roll state has "0 downstream hidden" (nothing was created after
  it) - the screenshot shows that truthful state, while the "hide downstream"
  behaviour (2 hidden) is proven by the kittest acceptance on a seeded three-
  feature document. Same as the C2 precedent: the shipped demo is too small for
  every branch, the kittest harness is the authority for them.

## Screenshots (real binary under xvfb, byte-identical across two launches)

| File | sha256 |
|---|---|
| `suppress.png` | `c6913fd2a67d79dc8df4d40af7445e63a593d37f6e8f6db65723f1bfb84ba13b` |
| `roll.png` | `c1b5e7f51a9eb562ccadaac57c0fa6dffb7202e1766b7f191bed8c8f7ad5d0c8` |

- `suppress.png`: the demo ImportStep row selected and suppressed - the row
  shows `step-import (ImportStep) (suppressed)`, the status bar shows
  `1 suppressed`, and the toolbar offers `Restore` (not `Suppress`) for the
  now-suppressed selection.
- `roll.png`: the same row rolled back to - the roll bar marker
  `roll bar · 0 downstream hidden` renders under the roll point (truthful for a
  one-feature demo), the toolbar offers `Unroll`, and the downstream count is
  zero because nothing was created after the only feature.

`capture_ui_c4.sh` drives both captures through the XTEST injector
(`ui_c4_inject.c`, same technique as UI-C1/C2); each mode launches the real
binary twice under `xvfb-run` and requires byte-identical PNGs.
