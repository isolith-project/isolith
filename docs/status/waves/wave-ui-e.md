# Wave ui-e status record

Wave E of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`,
section 5): the assembly lane - assembly tree, live-solver mates, diagnostics
(degrees-of-freedom + interference), exploded views - plus the UI-E6 navigation
cube. Branch `wave/ui-e` carries all of UI-E1..UI-E6 on top of `main`
(`a4cd2db`, the wave-ui-d checkpoint).

Written by card UI-E5 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-E1 | UI-ASSY-TREE: components, insert, ground, isolate | done |
| UI-E2 | UI-ASSY-MATES: mate tool against the live solver | done |
| UI-E3 | UI-ASSY-DIAGNOSTICS: dof readout and interference panel | done |
| UI-E4 | UI-ASSY-EXPLODE: exploded view control | done |
| UI-E6 | UI-NAV-CUBE: 3D orientation cube beside the existing button panel | done |
| UI-E5 | UI-WAVE-E-INTEGRATE: merge, gate, checkpoint, screenshots, tag | done |

Every wave-E card branch (the four assembly feature branches plus the UI-E6
nav-cube branch) IS an ancestor of the wave head (audit
`proof/UI-E5/transcripts/08-ancestors.txt`, 5/5): `ui/e1-assy-tree`,
`ui/e2-assy-mates`, `ui/e3-assy-diagnostics`, `ui/e4-assy-explode`,
`ui/e6-nav-cube`.

## Head sha

- Wave head: `1a1c0f1` (`merge-UI-E4`, containing all wave-E cards UI-E1..UI-E6).
- STEP 0 of the close card merged `github/wave/ui-e` into the card worktree
  (`ui/e5-integrate`) - the branch had been parked at a stale wave-C landing
  (`6275787`), so the merge was NOT empty (real first-run output in
  `proof/UI-E5/transcripts/00-step0-merge.txt`).
- `main` (`a4cd2db`) IS an ancestor of the wave head, so the checkpoint merge
  tree is byte-identical to the wave tree; `git diff --stat` HEAD vs
  `github/wave/ui-e` is empty. The six-leg gate therefore measures the same
  tree as the checkpoint merged head.
- Landing: PR #42 merged as `a68505da10107ad8ff8756cf242a83a82591f6d7`
  (`github/main`), annotated tag `wave-ui-e`
  (`166e0b05488f52bc3d5381fd6d562a16bfcfbb38`, peeled commit
  `5f8d9db35ab70670ac22bd87806ef23d2679e1fd`) -> checkpoint
  `5f8d9db35ab70670ac22bd87806ef23d2679e1fd`, Gitea
  mirror fast-forwarded to `a68505d` with the tag (verified by read-back), and
  `github/wave/ui-f`
  created at `a68505d`. Transcripts
  `proof/UI-E5/transcripts/landing/`.
- The UI-E5 record (this file, the README row, `proof/UI-E5/`) is folded into
  the single checkpoint commit and re-committed onto the wave branch, per the
  wave-close convention.

## Full gate on the checkpoint merged head

Checkpoint merged head: `41fc9104b788b3af786042c210c91f35d1f96380`, parents
`a4cd2db` (github/main) + `6b61b90` (wave/ui-e head). Transcripts in
`workstreams/modeler/ui/proof/UI-E5/transcripts/checkpoint-merged-head/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - 230 suites, aggregate 5793 passed / 0 failed (clean run at `-j 4`; one full-parallel run tripped the documented egui_kittest exit-time SIGSEGV teardown flake in `feature_from_sketch` + `feature_tree`, both green isolated, note in `03-test-workspace.txt`) |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed / 29 skipped; cargo test green (the documented kittest teardown flake surfaced on attempts 1-2 and re-ran clean on attempt 3; flake transcripts preserved as `05-precommit-attempt{2,3}.txt`) |
| `bash scripts/ui/shot.sh` | PASS (0) after a DOCUMENTED golden refresh - frame `6f81c1c5...`, byte-identical x2. The launch golden legitimately drifted: wave E wired the assembly-tree region into the model-tree column (the shot frame shows the shell), and no upstream E card delegated the refresh. Superseded golden preserved as `ui-a2-golden-before-ui-e.png`, refreshed solely through `scripts/ui/shot.sh` per the wave-close convention |

`scope_register_check`: 23/23 (`proof/UI-E5/transcripts/07-scope-register.txt`).
Unlike every wave before it, wave E UPGRADES two register rows, because a
wave-E green accepted test genuinely proves each:

- `interference-clash` -> `building` (evidence kind `test`, ref
  `crates/oep-app/tests/assy_diagnostics.rs`): UI-E3 proves the panel reports
  EXACTLY the known overlapping pair of the committed clash fixture
  (`i-base` x `i-block`, vol 750.0 mm3, loc 32.5,25,7.5) and no false positive
  on a non-overlapping pair.
- `exploded-views` -> `building` (evidence kind `test`, ref
  `crates/oep-app/tests/assy_explode.rs`): UI-E4 proves factor + per-component
  offsets move each component by exactly `before + factor * offset`, a
  per-component override moves only that component, and the exploded state
  round-trips byte-identically through save / reopen.

Both rows were `planned`/`decision`; the register JSON and markdown were
updated together and `scope_register_check` stays green.

## Screenshots (acceptance)

| Shot | sha256 | Shows |
|---|---|---|
| assembly-tree | `b59ed1fed33f75e7819ac8d78dd07348fbfd46a2765f23176abc6da8ad5049dd` | shipped 1280x800 launch: model tree + assembly tree with base/cap rows and the Insert/Ground/Isolate toolbar, diagnostics section |
| mate | `2e207c76d67079e6b879f0eb415c0cda40ddbee8a42d169b2180bcf391080473` | grounded base + a coincident mate added through the real binary: the Mates section shows the pinned report `solve dof=3 moved=1 max_dist=0.000e0`, kind coincident/distance and the added mate row `coincident cap:origin == base:top`. The Mates section sits below the fold at the shipped 1280x800 window, so this frame drives the SAME real binary at a 1280x1500 native window (assembly region is height-proportional, UI-A4) |
| interference | `42c29e2e2c42e6f64034f87e8d3eba7de27492f32c8a58aaaef97b9eb6365a14` (see `interference.png.sha256`) | the SAME mated state at the shipped 1280x800 window: the diagnostics panel live clash readout `interference: 1 pair, base x cap (overlap 2902.0 mm3 ...)` - a genuine overlap created by the coincident mate, detected by `oep_analysis::clash::detect_in_assembly` on the live document, never faked |
| explode | HONEST NOTE | the explode panel renders BELOW the fixed-height assembly region's clip in the shipped 1280x800 window: the mates ScrollArea consumes the region's available height and there is no scroll or window resize (verified 1800/2400 tall) that reveals it. The wave-E explode evidence is the UI-E4 kittest acceptance `--test assy_explode` 142/0 on the merged head. No fake frame is rendered. |

No image was fabricated; each real-binary frame is a genuine launch of the
merged-head binary driven through XTEST and is deterministic (byte-identical
across two independent captures, sha256 oracle in proof).

## What is not done / deferred

- The explode PANEL is shipped and test-proven (UI-E4), but it cannot produce
  a real-binary frame at the shipped 1280x800 window because the fixed-height
  assembly region clips it below the fold with no scroll reach (a layout
  consequence of the mates ScrollArea taking the available region height).
  Recorded as a finding for a future card; the accepted wave-E evidence for
  the explode surface is the UI-E4 kittest harness.
- The wave-C variables / kernel-features hosting gaps are unchanged by wave E.
- The `wave/ui-e`, `merge-UI-E5`, checkpoint and card branches are left in
  place so the wave history and record are preserved.
- No UI test was weakened, skipped or deleted to reach green.
