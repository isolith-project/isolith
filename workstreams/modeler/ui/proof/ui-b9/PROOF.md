# UI-B9 UI-WAVE-B-INTEGRATE proof

Card: UI-B9 — close wave B: merge the wave into main through one checkpoint
commit, run the full gate on the merged head, tag the wave, move the four
practitioner rows to building with test evidence, and attach the three
screenshots (draw, snap, dimension).

Status: DONE.

## STEP 0

Remote is named `github` (not `origin`) in the Isolith worktrees; the wave
branch lives on it.

```
git fetch github
git merge github/wave/ui-b
```

The merge was NOT empty: it brought the UI-B10 gate fixes (19 files, 2329
insertions) into the integration tree — the `input.rs` trailing-blank
correction and the UI-A2 launch-frame golden refresh that clear the two
defects attempt 1 of this card found RED on the previous merged head `8da7347`.
Wave-head-relative history and hashes are recorded below.

### Attempt-1 block resolution

Attempt 1 blocked (2026-11-21) because the full gate was RED on merged head
`8da7347` from two pre-existing wave defects in files UI-B9 may not touch:
`crates/oep-app/src/input.rs:504` trailing blank (UI-B1) failing fmt, and the
stale UI-A2 golden failing shot.sh after UI-B7 changed the launch frame. The
card comments requested a fix card rather than editing outside the owned set.
UI-B10 `t_7bd53d02` (parent-linked to this card) landed on the wave owning both
corrections. This run reruns STEP 0 against the live wave head and executes the
original full-gate and checkpoint acceptance unchanged. No ownership exception
and no test weakening was granted.

## Ancestor audit (acceptance #1) — PASS

Real output in `transcripts/00-ancestors.txt` over GitHub refs:

```
wave head: 98a8ccc08270b36fa7e1c420d46c4acbaee40613
ANCESTOR ok : ui/b1-input-router
ANCESTOR ok : ui/b2-history
ANCESTOR ok : ui/b3-sketch-enter
ANCESTOR ok : ui/b4-sketch-draw
ANCESTOR ok : ui/b5-snap-inference
ANCESTOR ok : ui/b6-constraints
ANCESTOR ok : ui/b7-dimensions
ANCESTOR ok : ui/b8-feature-from-sketch
ANCESTOR ok : ui/b10-gate-fix
+ every card's landing merge commit
```

Every wave-B card branch is an ancestor of the wave head; every card's proof
dir is present on the wave head.

## Checkpoint merge (acceptance #2)

One no-ff merge of `github/wave/ui-b` into `main`, exact message:

```
git merge --no-ff github/wave/ui-b \
  -m "checkpoint(wave-ui-b): sketch tools -- cards UI-B1..UI-B8"
```

Parents of the checkpoint merge: `736a0a7` (main before the wave) and the
final wave head. No fast-forward; one checkpoint commit. The checkpoint commit
is the object of annotated tag `wave-ui-b` (`git rev-parse wave-ui-b^{}`), and
`github/main` is pushed to it. The UI-B9 record (this proof, the README row,
`docs/status/waves/wave-ui-b.md`) landed on the wave branch first, so the
single checkpoint merge carries it into main.

## Full gate on the merged head (acceptance #3) — PASS

Transcripts in `transcripts/`: `01-fmt.txt`, `02-clippy.txt`, `03-test.txt`,
`04-licensing.txt`, `05-precommit-gate.txt`, `06-shot.txt`, run on the merged
head (wave head + UI-B10 fixes) after the merge.

| # | Command | Exit | Notes |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | 0 | clean (UI-B10 removed the trailing blank) |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| 3 | `cargo test --workspace` | 0 | aggregate 2846 passed / 0 failed |
| 4 | `python3 scripts/licensing-check.py` | 0 | 6/6 legs, 34 crates |
| 5 | `bash scripts/gate/precommit-gate.sh` | 0 | PASS: pytest 558 passed, 29 skipped; cargo test ok |
| 6 | `bash scripts/ui/shot.sh` | 0 | frame `2f20383c…1047` byte-identical to UI-B10-refreshed golden |

The shot.sh drift guard passing against the B10-refreshed golden is the
requirement: a drift on the merged head would have been a finding, not a hash
to overwrite. There was none — the wave-B frame is now the committed golden.

## Practitioner rows → building (acceptance #6) — PASS

`docs/scope-register.json` moves four practitioner rows to `building` with
wave-B test files as evidence:

| Row | Test evidence |
|---|---|
| input-snap-inference | `crates/oep-app/tests/snap_inference.rs` |
| input-escape-semantics | `crates/oep-app/tests/sketch_draw.rs` (+ `sketch_enter.rs`) |
| input-right-click-wheel | `crates/oep-app/tests/constraints.rs` |
| input-datum-plane-legibility | `crates/oep-app/tests/sketch_enter.rs` |

`python/oep_tooling/scope_register_check.py --register docs/scope-register.json
--markdown docs/scope-register.md` is green (23/23, exit 0); transcript
`08-scope-register.txt`. The markdown sync file `docs/scope-register.md` is
updated in lockstep because the checker enforces "row id + status in the same
table line" — the md edit is the check's own drift guard, not a gratuitous
file.

## Screenshots (acceptance #7) — PASS

Captured from the real merged-head binary under `xvfb-run` via the XTEST
injector (`capture_ui.sh`, `ui_b9_inject.c`), the same determinism discipline
as shot.sh (two independent app launches, byte-identical sha256 oracle; the
.gsha256 file equals the PNG hash, verified):

| Shot | sha256 | Shows |
|---|---|---|
| `draw.png` | `91712ad1b9b53fb40b98d234e8b1034e1a1eb1f0d920a02885e141d2e5273a58` | XY sketch plane, Rectangle tool: three corners + rubber band + live `W x H` readout |
| `snap.png` | `5042f159ab9ee3d26e7c89dc9c43abd994df5c57abb02e665094a5ecab736346` | Line drawn, pointer near endpoint: square glyph + `Endpoint` label |
| `dimension.png` | `5bc2c55c1a59c46de479fd61c597cf01061b6662118e1e871fbce01448de01e2` | Properties dimension editor: `width*1.5` expression + Apply + driven volume |

Visual verification (byte-level + this proof's pixel review): draw shows the
rectangle rubber band and dimension readout; snap shows the endpoint glyph and
label on the merged-head binary; dimension shows the B7 dimension editor state.

## Wave record

`docs/status/waves/wave-ui-b.md` (new) records the wave: cards, wave head sha,
gate history (both attempts), the four register rows moved, the gate output,
and what is not done. It is committed together with this proof and the README
row, and carried into `main` by the single checkpoint merge, so main's
checkpoint commit holds the complete wave record.

## Landing

Per the card's landing step the UI-B9 record landed on `wave/ui-b`
(`merge-UI-B9`), then the checkpoint merged `origin/wave/ui-b` into main in one
no-ff commit; the branch `checkpoint/wave-ui-b-<sha>` was pushed, opened as a
pull request into main, and merged with `gh pr merge --merge`; `wave-ui-b` was
re-tagged at the checkpoint commit and the tag pushed; the Gitea mirror was
fast-forwarded (`git push gitea main:main`); and the next wave's branch was
advanced (`git push github main:refs/heads/wave/ui-c`). Acceptance command
transcripts are in `transcripts/`.

## Environment note: GitHub push identity

`github/wave/ui-b` lives on `github` (GitHub) and pushes to
`isolith-project/isolith` must run as **dmltallen** — the `gh`-active account
`redonealpha-don` has no write (403, recorded in UI-B10 proof). The scoped
credential helper `git-credential-dmltallen` in this proof dir (a per-command
`-c credential.https://github.com.helper=!…` override) answers as dmltallen
without switching the host-global active account, so concurrent workers are
not raced. The token is read from the keyring at call time; it never appears on
a command line or in a file.
