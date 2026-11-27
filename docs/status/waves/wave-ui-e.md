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
- Landing: PR #<PR> merged as `<MAIN>` (`github/main`), annotated tag `wave-ui-e`
  (`<TAG>`) -> checkpoint `<CPT>`, Gitea mirror fast-forwarded to `<MAIN>`
  with the tag, and `github/wave/ui-f` created at `<MAIN>`. Transcripts
  `proof/UI-E5/transcripts/landing/`.
- The UI-E5 record (this file, the README row, `proof/UI-E5/`) is folded into
  the single checkpoint commit and re-committed onto the wave branch, per the
  wave-close convention.

## Full gate on the checkpoint merged head

Transcripts in `workstreams/modeler/ui/proof/UI-E5/transcripts/checkpoint-merged-head/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - <SUITES> suites, aggregate <PASS> passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6 |
| `bash scripts/gate/precommit-gate.sh` | PASS - <PRECOMMIT> |
| `bash scripts/ui/shot.sh` | PASS (0) - frame <SHOT>, byte-identical x2, <DRIFT> |

`scope_register_check`: <N>/<N> (`proof/UI-E5/transcripts/07-scope-register.txt`).
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
| assembly-tree | <A>/<B> | default launch of the merged head: model tree + assembly tree (base/cap rows, Insert/Ground/Isolate toolbar), the wave-E surfaces |
| mate | <A>/<B> | a coincident mate added through the real binary (grounded base, + Add mate, solve dof=3) with the report line |
| interference | <A>/<B> | the diagnostics panel INTERFERENCE readout after the same mate: real clash analysis detects `1 pair, base x cap (overlap 2902.0 mm3 ...)` - genuine overlap, live from `oep_analysis::clash::detect_in_assembly` |
| explode | <A>/<B> | honest note: the explode panel sits BELOW the fixed-height assembly region's clip in the shipped 1280x800 window (the mates scroll area consumes the available region height, so there is no scroll or resize that reveals it - verified at window heights 1800/2400). The wave-E explode evidence is the kittest acceptance `--test assy_explode` 142/0 on the merged head (real app::App, AccessKit slider SetValue + offset DragValues), committed under `proof/UI-E4/`. No fake frame is rendered. |

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
