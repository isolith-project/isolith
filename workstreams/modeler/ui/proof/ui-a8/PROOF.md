# UI-A8 UI-WAVE-A-INTEGRATE proof

Card: UI-A8 - merge wave A into main through one checkpoint, run the full gate on
the merged head, tag, and attach the three screenshots (shell, palette, viewport).

Status: BLOCKED (needs_input). The full gate is NOT green on the merged head and
the failing leg cannot be fixed within this card's FILES YOU OWN list.

## STEP 0 (done)

```
git fetch github
git merge github/wave/ui-a     # remote is named `github`, not `origin`, in this worktree
```
Brought the wave head into `ui/a8-integrate` (fast-forward to `d4f5e03`); STEP 0
meant "bring the wave code into the tree" and did. License check at that point
failed the crate-boundary leg (oep-ui-theme missing row) exactly as the addendum
predicted; see 02-licensing below.

## Licensing boundary fix (addendum, owned)

`UI-A3` added `crates/oep-ui-theme` to the workspace without a boundary row, so
the crate-boundary leg failed. Added the `oep-ui-theme` row as Apache-2.0 to
`docs/licensing-boundary.json` and `docs/licensing-boundary.md`. The honest crate
count is now 34 (UI-A1 already carried oep-app to 33; oep-ui-theme is the 34th).
`license_map_total` Apache-2.0 21 -> 22, AGPL-3.0 12. `docs/proof/licensing-check.json`
was regenerated (the addendum's own check report, committed alongside).

```
licensing check: total=6 pass=6 fail=0 | exit=0
  [PASS] crate-boundary: 34 crates match the boundary map
  [PASS] boundary-doc-sync: 34 rows in sync
```
Verified on the wave head dd4725f and on the merged head 3588429.

The fix was landed on `wave/ui-a` as `merge-UI-A8` (dd4725f), so the licensing
leg is green on the wave head before the checkpoint - the merged head inherits it.

## Checkpoint merge (acceptance #2)

```
git merge --no-ff github/wave/ui-a -m "checkpoint(wave-ui-a): walking skeleton -- cards UI-A1..UI-A9"
```
Result: `3588429` on local `main`, parents `8760cfe` (main) and `dd4725f` (wave head).
One checkpoint commit, no fast-forward. `github/main` has NOT been pushed (the
gate is not green, so main stays untouched - see below).

## Ancestor audit (acceptance #1) - PASS

`workstreams/modeler/ui/proof/ui-a8/audit_ancestors.sh` (real output in
transcripts/00-ancestors.txt):
```
ANCESTOR ok : ui/a1-deps      is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a2-harness   is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a3-theme     is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a4-layout    is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a5-viewport  is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a6-palette   is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a7-open-save is an ancestor of github/wave/ui-a
ANCESTOR ok : ui/a9-contrast  is an ancestor of github/wave/ui-a
```
Every wave-A card proof dir (ui-a1..ui-a9 except ui-a8 itself, which this card
produces) is present on the wave head. No wave-A card is missing a proof.

## Full gate on merged head 3588429

| # | Command | Exit | Transcript |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | 0 | 01-fmt.txt |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 02-clippy.txt |
| 3 | `cargo test --workspace` | 0 | 03-test.txt |
| 4 | `python3 scripts/licensing-check.py` | 0 (6/6, 34 crates) | 04a-licensing.txt |
| 5 | `bash scripts/gate/precommit-gate.sh` | 1 (pytest: 2 failed) | 05-precommit-gate.txt |
| 6 | `bash scripts/ui/shot.sh` | 0 (e60b86dc = UI-A9 golden) | 06-shot.txt |

**The full gate is NOT green: leg 5 fails.**

### Failing leg 5 in detail

```
[FAIL] pytest
   FAILED tests/test_brand_identity.py::test_retired_codename_absent_outside_allowed_paths
   FAILED tests/test_contributor_rules_decision.py::test_contributor_rules_decision_acceptance
   2 failed, 554 passed, 29 skipped
== gate FAIL ==
```

**Failure A - REAL, wave-introduced, CI-reproducible:**
`tests/test_contributor_rules_decision.py::test_contributor_rules_decision_acceptance`
fails on the merged head with exactly:
```
CODEOWNERS missing row for workspace member crates/oep-ui-theme (want 'crates/oep-ui-theme/.*')
```
Root cause: UI-A3 added `crates/oep-ui-theme` to the workspace (root `Cargo.toml`
members + `crates/oep-ui-theme/`) but never added the CODEOWNERS row that the
P0-M1-T04 contributor-rules acceptance requires. CODEOWNERS carries rows for
oep-app and oep-shell but not oep-ui-theme. Reproduces on a clean `git archive`
export of the merged head (transcripts/07-clean-export-merged.txt), so CI would
fail it too. This is a wave-A defect caught at the wave gate; the contributor-rules
test was never run by any earlier wave-A proof.

The fix would be two lines beside the oep-app rows (~line 98 of CODEOWNERS):
```
crates/oep-ui-theme/.* @don
/crates/oep-ui-theme/ @don
```
**CODEOWNERS is NOT in this card's FILES YOU OWN list.** The card states that
editing a file outside the list fails review even if the gate is green. UI-A8
therefore must not touch it.

**Failure B - environmental only, not wave-introduced, not on a clean CI tree:**
`tests/test_brand_identity.py::test_retired_codename_absent_outside_allowed_paths`
fails only when the gate runs inside the main worktree, because sibling, untracked
`.worktrees/` directories (t_ac965332, t_99963f11, ...) are materialized on disk
and contain retired-codename content; `.worktrees` is not in that test's SKIP_DIRS.
Verified: on a clean `git archive` export of the same merged head the test PASSES
(4 passed - transcripts/07-clean-export-merged.txt). `.worktrees/` is not tracked
(`git ls-files .worktrees/` empty), so the committed merged head is unaffected and
CI runs on a fresh checkout. Not a wave defect.

### Screenshots (acceptance #5 engine, ADDENDUM 2) - PASS, all show separated panels

Captured from the real merged-head binary under xvfb-run via an XTEST injector
(source + build in this proof dir), double-captured byte-identical (sha256
oracle), same discipline as shot.sh:

| Shot | sha256 | Shows |
|---|---|---|
| `shell.png` | `e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4` | full shell, byte-identical to UI-A9 golden; five separated regions (raised fill + hairline per region) |
| `palette.png` | `9d63ef12ac0d81a3cef52791c0c7e69ce6fd3d653f01b9c23fcf61423f9a7d7c` | Ctrl+K palette open over the separated shell |
| `viewport.png` | `f7cc1e582b56f97f67d2cf968f0b31dead06c99bf712866595088d47183234e1` | model orbited in the viewport, separated panels intact |

Confirmed visually (vision review, transcripts/common): the three screenshots
show the separated panels exactly as ADDENDUM 2 requires.

## Scope register (acceptance #6)

`docs/scope-register.json` history tail (checked on the wave): last touched by
older main commits (practitioner input contract / editor parity baseline), NO
wave-A commit touches it. The checkpoint merge diff for it is empty. See
transcripts/08-merged-head.txt for the merged head sha.

## Why this card is blocked

Acceptance #3 ("full gate green on the merged head, transcript in proof") cannot
be met: `bash scripts/gate/precommit-gate.sh` fails on a wave-A defect (UI-A3
omitted the `oep-ui-theme` CODEOWNERS row) and the file that must change,
`CODEOWNERS`, is not in this card's FILES YOU OWN list. The card rules are
explicit: out-of-list edits fail review even when the gate is green; a blocked
card is a valid outcome; a faked green is not. The checkpoint merge exists only
on local `main`; remote `github/main` is untouched and the annotated tag has NOT
been pushed, so no partial wave is merged.

Action needed (needs_input): decide who owns the CODEOWNERS row for
`crates/oep-ui-theme` - either (a) reopen UI-A3 / a small follow-up wave-A card
to add the two CODEOWNERS lines, or (b) explicitly grant `CODEOWNERS` to this
card's file list so UI-A8 can add them and re-run the gate.
