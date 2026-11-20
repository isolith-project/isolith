# UI-A1 proof — UI-DEPS-BASELINE (eframe/egui 0.37, crates/oep-app)

Real command output for every acceptance item is in this directory (files
`00`-`07`), each with the process exit code. This sheet records the context a
pure log dump does not.

## Acceptance mapping

| Acceptance | Proof file | Result |
|---|---|---|
| `cargo build -p oep-app` | `02-build.txt` | Finished dev profile, `build_exit=0` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `03-smoke.txt` | `frames=8 smoke ok`, `smoke_exit=0`, ~6 s wall |
| `cargo tree -p oep-app | grep -Ei 'gpl|agpl|sspl'` | `04-tree-gpl.txt` | no output, `grep_exit=1` (no match) |
| `python3 scripts/licensing-check.py` | `05-licensing.txt` | `total=6 pass=6 fail=0`, `licensing_exit=0` |
| `git diff --stat NOTICE` | `06-notice-diffstat.txt` | NOTICE +355 lines (regenerated) |
| `cargo test --workspace` | `07-test-workspace.txt` | 194 `test result: ok` units, `workspace_test_exit=0` |

## STEP 0 (always first)

`git fetch github` + `git merge github/wave/ui-a` (`01-step0.txt`). The wave
branch `wave/ui-a` is at `0ccef03`, identical to this card's base, so the merge
brought nothing. (The card names the remote "origin"; on this repo the wave
remote is `github`, the upstream source of truth.)

## Dependency facts

- `eframe` and `egui` resolved and locked at **0.37.0** (Cargo.lock), the only
  dependencies this card declares in `crates/oep-app/Cargo.toml` (via
  `[workspace.dependencies]` in the root Cargo.toml).
- Active renderer is the default **glow** backend (`NativeOptions` defaults are
  untouched). `wgpu` appears in the compile log and the lockfile only as a
  transitive dependency of `eframe`/`egui-wgpu` 0.37 default features; this card
  adds no `wgpu` entry to any manifest (that is wave D's subject).
- `--smoke` runs the real winit event loop; the frame budget (8) is counted in
  the per-frame `eframe::App::ui` callback with no sleep-based timing, and
  `egui::Context::request_repaint` is used in smoke mode so the idle loop issues
  real frames instead of waiting for input.

## License-boundary decisions this card records

1. **Font licenses added to the allow-list.** egui embeds its built-in fonts via
   `epaint_default_fonts@0.37.0`, SPDX `(MIT OR Apache-2.0) AND OFL-1.1 AND
   Ubuntu-font-1.0`. `OFL-1.1` (SIL Open Font License) and `Ubuntu-font-1.0`
   permissive font-data licenses, not copyleft, and the card's bar is no
   GPL/AGPL/SSPL (which `cargo tree` confirmed). They were added to the
   pure-permissive allow-list in `scripts/licensing-check.py` and documented in
   `docs/licensing-boundary.md`. This card is the only one adding the UI stack,
   so the boundary extension lands here, not later.
2. **Regenerated dependency manifest + NOTICE.** `scripts/licensing-check.py
   --regen` rebuilt `docs/dependency-licenses.json` (531 deps) and `NOTICE`
   (+355 lines) from the new lock, per D-UI-10 ("keep BOTH sides" policy of
   supply-chain: pin, lock, regenerate). The licensing check's own proof,
   `docs/proof/licensing-check.json`, is the machine record of the green run.

No GPL/AGPL/SSPL identifier is in the resolved tree (verified by
`04-tree-gpl.txt` and the allow-listed manifest).
