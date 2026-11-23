# UI-B1 UI-INPUT-ROUTER proof

Card: UI-B1 UI-INPUT-ROUTER - one event map to command tokens.
Branch: `ui/b1-input-router`. Wave branch: `wave/ui-b` (remote `github`).

STEP 0: `git fetch github` + `git merge github/wave/ui-b` returned "Already up
to date." - `wave/ui-b` IS the wave cut point `736a0a7` and equals this
branch's HEAD, so the merge brings nothing (no earlier wave-B card has landed;
this is the first card of wave B). Transcript `07-step0-merge.txt`.

Behaviour: pointer buttons and keys plus their modifiers resolve to command
tokens through ONE table, so the palette, the keyboard and the future toolbars
speak one language and nothing mutates the document directly.

## What was built

`crates/oep-app/src/input.rs` (new) is the single input table:

- `KEY_BINDINGS` (16 rows) and `POINTER_BINDINGS` (5 rows) map `(key,
  modifiers)` and `(pointer button, modifiers)` to command tokens = 21
  bindings, over the 20 the card asks for.
- Every token is one `oep_shell::commands` already validates: the tests run
  the exact `validate_token` the workbench CLI calls before dispatch over all
  21 rows, and `find_by_token` resolves each. The palette (which routes through
  the same registry via `commands::dispatch`) and this router therefore cannot
  drift from the CLI.
- `resolve_key` / `resolve_pointer` implement exact modifiers: `Ctrl+Shift+Z`
  resolves to `redo`, `Ctrl+Z` to `undo`, a key no row declares to nothing,
  and the whole command family (Ctrl / Command / Mac-Command, incl. a real
  Linux Ctrl press where egui-winit sets both `ctrl` and `command`) reaches
  the same row. The more-specific row precedes the plain row on the same key
  (Redo before Undo) because the runtime consumes with egui's logical matching
  (extra Shift/Alt ignored) - so the exact-match contract holds at runtime too.
- `poll` is wired in `app.rs` and runs **last** each frame, after the palette
  and doc panels, so every panel-owned key wins first: the palette still opens
  on Ctrl+K, the doc panel still cancels on Escape, Ctrl+O/S and the viewport
  drags are untouched. `poll` consumes the global command keys no panel owns,
  routes each through the shared registry, and records the outcome in a plain
  `String` report (`REPORT_KEY = ui_b1_input_report`) in the egui context
  store - the same oracle pattern UI-A6/UI-A7 use.
- Panel seam asserted in source: `panel_seam_toggles_are_not_claimed_by_the
  _table` proves the router does not claim K/Escape/O/S/plain-Enter, and the
  existing UI-A6 palette + UI-A7 doc suites pass unmodified below.

`app.rs` (wired, panel seam kept): declares `#[path = "input.rs"] pub mod
input;` and calls `input::poll(ui.ctx())` after `panels::doc::show`. No panel
file and no other crate was touched.

## The mutation guard (the important half)

`tests/input_router.rs::no_module_mutates_the_document_outside_doc_io_rs`
scans every `.rs` file under `crates/oep-app/src` and fails the build on any
direct document write outside `doc_io.rs` - the document's own `&mut` mutators
(`push_part/body/feature/assembly/drawing`) and direct mutation of its
collections (`parts`/`bodies`/`features`/`assemblies`/`drawings` `.push(
.insert( .remove( .clear( .pop(` and field reassignment). Read-only count
access (`d.bodies.len()`), the only document access the doc panel needs, is
not a mutation and passes. It runs on every `cargo test -p oep-app` invocation
(this target is part of the suite).

The scan is proven load-bearing in both halves:

1. Committed self-test `the_scanner_flags_planted_direct_document_mutation_
   and_passes_reads`: every one of the 34 signatures is caught on a planted
   line, and the read-only access doc.rs legitimately uses is not.
2. Live probe captured in `05-guard-fires.txt`: a planted direct mutation
   (`document.push_feature(f);`) added to `src/input.rs` makes the guard fail
   exit 101, citing `input.rs:508: .push_feature(`. After the probe is
   removed, `06-guard-clean.txt` shows the same test green.

## Files owned vs touched

Card-owned: `crates/oep-app/src/input.rs` (new), `crates/oep-app/src/app.rs`
(wired router, panel seam kept), `crates/oep-app/tests/input_router.rs` (new),
`workstreams/modeler/ui/README.md` (UI-B1 row added), and this proof tree.

Out-of-list files, with reason: none - no other crate or file was modified
(nothing outside the card's list changed; `git status` below confirms the only
changes are the owned files plus this proof).

## Acceptance (all real output under `transcripts/`)

- `cargo test -p oep-app --test input_router` -> 20 passed (13 UI-B1 tests +
  7 included module tests: config 3, theme guards 3, contrast guard 1). [01]
- `cargo test -p oep-app` -> all 9 targets green, exit 0 (every earlier suite
  - harness, layout, palette, doc_io, tree_empty_state, viewport, heading_ink
  - stays green under the wired router). [02]
- `cargo fmt --all -- --check` exit 0; `cargo clippy -p oep-app --all-targets
  -- -D warnings` exit 0. [03]
- `xvfb-run -a cargo run -p oep-app -- --smoke` -> `frames=8 smoke ok`, exit
  0. [04]
- Guard-fires proof: a planted direct document mutation makes
  `no_module_mutates_the_document_outside_doc_io_rs` fail citing the file;
  after removal the guard is green again. [05, 06]
- Step-0 merge brought nothing (wave/ui-b == branch HEAD == 736a0a7). [07]

No images were produced by this card (the behaviour is a router + guard, not a
visual; the card names no screenshot requirement), so there is no sha256 to
record - the proof is the real command output above.
