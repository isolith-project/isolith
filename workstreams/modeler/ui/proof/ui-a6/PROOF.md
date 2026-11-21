# UI-A6 UI-COMMAND-PALETTE proof

Card: UI-A6 UI-COMMAND-PALETTE - searchable command surface.
Branch: `ui/a6-palette`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: Ctrl+K opens a palette over the shell's existing command registry;
typing filters live; Enter runs the selected command through the same router
the CLI uses; every row shows the command label plus its shortcut or its
palette path; unknown input shows an honest "no command matches" line.

## What landed

- `crates/oep-app/src/commands.rs` - the app's half of the D-UI-2 one-mutation
  path, included into the crate from `palette.rs` via `#[path]` (so this card
  owns only its granted files, never touching `main.rs`/`app.rs`). It re-exports
  `oep_shell::commands::{commands, search, find_by_token, validate_token,
  CommandSpec}` - the palette's search and routing resolve through the SHARED
  oep-shell registry, never a fork or a copied command list. `dispatch(line)`
  validates the token with `oep_shell::commands::validate_token`, the exact
  function the workbench CLI (`examples/workbench.rs`) calls before dispatching,
  then names the command and the wave that wires its execution lane into the
  app (honest typed `Routed::Registered` with a lane, or `Routed::Unknown` with
  the CLI's own actionable message - never fabricated execution). `row_note`
  returns a command's shortcut when bound, else its palette path (the CLI usage
  line), else an explicit `GAP_NOTE` so a command with neither is visible as a
  gap rather than silently hidden.
- `crates/oep-app/src/panels/palette.rs` - fills the UI-A4 stub. Ctrl+K (Cmd on
  macOS, Ctrl elsewhere) toggles a single top-level `egui::Area` overlay
  (Order::Foreground, never a native panel, so the UI-A2 one-Pane contract and
  UI-A4 five-region geometry hold). Typing filters against
  `oep_shell::commands::search`; rows are `selectable_label`s showing
  `label | shortcut-or-path`; Enter routes through `commands::dispatch` and
  dismisses the palette; Esc closes; arrows move selection; an empty filter
  lists the whole index; an unknown filter renders an honest unselected row
  "no command matches '<q>'; ..." (plain labels carry no AccessKit name in
  egui 0.37, so the no-match line is an unselected row to stay queryable).
  Per-app state lives in the egui context data store - no global - and the
  UI-A6 routing spy (a router-invocation counter) is written there as a plain
  `String` report (`query=`/`open=`/`count=`/`outcome=` lines) so the test
  reads the app's real per-context state even though the source is compiled
  twice under `#[path]` (a `String` TypeId is shared across copies; the app's
  private struct is not). Closed, the host strip right-anchors a one-line
  affordance "Ctrl+K to open the command palette" plus the last-routed
  outcome, which is what the deterministic screenshot records.
- `crates/oep-app/tests/palette.rs` - the `--test palette` target: open/close
  on Ctrl+K/Esc, filter equals `oep_shell::commands::search` exactly (no
  fork), Enter routes through the shared registry and the spy counts router
  invocations through the same context (count +1 per Enter, unchanged on
  no-match/bare Enter - never a shell-out), honest no-match line, whole-index
  listing with `label | path` on every row. Includes `app.rs`/`config.rs`
  only (`app` already pulls the palette through `panels/mod.rs`; standalone
  re-inclusion would trip clippy `duplicate_mod`).
- `workstreams/modeler/ui/proof/ui-a6/` - this proof, transcripts, and the
  palette frame + sha256. The old UI-A4 shell golden is preserved as
  `a4-golden-before-palette.png`.
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - the A2 shot.sh
  golden refreshed to the UI-A6 palette frame (see below).

## Files touched outside the card list (with reason)

- `crates/oep-app/Cargo.toml` - the card's DETAIL mandates the palette search
  comes from `oep_shell::commands::search` and Enter routes through the same
  router the CLI uses; that is impossible without oep-shell on the app's
  dependency list, and "do not add a dependency not named in this card" is
  satisfied (oep-shell is the named dependency). No other manifest change.
- `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` - `shot.sh`
  (UI-A2's script, not owned by this card) is hardcoded to write its golden
  there and fail loudly on drift; this card's whole behaviour is to change the
  shell frame (the palette affordance line), so the drift guard fires as
  designed and recording the new frame as the golden IS the "palette frame,
  hash recorded" acceptance line. Old golden preserved beside this proof.
- `crates/oep-app/tests/palette.rs` - a new test target required by the
  acceptance line `cargo test -p oep-app --test palette`.

## Acceptance (real command output)

| Step | Command | Exit | File |
|---|---|---|---|
| step0 | `git fetch github && git merge github/wave/ui-a` | 0 (already up to date at dd5728f) | `transcripts/01-step0.txt` |
| palette | `cargo test -p oep-app --test palette` | 0 (8 passed) | `transcripts/02-palette-test.txt` |
| smoke | `xvfb-run -a cargo run -p oep-app -- --smoke` | 0 (frames=8 smoke ok) | `transcripts/03-smoke.txt` |
| shot drill | `bash scripts/ui/shot.sh` before golden refresh | 1 (drift guard, intended) | `transcripts/04-shot-golden-drift.txt` |
| shot run 1 | `bash scripts/ui/shot.sh` after refresh | 0 | `transcripts/05-shot-green-run1.txt` |
| shot run 2 | `bash scripts/ui/shot.sh` | 0, identical sha256 | `transcripts/06-shot-green-run2.txt` |
| gate | fmt check, clippy `-D warnings` all targets, `cargo test -p oep-app` | 0 | `transcripts/07-gate.txt` |

### 1. `cargo test -p oep-app --test palette` (exit 0)

```
running 8 tests
test ctrl_k_opens_the_palette_and_escape_closes_it ... ok
test typing_filters_through_the_shared_registry_not_a_fork ... ok
test enter_routes_through_the_shared_router_spy_counts_invocations ... ok
test unknown_input_shows_an_honest_no_match_line_and_does_not_route ... ok
test rows_show_label_and_shortcut_or_path_with_nothing_hidden ... ok
test config::tests::regions_partition_window_without_overlap_at_acceptance_sizes ... ok
test config::tests::missing_or_malformed_config_falls_back_safely ... ok
test config::tests::shell_layout_round_trips_same_sizes ... ok
test result: ok. 8 passed; 0 failed
```

The spy assertion the acceptance asks for, from transcript 02:

```
spy: typing then Enter must invoke the router exactly once
Enter with the palette closed must not invoke the router
Enter with no matching row must not invoke the router
```

Ctrl+K opens, Esc closes, typing filters to exactly `oep_shell::commands::search`
(no fork), Enter routes through `validate_token`/`find` (the CLI's router step)
and the spy counts one invocation per routed Enter. Never a shell-out.

### 2. `xvfb-run -a cargo run -p oep-app -- --smoke` (exit 0)

```
frames=8 smoke ok
```

The shell (palette compiled in, closed-hint line rendered every frame) runs its
smoke budget and exits cleanly.

### 3. `bash scripts/ui/shot.sh` - palette frame, hash recorded

Transcript 04 shows the drift guard firing loudly while the golden was still
the UI-A4 shell frame - expected, this card changed the frame (the top strip
now right-anchors "Ctrl+K to open the command palette"). The golden was then
refreshed and two independent runs produced byte-identical PNGs:

```
shot.sh: ok
  frame    : workstreams/modeler/ui/proof/ui-a2/frame.png
  sha256   : 66d7010e81ac29675fc8db0c12ed287f68961954255e1f301fe78d8d64c7a694
```

The palette frame (closed palette with the Ctrl+K affordance) lives at
`proof/ui-a6/frame.png` (+ `.sha256`) and as the refreshed golden under
`proof/ui-a2/`.

## Image provenance

`frame.png` is the deterministic frame of the real `isolith` binary on a
1280x800 Xvfb screen after the UI-A6 palette landed, captured twice with
byte-identical hashes:

```
$ sha256sum workstreams/modeler/ui/proof/ui-a6/frame.png
66d7010e81ac29675fc8db0c12ed287f68961954255e1f301fe78d8d64c7a694  workstreams/modeler/ui/proof/ui-a6/frame.png
```

Visual check of the frame (loaded and inspected with a vision model): the five
regions render on the light theme - "Command Palette Host" top-left, "Ctrl+K to
open the command palette" top-right, model tree / viewport / properties left-
centre-right with their stub lines, "Status Bar" bottom with the status line.
The palette affordance is visible, which is the "compiled in" proof the gate
wants.

## Notes for the review of this card

- Single mutation path, D-UI-2, done honestly: the app's `commands` seam holds
  no command list and forks nothing - `search`, the registry, `validate_token`
  and `find` all resolve in `oep_shell::commands`, the same functions the CLI
  uses. The `--test palette` cross-checks the rendered rows against
  `oep_shell::commands::search` directly, so a fork could not pass.
- "Enter runs the command through the same router the CLI calls": routed
  through `commands::dispatch`, which validates with the CLI's own
  `validate_token`. In wave A the app holds no editor/workbench session, so the
  honest typed outcome for a registered engine command is `Registered { name,
  lane }` naming the wave that wires the lane (B/E/G/A7 per the plan) - never a
  fabricated model edit. The spy proves the routing, not a fake execution.
- egui 0.37 notes encountered: `egui::Id::new` is deprecated in favour of
  `Id::unique`; plain `ui.label` widgets carry no AccessKit name in this
  version (the no-match line is therefore an unselected row); `request_focus`
  must target the stable TextEdit id; Area fade-in should be disabled for
  deterministic harness settling.
- The UI-A6 spy is written as a `String` report because the test compiles this
  source twice under `#[path]` (once via `app`, once standalone would trip
  clippy `duplicate_mod`, so the test includes only `app`); a std `String`
  TypeId is shared across copies, the private `PaletteState` is not.
- `clippy -p oep-app --all-targets -- -D warnings`, `cargo fmt --check` and the
  full `cargo test -p oep-app` (5 + 6 + 7 + 8) are green on the merged head.
- Pre-existing wave issue carried in, not introduced here: `oep-ui-theme` is
  still missing from `docs/licensing-boundary.*`, so
  `scripts/licensing-check.py` reports a crate-boundary FAIL flagged by UI-A2
  for the INTEGRATE card. Adding oep-shell back onto oep-app's dependency list
  does not change the boundary map (both crates are Apache-2.0 workspace
  members; serde_json was already in the lock and is on the allow-list), and
  licensing was never this card's gate.
