# UI-F3 UI-ANNOTATE: dimension, leader and note tools + the title block - proof

Wave F card. Annotation tools on the sheet - a width dimension, leaders and
notes - plus a title block that is fed from DOCUMENT PROPERTIES rather than
typed twice.

## Files owned by this card

- `crates/oep-app/src/panels/annotate.rs` (new: the dimension tool bound to a
  model variable, the Add leader / Add note records, and the document-bound
  title block - the panel stores NO `TitleBlock` copy; `title_block_readout`
  reads the live document every frame and `title_of` derives the drawing title
  from the part-name property)
- `crates/oep-app/src/panels/sheet.rs` (ADDITIVE: one `#[path]` include of
  `annotate.rs` next to the existing views include, and one
  `annotate::show(...)` call right after `views::show(...)` at the end of the
  drafting scroll content. The F1/F2 sheet controls' node rects are unchanged -
  verified: F1 `--test sheet` still green, now 165 passed.)
- `crates/oep-app/tests/annotate.rs` (new: acceptance harness, drives the REAL
  `app::App` through egui_kittest like the F2 test)
- `workstreams/modeler/ui/proof/UI-F3/` (this proof)
- `workstreams/modeler/ui/README.md` (row updated in the same commit)

The panel only places annotation records and formats the title block read-out.
The model value, view geometry and dimension label all come from the EXISTING
drafting seam: the committed bracket drawing seeded by
`oep_drafting::builder::drive_bracket_drawing`, the sheet's title block from
`oep_geo`, and the re-drive path through `oep-sketch` (variable set → feature
re-drive → re-render). There is no second title block, no second geometry path.

## Acceptance

```
cargo test -p oep-app --test annotate       -> green (5/5)
cargo test -p oep-app --test sheet          -> green (165 passed, F1/F2 nodes untouched)
xvfb-run -a cargo run -p oep-app -- --smoke -> frames=8 smoke ok, exit 0
```

Transcripts: `transcripts/01-annotate-acceptance.txt` (green `--test annotate`),
`transcripts/01b-numbers.txt` (the ordered numbered clauses with `--nocapture`),
`transcripts/02-smoke.txt`.

### 1. Title block bound to a document property - no second edit (both printed)

The part name is a document property. Editing it through the panel
(`apply_part_name`: ONE journaled `EditHistory` command that updates both the
part property on the geometry and the drawing's title block) must show the new
value in the title block with no second edit. The acceptance prints the document
property and the rendered title block:

```
DOC PROPERTY part_name = bracket-mk2
TITLE BLOCK tb_title = bracket-mk2
```

`assert part_name == new && tb_title == new && part_name == tb_title`. The new
value is also verified to be present as a rendered AccessKit label in the frame
(grounded in the rendered UI, not only in the oracle) while the original value's
label is gone - the display is live, not stale.

### 2. A dimension placed between two view points reads the model value within 1e-6

Placing the front-width dimension between the two view points resolves the model
value and measures the projected extent from the same re-driven geometry. The
acceptance prints value, measured and view geometry:

```
DIMENSION value = 80.000000000  measured = 80.000000000  geometry_w = 80.000000000  |value-measured| = 0.000e0
```

`assert |value - measured| < 1e-6`, `|value - geometry| < 1e-6`, and the model
width is the committed 80: `|value - 80| < 1e-6`.

### 3. Editing that dimension moves BOTH the view geometry and the label

Editing the dimension value (a real DragValue → 120) re-drives the model; both
the label and the view geometry must change and agree. Before/after are printed:

```
BEFORE edit: label=80 geometry=80
AFTER edit: label=120 measured=120 geometry=120 diff=0.000e0 agree=1
```

`assert label changed > 1e-9 && geometry changed > 1e-9` (never one without the
other), `|label - measured| < 1e-6`, `|label - geometry| < 1e-6`,
`agree == "1"`, and the edited value is 120 within 1e-6.

### 4. A leader and a note round-trip through save and reopen (JSON hashes printed)

Place a leader and a note, SAVE the canonical annotation JSON the panel
publishes, REOPEN it in a fresh panel seeded with that JSON, and assert both
annotations come back with the JSON hash byte-equal:

```
SAVED json hash = 7453889304312849599
REOPENED json hash = 7453889304312849599
```

`assert saved == reopened` byte-identical (FNV-1a 64 on the canonical bytes -
the wave's hash language) and both records restored (leaders=1, notes=1).

## Guards and full gates

- Mutation guard green: the document writes (apply part name via `TITLE_TOKEN`
  `annotate:title`, place dimension via `DIM_TOKEN` `annotate:dim`) both route
  through the journaled `doc_io::EditHistory`/`apply_sheet_edit` seam; no
  direct document write appears in `src/` in the panel. Leader and note records
  are annotation state carried as the canonical seed JSON (LiveValue published
  under the panel's oracle key), so placement persists through the seeded
  round-trip without leaving the journal for non-document data.
- clippy `-D warnings` (all targets, workspace): exit 0 - `transcripts/06-clippy.txt`.
- fmt `--check`: exit 0 - `transcripts/05-fmt.txt`.
- licensing 6/6 (34 crates / 535 deps, unchanged - oep-app adds no dependency,
  `Cargo.lock` untouched): exit 0 - `transcripts/07-licensing.txt`.
- `cargo test --workspace -j 1`: **226 suites / 6902 passed / 0 failed**, exit 0 -
  `transcripts/04-workspace-green.txt`. The documented egui_kittest teardown
  SIGSEGV under full parallel load (a pre-existing flake, see UI-F1/D4 metadata)
  hit `palette`/`pick_gpu`/`layout`/`nav` during capture runs
  (`transcripts/08-flake-parallel-app-run.txt`); every target passes isolated
  and the clean `-j 1` run is green.
- `cargo test -p oep-app` all 33 targets: green - `transcripts/03-app-targets.txt`.
- smoke: `frames=8 smoke ok`, exit 0 - `transcripts/02-smoke.txt`.

## Screenshot (deterministic: two independent captures byte-identical)

- `annotate.png` - the real merged-head binary at the default 1280x800 window.
  The Annotate section sits BELOW the drafting fold inside the drafting scroll
  area (F1 sheet controls, then the F2 views tools fill the region), so the
  capture wheel-scrolls the drafting scroll area, clicks `Place dimension (front
  width)` (which re-drives the model and renders the value editor + readout),
  re-clamps the scroll, clicks `Add leader` and `Add note`, and parks the
  pointer neutral before capturing. The frame shows the Annotate tools live:
  the part-name editor with the document-bound title block above the fold, the
  placed dimension readout `value=80.000000000 measured=80.000000000
  geometry_w=80.000000000`, the Add leader / Add note / Clear annotations
  buttons, BOTH restored records (`leader oep-leader-1 · LEADER 1: C/L THIS
  FEATURE`, `note oep-note-1 · NOTE 1: REMOVE ALL SHARP EDGES`) and the
  `added note 1` status line. The post-scroll tool coordinates were derived from
  a same-code kittest probe at the real 1280x800 window and re-calibrated
  against the first captured frame.

sha256 in `annotate.png.sha256`:

```
4d7ae4d62c2dbe810ee0d9bf1a7508d05ac514514dc83f95b78597b5fd908a31
```

Captured via `capture_f3.sh` (drives the real `isolith` binary under xvfb
through the XTEST injector `ui_f3_inject.c`), two independent launches,
byte-identical.
