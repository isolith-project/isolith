# UI-C7 UI-WAVE-C-INTEGRATE: merge, gate, checkpoint, screenshots (proof)

Card: `t_35ad6715`. Branch: `ui/c7-integrate`. Owned files (exactly):

- `workstreams/modeler/ui/README.md` (UI-C7 row)
- `workstreams/modeler/ui/proof/ui-c7/` (this dir)
- `docs/status/waves/wave-ui-c.md` (new, wave record)
- `docs/scope-register.json` (no wave-C test proves a register row -> byte-identical, re-verified green)
- `Cargo.lock` (untouched by this card; the STEP 0 fast-forward did not move it)

No crate was edited (checked: this card's diff touches only the files above plus the
delegated wave-C golden refresh, see below).

## STEP 0

`git fetch github && git merge github/wave/ui-c` fast-forwarded `736a0a7..5318a73`
(branch began at main's wave-A checkpoint `736a0a7`, per the card; the wave head
already contained the wave-B close and all of UI-C1..C6). NOT empty. Transcript:
`transcripts/00-step0.txt`.

## Full gate on the merged head

Transcripts in `transcripts/`, run on the merged head `5318a73`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - 218 targets, aggregate 4126 passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `a7594b124...` |

## Delegated wave-C golden refresh (shot.sh leg)

The `06-shot` leg failed on its first run with frame drift against the wave-B
golden because the merged-head default launch frame changed (wave-C cards added
the tree rows and the C2 empty-state line). This drift is INTENTIONAL and
recorded by the upstream cards themselves: UI-C1, UI-C2 and UI-C5 all wrote "the
golden refresh is the wave-C integrate card's job" in their README rows, with
the exact target hash (`a7594b124...`, captured byte-identical from the merged
head by C2's own `empty.png`). UI-C7 therefore refreshed the golden via the
sanctioned `scripts/ui/shot.sh` path (byte-identical x2, sha256 in `06-shot.txt`)
and preserved the superseded wave-B golden as
`proof/ui-a2/ui-a2-golden-before-ui-c7.png` (same preservation convention UI-B10
used). No test was weakened; the refreshed hash IS the current merged-head frame.

## Screenshots

Three real-binary screenshots captured from the merged-head `isolith` binary
under `xvfb-run` via the XTEST injector (`ui_c7_inject.c`), two independent
launches byte-identical each (sha256 as the determinism oracle):
`capture.sh` mode `tree` / `properties` / `measure`.

| Shot | sha256 | Shows |
|---|---|---|
| tree | `a7594b124ac8ea49c95ef49d2c25106bf37cf309ca808d652e9943bb417586ef` | default merged-head frame: live document tree rows (`edpart (Part)`, `step-shell-1 (Body)`, `step-import (ImportStep)`) + C2 empty state; identical to the refreshed wave-C golden |
| properties | `a1473024020db22c8e00ddaa2fa5825c5c218c0cf117f65f89a447f8df5ec97e` | demo ImportStep row selected: honest "No editable parameters for ImportStep", status "1 selected" |
| measure | `c02c76b30af87455123653f61b392a54ab0804b7066f47ddba52b198046d6ce3` | plain `M` surface open, point p0 + p1 picked: annotation "distance point p0 -> point p1 = 40.000 mm" |

### Screenshot "variables": evidence is the UI-C3 kittest harness (decided)

The C7 screenshot list includes "variables", and no real-binary variables frame
exists: `crates/oep-app/src/panels/variables.rs` is delivered
`#[path]`-includable and is NOT wired into any shell region (the shell still
hosts exactly the five wave-A regions; no `variables::show` call site exists in
`src/`). This card was blocked on that one bullet rather than faking a frame.

ORCHESTRATOR DECISION (2026-11-23): option (b) - the UI-C3 kittest harness is
accepted as the "variables" rendering evidence for wave C. No wiring card is
added in this wave and no crate exception is granted; the wiring step belongs to
a later card that owns the shell. Concretely, the accepted evidence on the same
merged head is `cargo test -p oep-app --test variable_table`: 133 passed / 0
failed, driving the real `variables::show` through `egui_kittest` against a real
`app::App`, including the seeded rows + usage counts, a live expression edit
re-resolving `base 60 -> width 120 -> volume 72000`, and a circular reference
refused whole with the named cycle and unchanged document hash.

The other three shots (tree / properties / measure) remain real-binary frames
captured from the merged head, exactly as required. No fake image was produced
at any point, and the underlying limitation - the panel is unhosted - is
recorded in the wave record and the README row as a known gap for a later wave.

## Register rows

No wave-C test file maps to a `docs/scope-register.json` row (wave-B already
moved the input-contract rows its tests proved; wave-C tests prove editor-panel
behaviour that has no register row). `scope_register_check` stays green
(23/23) on the merged head with the register byte-identical. Transcript:
`transcripts/07-scope-register.txt`.

## Ancestors

Every wave-C card branch is an ancestor of `github/wave/ui-c`
(`ui/c1-feature-tree`, `ui/c2-property-panel`, `ui/c3-variable-table`,
`ui/c4-rollback`, `ui/c5-measure`, `ui/c6-kernel-features` all verified).
Transcript: `transcripts/08-ancestors.txt`.

## Transcripts

- `transcripts/00-step0.txt` - STEP 0 fast-forward `736a0a7..5318a73` (not empty).
- `transcripts/01-fmt.txt` - workspace `cargo fmt --all -- --check`.
- `transcripts/02-clippy.txt` - `cargo clippy --workspace --all-targets -- -D warnings`.
- `transcripts/03-workspace-test.txt` - `cargo test --workspace` (218 targets, 4126 passed / 0 failed).
- `transcripts/04-licensing.txt` - `python3 scripts/licensing-check.py`.
- `transcripts/05-precommit.txt` - `bash scripts/gate/precommit-gate.sh`.
- `transcripts/06-shot.txt` - `bash scripts/ui/shot.sh` (golden refreshed to the wave-C frame).
- `transcripts/07-scope-register.txt` - `scope_register_check` green (23/23).
- `transcripts/08-ancestors.txt` - every wave-C branch = ancestor of origin/wave/ui-c.
- `transcripts/09-step0-merge.txt` - the actual STEP 0 command output.
