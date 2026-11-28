# UI-F4 UI-DRAWING-EXPORT: SVG and PDF export from the UI - proof

Wave F card. The drawing exports to SVG and PDF from the UI, with a print
preview, and the PDF matches the committed bracket drawing golden within a
documented tolerance.

## Files owned by this card

- `crates/oep-app/src/panels/drawing_export.rs` (new: export buttons, a print
  preview painted from the SAME primitive list the writers serialize, and
  honest named errors - `ExportError::NoRenderableSheet` and
  `ExportError::Io { path, source }`; the bytes are materialised fully in
  memory and written in one `fs::write`, so a failed export never leaves a
  partial file)
- `crates/oep-app/src/panels/sheet.rs` (ADDITIVE: one `#[path]` include of
  `drawing_export.rs` next to the existing views/annotate includes, and one
  `drawing_export::show(...)` call right after `annotate::show(...)` at the end
  of the drafting scroll content. The F1/F2/F3 controls' node rects are
  unchanged - verified: `--test sheet` 170 passed, `--test views` 169 passed,
  `--test annotate` 170 passed.)
- `crates/oep-app/tests/drawing_export.rs` (new: acceptance harness, drives the
  REAL `app::App` through egui_kittest like the F2/F3 tests)
- `workstreams/modeler/ui/proof/UI-F4/` (this proof)
- `workstreams/modeler/ui/README.md` (row updated in the same commit)

The panel owns no rendering path: it renders the live selected sheet with the
EXISTING drafting seam (`oep_drafting::render_sheet` over
`oep_drafting::solid_from_graph` + `oep_drafting::extract_variables`, the exact
limbs `render_document` composes, split so the SELECTED sheet exports) and
serializes with the CARD-MANDATED writers `oep_drafting::export::svg::write_drawing_svg`
and `oep_drafting::export::pdf::write_drawing_pdf`. No new writer, no second
rendering path.

## Acceptance

```
cargo test -p oep-app --test drawing_export -> green (5/5 acceptance + module tests)
xvfb-run -a cargo run -p oep-app -- --smoke  -> frames=8 smoke ok, exit 0
```

Transcripts: `transcripts/01-export-acceptance.txt` (green `--test
drawing_export`, 170 passed / 0 failed), `transcripts/01b-numbers.txt` (the
ordered numbered clauses), `transcripts/02-smoke.txt`.

### 1. SVG is deterministic: two exports of the same sheet are byte-identical

Two independent app launches export the committed bracket through the real
`Export SVG` button into two paths; both files are read back from disk, both
FNV-1a-64 hashes printed, bytes asserted equal:

```
SVG #1 hash = 7869596012267675978 (3158 bytes)
SVG #2 hash = 7869596012267675978 (3158 bytes)
```

`assert svg#1 == svg#2` byte-for-byte. The SVG is exactly the committed golden
size (3158 bytes).

### 2. PDF matches the committed bracket golden within a documented tolerance

The PDF is exported through the real `Export PDF` button and compared against
the committed `crates/oep-drafting/tests/golden/bracket-drawing.pdf`. Metric
compared: **page geometry (MediaBox)** and **element positions (content-stream
drawing ops)**, printed golden vs actual:

```
GOLDEN pdf: page 0 0 297 210, content ops 45
ACTUAL pdf: page 0 0 297 210, content ops 45
PDF byte-identical to golden: 1946 == 1946 bytes
```

`assert golden.media_box == actual.media_box`, `assert golden.ops == actual.ops`,
and `assert golden.bytes == actual.bytes` - the strongest claim (tolerance 0:
the exported PDF is byte-identical to the committed golden, 1946 bytes).

### 3. The print preview shows the same sheet content as the export (element count)

The preview paints the same `drawing_primitives` list the writers serialize and
reports its graphical count; the exported SVG (`<path`/`<rect`/`<text`
elements) and PDF (content ops) both serialize exactly that many:

```
preview elements = 45, SVG serialized elements = 45
PDF serialized elements = 45
```

`assert svg_elements == preview_elements && pdf_elements == preview_elements`.

### 4. A failed export (unwritable path) surfaces a named error and leaves no partial file

The export path is pointed at a location that cannot be written (a path
component that is a file). The UI report oracle surfaces a NAMED
`ExportError::Io` naming the path, and no file exists at the target:

```
error=ExportError::Io { path: ".../out.pdf", source: Not a directory (os error 20) }
last=export PDF failed: ExportError::Io { path: ".../out.pdf", source: Not a directory (os error 20) }
```

`assert error contains "ExportError::Io"` and `assert !target.exists()`.

### 5. The export runs from the real UI path in a kittest harness

Every clause above clicks the real `Export SVG` / `Export PDF` buttons in the
rendered `app::App` (AccessKit labels found in the tree) and reads the files
back from disk - never a unit call. The harness reports the buttons' rects and
cross-checks the panel-reported bytes/hash against the on-disk file.

## Guards and full gates

- Mutation guard green: the export panel only READS the document and writes
  files; no document mutation, no journal entry, so the guard stays green.
- clippy `-D warnings` (workspace, all targets): exit 0 - `transcripts/06-clippy.txt`.
- fmt `--check`: exit 0 - `transcripts/05-fmt.txt`.
- licensing 6/6 (34 crates / 535 deps, unchanged - oep-app adds no dependency,
  `Cargo.lock` untouched): exit 0 - `transcripts/07-licensing.txt`.
- `cargo test --workspace -j 1`: **234 suites / 7228 passed / 0 failed**, exit 0 -
  `transcripts/04-workspace.txt`.
- F1/F2/F3 regression (the drafting scroll grew by the export section; their
  node rects untouched): `--test sheet` 170/0, `--test views` 169/0,
  `--test annotate` 170/0 - `transcripts/03-f1f2f3-regression.txt`.
- smoke: `frames=8 smoke ok`, exit 0 - `transcripts/02-smoke.txt`.

## Screenshot (deterministic: two independent captures byte-identical)

- `export.png` - the real merged-head binary at the default 1280x800 window.
  The capture wheel-scrolls the drafting scroll area to its bottom clamp (the
  Export section is the LAST section, below sheet/views/annotate), clicks
  `Export SVG` at the post-scroll probe-derived coordinates (which really
  writes the target file: the capture dir holds a 3158-byte `drawing.svg`),
  parks the pointer neutral and captures. The frame shows the export tools
  live: the `Print preview` canvas painted from the same primitive list the
  writers serialize (the `45 graphical elements` readout), the export target
  path, and both `Export SVG` / `Export PDF` buttons.

sha256 in `export.png.sha256` (and `images.sha256`):

```
b7e49783d983718b30e6b9334eeba9a968ec4e89ee869d80d8d6f1aa7f803d1f
```

Captured via `capture_f4.sh` (drives the real `isolith` binary under xvfb
through the XTEST injector `ui_f4_inject.c`), two independent launches,
byte-identical.
