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
  tree as the checkpoint merged head.
- Landing: PR #<PR> merged as `<MAIN>` (`github/main`), annotated tag
  `wave-ui-f` (`<TAG>`) -> checkpoint `<CHECKPOINT>`, Gitea mirror
  fast-forwarded to `<MAIN>` with the tag, and `github/wave/ui-g` created at
  `<MAIN>`. Transcripts `proof/UI-F5/transcripts/landing/`.
- The UI-F5 record (this file, the README row, `proof/UI-F5/`) is folded into
  the single checkpoint commit and re-committed onto the wave branch, per the
  wave-close convention.

## Full gate on the checkpoint merged head

Checkpoint merged head: `ad90892` (pre-amend; the amended commit sha is in
the landing transcripts), parents `a68505d` (github/main) + `92601fd`
(wave/ui-f head at record time). Transcripts in
`workstreams/modeler/ui/proof/UI-F5/transcripts/checkpoint-merged-head/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) - `01-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0) - `02-clippy.txt` |
| `cargo test --workspace` | PASS (exit 0) - 235 suites, aggregate 7229 passed / 0 failed (clean at `-j 4`) - `03-test-workspace.txt` |
| `python3 scripts/licensing-check.py` | PASS (exit 0) - 6/6, 34 crates, 535 deps - `04-licensing.txt` |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 559 passed / 28 skipped, `cargo test` clean - `05-precommit.txt` |
| `bash scripts/ui/shot.sh` | PASS (exit 0) - frame `7fcf893f6ca1...` byte-identical x2, golden did NOT drift (no regolden) - `06-shot.txt` |

`scope_register_check`: 23/23 (`proof/UI-F5/transcripts/checkpoint-merged-head/07-scope-register.txt`).
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
| sheet | `e610d79e3390e0b531685cd1eba428caebbe04998beceda7322c3f74f63966fd` | shipped 1280x800 launch: the drafting panel's sheet section - sheet tree (`Sheet 1 - A4 landscape third-angle scale 1:1`), Add sheet, size/scale/projection selectors, placement-legality line |
| views | `1f51a4afce953f5aee0887bd6621922c32f2fb6139cf72661195dea27af602c8` | the F2 views tools clicked through the real binary: canvas with the accent base chip, the aligned projected-top chip, the hatched section outline and the detail view label |
| annotate | `fe3faf10c13510495f3f1d9fcaef694af339390f5455d0bf616b96c1cb4d4652` | dimension placed + leader/note records (at a 1280x1200 native window, wave-E mate precedent; see the note below): the part-name editor, the `value=80 measured=80 geometry_w=80` readout, both restored records |
| export-preview | `aae03ab9a94634ef6d868a9a428d708c602f2a87b0a7d3a96e02a87f6dad602f` | the F4 export section at the drafting scroll clamp: print-preview canvas (45 graphical elements), export path, both Export SVG / Export PDF buttons - the Export SVG click really wrote `drawing.svg` (3158 bytes) |

Each is a genuine real-binary frame from the merged-head `isolith` binary
under xvfb driven through XTEST, deterministic (byte-identical across two
independent captures, sha256 oracle in proof).

## What is not done / deferred

- Three drafting surfaces (sheet, views, export-preview) are fully reachable
  in the real binary AT the shipped 1280x800 window. The annotate RECORD ROWS
  render below the drafting scroll area's clip at 1280x800 (the F4 export
  section starts immediately below the annotation buttons, so no 800-tall
  scroll position reveals them - verified across multiple notches), so the
  annotate frame drives the SAME merged-head binary at a 1280x1200 native
  window (the wave-E mate precedent), revealing the full section including
  both records; no fake frame and no unreachable panel. A future layout card
  could give the drafting region a taller scroll or a dedicated annotate
  view; this close records the captured evidence as-is.
- The wave-C variables / kernel-features hosting gaps are unchanged by wave F.
- The `wave/ui-f`, `merge-UI-F5`, checkpoint and card branches are left in
  place so the wave history and record are preserved.
- No UI test was weakened, skipped or deleted to reach green.
