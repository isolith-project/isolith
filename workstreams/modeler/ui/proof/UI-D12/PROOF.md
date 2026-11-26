# UI-D12 UI-GATE-FIX: make the wave-D head pass its own gate

Card: `t_633acec0`. Branch: `ui/d12-gate-fix`. Scope: mechanical gate fixes on
the merged wave-D head so the wave's own gate is green for the right reasons.
No behaviour change, no test weakening, no golden hidden. The diff is comments
(em-dash text), whitespace (fmt), two clippy-required equivalent rewrites, and
three regenerated pixel artifacts.

Real command output for every leg is committed under `transcripts/`; this file
names each change, quotes the artifact table, and records the presentation-only
statement the card requires.

## Why this card exists

The wave-D close (UI-D6) blocked at the gate: four of six legs red, and every
red leg lived in files UI-D6 was forbidden to touch. During this card's first
attempt the clippy leg and the em-dash guard each proved to carry a fifth root
cause in viewer test files the card had not listed, so this card blocked; the
orchestrator granted option (a) on 2026-11-26, extending the owned list to the
four viewer test files (same mechanical class: fmt / em-dash text / the one
named clippy fix). Nothing outside the granted list was touched.

## STEP 0

`git fetch github` then `git merge github/wave/ui-d`: **Already up to date.**
The wave head `1097f76` was already fast-forwarded into this worktree, so the
merge brought nothing (`transcripts/00-step0.txt`). No rebase, no reset.

## Changes, per owned file (reviewer checklist)

| File | What changed |
|---|---|
| `crates/oep-app/src/panels/pick_gpu.rs` | fmt only: const-block assert laid out by rustfmt (whitespace, line 486) |
| `crates/oep-viewer/tests/perf_budget.rs` | fmt (lines 187/195/316 layout) + em-dash text (10) + the clippy constant binding at line 233 |
| `crates/oep-viewer/src/render.rs` | em-dash text only (3) |
| `crates/oep-viewer/src/gpu/resident.rs` | em-dash text only (5) |
| `crates/oep-viewer/src/gpu/idbuffer.rs` | em-dash text only (2) |
| `crates/oep-viewer/src/gpu/shading.rs` | em-dash text only (11) |
| `crates/oep-viewer/src/gpu/device.rs` | em-dash text only (3) |
| `crates/oep-viewer/src/gpu/mod.rs` | em-dash text only (22; `alpha ≈ 128` U+2248 preserved) |
| `crates/oep-viewer/tests/resident_scene.rs` | em-dash text only (3) |
| `crates/oep-viewer/tests/gpu_backend.rs` | em-dash text only (2) |
| `crates/oep-viewer/tests/render_axis_views.rs` | em-dash text only (2) + the clippy fix at line 297 |
| `demos/editor-sketch-extrude/present-1080p-{before,after}.png` | regenerated through the committed generator |
| `workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` | refreshed through `scripts/ui/shot.sh` |
| `workstreams/modeler/ui/proof/ui-a2/ui-a2-golden-before-ui-d.png` | the superseded wave-C golden, preserved |
| `workstreams/modeler/ui/proof/UI-D12/` | this proof |
| `workstreams/modeler/ui/README.md` | UI-D12 row |

Every em dash (`U+2014`) in the ten granted viewer files was replaced with
plain punctuation (`:`, `,`, `;`, `-`), comment and string text only. Verified
by construction: 63 anchors, each unique, each replacing only the em dash and
its adjacent spacing; post-state has zero em dashes in all ten files.

### The two clippy fixes (behaviour identical)

1. `crates/oep-viewer/tests/perf_budget.rs:233`
   `assert!(MEASURED_FRAMES >= 120, "the card requires at least 120 measured
   frames")` moved into `const { assert!(...) }` - clippy's own suggested fix
   for `clippy::assertions_on_constants`. The guard still fires if
   `MEASURED_FRAMES < 120` (now at compile time); the message is kept; nothing
   deleted or commented out.
2. `crates/oep-viewer/tests/render_axis_views.rs:297`
   `msg.chunks_exact(64)` -> `msg.as_chunks::<64>().0` for
   `clippy::chunks_exact_to_as_chunks`. Provably the same loop: `msg` is
   padded to a multiple of 64 immediately before this loop, so `as_chunks`
   yields exactly the same 64-byte chunks as `chunks_exact` (the remainder
   slice is empty) and `chunk[i*4..]` indexing is unchanged on `&[u8; 64]`.
   No index arithmetic added, nothing deleted.

## Regenerated artifact table (old -> new sha256, and why)

Ran with the committed generators on the committed wave-D head, no parameter
changed anywhere.

| Artifact | Old sha256 | New sha256 | Why one line |
|---|---|---|---|
| `demos/editor-sketch-extrude/present-1080p-before.png` | `84adda8768b2e458748b5b8deb07ba10d7f8176365d46e7dd1846623768fbf0e` | `1d0676b8ffe82bee11afe662cc8417fdb05fc23aa45c171e5c7b70f0528caff5` | Wave-D renderer draws from resident buffers and adds the nav cube / shading / measure surfaces, so the same generator renders a moved present frame. |
| `demos/editor-sketch-extrude/present-1080p-after.png` | `f58523a61919ca8a1cc295be483f7788b4915f4a8d15c042d0c00539049a5f64` | `bcf33a5dd36127c890e79b35b1f4c3cac5a5b638eb018d213a9a7be2ad73cdeb` | Wave-D pixel change, same reason as before (width-120 live frame). |
| `workstreams/modeler/ui/proof/ui-a2/frame.png` (launch golden) | `a7594b124ac8ea49c95ef49d2c25106bf37cf309ca808d652e9943bb417586ef` | `56afced6473393b20a00388e1a1057032b2eeb0d6721fe01caebc43b23fd0f45` | Wave D visibly changed the launch frame (view cube, shading panel, measure surface); golden refreshed through `shot.sh`, byte-identical to UI-D6's recorded drift hash and stable across two independent captures. |

Superseded copies (sha256 in `transcripts/100-artifacts.sha256`):
`workstreams/modeler/ui/proof/UI-D12/superseded/present-1080p-before.png`,
`.../present-1080p-after.png`, and the preserved golden
`workstreams/modeler/ui/proof/ui-a2/ui-a2-golden-before-ui-d.png`.

Regeneration proof: `transcripts/05-present-regen.txt` (the generator's own
stdout with the `1920x1080 present grid/axes/labels` markers), and
`tests/test_interactive_shell_acceptance.py::test_present_hd_committed_artifacts_regenerate_byte_identical`
passes on the committed tree (`python -m pytest -q <...>::<test>` -> 1 passed).

## Statement: pixels moved ONLY through the presentation layer

The only pixel-carrying artifacts changed on this card are named here:
`demos/editor-sketch-extrude/present-1080p-before.png`,
`demos/editor-sketch-extrude/present-1080p-after.png`, and the launch golden
`workstreams/modeler/ui/proof/ui-a2/frame.png`. Each was regenerated with its
committed generator (the `oep-shell` workbench `sk:render:...:1920:1080:present`
path for the present pair; `scripts/ui/shot.sh` for the launch golden) using
the identical commands and parameters on the identical committed wave-D head.
The Rust diff of this card is comments and whitespace plus the two
clippy-required equivalent rewrites; no shader, projection, scene data,
engine code, generator, or test assertion changed. The pixel delta is entirely
produced by the wave-D presentation pipeline that already landed on the wave
head (resident-buffer draw path, nav cube, shading and measure surfaces).

## Gate on the committed tree (all real transcripts committed)

| Leg | Transcript | Result |
|---|---|---|
| `cargo fmt --all -- --check` | `transcripts/01-fmt.txt` | exit 0 (`FMT_EXIT=0`) |
| `cargo clippy --workspace --all-targets --keep-going -- -D warnings` | `transcripts/02-clippy.txt` | exit 0 (`CLIPPY_EXIT=0`, 34 crates, all targets) |
| `cargo test --workspace` | `transcripts/03-test-workspace.txt` | exit 0, 225 suites, 4608 passed / 0 failed / 2 ignored |
| `python3 scripts/licensing-check.py` | `transcripts/04-licensing.txt` | exit 0, 6/6, 34 crates, 535 deps |
| `bash scripts/gate/precommit-gate.sh` | `transcripts/05-precommit.txt` | exit 0 (`gate PASS`), pytest 559 passed / 0 failed / 28 skipped - both UI-D6-named failures green |
| `bash scripts/ui/shot.sh` | `transcripts/06-shot-run1.txt`, `06-shot-run2.txt` | exit 0 both, two consecutive runs byte-identical (`56afced6...`) |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `transcripts/07-smoke.txt` | exit 0, `frames=8 smoke ok` |

### Pre-existing flake, handled as the prior wave cards did (D4/D7/D8-documented)

The gate's `cargo test` leg intermittently aborts with an exit-time
`signal: 11 SIGSEGV` in an egui_kittest harness binary AFTER all its
assertions pass (pick_gpu 123/123 pass; the crash is harness teardown, not a
test failure). D8 reproduced the identical intermittent crash on its unmodified
base commit (`transcripts/12-baseline-pickgpu-flake.txt`), confirming it is
pre-existing. This card's handler: keep the first flaked gate attempt as
`transcripts/05-precommit-flake1.txt` (pick_gpu teardown SIGSEGV, zero failing
tests) and re-run to the clean `transcripts/05-precommit.txt` (`gate PASS`).
The full `cargo test --workspace` (transcript 03) also landed clean, exit 0,
which independently proves the workspace. This card's diff touches no kittest
harness logic (comment/whitespace/clippy-equivalent only) and, once again, the
same pick_gpu binary passes clean in isolation between crashes.
