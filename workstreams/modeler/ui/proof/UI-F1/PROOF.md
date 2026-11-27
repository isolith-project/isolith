# UI-F1 UI-SHEET: sheet setup and sheet tree — proof

Wave F card. Drawing sheets are first-class objects: size, scale, projection
angle and multiple sheets, selectable through a sheet tree.

## Files owned by this card

- `crates/oep-app/src/panels/sheet.rs` (new: sheet tree + sheet properties,
  hosts the F2-F4 sub-panels via `#[path]` includes as those cards land)
- `crates/oep-app/src/panels/mod.rs` (ADDITIVE: `pub mod sheet;` the Drafting
  region `RegionKind::Drafting`, `RegionKind::ALL` 6 -> 7)
- `crates/oep-app/src/app.rs` (ADDITIVE: Drafting region rendered after the
  properties region)
- `crates/oep-app/src/config.rs` (ADDITIVE: the region's rect, nested in the
  lower 48% of the properties column, exactly like the assembly region nests
  in the model-tree column - the five locked UI-A4 rects stay unchanged)
- `crates/oep-app/Cargo.toml` (ADDITIVE: `oep-drafting` path dependency)
- `Cargo.lock` (one line: the `oep-app` package's dependency list gains
  `oep-drafting`; no new crate, no version motion)
- `crates/oep-app/tests/sheet.rs` (new: acceptance harness)
- `workstreams/modeler/ui/proof/UI-F1/` (this proof)
- `workstreams/modeler/ui/README.md` (row updated in the same commit)

The panel uses the EXISTING drafting seam only: `oep_drafting::layout`
(placement legality re-derived from `layout_views` in a module test) and
`oep_drafting::builder::drive_bracket_drawing` for the committed bracket
fixture. No second layout engine, no second fixture.

## Acceptance

```
cargo test -p oep-app --test sheet        -> green (155 passed, 0 failed)
xvfb-run -a cargo run -p oep-app -- --smoke -> frames=8 smoke ok, exit 0
```

Transcripts: `transcripts/01-sheet-acceptance.txt`, `transcripts/02-smoke.txt`.

### 1. Sheet properties round-trip: A3, 1:2, first-angle, byte-identical

The committed bracket drawing opens with one sheet (`oep-sheet-1`, A4
landscape, third-angle, scale 1:1, three views). The acceptance clicks the
real UI: select `Sheet 1 · A4 landscape · third-angle · scale 1:1`, then `A3`,
`1:2`, `first-angle`. The oracle reports:

```
after edit:
size=420x297
size_name=A3 landscape
scale=1:2
projection=first-angle
views=3
edited doc_hash = 13290092033220856568
```

Save (the canonical `.oepdoc` JSON the panel publishes every frame), reopen it
both as a fresh `oep_geo::Document` and by re-seeding a fresh app harness, and
assert every field byte-identical - both records printed:

```
SAVED   sheet record = Sheet { id: "oep-sheet-1", ... size: 420x297,
                              projection: FirstAngle, title_block.scale: "1:2", ... }
REOPEN  sheet record = Sheet { ... identical ... }
assert reopened_sheet == saved_sheet (byte-identical)
assert reopened_doc.to_json_pretty() bytes == saved bytes
reopened in UI: sel=oep-sheet-1 size=420x297 scale=1:2 projection=first-angle doc_hash=13290092033220856568
```

### 2. Projection angle changes which views are LEGAL

On the committed third-angle sheet, the first-angle top placement (top below
front) is REFUSED with the named error shown in the UI; flipping the sheet to
first-angle makes the same placement legal. Printed from the run:

```
third-angle attempt:
placement_status=refused
placement_error=SheetError::IllegalPlacement: cannot place the top view below the front view. On a third-angle sheet the sole legal placement for the top view is above the front view.
named refusal = SheetError::IllegalPlacement: cannot place the top view below the front view. On a third-angle sheet the sole legal placement for the top view is above the front view.
```

The test asserts the error is named (`SheetError::IllegalPlacement`), names
the view (`top`), the placement (`below the front view`) and the sheet
projection (`third-angle`), and that the refusal text is genuinely a rendered
AccessKit label - then that the legal placement (top above) is accepted, and
after switching the sheet to first-angle the same "top below" placement is
now `legal` with `placement_error=-`. A module test (`legal_placement_matches_layout_seam`)
re-derives the rule from the EXISTING `oep_drafting::layout::layout_views`
output so the UI rule can never drift from the seam.

### 3. Two sheets coexist, independently selectable, each keeps its own properties

`Add sheet` creates `oep-sheet-2` (A4, third-angle, 1:1, no views) and selects
it; sheet 1 is then edited to A3/1:2/first-angle. Reselecting sheet 2 keeps its
own defaults, reselecting sheet 1 keeps its own A3/1:2/first set, and the
canonical document holds both distinct records (printed in the transcript).

## Guards and full gates

- Mutation guard green: all sheet edits (size/scale/projection/add) go through
  `doc_io::EditHistory::apply` (the UI-B2 routed seam) - no direct document
  write appears in `src/` (`transcripts/01-sheet-acceptance.txt`).
- clippy `-D warnings` (all targets, workspace): exit 0 - `transcripts/03-clippy.txt`.
- fmt `--check`: exit 0 - `transcripts/04-fmt.txt`.
- licensing 6/6 (34 crates / 535 deps, unchanged): exit 0 - `transcripts/05-licensing.txt`.
- `cargo test --workspace`: **231 suites / 6284 passed / 0 failed**, exit 0 -
  `transcripts/06-workspace.txt`. Known flake: the documented egui_kittest
  teardown SIGSEGV under full parallel load hits the failing-run logs
  (`layout`), and the clean re-run is green (see `transcripts/07-workspace-flake.txt`).
- smoke: `frames=8 smoke ok`, exit 0 - `transcripts/02-smoke.txt`.

## Screenshot (deterministic: two independent captures byte-identical)

- `sheet.png` - the drafted region of the real merged-head binary: the sheet
  tree (`Sheet 1 · A4 landscape · third-angle · scale 1:1`), `Add sheet`, the
  size/scale/projection selectors, and the view-placement legality line.

sha256 in `sheet.png.sha256`:

```
9b7bcbb67d52a4a87d9a6ed7e17432b1332efe4d56e5aa4f119ad256591c267a
```

Captured via `capture_f1.sh` (drives the real `isolith` binary under xvfb
through the XTEST injector `ui_f1_inject.c`), two independent launches,
byte-identical.
