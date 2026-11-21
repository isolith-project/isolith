# UI-B3 UI-SKETCH-ENTER: pick a datum plane and enter sketch

Behaviour implemented and proven (wave B, card UI-B3):

- **Plane pick on click** goes through the existing viewport pick seam
  (`oep_viewer::pick::unproject`, the same seam `--test viewport` drives) - no
  second picker. The three principal datum planes (XY/XZ/YZ) sit through the
  scene centre (`panels/sketch.rs::datum_planes`); the nearest sheet under the
  cursor wins (exact-tie tiebreak to the most frontal sheet is deterministic).
- **Camera normal within 1e-6**: `normal_orientation` swings yaw/pitch so the
  forward vector is parallel to the picked plane's normal (asserted
  numerically by `--test sketch_enter`, never by screenshot). Pitch is clamped
  away from exactly vertical so the fixed-world-up look-at never degenerates.
- **Sketch mode + grid layer**: `panels/sketch.rs` owns the sketch state in
  the egui context store and publishes a `String` oracle
  (`mode=/plane=/label=/normal=/grid=`). While a plane is active the viewport
  draws the highlight outline + centre marker + sketch grid projected onto the
  plane (`grid_overlay`), and the model tree shows a Sketch node naming the
  active plane (through the cross-`#[path]`-copy-safe oracle).
- **Escape leaves sketch mode without touching the document** (the doc panel's
  oracle is byte-identical before and after).

## Acceptance (real command output in `transcripts/`)

1. `cargo test -p oep-app --test sketch_enter` — 18 passed, exit 0
   (`01-sketch-enter.txt`). The three acceptance behaviours are the three
   focused tests: click plane -> mode active + camera normal 1e-6 + grid layer
   on + tree sketch node; Escape -> idle, grid off, document untouched; a
   click off any datum plane stays in normal mode.
2. `cargo test -p oep-app` — all 10 test targets green (153 tests total), exit
   0 (`02-oep-app-full.txt`). UI-A6 palette / UI-A7 doc / UI-B1 router / UI-A5
   viewport suites all stay green underneath.
3. `xvfb-run -a cargo run -p oep-app -- --smoke` — `frames=8 smoke ok`, exit 0
   (`03-smoke.txt`).

## Visual proof

`sketch.png` (sha256 `bec40eabd3fef0290e037bafc3ffe1b9253ef3a5bb696f968e73f3672f94bf43`,
byte-identical across two independent launches under xvfb, driven by a real
left click through the XTEST injector `ui_b3_inject`) shows the app in sketch
mode: the model tree lists the Sketch node with the active plane, the status
bar reads the active plane, and the viewport carries the plane highlight, the
sketch grid layer and the centre pick marker. Captured by
`capture_sketch.sh`, decoded with the same strict XWD->PNG decoder the wave
uses; determinism is enforced (nondeterministic captures abort the script).

## Formatting / lint

`04-fmt-clippy.txt`: clippy `-D warnings` over all oep-app targets exits 0.
The only rustfmt delta rustfmt 1.9.0 reports through the `#[path]` chain is a
pre-existing trailing blank line in UI-B1's `src/input.rs` (committed at the
wave head 780dc56, outside this card's file list) - `06-input-fmt-drift.txt`.
The four files owned by this card are fmt-clean; UI-B1's `input.rs` should be
re-formatted by the wave integrator at merge time (one whitespace line).

## Files owned by this card (exactly the assigned list)

- `crates/oep-app/src/panels/sketch.rs` (new): sketch mode state, datum-plane
  geometry, pick resolution, camera-normal orientation, grid-layer projection.
- `crates/oep-app/src/panels/viewport.rs`: plane picking on click through the
  existing unproject seam, camera swing, Escape-leaves-sketch, grid-layer
  overlay painting, sketch-aware status bar.
- `crates/oep-app/src/panels/tree.rs`: Sketch node appears while in sketch
  mode (reads the sketch report oracle).
- `crates/oep-app/tests/sketch_enter.rs` (new): the acceptance target.

No other crate or file is touched; no Cargo.toml / dependency change.
