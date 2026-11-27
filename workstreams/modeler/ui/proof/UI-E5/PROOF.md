# UI-E5 UI-WAVE-E-INTEGRATE: merge, gate, checkpoint, screenshots, tag (proof)

Card: `t_e4067ca1`. Branch: `ui/e5-integrate`. Owned files (exactly):

- `docs/status/waves/wave-ui-e.md` (new wave record)
- `docs/scope-register.json` + `docs/scope-register.md` (the ONE legitimately
  upgraded pair of rows, both proven by wave-E tests - see STEP REGISTER below)
- `workstreams/modeler/ui/README.md` (UI-E5 row)
- `workstreams/modeler/ui/proof/UI-E5/` (this dir: proof + gate/landing transcripts + screenshots)
- `workstreams/modeler/ui/proof/ui-a2/frame.png*` (only via `scripts/ui/shot.sh`
  if the golden legitimately drifted - it did NOT on this run, see 06-shot.txt)

No crate was edited and no file outside the owned list was touched.

## STEP 0

`git fetch github && git merge github/wave/ui-e` from the card worktree
(`ui/e5-integrate`). NOT empty: the branch was parked at a stale wave-C
landing (`6275787`); the merge brought in the whole wave-E head (`1a1c0f1`,
24 files) incl. UI-E1..UI-E6. Transcript: `transcripts/00-step0-merge.txt`.

Every wave-E card branch IS an ancestor of `github/wave/ui-e` (audit
`transcripts/08-ancestors.txt`, 5/5): `ui/e1-assy-tree`, `ui/e2-assy-mates`,
`ui/e3-assy-diagnostics`, `ui/e4-assy-explode`, `ui/e6-nav-cube`.

## Checkpoint merged head

- `git switch -c checkpoint/wave-ui-e-1a1c0f1 github/main` then
  `git merge --no-ff github/wave/ui-e` -> checkpoint commit `<CPT>`,
  parents `a4cd2db` (github/main) + `1a1c0f1` (wave/ui-e head).
- `main` IS an ancestor of the wave head, so the checkpoint tree is
  byte-identical to the wave tree; `git diff --stat HEAD github/wave/ui-e`
  is EMPTY. The six-leg gate on the checkpoint merged head therefore measures
  exactly the wave tree (same numbers as a wave-head gate).

## STEP REGISTER (wave E upgrades two rows - the first register change since wave A)

`interference-clash` and `exploded-views` were `planned`/`decision`. Wave E
carries a green accepted test for EACH, so both move to `building`/`test`
(the register's honesty rule ceiling), citing the exact test files:

- `interference-clash` <- `crates/oep-app/tests/assy_diagnostics.rs` (UI-E3,
  143 passed / 0 failed on the merged head): the committed clash fixture
  reports EXACTLY the known overlapping pair `i-base` x `i-block`
  (`pairs=1`, vol 750.0 mm3, loc 32.5,25,7.5) and a non-overlapping pair
  reports `pairs=0` (no false positive).
- `exploded-views` <- `crates/oep-app/tests/assy_explode.rs` (UI-E4,
  142 passed / 0 failed on the merged head): factor 1 & 2 move each component
  by exactly `before + factor * offset`, per-component overrides move only
  that component, and the exploded state round-trips byte-identically through
  save / reopen.

JSON + markdown updated together; `scope_register_check` green
(transcript `checkpoint-merged-head/07-scope-register.txt`).

## Full gate on the checkpoint merged head `<CPT>`

Transcripts in `transcripts/checkpoint-merged-head/` (real command output).
Filled below at the gate step.

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | (see 01-fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | (see 02-clippy.txt) |
| `cargo test --workspace` | (see 03-test-workspace.txt) |
| `python3 scripts/licensing-check.py` | (see 04-licensing.txt) |
| `bash scripts/gate/precommit-gate.sh` | (see 05-precommit.txt) |
| `bash scripts/ui/shot.sh` | (see 06-shot.txt) |

## Screenshots (acceptance)

Three real-binary frames from the merged-head `isolith` binary under
`xvfb-run` via the XTEST injector (`ui_e5_inject.c`, `capture_e5.sh`), the
same pattern as `proof/UI-E6` / `proof/UI-D6`: two independent launches per
mode must produce byte-identical PNGs; the `.sha256` is the oracle.

| Shot | sha256 | Shows |
|---|---|---|
| assembly-tree | (filled at capture) | default launch: model tree + assembly tree with base/cap rows, Insert/Ground/Isolate toolbar |
| mate | (filled at capture) | grounded base + a coincident mate added (+ Add mate): solve report `dof=3` and the mate row |
| interference | (filled at capture) | the SAME mated state, diagnostics panel live clash readout: `interference: 1 pair, base x cap (overlap 2902.0 mm3 ...)` - a real overlap detected by `oep_analysis::clash::detect_in_assembly` on the live document |
| explode | HONEST NOTE | the explode panel renders BELOW the fixed-height assembly region's clip in the shipped 1280x800 window: the mates ScrollArea consumes the region's available height and there is no scroll or window resize (verified 1800/2400 tall) that reveals it. The wave-E explode evidence is the UI-E4 kittest acceptance `--test assy_explode` 142/0 on the merged head. No fake frame is rendered. |

## Folding into the checkpoint

This record, the gate transcripts, the screenshots and the wave record
(`docs/status/waves/wave-ui-e.md`) are folded into the single no-ff checkpoint
commit by amending it (`--amend --no-edit`: same parents, code tree untouched,
docs only). The amended commit is the one pushed, PR-merged and tagged. The
landing evidence (PR, main sha, tag, Gitea mirror, `wave/ui-f` advance) is
recorded under `transcripts/landing/` after the landing, on the wave branch,
per the wave-close convention.

## Ancestors

`transcripts/08-ancestors.txt` - see STEP 0 above (5/5).
