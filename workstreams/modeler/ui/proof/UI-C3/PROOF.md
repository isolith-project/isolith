# UI-C3 UI-VARIABLE-TABLE: units, expressions, usage count (proof)

Card: `t_706262f7`. Branch: `ui/c3-variable-table`. Files owned (exactly two
crates files, per the card):

- `crates/oep-app/src/panels/variables.rs` (new)
- `crates/oep-app/tests/variable_table.rs` (new)

All other files are the proof + README row the card also owns.

## What landed

A variable table that drives the model. `panels/variables.rs` is the
self-contained panel + egui-free engine (the same `#[path]`-includable seam the
B7 dimension engine uses). The table lives in the model document as ONE
`VariableSet` feature whose parameters carry per-variable string records
(`vars:list` plus `var:expr:<name>` / `var:unit:<name>`), so rows are derived
from the live document every frame.

The three named behaviours are proven by `--test variable_table` (133 passed):

1. **A reference resolves through the EXISTING UI-B7 expression path.** Every
   expression VALUE is resolved by `dimensions::resolve_expression` (the same
   scalar surface and the same `oep_sketch::varset::VariableSet` store the B7
   dimension editor drove); there is no second parser. Cross references work:
   `weight = base*2`, `height = width/2`, and deeper chains settle
   topologically. `referenced_names` extracts WHICH names an expression
   mentions (dependency analysis the B7 resolver does not expose), using the
   same identifier rule; it computes no arithmetic itself.
2. **Circular references are detected and refused with a NAMED error.** A
   table that can no longer be ordered topologically is refused as a whole and
   the error names the cycle path (`circular reference: width -> height ->
   width`). A refused edit leaves the document byte-identical (hash-pinned) and
   the journal untouched.
3. **Each row reports how many places use it, computed from the document.**
   `usage_of` re-scans the live document every call (each variable's
   expression plus every string-form feature parameter); nothing is
   bookkept. The test proves the counts MOVE when an edit changes who
   references whom.

The table also drives the model through the B7 drive lane: with `width` defined
by a cross reference, `driven_volume` reports the document's own mass property
(`base=40 -> width=80 -> 32000`; editing base to 60 re-resolves width to 120
and the volume moves to 72000 - derived from the driven document, never a bare
literal in the test). All edits are one journaled command through
`doc_io::EditHistory::apply` (the UI-B2 seam): undo restores the exact prior
document hash. The panel publishes a `String` report oracle every frame
(`ui_c3_variables_report`, rows + resolved values + units + usage + driven
volume + hash + last named error) and the shared resolved set under
`isolith.ui.variables.resolved` for whoever drives the model.

The panel is driven end-to-end through `egui_kittest` (`Harness::build_ui_state`
with a real `app::App` as state, seeded with a canonical `.oepdoc` before its
first frame): rows render with resolved values/usages/volume, a real expression
field edit through the assistive-tech SetValue path re-resolves the cross
reference and moves the volume, and a circular edit is refused with the named
cycle while the document hash stays put.

## Acceptance (real command output in `transcripts/`)

| Check | Result |
|---|---|
| `cargo test -p oep-app --test variable_table` | 133 passed / 0 failed |
| `cargo test -p oep-app` | 1949 passed / 0 failed across 18 targets |
| `cargo test --workspace` | 3191 passed / 0 failed |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 |
| `cargo fmt --all -- --check` | clean (exit 0) |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean |
| UI-B1 mutation guard | green (runs in `cargo test -p oep-app`) |
| UI-A3 / UI-A12 heading-ink + no-raw-hex guards | green |

## Transcripts

- `transcripts/00-step0-merge.txt` - STEP 0: card branch cut from main at
  `736a0a7`, merged `github/wave/ui-c` (wave head `d450ab4` = wave-B union +
  UI-C1). Not empty. (The transcript is re-captured on the already-merged
  head, hence "Already up to date".)
- `transcripts/01-fmt.txt` - workspace `cargo fmt --all -- --check`.
- `transcripts/02-clippy.txt` - `cargo clippy -p oep-app --all-targets -- -D warnings`.
- `transcripts/03-variable_table.txt` - the card's own test target.
- `transcripts/04-oep-app.txt` - full `cargo test -p oep-app` (per-target + aggregate).
- `transcripts/05-workspace.txt` - full `cargo test --workspace` aggregate.
- `transcripts/06-smoke.txt` - `--smoke` under xvfb.

## No screenshots this card (honest note)

The card owns exactly two crate files. `variables.rs` is delivered as a
`#[path]`-includable panel + engine and is NOT wired into the shell regions by
this card (that would require editing `panels/mod.rs` / `app.rs`, files this
card does not own). There is therefore no real-binary frame to screenshot this
card; the panel's rendering is proven through the kittest harness (AccessKit
tree + report oracle), and the C7 wave-close will need a wiring step before a
"variables" screenshot can be captured from the running app. No image was
produced by this card, so there are no sha256 files here.
