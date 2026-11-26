# UI-D6 UI-WAVE-D-INTEGRATE: merge, gate, checkpoint, screenshots, tag — BLOCKED

Card: `t_62060655`. Branch: `ui/d6-integrate`. Owned files (exactly):
`docs/status/waves/wave-ui-d.md`, `workstreams/modeler/ui/README.md` (UI-D6 row),
`workstreams/modeler/ui/proof/UI-D6/`.

STATUS: BLOCKED (needs_input) at the wave-close gate. The merged wave head does
NOT pass the full six-leg gate. Four of six legs are red, and every red leg's
root cause lives in files this card does not own and is forbidden to edit
("DO NOT edit any crate", "Touch no file outside your owned list"). A faked
green is not an outcome; this record commits the real command output.

## STEP 0

`git fetch github` then `git merge github/wave/ui-d`: fast-forwarded the wave
head `ba7e3bc` -> `1097f76` (transcript `transcripts/09-step0-merge.txt`). Not
empty. The wave head carries UI-D5's landing (the wave was fast-forwarded to
`1097f76` so every wave-D card branch is an ancestor, per the orchestrator's
UI-D5 handoff: `git push github github/ui/d5-perf:refs/heads/wave/ui-d`).

Every wave-D card branch IS an ancestor of `github/wave/ui-d`
(`transcripts/08-ancestors.txt`, all nine checked green). `ui/d11-depth-sort`
was NOT merged and is correctly NOT an ancestor (deferred per orchestrator
addendum 2026-11-25; not part of wave D, parked in `scheduled`).

## Merge topology (for the checkpoint, not built)

`github/main` (`bed3d50`) is NOT an ancestor of the wave head: main carries two
post-wave-C docs PRs (readme-refresh #32, brand-avatar #33) not on the wave.
Merge base is `6275787` (the wave-C checkpoint). The would-be checkpoint tree
(main + wave) differs from the wave head only by those two docs commits; none
of the gate failures below is touched by them, so the checkpoint merged head
would be equally red on the same four legs.

## Full gate on the merged head (wave head `1097f76`, tree identical to the
## wave head: `git diff --quiet HEAD github/wave/ui-d` -> identical)

Real transcripts in `transcripts/`:

| Leg | Result (verbatim transcript) |
|---|---|
| `01-fmt.txt` | FAIL (exit 1) — `crates/oep-app/src/panels/pick_gpu.rs:486` (UI-D3 file, const-block fmt) + `crates/oep-viewer/tests/perf_budget.rs:187,195,316` (UI-D5 file) |
| `02-clippy.txt` | FAIL (exit 101) — `crates/oep-viewer/tests/perf_budget.rs:233`, `clippy::assertions-on-constants` under `-D warnings` (UI-D5 file) |
| `03-test-workspace.txt` | PASS (exit 0) — clean re-run; the single first-run failure was `--test nav` SIGSEGV at exit, the documented egui_kittest exit-time flake (D4/D8-documented); isolated `nav` 122/0, twice |
| `04-licensing.txt` | PASS (exit 0) — 6/6, 34 crates, 535 deps |
| `05-precommit.txt` | FAIL — pytest 2 failed / 556 passed / 29 skipped: `tests/test_interactive_shell_acceptance.py::test_present_hd_committed_artifacts_regenerate_byte_identical` (committed `demos/editor-sketch-extrude/present-1080p-*.png` no longer regenerate byte-identical under wave-D rendering) + `tests/test_preview_viewer_acceptance.py::test_preview_viewer_acceptance` (em-dash text guard fails in D-crate files: `render.rs` [D8], `gpu/resident.rs` [D10], `gpu/idbuffer.rs` [D3], `gpu/shading.rs` [D2], `gpu/device.rs` [D1], `gpu/mod.rs`) |
| `06-shot.txt` | FAIL (exit 1) — `scripts/ui/shot.sh` frame drift: current `56afced6…f45` deterministic across two independent runs, committed wave-C golden `a7594b124…ef` |

## Scope register

`docs/scope-register.json` byte-identical on the merged head (no wave-D test
proves a register row; no read of it is required for close). No screenshot set
was produced: a real-binary frame is not enough on its own, and the acceptance
bullet "full gate green on the merged head (six transcripts)" is unmet.

## The decision this card needs (block reason)

Every red leg is owned by an earlier wave-D card's files or a non-card
artifact, none in this card's owned list:

1. **fmt** — `crates/oep-app/src/panels/pick_gpu.rs` (UI-D3) and
   `crates/oep-viewer/tests/perf_budget.rs` (UI-D5). CI runs the SAME
   `cargo fmt --all -- --check` (ci.yaml), so the checkpoint PR would be red.
   (D3's own fmt transcript records `fmt exit: 1`; the wave head was never
   fmt-clean for these files.)
2. **clippy** — `crates/oep-viewer/tests/perf_budget.rs` (UI-D5).
3. **precommit pytest, leg A** — `crates/oep-viewer/src/gpu/*.rs` +
   `render.rs` (UI-D1/D2/D3/D8/D10 files) violate the standing no-em-dash text
   guard (`tests/test_preview_viewer_acceptance.py`, was green at C7).
4. **precommit pytest, leg B** — `demos/editor-sketch-extrude/present-1080p-*.png`
   (committed by MEDIA-RENDERER `26ab067`, a non-wave-D feature) no longer
   regenerate byte-identical under the wave-D renderer. Regenerating them is a
   golden-refresh decision that belongs to the MEDIA-RENDERER area, not this card.
5. **shot.sh** — `workstreams/modeler/ui/proof/ui-a2/frame.png*` is in this
   card's owned list ONLY if the golden legitimately drifted. The drift is
   subtle (both frames are valid, visually near-identical; the delta is the
   wave-D renderer pipeline, e.g. D7 clip_w / D8 vec4 / D10 resident), and no
   wave-D card delegated the refresh (each claimed "no shot.sh drift"). Per the
   skill: "shot.sh drift is a finding unless the upstream cards delegated the
   golden refresh".

This card cannot green legs 1-4 without editing crates (forbidden) or
non-owned artifacts (forbidden). Options for the orchestrator/Don:
(a) cut gate-fix card(s) owned by UI-D3/UI-D5 (and the demos artifact owner) to
land the fmt/clippy/em-dash/present-PNG fixes on the wave branch, then re-run
this integrate card; or
(b) explicitly grant this card owned-file exceptions for exactly those files;
or
(c) accept a documented deviation (this card does NOT recommend faking green).
