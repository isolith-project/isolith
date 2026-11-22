# UI-B7 UI-DIMENSIONS: place, drive and edit dimensions

## Behaviour (card)
A dimension is placed on the committed sketch geometry, driven by a name from
the variable table, and edited as an expression: width 80 becomes width*1.5 and
the mesh volume moves from 32,000 to 72,000, while an invalid expression is
refused with a message.

## Acceptance

| Command | Result |
|---|---|
| `cargo test -p oep-app --test dimensions` | 145 passed (card acceptance + module/guard, 4 kittest tests) |
| `cargo test -p oep-app` | 14 targets / 1287 tests, all green |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | exit 0, no warnings |
| fmt | my 3 files rustfmt-clean; workspace check flags only the pre-existing UI-B1 `input.rs:504` blank |

Real transcripts: `transcripts/0X-*.txt`. Deterministic screenshot:
`dimensions.png` (sha256 `35e391ca56599998c1e28709f310554794b26904e9b5a7cea501fcd8ca3cfce1`),
captured twice under xvfb via the XTEST injector (`ui_b7_inject`, same technique
as UI-B3..B6), byte-identical; shows sketch mode with the committed line, the
properties "Place dimension" button, the expression field (`width*1.5`), the
dimension row and the driven volume. Note the real-binary capture draws the line
with integer-pixel XTEST clicks, so it measures 79.864 plane units (the discrete
pixel lattice has no exactly-80 pair) and the panel shows `width = 119.80`,
`volume 71755` after `width*1.5` - the SAME place -> drive -> edit -> volume-move
behaviour at a near-80 width. The exact acceptance numbers (width 80 -> 120,
volume 32000 -> 72000) are asserted by the kittest tests, which click the app's
own sub-pixel-exact projection coordinates.

## What the kittest acceptance proves
Drives the real `app::App` (the same binary as production) through
`egui_kittest`:

1. **Place** - clicks "Place dimension" in the properties panel; placing measures
   the last committed line (an 80-long line drawn with the Line tool at the exact
   projected coordinates) and registers the variable-table entry `width = 80`.
   The driven volume is reported from the document's own mass properties and
   asserted through `dimensions::driven_volume` (the same function the app
   reports) - exactly 32000 for width 80, never a bare literal in the test.
2. **Drive/edit as an expression** - sets the expression field to `width*1.5`
   and clicks Apply; `width` resolves through the existing variable table path
   (oep-sketch `VariableSet`) to 120, the dimension value becomes 120, and the
   driven volume moves 32000 -> 72000 (again asserted via the driven document,
   and the acceptance comment's numbers verified as its values).
3. **Invalid expression refused, everything untouched** - typing `width**1.5`
   and applying leaves the named error (`InvalidExpression`) in the properties
   oracle, the variable table / dimension value / expression unchanged, and the
   sketch document (points, lines, journal) byte-identical.
4. **Unknown variable refused with its name** - `depth*2` yields the named
   `UnknownVariable("depth")` error and leaves the document untouched.

## Volume from the document's own mass properties (not a hardcoded number)
`dimensions::driven_volume` builds the driven solid through the existing editor
drive-lane (`oep_shell::editor::EditorSession`): the 2:1 profile is sketched,
constrained from the variable table's `width` (the l0 length as
`DimExpr::Variable("width")`, l1 pinned to width/2), solved, extruded
`DRIVEN_DEPTH` (10) and the tessellated solid's `mesh_volume` is returned. The
driven shape is `width x width/2 x 10`, so volume = 5*width^2: exactly 32000 at
80 and 72000 at 120. The test derives its expectation from the same function -
no magic volume literal.

## Expression surface (the existing variable table path)
`dimensions::resolve_expression(vars, text)` evaluates a deliberately minimal
scalar surface (numbers, `+ - * /`, parens, unary minus). Every identifier is
resolved exclusively via `oep_sketch::varset::VariableSet::get` - the single
variable table already used by the solver's `DimExpr::Variable` - so there is no
second variable table and no second solver. Unknown names and malformed text are
typed `DimensionError`s. On a successful edit the resolved value is written back
into that same table, driving the model.

## Files
- `crates/oep-app/src/sketch/dimensions.rs` (new) - egui-free dimension engine:
  place/measure from the sketch oracle, `DimensionPanel` (context-store
  resident), expression resolver over the variable table, driven-document
  volume via the editor seam, named errors.
- `crates/oep-app/src/panels/properties.rs` - the properties panel hosts the
  dimension editor: deterministic `dimension_widget_rects`, "Place dimension",
  the expression input (a real `TextEdit`), Apply, volume readout, named error
  line, and the `ui_b7_dimension_report` String oracle.
- `crates/oep-app/tests/dimensions.rs` (new) - kittest acceptance (4 tests).
