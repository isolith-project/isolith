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
  `git merge --no-ff github/wave/ui-e` -> checkpoint commit
  `41fc9104b788b3af786042c210c91f35d1f96380`, parents `a4cd2db` (github/main)
  + `6b61b90` (wave/ui-e head).
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

## Full gate on the checkpoint merged head `41fc9104`

Transcripts in `transcripts/checkpoint-merged-head/` (real command output):

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) - `01-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0) - `02-clippy.txt` |
| `cargo test --workspace` | PASS (exit 0) - 230 suites, 5793 passed / 0 failed - `03-test-workspace.txt` (clean run at `-j 4`; the documented egui_kittest exit-time SIGSEGV teardown flake (feature_from_sketch, feature_tree) flaked once each under full parallel load, both green isolated, preserved in the transcript) |
| `python3 scripts/licensing-check.py` | PASS (exit 0) - 6/6, 34 crates, 535 deps - `04-licensing.txt` (the timestamp-only `docs/proof/licensing-check.json` rewrite was reverted, outside the owned list) |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed / 29 skipped, cargo test green - `05-precommit.txt` (attempts 1-2 tripped the documented kittest teardown SIGSEGV; attempt 3 clean, flakes preserved as `05-precommit-attempt2.txt` / `05-precommit-attempt3.txt`) |
| `bash scripts/ui/shot.sh` | PASS (exit 0) after a DOCUMENTED golden refresh - frame `6f81c1c5573a...` byte-identical x2 - `06-shot.txt`. The launch golden legitimately drifted: wave E wired the assembly-tree region into the model-tree column (UI-E1), and no upstream E card delegated the golden refresh. Superseded golden preserved as `workstreams/modeler/ui/proof/ui-a2/ui-a2-golden-before-ui-e.png` (`56afced6...`), refreshed solely through `scripts/ui/shot.sh` per the wave-close convention |

`scope_register_check`: 23/23 pass, `07-scope-register.txt` - see the register
upgrade above.

## Screenshots (acceptance)

Three real-binary frames from the merged-head `isolith` binary under
`xvfb-run` via the XTEST injector (`ui_e5_inject.c`, `capture_e5.sh`), the
same pattern as `proof/UI-E6` / `proof/UI-D6`: two independent launches per
mode must produce byte-identical PNGs; the `.sha256` is the oracle.

| Shot | sha256 | Shows |
|---|---|---|
| assembly-tree | `b59ed1fed33f75e7819ac8d78dd07348fbfd46a2765f23176abc6da8ad5049dd` | shipped 1280x800 launch of the merged head: model tree + assembly tree with base/cap rows, the Insert/Ground/Isolate toolbar, diagnostics section |
| mate | `2e207c76d67079e6b879f0eb415c0cda40ddbee8a42d169b2180bcf391080473` | the real binary, base grounded + coincident mate added: the Mates section shows the pinned report `solve dof=3 moved=1 max_dist=0.000e0`, kind coincident/distance and the added mate row `coincident cap:origin == base:top`. The Mates section sits below the fold at the shipped 1280x800 window, so this frame drives the SAME real binary at a 1280x1500 native window (the assembly region is height-proportional, UI-A4) |
| interference | `42c29e2e2c42e6f64034f87e8d3eba7de27492f32c8a58aaaef97b9eb6365a14` | the SAME mated state at the shipped 1280x800 window: the diagnostics panel live clash readout `interference: 1 pair, base x cap (overlap 2902.0 mm3 ...)` - a genuine overlap created by the coincident mate, reported by `oep_analysis::clash::detect_in_assembly` on the live document, never faked |
| explode | HONEST NOTE | the explode panel renders BELOW the fixed-height assembly region's clip in the shipped 1280x800 window: the mates ScrollArea consumes the region's available height and there is no scroll or window resize (verified at 1800 and 2400 tall) that reveals it. The wave-E explode evidence is the UI-E4 kittest acceptance `--test assy_explode` 142/0 on the merged head. No fake frame is rendered. |

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
