# UI-B5 UI-SNAP-INFERENCE: endpoint, midpoint, centre, on-edge, intersection, h/v

Behaviour implemented and proven (wave B, card UI-B5):

- **While drawing, the pointer snaps to the nearest inference candidate.** The
  pure engine lives in the new `crates/oep-app/src/sketch/snap.rs` (egui-free,
  like B4's `tools.rs`): [`snap::infer`] takes the committed sketch document
  and the pointer's raw plane coordinate and returns the winning candidate -
  an [`snap::Snap`] carrying the exact snapped coordinate, the inference kind
  and the pointer-to-snap distance. Selection is explicit and tested:
  **priority, then distance, then document order**. Every candidate within
  `SNAP_RADIUS` (5 plane units = half a sketch-grid cell) competes; the lowest
  priority rank wins, ties within a rank go to the nearest, equal candidates
  fall back to document order for determinism.
- **The six inference types, each with one test asserting the exact snapped
  coordinate from a fixture sketch**: Endpoint (the end of a committed line),
  Midpoint (the halfway point of a line), Centre (the centre of a circle),
  On-edge (the closest point on a line segment, proven on a rotated diagonal
  too), Intersection (the crossing of two line segments), and h/v - the
  Horizontal / Vertical alignment guides that snap the pointer into a point's
  row or column. A fixture sketch (10 points, 4 lines, 1 circle) exercises all
  of them in `--test snap_inference`.
- **Priority order explicit and tested**: the card's named pair endpoint beats
  on-edge (asserted with the on-edge candidate *nearer* in distance yet losing
  on priority); intersection beats on-edge, midpoint beats on-edge, centre
  beats on-edge, and the ambient h/v guides rank last (on-edge beats the row
  guide at equal distance and coordinate). Endpoint means a line end, so a
  circle centre infers as Centre (never Endpoint) and a shared corner of two
  lines infers as Endpoint (rank 0, not Intersection).
- **A glyph plus a label naming which inference fired.** The firing inference
  is carried on `SketchState::snap_hit`; the preview layer paints a per-kind
  glyph (solid square = Endpoint, X = Intersection, triangle = Midpoint, dot
  in a ring = Centre, diamond = On-edge, guide line = h/v) at the snapped
  point and a text label (`Endpoint`, `Intersection`, `Midpoint`, `Centre`,
  `On edge`, `Horizontal`, `Vertical`) beside it, in accent ink from the
  semantic token palette. The oracle publishes `snap=on/off`, `infer=<kind>`
  and the exact `snapx=/snapy=` coordinate so the acceptance reads which
  inference fired and where the pointer is being drawn to.
- **Clicks land exactly on the snapped coordinate.** Both the hovered pointer
  (rubber band + inference) and the placement click resolve through
  `effective_uv`, which snaps the raw plane coordinate through `snap::infer`
  to the winning candidate before the gesture machine places the vertex.
- **The toggle turns inference off entirely, and is a command token like any
  other.** `sk:snap` lives in the same `sk:` family as `sk:point`/`sk:line`;
  its dispatch family head validates through the CLI/router step
  (`oep_shell::commands::validate_token("sk")`), it is driven as a plain
  sketch-mode key (F8, free in the UI-B1 table) the way the tool keys are, and
  its name is reported in the oracle's `last=` line. Off means fully off: no
  inference reported, no glyph, no label, and clicks land on the raw
  coordinate (asserted in the kittest).

## Acceptance (real command output in `transcripts/`)

1. `cargo test -p oep-app --test snap_inference` - 101 passed, exit 0
   (`01-snap-inference.txt`). The per-type tests
   (`endpoint_/midpoint_/centre_/on_edge_/on_edge_rotated_/intersection_/
   horizontal_/vertical_`) each assert the **exact dropped plane coordinate**
   from the fixture; the priority tests pin the precedence pairs
   (`endpoint_beats_on_edge_even_when_the_edge_is_nearer`, intersection and
   midpoint and centre the same, `on_edge_beats_the_ambient_row_alignment`,
   `endpoint_beats_intersection_at_a_shared_corner`,
   `endpoint_is_a_line_end_not_every_committed_point`); the toggle tests prove
   disabling (`non_positive_radius_disables_inference`,
   `toggle_is_a_command_token_like_any_other`) and the two kittest runs drive
   the REAL app - hover fires Endpoint with the exact snapped coordinate and
   the click lands exactly on it
   (`pointer_snaps_and_lands_on_the_endpoint_with_glyph_and_label`), and F8
   turns inference off so the same click lands raw
   (`snapping_toggle_disables_inference_and_lands_raw_coordinates`, which also
   restores inference by toggling back on).
2. `cargo test -p oep-app` - all 13 test targets green
   (48+54+51+9+18+61+52+53+87+84+101+52+51 = 721 tests), exit 0
   (`02-oep-app-full.txt`). UI-A2..A12, UI-B1..B4 suites stay green
   underneath; the new oracle fields are additive and no prior assertion
   parses the full report text.
3. `xvfb-run -a cargo run -p oep-app -- --smoke` - `frames=8 smoke ok`, exit 0
   (`03-smoke.txt`).
4. `cargo clippy -p oep-app --all-targets -- -D warnings` - exit 0, and this
   card's three owned files are rustfmt-clean (`04-fmt-clippy.txt`).

## Visual proof

Two deterministic screenshots of the real binary under xvfb, each captured
twice from independent launches and required to be byte-identical
(`06-determinism.txt`; the `.sha256` files are the oracle):

- `snap_on.png` (sha256
  `f2efc0dcda8e5d2f4b6657816d74665e641ec780baa45afad3265ed1a9abc06b`): after
  entering XY sketch and drawing one line (two clicks), the pointer hovers a
  few pixels off the line's endpoint and the **Endpoint** inference fires -
  the solid accent-square glyph paints on the exact endpoint and the label
  names the fired inference.
- `snap_off.png` (sha256
  `12d832e603a07a8a7e1f11c2ee287a7bbaa386419f01972496e3822b1c2916ae`): the
  same document and the same hover, with inference toggled off through F8 (the
  `sk:snap` command token) - no glyph, no label, and the painted status text
  reads `snap off (sk:snap)`.

Driven by `capture_snap.sh` (on|off) through the XTEST injector `ui_b5_inject.c`
(the UI-B4 injector, kept byte-identical), reusing the UI-B4 capture's
verified sheet coordinates at 1280x800.

## Formatting / lint

`04-fmt-clippy.txt`: clippy `-D warnings` over all oep-app targets exits 0.
`snap.rs` and `panels/sketch.rs` are rustfmt-clean (individual file checks;
the `#[path]` recursion reaches the wave-head `src/input.rs`, whose trailing
blank line is the ONLY delta reported for the test file - it is UI-B1's file,
outside this card's list, and is byte-identical to the head; UI-B2/B3/B4
document the same condition). `cargo fmt --all -- --check` therefore still
flags that same pre-existing line, left untouched for the wave integrator.

## Files owned by this card (exactly the assigned list)

- `crates/oep-app/src/sketch/snap.rs` (new): the egui-free inference engine
  (`Inference` kinds with explicit priority, `Snap`, `infer`, `SNAP_RADIUS`,
  the `sk:snap` toggle command token).
- `crates/oep-app/src/panels/sketch.rs`: the interactive half - the plain-key
  toggle (F8 = `sk:snap`), `effective_uv` snapping the hovered pointer and the
  placement click, the `snap_hit` state and its `snap=`/`infer=`/`snapx=`/
  `snapy=` oracle lines, and the per-kind glyph + label painted in
  `draw_preview`.
- `crates/oep-app/tests/snap_inference.rs` (new): the acceptance target.
- `workstreams/modeler/ui/README.md` (UI-B5 row), `workstreams/modeler/ui/
  proof/UI-B5/` (this proof).

No other crate or file is touched; no Cargo.toml / dependency change.

## Notes on scoping (honest substitutions, for the integrate card)

- **The toggle's engine-side command record is not registered yet.** `sk:snap`
  is a real `sk:`-family command line whose family head validates through the
  exact CLI/router step (`validate_token("sk")`), and the kittest drives it
  through the sketch-mode key. Adding the `sk:snap` line to `oep-shell`'s
  registry is outside this card's file grant (that crate is not owned here);
  the token is spelled so a later wiring card can register it without a
  spelling change, and the card documents that resolution honestly - it never
  fabricates a registry entry that does not exist.
- **Endpoint = the end of a line, not any committed point.** In this wave's
  `SketchDoc` a circle's centre is a committed point, so if Endpoint were "any
  point" no circle centre could ever infer as Centre (the Endpoint rank would
  always win at the same coordinate). Endpoint is therefore defined as "a
  point that terminates a committed line" (asserted in
  `endpoint_is_a_line_end_not_every_committed_point`); a circle-centre-only
  point infers as Centre and a free point earns only its h/v guides.
- **h/v are two kinds, both tested.** The card's `h/v` slot is delivered as
  the two alignment guides `Horizontal` (snap into a point's row) and
  `Vertical` (snap into a point's column), each with its own exact-coordinate
  test, glyph and label.

## Step 0

`05-step0-merge.txt`: the branch was cut from main `736a0a7`; `git merge
github/wave/ui-b` (the wave lives on the `github` remote - NAS Gitea does not
host `wave/ui-b` - so STEP 0's `origin/wave/ui-b` maps to `github/wave/ui-b`)
brought the whole earlier wave (UI-B1..B4, the merged head `38dcdb6` == `github/
wave/ui-b`) with no conflicts. This card's files are the only delta on top.
