# UI-B4 UI-SKETCH-DRAW: point, line, polyline, rectangle, circle

Behaviour implemented and proven (wave B, card UI-B4):

- **Rubber-band drawing with live preview and typed dimensions.** The five
  draw tools (Point **P**, Line **L**, Polyline **N**, Rectangle **R**,
  Circle **C** - plain keys, free in the UI-B1 table) live in the sketch
  module's per-frame interaction, run from `read_state` before the viewport
  handles clicks/Escape. A click inside the viewport resolves onto the active
  datum plane through the existing pick seam
  (`oep_viewer::pick::unproject` + `ray_plane_uv`, the same seam UI-B3's
  plane picker uses) and feeds the pure gesture machine in
  `src/sketch/tools.rs` (egui-free, unit-tested). The preview is **drawn from
  tool state only**: placed-vertex markers, the chain/rubber band to the
  cursor, the circle bound, and a live numeric dimension readout
  (`len …` / `W x H` / `r …`) painted in a non-interactive foreground layer -
  nothing is committed until the gesture completes.
- **Four clicks produce a closed rectangle that lands in the document as four
  points and four lines.** Point/Line/Rectangle/Circle auto-complete at their
  fixed click count (1/2/4/2); a rectangle's fourth click returns a
  `FinishedShape` whose four lines close back to the first point (`l3: p3->p0`)
  and commits it. Polyline is an open chain committed by Enter.
- **Escape cancels one step only; a second Escape leaves the tool.** The
  gesture machine (`Gesture::escape`) pops exactly one placed vertex on the
  first Escape (the tool stays selected); a second Escape leaves the tool
  (the remaining preview is dropped); with no tool active Escape is left for
  the viewport panel, which exits sketch mode.
- **Document commands through the UI-B1 router and the UI-B2 journal;
  nothing writes geometry directly.** A completed gesture is applied one
  command at a time through the B2 `history::Journal` (hash-before -> mutate
  -> hash-after, a fresh entry per discrete `sk:point` / `sk:line` command).
  Every command line is one the shared `oep_shell::commands` registry
  registers - the dispatch family `sk` validates through the CLI's
  `validate_token` and the full name resolves through `commands::find`
  (`tools::command_is_registered`, asserted in tests + on every commit). The
  sketch document (`tools::SketchDoc`, ids `p0..`/`l0..`/`c0..`) is the only
  commit target and is written solely inside that journaled seam.
- **Sketch document is real and hashable.** `SketchDoc` (points / lines /
  circles) serialises to a canonical std-only text hashed with the engine's
  FNV-1a-64 scheme (same constants as `doc_io::document_hash`), so every
  journal entry records a genuine before/after fingerprint, and the oracle
  (`mode=/plane=/grid=/tool=/steps=/preview_*/dim*/docpoints=/doclines=/
  circles=/journal=/point=/lineseg=/last=`) lets the acceptance read exactly
  what landed in the document.

## Acceptance (real command output in `transcripts/`)

1. `cargo test -p oep-app --test sketch_draw` - 77 passed, exit 0
   (`01-sketch-draw.txt`). The six focused behaviours:
   `four_clicks_build_a_closed_rectangle_in_the_document` (4 points at the
   four click sites, 4 lines closing `l3 p3->p0`, journal = 8 commands),
   `preview_is_never_committed_until_the_rectangle_completes` (docpoints/
   doclines stay 0 through clicks 1-3 while preview steps grow),
   `escape_cancels_one_step_then_a_second_leaves_the_tool` (3->2->tool left
   ->sketch exited, document untouched throughout), plus line, point and
   polyline paths.
2. `cargo test -p oep-app` - all 12 test targets green (560 tests:
   43+49+46+9+18+56+47+48+77+74+47+46), exit 0 (`02-oep-app-full.txt`).
   UI-A2..A12, UI-B1/B2/B3 suites all stay green underneath.
3. `xvfb-run -a cargo run -p oep-app -- --smoke` - `frames=8 smoke ok`, exit 0
   (`03-smoke.txt`).
4. `cargo clippy -p oep-app --all-targets -- -D warnings` - exit 0, and this
   card's three files are rustfmt-clean (`04-fmt-clippy.txt`).

## Visual proof

`draw.png` (sha256
`66e7e19e5566caf4e28775f228376ffe2dfb535d7ae1125d12db044410db7b12`,
byte-identical across two independent launches under xvfb, `draw.png.sha256`)
shows the real binary on the **XY (Top)** sketch plane with the **Rectangle**
tool active and three preview corners placed: the vertex markers, the chain
edges, the live rubber band to the hovered cursor, and the live dimension
readout `35.11 x 20.93` (demo scene is the canonical 80x40x10 solid; dims
come from the placed plane coordinates). Driven by `capture_draw.sh` through
the XTEST injector `ui_b4_inject.c` (extends UI-B3's injector with key +
move actions); the click pixels were derived from the app's own
world-to-panel math (`capture_probe.rs` was a temporary dev-scratch test,
removed before ship).

## Formatting / lint

`04-fmt-clippy.txt` + `06-fmt-drift.txt`: clippy `-D warnings` over all
oep-app targets exits 0 and this card's three owned files are rustfmt-clean.
`cargo fmt --all -- --check` still flags ONLY the pre-existing trailing blank
line at the end of UI-B1's `src/input.rs` (committed at the wave head, outside
this card's file list, byte-identical to the head; UI-B2/B3 document the same
condition). That file is deliberately left untouched here - the wave
integrator should re-format it at merge time (one whitespace line).

## Files owned by this card (exactly the assigned list)

- `crates/oep-app/src/sketch/tools.rs` (new): the egui-free draw-tool state
  machine (Tool / Gesture / FinishedShape / SketchDoc + hash), Escape
  semantics, and the registry-faithfulness contract.
- `crates/oep-app/src/panels/sketch.rs`: the draw interaction (tool keys,
  Enter, Escape, click/hover resolution through the pick seam), the preview
  layer, the journaled commit seam, the extended oracle, and the exact
  viewport-rect derivation the interaction clicks against.
- `crates/oep-app/tests/sketch_draw.rs` (new): the acceptance target.
- `workstreams/modeler/ui/README.md` (UI-B4 row), `workstreams/modeler/ui/
  proof/UI-B4/` (this proof).

No other crate or file is touched; no Cargo.toml / dependency change.

## Notes on scoping (honest substitutions, for the integrate card)

- **"Through the UI-B2 journal" is the B2 journal itself.** `doc_io::
  EditHistory` is typed to an `oep_geo` Document whose feature params need
  serde_json, and neither serde_json nor oep-sketch is a dependency this card
  may add. The commit seam therefore reuses the B2 journal's own
  `history::Journal` + `DragId` directly (same push/hash/capacity semantics)
  over the sketch module's own `SketchDoc`, hashed with the same FNV-1a-64
  constants the engine uses. The tokens journaled and the registry checks are
  exactly the router/CLI contract. Wiring the sketch document INTO the
  visible part document (UI-A7's `doc_io` document) is left to the integrate
  card - this card keeps "one behaviour".
- **"Typed dimensions"** is delivered as the live numeric readout on the
  rubber band (length / width>height / radius in plane units). Driving and
  editing dimension values is UI-B7's card.
- **Escape semantics** are exactly the card's words: one Escape cancels one
  step (not the whole gesture); the second Escape leaves the tool (remaining
  preview dropped, sketch mode kept); a further Escape exits sketch via the
  unchanged UI-B3 viewport path.

## Step 0

`05-step0-merge.txt`: `git merge github/wave/ui-b` on the branch cut from
main `736a0a7` brought the merged wave head `270e833` (UI-B1/B2/B3, 39 files)
with no conflicts; this card's work then forked from the wave head as the
wave rules require.
