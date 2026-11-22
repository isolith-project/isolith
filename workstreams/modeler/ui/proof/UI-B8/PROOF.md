# UI-B8 UI-FEATURE-FROM-SKETCH: extrude/revolve dialog with preview

**Status: green. Acceptance commands all exit 0.**

## Behaviour

Finishing a sketch into a solid goes through a **Finish sketch** dialog with a
**live preview**. The card's three acceptance claims, each asserted by the test
suite `crates/oep-app/tests/feature_from_sketch.rs` (real `egui_kittest`
harness driving the real `app::App`):

1. **The preview frame differs from the pre-feature frame.** Opening the
   dialog derives a **document clone marked preview** (an
   `oep_shell::editor::EditorSession` seeded with the committed geometry,
   solved and swept). The viewport's preview frame path swaps in that derived
   solid's rasterized frame, so the published viewport frame hash changes
   while the dialog is up, and **returns to the exact pre-feature hash** when
   the dialog closes (asserted).
2. **Cancel leaves the document byte-identical, asserted by hash equality.**
   The feature document digest (`features::document_digest`, FNV-1a-64 over
   the committed geometry lines + applied-feature records) is byte-identical
   before opening the dialog and after cancelling it; the committed sketch
   document block is byte-identical; and the feature journal holds exactly
   zero entries (the preview clone never entered the journal until Apply).
3. **The STEP export from the UI matches the CLI export hash.** The UI's
   "Export STEP" button routes through `oep_shell::editor::EditorSession::
   export_step` - the exact method `oep-workbench export:step` dispatches.
   The acceptance compares the UI-exported file's SHA-256 with the CLI-lane
   export of the same committed model, byte-for-byte; the real CLI run in
   `transcripts/07b-real-cli-export.txt` produces the identical hash
   (`ae63918a…`) to the UI button.

The dialog offers both kinds: **Extrude** (depth in mm, default 10) and
**Revolve** (angle in radians) live-render as you step or type the amount, and
switching kinds re-derives the preview (asserted). Apply journals exactly one
entry under the registry-validated token `feat:extrude` / `feat:revolve` (the
CLI's own editor feature names) with before/after digests (asserted).

## Files owned (exactly the card list)

| File | What |
|---|---|
| `crates/oep-app/src/panels/features.rs` (new) | the dialog, the document-clone preview, the feature journal, the STEP export, the `ui_b8_feature_report` oracle, deterministic widget rects |
| `crates/oep-app/src/panels/viewport.rs` | the preview frame path: `features::poll` (F open / Escape cancel), `features::preview_frame` swapped into the published frame + hash, `features::draw` painted over the viewport |
| `crates/oep-app/tests/feature_from_sketch.rs` (new) | the acceptance suite (5 kittest acceptance tests + the panel's own module tests) |

No other crate touched. No dependency added (`oep-shell`, `oep-sketch`,
`oep-viewer`, `oep-import`, `oep-ui-theme` all already in oep-app's manifest;
`cargo tree`/manifest unchanged).

## Design notes (the honesty lanes)

- **Document clone marked preview**: `features::build_clone` seed an
  `EditorSession` (the CLI's interactive-editor surface) from the committed
  geometry lines parsed from the sketch `String` oracle - never a typed sketch
  across `#[path]` copies. `rebuild_preview` re-builds it whenever kind or
  amount changes, so the preview is live. The clone lives only in the open
  dialog; nothing touches the committed sketch or the journal until Apply.
- **Journal**: the panel owns its own B2-style `history::Journal` (the feature
  lane). Preview never writes it; Apply pushes exactly one entry with
  before/after digests; the oracle publishes `journal=` for the test.
- **Preview frame path** (viewport.rs): `features::preview_frame` rasterizes
  the clone's scene through the existing deterministic CPU renderer at the
  panel size and returns `(image, hash)`; the viewport publishes that hash in
  the shared scalar snapshot when a preview/applied solid is current, and the
  plain backend frame otherwise - so B1..B7 frames are byte-identical, and the
  preview/cancel contract is a real published value.
- **Export honesty**: no second writer. Both the UI and the CLI go through
  `EditorSession::export_step`; the UI re-derives the session from the
  document (B7 drive-lane precedent), so the bytes come from the model, not a
  frozen copy. STEP embeds the export file name, so the acceptance writes UI
  and CLI exports with the SAME basename in different directories (probe-confirmed
  byte-equality condition) and compares SHA-256.

## Acceptance transcripts (real command output)

- `transcripts/00-header.txt` - card branch head (= wave head b1ce450 after STEP 0)
- `transcripts/01-feature_from_sketch.txt` - `cargo test -p oep-app --test feature_from_sketch` (148 passed)
- `transcripts/02-oep-app-full.txt` - `cargo test -p oep-app` (16 targets / 1604 passed, 0 failed)
- `transcripts/03-smoke.txt` - `xvfb-run -a cargo run -p oep-app -- --smoke` (`frames=8 smoke ok`, exit 0)
- `transcripts/04-clippy.txt` - `cargo clippy -p oep-app --all-targets -- -D warnings` (exit 0)
- `transcripts/05-fmt.txt` - `cargo fmt --all -- --check`: my three files rustfmt-clean; the ONLY flag is the pre-existing UI-B1 `input.rs:504` trailing blank (documented unchanged from B2..B7, not owned here)
- `transcripts/06-step0-merge.txt` - STEP 0 merge of `github/wave/ui-b` (b1ce450; brings UI-B1..UI-B7 into the card tree - not empty)
- `transcripts/07-ui-vs-cli-export.txt` - the test's UI-export and CLI-lane export hashes (identical)
- `transcripts/07b-real-cli-export.txt` - the REAL `oep-workbench` CLI exporting the same model; hash equals the UI button's export hash

## Screenshot

`feature_from_sketch.png` (sha256 `e25e71813d1cb43ac6f056d3be8f81d3cb53cc26f7963c2a6d0cadc1f8899470`)
captured twice under xvfb via an XTEST injector (`ui_b8_inject.c`, same
technique as B4..B7), byte-identical both launches (determinism oracle in
`feature_from_sketch.png.sha256`). The frame shows the **Finish sketch**
dialog open over the viewport with the **Extrude** kind selected, the amount
field (`10` + `-`/`+` steppers), **Apply**/**Cancel**, and the live preview
designation *"live preview in viewport"* - the derived solid is rendered in
the viewport behind the dialog and the dialog's readout reports the solid's
own volume and face count from the derived document.
