# UI-F5 UI-WAVE-F-INTEGRATE: merge, gate, checkpoint, screenshots, tag (proof)

Card: `t_5040d1d6`. Branch: `ui/f5-integrate`. Owned files (exactly):

- `docs/status/waves/wave-ui-f.md` (new wave record)
- `docs/scope-register.json` + `docs/scope-register.md` (the ONE row upgraded to
  `shippable` - the register's first - see STEP REGISTER below)
- `workstreams/modeler/ui/README.md` (UI-F5 row)
- `workstreams/modeler/ui/proof/UI-F5/` (this dir: proof + gate/landing
  transcripts + screenshots + capture tooling)
- `workstreams/modeler/ui/proof/ui-a2/frame.png*` (via `scripts/ui/shot.sh`;
  the golden did NOT drift this run - leg 06)

No crate was edited and no file outside the owned list was touched.

## STEP 0

`git fetch github && git merge github/wave/ui-f` from the card worktree
(`ui/f5-integrate`). NOT empty: the branch was parked at a stale wave-C
landing (`6275787`); the merge brought in the whole wave-F head (`eaedab0`)
incl. UI-F1..UI-F4 and UI-D11. Transcript: `transcripts/00-step0-merge.txt`.

Every wave-F card branch IS an ancestor of `github/wave/ui-f` (audit
`transcripts/08-ancestors.txt`, 5/5): `ui/f1-sheet`, `ui/f2-views`,
`ui/f3-annotate`, `ui/f4-drawing-export`, `ui/d11-depth-sort`.

## Checkpoint merged head

- `git switch -c checkpoint/wave-ui-f-eaedab0 github/main` then
  `git merge --no-ff github/wave/ui-f` -> checkpoint commit
  `ad90892`, parents `a68505d` (github/main) + `92601fd` (wave/ui-f head at
  record time).
- `main` IS an ancestor of the wave head, so the checkpoint tree is
  byte-identical to the wave tree; `git diff --stat HEAD github/wave/ui-f`
  is EMPTY (real output in `checkpoint-merged-head/00-head.txt`). The six-leg
  gate on the checkpoint merged head therefore measures exactly the wave tree.
- The close record, gate transcripts, screenshots and capture tooling are
  folded into this single no-ff checkpoint commit (docs only, `--amend
  --no-edit`); the tree is unchanged, so the gate numbers stand. The amended
  commit sha (the one pushed, merged and tagged) is recorded in the landing
  transcripts.

## STEP REGISTER (wave F makes the register's FIRST `shippable` claim)

`2d-drafting` was `planned`/`decision`. Wave F carries the drafting lane end
to end with a green accepted test for EACH surface on the committed bracket
drawing, plus a committed export golden, so the row moves to `shippable`/`test`
(the honest-claims ceiling for a capability proven on real material):

- `2d-drafting` <- `crates/oep-app/tests/drawing_export.rs` (UI-F4, 170/0 on
  the merged head, transcript leg 03) with the committed PDF golden
  (`GOLDEN pdf: page 0 0 297 210, content ops 45`; `PDF byte-identical to
  golden: 1946 == 1946 bytes`) and deterministic SVG (3158 bytes, byte-identical
  across independent exports). The other three drafting suites are the rest of
  the end-to-end proof: `sheet.rs` 170/0, `views.rs` 169/0, `annotate.rs` 170/0
  (all on the merged head).

Why this is honest `shippable`: the register's definition is "engineers can do
the main job end to end; evidence is a green accepted test proving the
capability against real material", and `shippable` requires evidence kind
`test`. The drafting main job - sheets, view placement, dimensions / leaders /
notes / title block, SVG and PDF export - is exactly what wave F ships, each
step proven by a green accepted test against the committed bracket drawing,
with the export matching a committed golden byte-for-byte. The note in the
register states the boundary honestly: the lane drafts the live committed
drawing document (new sheet creation yes; authoring a new part MODEL remains
the upstream modelling lanes).

JSON + markdown updated together; `scope_register_check` green 23/23
(transcript `checkpoint-merged-head/07-scope-register.txt`).

## Full gate on the checkpoint merged head

Transcripts in `transcripts/checkpoint-merged-head/` (real command output,
one per leg):

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) - `01-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0), Finished in 54.79s - `02-clippy.txt` |
| `cargo test --workspace` | PASS (exit 0) - 235 suites, aggregate 7229 passed / 0 failed - `03-test-workspace.txt` (clean at `-j 4`; key suites on the merged head: sheet 170/0, views 169/0, annotate 170/0, drawing_export 170/0, gpu_shading 172/0, gpu_backend 6/0, pick_gpu 170/0, render_axis_views 4/0, resident_scene 6/0, nav 169/0, depth_sort_defect 1/0) |
| `python3 scripts/licensing-check.py` | PASS (exit 0) - 6/6, 34 crates, 535 deps - `04-licensing.txt` |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 559 passed / 28 skipped, `cargo test` clean, gate PASS - `05-precommit.txt` (one clean run: the documented egui_kittest exit-time SIGSEGV teardown flake did NOT surface this run) |
| `bash scripts/ui/shot.sh` | PASS (exit 0) - frame `7fcf893f6ca1...`, byte-identical x2 - `06-shot.txt`. The launch golden did NOT drift: wave-F's drafting panel (F1..F4) does not change the shipped launch frame, and UI-D11 had already refreshed the golden on this same wave head. No regolden, no superseded file. |

`scope_register_check`: 23/23 pass - `07-scope-register.txt` - see the
register upgrade above.

## Screenshots (acceptance)

Four real-binary frames from the merged-head `isolith` binary under
`xvfb-run` via the XTEST injector, two independent launches per mode
byte-identical (the `.sha256` is the determinism oracle):
`capture_ui_f5.sh` (sheet / views / export-preview at the shipped 1280x800
window) and `capture_annotate_tall.sh` (annotate at a 1280x1200 native
window, the wave-E mate precedent - see the annotate note).

| Shot | sha256 | Shows |
|---|---|---|
| sheet | `e610d79e3390e0b531685cd1eba428caebbe04998beceda7322c3f74f63966fd` | shipped 1280x800 launch: the drafting sheet section - sheet tree (`Sheet 1 - A4 landscape third-angle scale 1:1`), Add sheet, size/scale/projection selectors, view-placement legality |
| views | `1f51a4afce953f5aee0887bd6621922c32f2fb6139cf72661195dea27af602c8` | the F2 views tools clicked through the real binary: the Views tool grid (Place base front / projected top / projected right / section / detail / Clear views) and the white sheet-preview canvas showing ALL FOUR placed views - the base view chip, the aligned projected-top chip, the hatched section outline, and the detail view label |
| annotate | `fe3faf10c13510495f3f1d9fcaef694af339390f5455d0bf616b96c1cb4d4652` | the annotate surface with a REAL placed dimension and BOTH restored records (see note below): title block, Edit part name, the `value=80.000000000 measured=80.000000000 geometry_w=80.000000000` readout, Add leader / Add note / Clear annotations, and the two records `leader oep-leader-1 ... LEADER 1: C/L THIS FEATURE` and `note oep-note-1 ... NOTE 1: REMOVE ALL SHARP EDGES`. Frame is 1280x1200 (native window resized like the wave-E mate capture): at the shipped 1280x800 window the export section starts immediately below the annotation buttons, so no scroll position reveals the record rows there |
| export-preview | `aae03ab9a94634ef6d868a9a428d708c602f2a87b0a7d3a96e02a87f6dad602f` | the F4 export section at the drafting scroll-area bottom clamp: the print-preview canvas with the `45 graphical elements (same content the export writes)` readout, the export target path, and the Export SVG / Export PDF buttons - the Export SVG click REALLY wrote `drawing.svg` (3158 bytes) through the UI in the capture session |

### Annotate capture note (honest)

The annotation RECORD ROWS (`leader oep-leader-1`, `note oep-note-1`) render
below the drafting scroll area's clip at the shipped 1280x800 window: on the
merged head the annotate section sits immediately above the F4 export section,
so no scroll position at 800 tall reveals rows below the buttons without the
export section crowding them out (verified at multiple scroll notches). Per
the wave-E precedent (the mate capture drove the same binary at 1280x1500),
the annotate frame drives the SAME merged-head binary at a 1280x1200 native
window (XResizeWindow, `ui_f5_resize.c`), which reveals the full annotate
section. The records' content is additionally proven by the UI-F3 kittest
acceptance `--test annotate` 170/0 on the merged head. No fake frame: every
element shown is real output of the merged-head binary driven through XTEST.

## Folding into the checkpoint

This record, the gate transcripts, the screenshots and the capture tooling
are folded into the single no-ff checkpoint commit by amending it (`--amend
--no-edit`: same parents, code tree untouched, docs only). The amended commit
is the one pushed, PR-merged and tagged. The landing evidence (PR, main sha,
tag, Gitea mirror, `wave/ui-g` advance) is recorded and finalized under
`transcripts/landing/` after the landing, on the wave branch, per the
wave-close convention.

## Ancestors

`transcripts/08-ancestors.txt` - see STEP 0 above (5/5).
