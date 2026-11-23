# UI-B2 UI-HISTORY proof

Card: UI-B2 UI-HISTORY - journal undo/redo with drag coalescing.
Branch: `ui/b2-history`. Wave branch: `wave/ui-b` (remote `github`).

STEP 0: `git fetch github` + `git merge github/wave/ui-b`. Unlike B1 (the
first card of wave B, where the merge brought nothing because `wave/ui-b` was
the cut point), this card is the SECOND card: the wave head already carried
UI-B1's merge commit `780dc56`, and merging it into this branch brought UI-B1
into the tree (13 files: `src/input.rs`, `tests/input_router.rs`, app wiring,
proof, README row) before any of this card's work started - so the router and
its mutation guard are present underneath the journal. Transcript
`00-step0-merge.txt` (the fetch/merge is green; the tree head shows the B1
merge in place).

Behaviour (card): one drag is one undo entry. The journal coalesces a
continuous pointer gesture into a single command; undo restores the prior
mesh hash and redo returns to the same hash.

## What was built

`crates/oep-app/src/history.rs` (new) - the pure command journal (D-UI-4):

- It owns ORDERING and COALESCING, never document state. An `Entry` is the
  command token plus the hashes before/after; there is no document reference
  anywhere in the module (the journal is `std`-only, egui-free).
- Coalescing rule (the card): consecutive commands with the same `token` and
  the same `DragId` collapse into one entry until the gesture ends
  (`Journal::end_drag`). The merged entry keeps the FIRST `before` and the
  LAST `after`, so one pointer drag is exactly one undo step. A new drag id,
  an ended gesture, a different token, or a discrete command (`drag = None`)
  each start a fresh entry.
- `undo()` returns the popped entry's `before` hash; `redo()` returns its
  `after` hash - the exact-hash contract the acceptance demands.
- Memory bound: `with_capacity(n)` keeps the newest `n` entries (32-step
  semantics, parity with `oep-shell`'s `HISTORY_CAPACITY`); `drop_oldest()`
  lets the state owner trim an aligned store in lock-step.

`crates/oep-app/src/doc_io.rs` (edited) - the routed mutation seam:

- `#[path = "history.rs"] pub mod history;` - the journal is compiled into
  the crate through this one include, exactly like the panels' `#[path]`
  pattern, so test targets that include `doc_io.rs` get one copy (no
  `duplicate_mod`).
- `EditHistory` is the other half of D-UI-4: the journal plus the bounded
  canonical-JSON snapshot store that OWNS the document states behind the
  journal's hashes. `apply(document, token, drag, mutate)` hashes before,
  runs the caller's mutation, hashes after, and journals (skipping the
  redundant before-snapshot when the command only extends an open drag -
  via `Journal::would_coalesce`). `undo`/`redo` restore the REAL document
  from the store and `debug_assert` exactness (`hash before == hash after
  undo`, `hash after == hash after redo`).
- `trim()` bounds the journal AND its snapshot store together so the hashes
  and the states they point at never diverge.
- This file stays the one sanctioned document-I/O module: the UI-B1 mutation
  guard (`--test input_router`) still passes over the new code, because no
  other `src/` module gained a document write. (`history.rs` touches no
  document; the only mutations happen inside `EditHistory::apply`'s caller
  closure or the seam itself, which is the sanctioned module.)

`crates/oep-app/tests/history.rs` (new) - the acceptance, with REAL
`oep_import::Document`s (never fabricated hashes):

- `fifty_drag_commands_collapse_to_one_entry_and_undo_redo_are_exact`: 50
  sequential drag commands (token `view`, one drag id) against a real
  document collapse to ONE journal entry - and every intermediate command
  genuinely changed the document hash, so the collapse is a real coalesce,
  not a stream of no-ops. Undo lands the document on the PRIOR mesh hash
  (the entry's `before`), redo returns it to the SAME hash (the entry's
  `after`).
- `undo_redo_walk_mixed_history_with_exact_hashes`: the exactness holds at
  every step across discrete edits + a coalesced drag.
- Coalescing scope: new drag id, ended gesture (`end_drag`), different
  token, and discrete commands each start a fresh entry.
- `new_edit_after_undo_truncates_the_redo_branch` (the classic CAD rule),
  `bounded_capacity_drops_the_oldest_and_undo_stays_exact` (32-step
  semantics), `journal_records_token_and_hashes_only` (state-free journal),
  `registry_backed_journal_tokens_are_validated_command_tokens`.
- 8 pure-journal module tests also live in `history.rs`'s `#[cfg(test)] mod
  tests`, so they run inside every oep-app test target (the module-test
  convention this crate already uses for config/theme/contrast).

## Files owned vs touched

Card-owned: `crates/oep-app/src/history.rs` (new),
`crates/oep-app/src/doc_io.rs` (routed mutation seam),
`crates/oep-app/tests/history.rs` (new), `workstreams/modeler/ui/README.md`
(UI-B2 row added), and this proof tree.

Out-of-list files, with reason: none. `input.rs`, `app.rs`, every panel, and
every other crate are untouched (`git status` below shows only the owned
files plus this proof).

## Acceptance (all real output under `transcripts/`)

- `cargo test -p oep-app --test history` -> 18 passed, exit 0. [01]
- `cargo test -p oep-app` -> all 10 targets green, exit 0 (every earlier
  suite - harness, layout, palette, doc_io, tree_empty_state, viewport,
  heading_ink, input_router - stays green underneath the new seam; the
  history module tests run inside each). [02]
- `cargo clippy -p oep-app --all-targets -- -D warnings` exit 0. [03]
- `xvfb-run -a cargo run -p oep-app -- --smoke` -> `frames=8 smoke ok`, exit
  0. [04]
- STEP 0 merge brought UI-B1 (13 files, `780dc56`), transcript captured.
  [00]
- fmt, honestly: `cargo fmt --all -- --check` flags ONLY the pre-existing
  trailing blank line at `crates/oep-app/src/input.rs:504`. That file is
  byte-identical to `github/wave/ui-b` (UI-B1's own fmt transcript shows the
  same diff), it is NOT owned by this card, and editing it would fail review
  - so this card leaves it untouched and reports it for the wave integrate
  card. This card's three files are fmt-clean (no diff names them). [03]

No images were produced by this card (the behaviour is the journal + routed
mutation seam, not a visual change to rendered output - the card names no
screenshot requirement), so there is no sha256 to record; the proof is the
real command output above.
