# UI-A4 UI-LAYOUT proof

Card: UI-A4 UI-LAYOUT - three-panel shell, status bar, and the panel seam.
Branch: `ui/a4-layout`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: the main window is a stable shell of five named regions with
persisted sizes, and the palette and document panels exist as compiling stubs
so UI-A5, UI-A6 and UI-A7 each own one file later without editing `app.rs`.

## What landed

- `crates/oep-app/src/config.rs` - `ShellLayout` (palette height, model tree
  width, properties width, status bar height) with a tiny key=value persisted
  format (no serde or new dependency), lossless `as_text`/`from_text`
  round trip, `save_to`/`load_from`, and a pure `regions(window) -> ShellRegions`
  partition that is clamped so the five rects always tile the window without
  overlap or clipping at any size, in particular 1280x800 and 1920x1080.
- `crates/oep-app/src/panels/mod.rs` plus `tree.rs`, `viewport.rs`,
  `properties.rs`, `status.rs` - the four rendering panels, each with a header
  and stub content; `viewport.rs` is the seam UI-A5 fills.
- `crates/oep-app/src/panels/palette.rs` and `doc.rs` - compiling stubs
  exporting `pub fn show(ctx: &egui::Context, app: &mut App)` with documented
  TODOs naming UI-A6 and UI-A7; `app.rs` calls both every frame, so those
  cards own one file each with no `app.rs` edit.
- `crates/oep-app/src/app.rs` - draws the five regions inside the single
  `CentralPanel` from `config::ShellLayout::regions`. Each native egui panel
  registers as an AccessKit `Pane`, and UI-A2's harness asserts exactly one
  Pane, so the five regions are labelled child Uis with an explicit
  GenericContainer role and explicit bounds (`get_all_by_label` hits them and
  `bounding_box()` returns the computed rect at any window size). The App
  loads persisted sizes at startup and persists them on exit (only when a
  config dir exists, so tests never scatter files).
- `crates/oep-app/tests/layout.rs` - the `--test layout` target: five regions
  reachable by accessible name at both resolutions, region geometry read from
  the AccessKit tree tiles the central pane with no overlap/clipping, the
  documented D-UI-5 spatial positions hold, and a persisted non-default layout
  comes back identical after save/load and is restored by the app on startup
  (ISOLITH_CONFIG_DIR pointed at a throwaway dir).
- `workstreams/modeler/ui/proof/ui-a4/` - this proof, transcripts, and the
  shell frame + sha256.
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - the A2 shot.sh
  golden refreshed to the new shell frame (see below).

## Files touched outside the card list (with reason)

The card's acceptance for the screenshot leg is the literal command
`bash scripts/ui/shot.sh`. That script (UI-A2's, not owned by this card) is
hardcoded to write its golden reference under `proof/ui-a2/` and to fail loudly
on any drift. This card's whole behaviour is to change the shell frame, so the
drift guard fires exactly as designed - it refuses to *silently* rewrite the
golden - and the correct wave action is to record the new shell frame as the
new golden, which is precisely what the "# shell frame, hash recorded"
acceptance line means. Two files under `proof/ui-a2/` therefore move with the
shell:

- `proof/ui-a2/frame.png` - now the UI-A4 shell frame (53 KB, previously the
  A2 empty-shell 36 KB).
- `proof/ui-a2/frame.png.sha256` - now `49749550…9a9d` (previously
  `e20b6d5d…0da1`). The old golden is preserved beside this proof as
  `ui-a2-golden-before-layout.png`.

The already-existing `egui_kittest` dev-dependency supports the new test
target; no Cargo manifest, no dependency and no `main.rs` change was needed.
`crates/oep-app/src/panels/` and `tests/layout.rs` are new files under the
crate this card owns.

## Acceptance (real command output)

| Step | Command | Exit | File |
|---|---|---|---|
| step0 | `git fetch github && git merge github/wave/ui-a` | 0 (already up to date at 8675207) | `transcripts/01-step0.txt` |
| layout | `cargo test -p oep-app --test layout` | 0 (7 passed) | `transcripts/02-layout-test.txt` |
| smoke | `xvfb-run -a cargo run -p oep-app -- --smoke` | 0 (frames=8 smoke ok) | `transcripts/03-smoke.txt` |
| shot drill | `bash scripts/ui/shot.sh` before golden refresh | 1 (drift guard, intended) | `transcripts/04-shot-golden-drift.txt` |
| shot run 1 | `bash scripts/ui/shot.sh` after refresh | 0 | `transcripts/05-shot-green-run1.txt` |
| shot run 2 | `bash scripts/ui/shot.sh` | 0, identical sha256 | `transcripts/06-shot-green-run2.txt` |
| gate | fmt, clippy `-D warnings`, oep-app + theme tests | 0 | `transcripts/07-gate.txt` |

### 1. `cargo test -p oep-app --test layout` (exit 0)

```
running 7 tests
test config::tests::missing_or_malformed_config_falls_back_safely ... ok
test config::tests::regions_partition_window_without_overlap_at_acceptance_sizes ... ok
test config::tests::shell_layout_round_trips_same_sizes ... ok
test persisted_layout_round_trips_and_app_restores_same_sizes ... ok
test regions_are_in_the_documented_spatial_positions ... ok
test region_geometry_tiles_window_without_overlap_or_clipping ... ok
test five_regions_reachable_by_accessible_name_at_both_resolutions ... ok

test result: ok. 7 passed; 0 failed
```

Five regions by accessible name (model tree, viewport, properties, status bar,
command palette host) at both 1280x800 and 1920x1080; geometry asserts the
regions tile the central pane without overlap or clipping at both sizes; the
config round-trip test asserts the same sizes come back.

### 2. `xvfb-run -a cargo run -p oep-app -- --smoke` (exit 0)

```
frames=8 smoke ok
```

The shell (including the `doc::show`/`palette::show` seam calls) renders for
the smoke budget and exits cleanly.

### 3. `bash scripts/ui/shot.sh` - shell frame, hash recorded

The drift proof (transcript 04) shows the A2 golden was still the empty shell
frame and the guard fired loudly on the new shell - expected, this is the
card's behaviour. The golden was then refreshed to the shell frame and two
independent runs produced byte-identical PNGs at the same sha256:

```
shot.sh: ok
  frame    : workstreams/modeler/ui/proof/ui-a2/frame.png
  sha256   : 49749550d281182113d8dc011193c50148bc136cff270f99fbe30591c9756a9d
```

The shell frame lives at `proof/ui-a4/frame.png` (+ `.sha256`, verifiable with
`sha256sum`), and the same file is the refreshed golden under `proof/ui-a2/`.

## Image provenance

`frame.png` is the deterministic frame of the real `isolith` binary on a
1280x800 Xvfb screen after the UI-A4 shell landed, captured twice with
byte-identical hashes:

```
$ sha256sum workstreams/modeler/ui/proof/ui-a4/frame.png
49749550d281182113d8dc011193c50148bc136cff270f99fbe30591c9756a9d  workstreams/modeler/ui/proof/ui-a4/frame.png
```

## Notes for the review of this card

- The single-Pane AccessKit constraint from UI-A2's `tests/harness.rs` is what
  shaped the implementation: the five regions are labelled child Uis with
  explicit bounds, not native egui panels (each native panel would be a Pane).
  The A2 harness test still passes (3 harness tests + 3 config unit tests =
  6 in that target, transcript 07).
- No dependency was added; the persisted config format is hand-rolled
  key=value (lossless float round trip via Rust's shortest repr), so the
  dependency surface stays exactly what the wave assigned.
- `cargo fmt --check`, `clippy -D warnings` (oep-app all targets) and the
  oep-app + oep-ui-theme test suites are green on the merged head.
- Pre-existing wave issue carried in, not introduced here: `oep-ui-theme` is
  still missing from `docs/licensing-boundary.*`, so `scripts/licensing-check.py`
  reports the crate-boundary FAIL UI-A2 already flagged for the INTEGRATE card.
