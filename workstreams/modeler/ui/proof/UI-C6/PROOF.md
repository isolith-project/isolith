# UI-C6 UI-KERNEL-FEATURES: fillet, chamfer, shell, boolean dialogs

**Card:** UI-C6 UI-KERNEL-FEATURES (wave C, editor parity program)
**Branch:** `ui/c6-kernel-features`
**Files owned (touched):** `crates/oep-app/src/panels/kernel_features.rs` (new),
`crates/oep-app/tests/kernel_features.rs` (new), plus the `oep-app` root-manifest
edit adding the card-mandated OCC-seam dependency (`oep-geo` path dep + a
`kernel` feature forwarding to `oep-geo/kernel`) and the resulting `Cargo.lock`
update -- the same card-owning precedent UI-B6 set for `oep-sketch`. No other
crate touched.

## Behaviour (the card)

Fillet, chamfer, shell and boolean are DIALOGS over the OCC seam. With the
kernel available the geometry is real; without it the app says the feature is
unavailable instead of drawing fake geometry. Availability is DETECTED, never
assumed; the unavailable path is a tested state with an honest message. With
the kernel, fillet produces the expected face count on the fixture.

## Design summary

`panels/kernel_features.rs` is a self-contained engine + panel, delivered
`#[path]`-includable exactly like UI-C3's `variables.rs` (the wave-C pattern
for panels the owned-files constraint keeps out of the shell; a wiring step on
the wave-C integrate card hosts it). The one seam the dialogs use is
oep-geo's typed kernel module -- the OCC seam that fronts oep-engine-occ (the
OCCT binding) -- reached only through its public typed functions, never an
OCCT identifier.

- **Availability is detected at runtime, once per panel lifetime**
  ([`probe_kernel`]): the panel opens a real `oep_geo` session with
  `sys_new()` and keeps that session plus its committed fixture while it
  exists. With the `kernel` feature off (the default workspace build, no
  OCCT linked) `sys_new` returns `KernelError::EngineUnavailable` and the
  panel's oracle reports `kernel=0` with `reason=` naming the seam's typed
  state; with it on, a real session opens (`kernel=1`) and the dialogs run
  real B-Rep operations.
- **No fake geometry, ever.** Every dialog run reports `geometry=0` /
  `faces=-` / `volume=-` until a real kernel operation measured real numbers;
  the oracle holds the seam's actual values or a dash, never a fabricated
  face count or volume. A missing kernel makes every dialog take the honest
  unavailable path ("kernel engine unavailable ...").
- **Shell is honest in BOTH builds.** The seam has no shell/thicken operation
  today, so the shell dialog reports exactly that ("no shell operation in the
  kernel seam") and produces no geometry even when a kernel session is open.
- **The fixture.** The dialogs operate on the committed corpus
  `box-reference` unit cube (corner (0,0,0)-(1,1,1)), the same footprint the
  oep-geo seam acceptance builds, plus the half-overlapping second unit box
  the seam's boolean goldens use. The fixture's 24 edge occurrences come from
  `solid_edges` (the seam's own answer); fillet/chamfer name edge 0.
- **With the kernel, fillet produces the expected face count.** Running the
  fillet dialog (default radius 0.2) yields 7 faces at the seam golden's
  volume (0.9914159265358977) -- both asserted exactly against the committed
  `crates/oep-geo/tests/golden/kernel-seam-golden.json` numbers, with the
  same "an OCCT bump that moves topology is a reviewed change" contract the
  oep-geo seam test applies.
- **Typed refusal on invalid input.** A non-positive / non-numeric radius or
  distance and a non-`union|difference|intersection` op are refused with the
  field + reason named (`error_field` / `error`), never a kernel panic, and
  leave `geometry=0`.
- **The `kernel` feature.** `oep-app` gains `feature = ["oep-geo/kernel"]`
  (default `[]`, so the default build stays C++/OCCT-free). The same
  `tests/kernel_features.rs` therefore compiles and greens in BOTH builds:
  each test reads `kernel=` from the panel's own detection and asserts the
  branch the runtime actually produced.

## Acceptance (real command output in transcripts/)

| Command | Result | Transcript |
|---|---|---|
| `cargo test -p oep-app --test kernel_features` (default build, kernel ABSENT) | 122 passed / 0 failed, EXIT=0 | `01-kernel-features-default.txt` |
| `cargo test -p oep-app --features kernel --test kernel_features` (kernel PRESENT, real OCCT linked) | 122 passed / 0 failed, EXIT=0 | `02-kernel-features-kernel.txt` |
| `cargo test -p oep-app` | 22 targets / 0 failed (incl. `tests/kernel_features.rs` 122/0; frozen contracts tree_empty_state, sketch_enter, feature_tree, property_panel, rollback, variable_table, heading_ink all green, none modified) | `03-oep-app.txt` |
| `cargo test --workspace` | see `WS_EXIT=` in `04-workspace.txt` | `04-workspace.txt` |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean, EXIT=0 | `05-clippy.txt` |
| `cargo fmt --all -- --check` | clean, EXIT=0 | `06-fmt.txt` |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, EXIT=0 | `07-smoke.txt` |
| `cargo test -p oep-geo --features kernel --test kernel_seam` (seam reference: real OCCT, fillet golden) | 16 passed / 0 failed, EXIT=0 | `08-seam-reference.txt` |
| Landing (push, merge to `wave/ui-c`, remote read-back) | recorded | `09-landing.txt` |

The `kernel_features` suite proves each sentence of the card:

1. **availability is detected, never assumed** -
   `availability_is_detected_and_reported_honestly` runs in BOTH builds and
   asserts the panel publishes its own runtime detection: `kernel=1` with the
   fixture's real 24 edge occurrences and empty reason, or `kernel=0` with a
   reason naming the seam state and `edge_count=0`. Before any run there is
   no geometry (`geometry=0`, `faces=-`, `volume=-`) -- real or fake.
2. **the unavailable path is a tested state with an honest message** -
   `dialogs_without_kernel_are_honest_unavailable_not_fake_geometry` runs
   every dialog under a missing kernel and asserts `geometry=0`, `faces=-`,
   `volume=-` and a readable reason naming the feature unavailable, and
   `shell_dialog_is_honestly_no_geometry` asserts the shell states its real
   reason with no geometry in BOTH builds.
3. **with the kernel, fillet produces the expected face count** -
   `fillet_produces_expected_face_count_on_the_fixture` runs the fillet
   dialog on the fixture and asserts `run_fillet=1`, `geometry=1`,
   `faces=7` (the seam golden) at the golden volume 0.9914159265358977
   within 1e-9 relative -- the card's headline acceptance.
4. **chamfer and boolean are real geometry too** - chamfer yields 7 faces at
   volume 0.98; union of the two fixture solids yields 14 faces at volume
   1.5 (both the seam goldens), so the dialogs genuinely sit on the OCC seam,
   not on constants.
5. **invalid input is a typed refusal, not a panic** - `invalid_fillet_amount`
   (negative and non-numeric radius refused with the `radius` field + reason;
   a positive amount then runs real geometry) and `invalid_boolean_op`
   (a non-token op refused naming the accepted set) both leave `geometry=0`.

## STEP 0

Recorded in `00-step0.txt`. The merge was a FAST-FORWARD and NOT empty: this
branch forked from `736a0a7` (before the wave-B close), and merging
`github/wave/ui-c` (`ddd6d17`) brought the whole landed wave-B close + UI-C1
through UI-C4 (226 files). The whole gate above was re-run against the merged
head.

## Landing (merged head, after UI-C5 landed mid-run)

While this card ran, the wave head advanced past my STEP 0 to `2c1c900`
(UI-C5 UI-MEASURE landed mid-run). The landing merge `merge-UI-C6`
(`366c0a3`, parents `2c1c900` + card branch `f185c4f`) therefore met UI-C5's
code; every path auto-merged EXCEPT `workstreams/modeler/ui/README.md`, where
both UI-C5 and UI-C6 register a row right after the UI-C4 row. This is NOT a
root manifest, so per the card rule the conflict is documented (kanban
comment #222 on this task naming the file and the two intents) and resolved
as a pure union keeping BOTH rows ordered by card id (UI-C5 then UI-C6) --
dropping either side would false-erase a registered card row.

The ENTIRE acceptance was re-run against the merged head `366c0a3`
(transcripts 01-08 are the post-merge output): `kernel_features` 122/0 in
both the default and `--features kernel` builds, `cargo test -p oep-app` 22
targets / 0 failed (now including C5's `tests/measure.rs` alongside
`tests/kernel_features.rs`; frozen contracts untouched), `cargo test
--workspace` green (`WS_EXIT=0`), clippy `-D warnings`, fmt and `--smoke`
(`frames=8 smoke ok`) all clean. Transcript `09-landing.txt` records the
push / merge / remote read-back.

## Honest notes

- **No real-binary screenshot in this card.** The panel is delivered
  `#[path]`-includable and NOT wired into the shell regions (owned-files
  constraint: `app.rs` / `panels/mod.rs` are not owned by this card), so there
  is no shell frame that shows it -- the same honest position UI-C3 took.
  The rendering and both kernel branches are proven through the `egui_kittest`
  harness driving the real `kernel_features::show` with a real `app::App`.
  No PNG images are produced by this card, so no per-image sha256 is
  included (every image this card produces would have one; there are zero).
- **The "kernel PRESENT" proof is the `--features kernel` build.** It
  forwards to `oep-geo/kernel` -> `oep-engine-occ/build-engine`, compiling
  the cxx bridge + C++ glue and linking the pinned static OCCT 8.0.1 install
  (LGPL-2.1 + OCCT Exception 1.0, the seam's existing posture, test-target
  only: the default workspace build links no OCCT). `08-seam-reference.txt`
  shows the same sealed seam passing its own golden suite on this host.
- **Root-manifest edit.** Adding the card-mandated OCC-seam dependency to
  `crates/oep-app/Cargo.toml` (+ `Cargo.lock`) is the same card-owned pattern
  UI-B6 used for `oep-sketch`; the `kernel` feature is inert by default.
- Every commit carries the DCO sign-off (the repo's commit-msg hook appends
  `Signed-off-by: Don Allen <don@redonealpha.com>` automatically); `--no-verify`
  is never used.
