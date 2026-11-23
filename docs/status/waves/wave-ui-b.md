# Wave ui-b status record

Wave B of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`, section 5): sketch tools and the input contract. Branch `wave/ui-b` cut from main at `736a0a7`.

Written by card UI-B9 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-B1 | UI-INPUT-ROUTER: one event map to command tokens | done |
| UI-B2 | UI-HISTORY: journal undo/redo with drag coalescing | done |
| UI-B3 | UI-SKETCH-ENTER: pick a datum plane and enter sketch | done |
| UI-B4 | UI-SKETCH-DRAW: point, line, polyline, rectangle, circle | done |
| UI-B5 | UI-SNAP-INFERENCE: endpoint, midpoint, centre, on-edge, intersection, h/v | done |
| UI-B6 | UI-CONSTRAINTS: apply and remove constraints | done |
| UI-B7 | UI-DIMENSIONS: place, drive and edit dimensions | done |
| UI-B8 | UI-FEATURE-FROM-SKETCH: extrude/revolve dialog with preview | done |
| UI-B10 | UI-GATE-FIX: formatting and intentional launch-frame update | done |
| UI-B9 | UI-WAVE-B-INTEGRATE: merge, gate, checkpoint, register rows | done |

Every wave-B card branch is an ancestor of the wave head (audit in `proof/ui-b9/transcripts/00-ancestors.txt`), and every card's proof dir is present on the wave head.

## Head sha

- Wave head: `98a8ccc` (`merge-UI-B10`, the gate fix; contains all wave-B cards UI-B1..UI-B8 plus UI-B10).
- Checkpoint merge onto main: one no-ff commit, `checkpoint(wave-ui-b): sketch tools -- cards UI-B1..UI-B8`, parents `38fae0c` (main at wave close; main had moved from the wave-B fork base `736a0a7` to `38fae0c` via PR #29, the name-reservations closeout) + the wave head. The UI-B9 record (this file, the README row, `proof/ui-b9/`) lands on the wave branch and is carried into main by that single checkpoint commit.
- Annotated tag `wave-ui-b` pushed on github/main.

## Gate history (UI-B9, both attempts)

Attempt 1 of UI-B9 (2026-11-21) found the full gate RED on the then-merged head `8da7347` from two pre-existing wave defects in files UI-B9 is barred from touching: (1) a trailing blank at `crates/oep-app/src/input.rs` EOF from UI-B1 (`cargo fmt --all -- --check` exit 1) and (2) the UI-A2 default launch-frame golden no longer matching the merged-head frame since UI-B7 introduced the dimension editor (`bash scripts/ui/shot.sh` exit 1). The card blocked for a decision instead of editing files outside its owned set.

Resolution: UI-B10 `t_7bd53d02` (parent-linked to UI-B9) landed on the wave owning exactly those two corrections: the `input.rs` trailing blank removed and the UI-A2 golden refreshed to the intentional wave-B frame (`2f20383c1ae851675499c96e43980ebc1b6dbb45235bb64b94cce55a400c1047`), preserving the superseded golden as `ui-a2-golden-before-ui-b9.png`. With UI-B10 merged, the full gate is green on the merged head.

## Full gate on the merged head

Transcripts in `workstreams/modeler/ui/proof/ui-b9/transcripts/`, run on the merged head after UI-B10 landed:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - aggregate 2846 passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps allow-listed |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `2f20383c…1047` byte-identical to the UI-B10-refreshed golden (no drift) |

The shot.sh drift guard passing against the refreshed golden is the wave-B requirement: a drift on the merged head would have been a finding, not a hash to overwrite. There was none.

## Practitioner rows moved to building

Per the plan (Wave B: "the four practitioner rows move to `building` with the new test as evidence"), four rows of the practitioner input contract in `docs/scope-register.json` now read `building`, each cited to its green wave-B test file:

| Row | Evidence (test) |
|---|---|
| input-snap-inference | `crates/oep-app/tests/snap_inference.rs` (UI-B5) |
| input-escape-semantics | `crates/oep-app/tests/sketch_draw.rs` (escape ladder) + `sketch_enter.rs` (UI-B4/UI-B3) |
| input-right-click-wheel | `crates/oep-app/tests/constraints.rs` (context menu applies constraints, UI-B6) |
| input-datum-plane-legibility | `crates/oep-app/tests/sketch_enter.rs` (UI-B3) |

`python/oep_tooling/scope_register_check.py --register docs/scope-register.json --markdown docs/scope-register.md` is green (23/23, exit 0); transcript `08-scope-register.txt`.

## Screenshots (acceptance #7)

Three screenshots of the real merged-head binary (captured under `xvfb-run` via the XTEST injector `ui_b9_inject.c` through `capture_ui.sh`, not the default frame; two independent launches byte-identical per shot):

| Shot | sha256 | Shows |
|---|---|---|
| draw | `91712ad1b9b53fb40b98d234e8b1034e1a1eb1f0d920a02885e141d2e5273a58` | XY sketch plane, Rectangle tool: three corners placed, rubber band to the cursor, live `W x H` readout |
| snap | `5042f159ab9ee3d26e7c89dc9c43abd994df5c57abb02e665094a5ecab736346` | Line drawn, pointer hovering near the endpoint: square snap glyph + `Endpoint` label |
| dimension | `5bc2c55c1a59c46de479fd61c597cf01061b6662118e1e871fbce01448de01e2` | Properties dimension editor: `width*1.5` expression field, Apply, driven volume readout |

## What is not done

- Wave B covers the sketch input contract up to a finished extruded/revolved feature from the sketch. Assembly, drafting, PDM and GPU viewport work are later waves (E, F, G, D) per the plan.
- The `wave/ui-b` branch is left in place (not deleted) after the checkpoint, so the wave history and record are preserved.
- No UI tests were weakened, skipped or deleted to reach green; the two wave-B gate defects found at the close were cleared by their own fix card UI-B10 on the wave.
