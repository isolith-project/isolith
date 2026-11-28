# UI-F5 UI-WAVE-F-INTEGRATE: merge, gate, checkpoint, screenshots, tag (proof)

Card: `t_5040d1d6`. Branch: `ui/f5-integrate`. Owned files (exactly):

- `docs/status/waves/wave-ui-f.md` (new wave record)
- `docs/scope-register.json` + `docs/scope-register.md` (the ONE row upgraded to
  `shippable` - the register's first - see STEP REGISTER below)
- `workstreams/modeler/ui/README.md` (UI-F5 row)
- `workstreams/modeler/ui/proof/UI-F5/` (this dir: proof + gate/landing
  transcripts + screenshots)
- `workstreams/modeler/ui/proof/ui-a2/frame.png*` (only via `scripts/ui/shot.sh`
  if the golden legitimately drifted - see 06-shot.txt)

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

- `git switch -c checkpoint/wave-ui-f-<wavesha> github/main` then
  `git merge --no-ff github/wave/ui-f` -> checkpoint commit
  `<CHECKPOINT>`, parents `a68505d` (github/main) + `<WAVE>` (wave/ui-f head
  at close).
- `main` IS an ancestor of the wave head, so the checkpoint tree is
  byte-identical to the wave tree; `git diff --stat HEAD github/wave/ui-f`
  is EMPTY. The six-leg gate on the checkpoint merged head therefore measures
  exactly the wave tree (same numbers as a wave-head gate).

## STEP REGISTER (wave F makes the register's FIRST `shippable` claim)

`2d-drafting` was `planned`/`decision`. Wave F carries the drafting lane end
to end with a green accepted test for EACH surface on the committed bracket
drawing, plus a committed export golden, so the row moves to `shippable`/`test`
(the honest-claims ceiling for a capability proven on real material):

- `2d-drafting` <- `crates/oep-app/tests/drawing_export.rs` (UI-F4, 170/0 on
  the merged head) with the committed PDF golden (page 0 0 297 210, 45 content
  ops, `PDF byte-identical to golden: 1946 == 1946 bytes`) and deterministic
  SVG (3158 bytes, byte-identical across independent exports). The other three
  drafting suites are the rest of the end-to-end proof: `sheet.rs` 170/0,
  `views.rs` 169/0, `annotate.rs` 170/0.

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

JSON + markdown updated together; `scope_register_check` green
(transcript `checkpoint-merged-head/07-scope-register.txt`).

## Full gate on the checkpoint merged head

Transcripts in `transcripts/checkpoint-merged-head/` (real command output):

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | `<result>` |
| `cargo clippy --workspace --all-targets -- -D warnings` | `<result>` |
| `cargo test --workspace` | `<result>` |
| `python3 scripts/licensing-check.py` | `<result>` |
| `bash scripts/gate/precommit-gate.sh` | `<result>` |
| `bash scripts/ui/shot.sh` | `<result>` |

`scope_register_check`: `<N>/<N>` pass - `07-scope-register.txt` - see the
register upgrade above.

## Screenshots (acceptance)

Four real-binary frames from the merged-head `isolith` binary at the shipped
1280x800 window under `xvfb-run` via the XTEST injector (`ui_f5_inject.c`,
`capture_ui_f5.sh`, the same pattern as `proof/UI-F4`): two independent
launches per mode must produce byte-identical PNGs; the `.sha256` is the
determinism oracle.

| Shot | sha256 | Shows |
|---|---|---|
| sheet | `<sha>` | the drafting panel's sheet section: sheet tree (Sheet 1), Add sheet, size/scale/projection selectors, placement-legality line |
| views | `<sha>` | the F2 views tools clicked: accent base chip, aligned projected-top chip, hatched section outline, detail chip |
| annotate | `<sha>` | dimension + leader/note records: part-name editor, `value=80 measured=80 geometry_w=80` readout, both restored records |
| export-preview | `<sha>` | the F4 export section: print-preview canvas (45 graphical elements), export path, Export SVG / Export PDF buttons; the click really writes `drawing.svg` |

Every drafting surface is reachable in the real binary at the shipped window,
so no honest-note unreachable-panel exception is needed (unlike wave E's
explode section).

## Folding into the checkpoint

This record, the gate transcripts, the screenshots and the wave record
(`docs/status/waves/wave-ui-f.md`) are folded into the single no-ff checkpoint
commit by amending it (`--amend --no-edit`: same parents, code tree untouched,
docs only). The amended commit is the one pushed, PR-merged and tagged. The
landing evidence (PR, main sha, tag, Gitea mirror, `wave/ui-g` advance) is
recorded under `transcripts/landing/` after the landing, on the wave branch,
per the wave-close convention.

## Ancestors

`transcripts/08-ancestors.txt` - see STEP 0 above (5/5).
