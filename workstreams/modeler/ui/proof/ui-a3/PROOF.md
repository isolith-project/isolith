# UI-A3 UI-THEME — proof

Card: t_d34078c7. Behaviour: semantic tokens in `crates/oep-ui-theme`, light
default, app installs them as the egui style/visuals at startup, and a guard
test fails on a raw hex colour literal anywhere in `crates/oep-app/src` except
`theme.rs`.

Wave: A of the editor parity program (plan section 5). Wave branch `wave/ui-a`.
Your authoring branch: `ui/a3-theme` (worktree ui-a3).

## Files shipped

- `crates/oep-ui-theme/Cargo.toml`
- `crates/oep-ui-theme/src/lib.rs`
- `crates/oep-ui-theme/src/tokens.rs`
- `crates/oep-ui-theme/tests/contrast.rs`
- `Cargo.toml` (workspace members: one line added)
- `crates/oep-app/src/theme.rs` (token → egui `Visuals` mapping + raw-hex guard)
- `workstreams/modeler/ui/README.md` (row added)

## Files touched outside the card list (with reason)

The card's DETAIL requires "the app installs the tokens as an egui
style/visuals at startup" and acceptance requires `cargo test -p oep-app` to
include the raw-hex guard test. The guard test lives in `crates/oep-app/src/
theme.rs`; for it to run under that package and for the app to install the
style, `theme.rs` must be a reachable module of the `oep-app` binary. That
needs exactly three mechanical lines outside the literal file list, mirroring
the UI-A1 deps card's documented out-of-list edits:

- `crates/oep-app/src/main.rs`: `mod theme;` + call
  `theme::install(&cc.egui_ctx, oep_ui_theme::Mode::default())` in the
  `run_native` creation closure. (The `egui::Context` from `CreationContext`
  is the only startup hook; `App::new` has no ctx.)
- `crates/oep-app/Cargo.toml`: add `oep-ui-theme = { path = "../oep-ui-theme" }`
  so `theme.rs` can `use oep_ui_theme`. No new external dependency; the crate
  is a workspace member.
- `Cargo.lock`: gains the `oep-ui-theme [[package]]` entry for the new member
  (no new registry deps — egui already locked by UI-A1). A new workspace
  member updates the workspace lock by construction.

No other file, and no other crate, was touched.

## Acceptance (real command output)

Below are the raw transcripts; each was run from the worktree root with the
tree in the committed state.

### 1. `cargo test -p oep-ui-theme`

```
$ cargo test -p oep-ui-theme
   Compiling oep-ui-theme v0.1.0 (.../ui-a3/crates/oep-ui-theme)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.71s
     Running unittests (target/debug/deps/oep_ui_theme-*)

running 4 tests
test tokens::tests::default_mode_is_light ... ok
test tokens::tests::every_token_differs_between_modes ... ok
test tokens::tests::palette_resolves_all_tokens ... ok
test tokens::tests::token_resolves_per_mode ... ok

test result: ok. 4 passed; 0 failed

     Running tests/contrast.rs (target/debug/deps/contrast-*)

running 4 tests
test non_text_contrast_dark ... ok
test non_text_contrast_light ... ok
test text_contrast_dark ... ok
test text_contrast_light ... ok

test result: ok. 4 passed; 0 failed
```

Exit code 0. Contrast asserted at WCAG 4.5:1 (text_primary, text_muted on
both surfaces) and 3:1 (accent, border, focus_ring, danger on both surfaces),
in both light and dark.

### 2. `cargo test -p oep-app` (includes raw-hex guard)

```
$ cargo test -p oep-app
   Compiling oep-app v0.1.0 (.../ui-a3/crates/oep-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.29s
     Running unittests (target/debug/deps/isolith-*)

running 2 tests
test theme::guard::no_raw_hex_or_direct_colour_construction_in_crates_oep_app_src ... ok
test theme::guard::the_guard_tools_themselves_detect_literals ... ok

test result: ok. 2 passed; 0 failed
```

Exit code 0.

### 2b. The guard fails on a deliberate violation (transcript in proof)

A constant `const _VIOLATION: u32 = 0x1F6FEB;` was injected into
`crates/oep-app/src/app.rs` (outside `theme.rs`), then the guard was run:

```
$ cargo test -p oep-app theme::guard
running 2 tests
test theme::guard::the_guard_tools_themselves_detect_literals ... ok
test theme::guard::no_raw_hex_or_direct_colour_construction_in_crates_oep_app_src ... FAILED

failures:
---- theme::guard::no_raw_hex_or_direct_colour_construction_in_crates_oep_app_src stdout ----
thread '...' panicked at crates/oep-app/src/theme.rs:141:9:
raw colour literals outside crates/oep-app/src/theme.rs:
  <worktree>/crates/oep-app/src/app.rs:15: hex literal 0x1F6FEB

test result: FAILED. 1 passed; 1 failed
```

The violation was reverted; the tree is clean and the guard passes again
(shown in transcript 2 above post-revert).

### 3. `xvfb-run -a cargo run -p oep-app -- --smoke`

```
$ xvfb-run -a cargo run -p oep-app -- --smoke
   Compiling oep-app v0.1.0 (.../ui-a3/crates/oep-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.39s
     Running `target/debug/isolith --smoke`
libEGL warning: DRI3 error: Could not get DRI3 device
libEGL warning: Ensure your X server supports DRI3 to get accelerated rendering
frames=8 smoke ok
```

Exit code 0, with the theme installed on the egui context at startup. The
only output beyond the smoke line is the software-rendering DRI3 warnings
expected on the Xvfb/virgl path; no colour, style or crash regressions.

## Worktree state

STEP 0 merged `origin/wave/ui-a` (remote `github`, primary per UI-A1's
handoff): brought the UI-A1 seam (`crates/oep-app`, eframe/egui 0.37) into the
tree. Merge commit `5f8c35e` (merge of `github/wave/ui-a` at `7a3e47a`).

Nothing from UI-A2 was on the wave head at STEP 0 (it is a concurrent card),
so the harness test was not present — expected, and not this card's file.
