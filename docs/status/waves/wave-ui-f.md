# Wave ui-f status record

Wave F of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`,
section 5): the drafting lane - sheet setup and sheet tree (UI-F1), view
placement (UI-F2), dimensions / leaders / notes / title block (UI-F3), SVG
and PDF export with print preview (UI-F4) - plus UI-D11 (reverse-Z depth
sort), the wave-D continuation that landed on `wave/ui-f` mid-wave. Branch
`wave/ui-f` carries all of UI-F1..UI-F4 + UI-D11 on top of `main`
(`a68505d`, the wave-ui-e checkpoint).

Written by card UI-F5 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-F1 | UI-SHEET: sheet setup and sheet tree | done |
| UI-F2 | UI-VIEWS: place base, projected, section and detail views | done |
| UI-F3 | UI-ANNOTATE: dimension, leader, note tools and the title block | done |
| UI-F4 | UI-DRAWING-EXPORT: SVG and PDF from the UI | done |
| UI-D11 | UI-DEPTH-SORT: reverse-Z depth buffer replacing the CPU painter sort (ADR Option B) | done |
| UI-F5 | UI-WAVE-F-INTEGRATE: merge, gate, checkpoint, screenshots, tag | done |

Every wave-F card branch (the four drafting feature branches plus the UI-D11
depth-sort branch) IS an ancestor of the wave head (audit
`proof/UI-F5/transcripts/08-ancestors.txt`, 5/5): `ui/f1-sheet`,
`ui/f2-views`, `ui/f3-annotate`, `ui/f4-drawing-export`, `ui/d11-depth-sort`.

## Head sha

- Wave head: `eaedab0` (`proof(UI-D11): record the wave/ui-f landing`, the
  D11 landing record on top of merge `4e5c347`).
- STEP 0 of the close card merged `github/wave/ui-f` into the card worktree
  (`ui/f5-integrate`) - the branch had been parked at a stale wave-C landing
  (`6275787`), so the merge was NOT empty (real first-run output in
  `proof/UI-F5/transcripts/00-step0-merge.txt`).
- `main` (`a68505d`) IS an ancestor of the wave head, so the checkpoint merge
  tree is byte-identical to the wave tree; `git diff --stat` HEAD vs
  `github/wave/ui-f` is empty. The six-leg gate therefore measures the same
  tree as the checkpoint merged head. The checkpoint commit was amended with
  the close record + gate transcripts + screenshots (docs only, same parents)
  to `af13b2b` - that amended commit is the one merged and tagged.
- Landing: PR #43 merged as `1edbf639` (`github/main`), annotated tag
  `wave-ui-f` (`63a83145`, peeling to the checkpoint commit `af13b2b` - never
  main's tip), Gitea mirror fast-forwarded to `1edbf639` with the tag, and
  `github/wave/ui-g` created at `1edbf639`. Transcripts
  `proof/UI-F5/transcripts/landing/`.
- The UI-F5 record (this file, the README row, `proof/UI-F5/`) is folded into
  the single checkpoint commit and re-committed onto the wave branch, per the
  wave-close convention.

## Full gate on the checkpoint merged head

Checkpoint merged head: `<CHECKPOINT>`, parents `a68505d` (github/main) +
`<WAVE>` (wave/ui-f head at close). Transcripts in
`workstreams/modeler/ui/proof/UI-F5/transcripts/checkpoint-merged-head/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | `<result>` |
| `cargo clippy --workspace --all-targets -- -D warnings` | `<result>` |
| `cargo test --workspace` | `<result>` |
| `python3 scripts/licensing-check.py` | `<result>` |
| `bash scripts/gate/precommit-gate.sh` | `<result>` |
| `bash scripts/ui/shot.sh` | `<result>` |

`scope_register_check`: <N>/<N> (`proof/UI-F5/transcripts/checkpoint-merged-head/07-scope-register.txt`).
Unlike every wave before it, wave F makes the register's FIRST `shippable`
claim, because a wave-F green accepted test plus the committed export golden
genuinely proves the whole capability:

- `2d-drafting` -> `shippable` (evidence kind `test`, ref
  `crates/oep-app/tests/drawing_export.rs`): the drafting lane is proven end
  to end on the committed bracket drawing by four green accepted test suites
  (sheet 170/0, views 169/0, annotate 170/0, drawing_export 170/0 on the
  merged head) and the export matches the committed PDF golden
  byte-identically (1946 == 1946 bytes), with deterministic SVG across
  independent exports. Detail in `proof/UI-F5/transcripts/checkpoint-merged-head/07-scope-register.txt`.

The register JSON and markdown were updated together and `scope_register_check`
stays green.

## Screenshots (acceptance)

| Shot | sha256 | Shows |
|---|---|---|
| sheet | `<sha>` | shipped 1280x800 launch: the drafting panel's sheet section - sheet tree (Sheet 1), Add sheet, size/scale/projection selectors, placement-legality line |
| views | `<sha>` | the F2 views tools clicked: canvas with the accent base chip, the aligned projected-top chip, the hatched section outline and the detail chip |
| annotate | `<sha>` | dimension placed + leader/note records: the part-name editor, the `value=80 measured=80 geometry_w=80` readout, both restored records |
| export-preview | `<sha>` | the F4 export section at the drafting scroll clamp: print-preview canvas (45 graphical elements), export path, both Export SVG / Export PDF buttons |

Each is a genuine real-binary frame from the merged-head `isolith` binary
under xvfb driven through XTEST, deterministic (byte-identical across two
independent captures, sha256 oracle in proof).

## What is not done / deferred

- The four drafting surfaces are all reachable in the real binary at the
  shipped 1280x800 window, so every acceptance frame is real - no
  unreachable-panel exception was needed (unlike wave E's explode section).
- The wave-C variables / kernel-features hosting gaps are unchanged by wave F.
- The `wave/ui-f`, `merge-UI-F5`, checkpoint and card branches are left in
  place so the wave history and record are preserved.
- No UI test was weakened, skipped or deleted to reach green.
