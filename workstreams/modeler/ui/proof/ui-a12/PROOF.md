# UI-A12 UI-HEADING-INK proof

Card: UI-A12 UI-HEADING-INK - strong text is painted in the accent ink and
disappears on light panels.
Branch: `ui/a12-heading-ink`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: every heading and every emphasised label in the shell reads in both
themes. "Command Palette Host", "Model Tree", "Properties", "Status Bar", the
palette title, the palette row labels and the tree's open-file line are all
visibly dark on light surfaces and visibly light on dark surfaces.

## The bug and the fix

Wave-A gate frames showed the headings as pure white: pixel analysis of the
UI-A2 golden (`1798df38…`) showed the tree panel's heading band is white and
the separator under it is drawn, i.e. the glyphs were painted in the panel's
own fill colour. Root cause, verified against the vendored egui 0.37 source
(`egui-0.37.0/src/widget_text.rs::get_text_color` and
`src/style.rs::strong_text_color`):

- `RichText::strong()` resolves its colour from
  `Visuals::strong_text_color()`, which is `widgets.active.text_color()`, i.e.
  `widgets.active.fg_stroke.color` - the accent ink.
- `theme.rs` sets `active: widget(p.accent, p.focus_ring, active_ink)` where
  `active_ink = best_on(p.accent, …)` correctly picks `surface_raised` (white)
  for the light accent. White-on-accent is right for text on an accent-filled
  button, but every other `.strong()` heading then painted white-on-white and
  vanished. `Visuals::text_color()`'s `override_text_color` path (text_primary)
  does not apply to `strong()`.

Fix, entirely inside `crates/oep-app/src/theme.rs` plus call sites:

- `theme::heading(mode, text)` - returns a strong `RichText` whose colour is
  explicitly the `TEXT_PRIMARY` token for `mode`, never `widgets.active`. egui
  resolves an explicit `color()` ahead of the `strong()` ink (first branch of
  `get_text_color`), so the emphasis flag can no longer repaint headings.
- `theme::mode_of(ui)` - the installed `Mode`, derived from the egui style's
  `dark_mode` exactly as `app.rs`' region painter already does (one shared
  source of truth instead of a mode threaded through every panel).
- `theme::on_accent(mode, text)` - the one emphasised label genuinely painted
  on the accent fill: the selected command-palette row. It needs the
  best-on-accent ink, not `TEXT_PRIMARY` (the original `.strong()` produced
  exactly this ink there via `strong_text_color()`), so a second, explicitly
  named helper carries it and keeps white-on-blue readable.
- The eight `.strong()` call sites (app.rs palette-host heading, tree heading +
  open-file line, properties heading, status-bar heading, doc recent-overlay
  title, palette title + selected row) now go through the helpers. The selected
  palette row is the only `on_accent` usage; the other seven use `heading`.
  `widgets.active`'s ink is untouched (white on the blue accent button is
  correct; the contrast suites cover it).
- Guard: `theme::guard::no_strong_text_outside_theme_rs` fails the whole crate
  if `.strong()` appears anywhere in `crates/oep-app/src` outside `theme.rs`,
  and it is proven to fire (probe planted, failure captured, reverted).

Contrast restored (WCAG, computed from the tokens): `TEXT_PRIMARY` on
`SURFACE_RAISED` is 16.56:1 in light and 11.10:1 in dark; the light-mode accent
ink (`SURFACE_RAISED` white) is 4.63:1 on the light accent and stays white on
the accent button.

## Files owned vs touched

Card-owned files touched: `theme.rs`, `app.rs`, `panels/tree.rs`,
`panels/properties.rs`, `panels/status.rs`, `panels/palette.rs`,
`panels/doc.rs`, `tests/heading_ink.rs` (new), `README.md` (row added),
`workstreams/modeler/ui/proof/ui-a12/` (this proof),
`workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` (deliberate golden
refresh).

Out-of-list files, with reason: the six pre-existing test targets that pull
`app.rs` in verbatim via `#[path]` - `tests/harness.rs`, `tests/layout.rs`,
`tests/palette.rs`, `tests/doc_io.rs`, `tests/tree_empty_state.rs`,
`tests/viewport.rs`. The card requires app.rs to produce its headings through
`theme::heading`, which makes `app.rs` reference `crate::theme`; every test
crate that compiles `app.rs` therefore needs the `theme` module declared, the
same mechanical line these files already carry for `config` (`#[path]` +
`#[allow(dead_code)]`). No other line in those files changed; no test logic
changed. In exchange the token guards (raw-hex, no-strong) now run in every
oep-app test target, exactly like UI-A9's contrast guard already did.

## Acceptance (all real output under `transcripts/`)

- `cargo test -p oep-app --test heading_ink` -> 9 passed (token assertions in
  light and dark, strong-ink proof, on_accent/heading distinction, source
  "produced through the helper" checks, plus the theme guard tests). [03]
- `cargo test -p oep-app` -> all 8 targets green: bin 7, doc_io 13, harness 10,
  heading_ink 9, layout 11, palette 12, tree_empty_state 11, viewport 10. [04]
- `cargo test -p oep-ui-theme` -> token + contrast suites unchanged: 4 + 4. [05]
- `cargo fmt -p oep-app -p oep-ui-theme --check` exit 0; `cargo clippy -p
  oep-app -p oep-ui-theme --all-targets -- -D warnings` exit 0. [06]
- `xvfb-run -a cargo run -p oep-app -- --smoke` -> `frames=8 smoke ok`, exit 0.
  [07]
- Deliberate golden refresh (headings become visible, so the frame changes):
  old golden preserved as `ui-a2-golden-before-a12.png`; new golden recorded;
  two independent `bash scripts/ui/shot.sh` runs byte-identical. [08, 09]
- Guard-fires proof: a bare `.strong()` planted in a scratch copy under
  `crates/oep-app/src/` makes `no_strong_text_outside_theme_rs` fail citing
  the file; after the probe is removed the guard is green again. [01, 02]
- Pixel before/after on the golden frames: every emphasised heading band goes
  from 0 dark glyph pixels to visible glyphs. [10]

### Old and new golden hashes, side by side

```
old (pre-A12): 1798df384f4bf05fb76df0d2caf96c140e6d1098e52a3e1247120d37533673fe
new (A12)    : 59ae82bf991d455a36abd368d345e4e6c92854d856bdc8181614a757579d989f
```

`frame.png` in this proof dir === `proof/ui-a2/frame.png` (sha256
`59ae82bf…9d989f`), the same golden shot.sh records.

### Before / after: which heading was invisible

Before the fix every strong heading was painted in the panel's own fill
colour (`SURFACE_RAISED` white via the `widgets.active` ink) and vanished on
the light shell. **Naming the card's headline example: the "Model Tree"
heading was invisible** - its glyphs were pure white on the white panel, the
band showed only the separator under where the text should be. The same
invisibility hit "Command Palette Host", "Properties", "Status Bar", the tree's
open-file line (`edpart.step *`), the palette title and the selected palette
row over the closed palette, and the recent-documents overlay title.

Dark glyph pixels in the golden frames (threshold `max(R,G,B) < 0x60`; the
light-mode `TEXT_PRIMARY` ink is `0x1B-0x24`, so headings count):

```
region                                        before_fix  after_fix
Model Tree heading (0,32)-(240,60)                     0         90
Properties heading (980,32)-(1280,60)                  0         68
Command Palette Host strip (0,0)-(320,32)              0        169
Status Bar heading glyphs (4,761)-(64,774)             0         73
```

After the fix all four band shapes shown above carry dark-on-light glyphs (the
shell is captured in the standing light default), and the heading tests assert
the same invariant in both modes from the `TEXT_PRIMARY` token value.
