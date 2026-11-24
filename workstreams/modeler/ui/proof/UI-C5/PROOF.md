# UI-C5 UI-MEASURE: distance, angle, radius

## Behaviours landed (as the card reads them)

- **Pick two entities, the measurement is annotated on screen.** Plain `M`
  opens the measure surface over the viewport (`panels/measure.rs`, wired from
  `viewport.rs` like `sketch.rs` / `features.rs`). Clicking an entity button
  picks it; a second click picks the second entity, and the surface draws the
  annotation - an accent leader between the two anchors in the preview box plus
  a value `Label` line naming the pair and the measured value.
- **Measurement math lives in the geometry layer, not the panel.** The
  egui-free engine section of `measure.rs` (`distance`, `line_angle_deg`,
  `measure`, the fixture accessors) is the card's geometry layer: pure
  functions over the committed bracket fixture's plane coordinates. The panel
  `draw`/`oracle_lines` only FORMAT the engine's number and DRAW it; no drawing
  path recomputes or resolves a measurement.
- **Values match the analytic goldens for the committed bracket fixture.**
  The committed fixture is a 40 x 30 profile (`p0 (0,0)`, `p1 (40,0)`,
  `p2 (40,30)`, `p3 (0,30)`, the four edge lines `l0..l3`) plus a 10-unit
  circle `c0` through centre `p4 (20,15)`. Analytic goldens asserted within
  `MEASURE_TOLERANCE` (1e-6): distance `p0->p1` = 40, distance `p0->p2` = 50
  (the 3-4-5 diagonal), angle `l0-l1` = 90 deg, radius `c0` = 10. An
  unsupported pair (point + line) is refused with a named error listing the
  supported pairs - never a fabricated number.
- **The annotation appears in the snapshot.** The real merged-binary frame
  under xvfb (below) shows the measure card with the full annotation line
  `distance point p0 -> point p1 = 40.000 mm` painted on the viewport, and the
  kittest harness asserts the same annotation is present in the AccessKit tree
  (the rendered label node).

## Files (per the card's owned list)

- `crates/oep-app/src/panels/measure.rs` (new)
- `crates/oep-app/src/panels/viewport.rs` (wires `measure::poll` + `measure::draw`)
- `crates/oep-app/tests/measure.rs` (new)

## Acceptance (real command output in transcripts/)

| Check | Result |
|---|---|
| `cargo test -p oep-app --test measure` | 106 passed / 0 failed (transcripts/03) |
| `cargo test -p oep-app` | green across all targets including the new measure target (transcripts/04) |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean (transcripts/02) |
| `cargo fmt --all -- --check` | clean (transcripts/01) |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 (transcripts/05) |

## Snapshot

`measure.png` was captured from the real merged-head binary under xvfb via the
XTEST injector (`measure_inject.c`): press `M`, click point p0, click point
p1. Two independent launches produce byte-identical PNGs (determinism
oracle); sha256 in `measure.png.sha256`.

It shows the viewport with the measure card at the bottom-left: the fixture
wireframe + circle in the preview box, the nine entity pick rows, and the
full annotation line `distance point p0 -> point p1 = 40.000 mm`.
