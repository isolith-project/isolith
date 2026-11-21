# UI-A9 UI-SHELL-CONTRAST proof

Card: UI-A9 UI-SHELL-CONTRAST - give the five regions visible separation.
Branch: `ui/a9-contrast`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: in both themes the model tree and the properties panels read as
raised panels instead of labels floating on one flat surface. Every region
carries a hairline border from the BORDER token; the viewport frame sits
inside its border; the palette host strip and the status bar are separated
from the viewport by the same hairline. Light stays default; dark resolves the
same tokens (never hand-inverted).

## What landed

- `crates/oep-app/src/panels/mod.rs` - the single frame treatment and its
  guard test. `RegionKind` names the five regions; `region_frame(kind, mode) ->
  egui::Frame` returns the raised-panel frame (fill = `Palette::surface_raised`,
  resolved per mode; hairline = 1px `Palette::border`). The structural guard
  (`contrast_test`, `#[cfg(test)]`) fails if the treatment is flattened: for
  every region, in both modes, it asserts the frame carries a stroke (width >=
  1.0, colour == the BORDER token) and a fill (== SURFACE_RAISED, != the window
  SURFACE). It asserts on the `egui::Frame` the shell actually paints - never
  raw pixels.
- `crates/oep-app/src/app.rs` - `region()` now paints each region's frame
  around its child Ui: the raised fill behind the content and the 1px hairline
  on top (so the viewport rasterizer's frame stays inside the border). Pure
  painting: `max_rect` and the AccessKit bounds are unchanged, so the layout
  test's geometry assertions are untouched. Mode is read from the live egui
  visuals (`visuals.dark_mode`), so whatever theme main.rs installs is the mode
  the frames resolve - dark included.

### Why the helper lives in `panels/mod.rs`, not `theme.rs`

The card lets "a helper that returns the egui::Frame for a region" live where
the next worker can find it, and requires the raw-hex / `Color32::from_*` guard
test to stay green. `theme.rs` is the sanctioned token->visuals seam in the
binary, but the integration test crates (`layout`, `harness`, `viewport`,
`palette`, `doc_io`) include `app.rs` (and via it `panels`) by `#[path]` and do
NOT compile `theme.rs` (only main.rs declares it). A helper in `theme.rs`
referenced from `app.rs` would not compile under any of those test targets.

The frame is therefore built in `panels/mod.rs`, but only from tokens already
resolved by `oep_ui_theme::Palette` - `surface_raised` and `border`. It contains
no raw hex literal and no `Color32::from_*`, so the UI-A3 guard test needs no
scan extension and stays green (proven in transcript 02: both guard tests ok in
the bin unit-test target). No new token was needed: SURFACE_RAISED and BORDER
already have both light and dark values, and the contrast suite in
`oep-ui-theme` is untouched and green.

## Files owned and touched

- `crates/oep-app/src/app.rs` (owned) - region() paints fill + hairline.
- `crates/oep-app/src/panels/mod.rs` (owned) - RegionKind, region_frame, the
  flatten guard test.
- `crates/oep-app/src/panels/{tree,viewport,properties,status,palette,doc}.rs`
  (owned) - untouched; region geometry/behaviour all unchanged.
- `crates/oep-app/src/theme.rs` (owned) - untouched; guard green as-is.
- `crates/oep-ui-theme/src/tokens.rs` (owned, conditional) - untouched; no
  missing token.
- `workstreams/modeler/ui/proof/ui-a9/` (owned) - this proof.
- `workstreams/modeler/ui/README.md` (owned) - UI-A9 row.
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - the shot.sh
  golden. UI-A9's whole behaviour changes the shell (regions now carry fill +
  hairline), so per the card's acceptance the old golden is preserved beside
  this proof (`ui-a2-golden-before-contrast.png`) and the new frame recorded as
  the golden, as A5/A6/A7 did before.
- `crates/oep-app/tests/*` - untouched, all green unmodified (layout.rs
  included, per the no-geometry-change rule).

## Acceptance

All commands ran on the merged head (STEP 0 merged `github/wave/ui-a` @
`29614e5`, UI-A7 in the tree). Transcripts are in `transcripts/`.

- `cargo test -p oep-app` - exit 0. Bin unit tests 6/6 (incl. the new
  `contrast_test` + both theme guard tests), doc_io 10, harness 7, layout 8,
  palette 9, viewport 7. The flatten guard runs in every one of these targets
  (each includes `panels/mod.rs` via #[path]), so it is load-bearing across
  every test binary, exactly as a structural guard should be. (transcript 02)
- `cargo test -p oep-ui-theme` - exit 0; token + contrast suites unchanged and
  green (4 + 4). (transcript 03)
- `cargo clippy -p oep-app -p oep-ui-theme --all-targets -- -D warnings` - exit
  0. (transcript 05)
- `xvfb-run -a cargo run -p oep-app -- --smoke` - exit 0, frames=8 smoke ok.
  (transcript 04)
- Golden refresh: `rm frame.png.sha256` then `bash scripts/ui/shot.sh` writes
  the new golden; the second `shot.sh` is byte-identical. (transcripts 06, 07)

## Old vs new golden

| frame | sha256 |
|---|---|
| old (pre-contrast, preserved) | `2695a608982b2eaa797a259f7c5796b2714f5021e903659eec61041255bcb0dc` |
| new (this card, recorded as golden) | `e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4` |

## Pixel evidence of the treatment

The shot pixel scan (reproducible from `frame.png`) shows the five regions
reading as raised panels separated by hairlines in the light default:

- Model tree fill is `255,255,255` (SURFACE_RAISED) - the old golden was
  `246,247,249` (SURFACE). Same for properties, palette host and status bar.
- Hairline seams (BORDER = `130,139,150`, 1px) at every region boundary:
  tree|viewport at x=240, viewport|properties at x=964, palette|viewport at
  y=32, viewport|status at y=758, and the shell's outer frame (x=1264 right,
  y=784 bottom).
- Dark mode uses the same tokens resolved per mode (`region_frame` resolves
  `Palette::resolve(Mode::Dark)`); the guard test asserts both modes carry a
  distinct raised fill + hairline, so dark is deliberate, never an inversion.
