# Wave ui-c status record

Wave C of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`, section 5): model tree, properties, variables. Branch `wave/ui-c` cut from main at `736a0a7`.

Written by card UI-C7 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-C1 | UI-FEATURE-TREE: real document tree | done |
| UI-C2 | UI-PROPERTY-PANEL: per-feature parameter editor | done |
| UI-C3 | UI-VARIABLE-TABLE: units, expressions, usage count | done |
| UI-C4 | UI-ROLLBACK: suppress and roll the tree | done |
| UI-C5 | UI-MEASURE: distance, angle, radius | done |
| UI-C6 | UI-KERNEL-FEATURES: fillet, chamfer, shell, boolean dialogs | done |
| UI-C7 | UI-WAVE-C-INTEGRATE: merge, gate, checkpoint, screenshots | done |

Every wave-C card branch is an ancestor of the wave head (audit in `proof/ui-c7/transcripts/08-ancestors.txt`), and every card's proof dir is present on the wave head.

## Head sha

- Wave head: `bc69f8b` (`merge-UI-C7`, the gate proof + decision record; contains all wave-C cards UI-C1..UI-C6 plus the UI-C7 record; the pre-record wave head was `5318a73`, fast-forwarded `736a0a7..5318a73`).
- Checkpoint merge commit: `c5aea6d`, parents `1e3b5dd` (`main`) + `bc69f8b` (wave head). `main` was an ancestor of the wave head, so the checkpoint code tree is byte-identical to the wave head. The full six-leg gate was re-run on that checkpoint merged head and passed (transcripts `proof/ui-c7/transcripts/checkpoint-merged-head/`).
- Landing: PR #31 merged as `6275787` (`github/main` fast-forwarded `1e3b5dd..6275787`), annotated tag `wave-ui-c` (`88bc91d`) -> checkpoint `c5aea6d`, Gitea mirror fast-forwarded to `6275787` with the tag, and `github/wave/ui-d` created at `6275787`. Transcripts `proof/ui-c7/transcripts/landing/`.
- Wave closes by the UI-C7 record landing on the wave branch and one no-ff checkpoint commit carrying it into main (per the wave-B close procedure); annotated tag `wave-ui-c` pushed on github/main.
- The UI-C7 record (this file, the README row, `proof/ui-c7/`) lands on the wave branch so the single checkpoint merge carries it into main.

## Full gate on the merged head

Transcripts in `workstreams/modeler/ui/proof/ui-c7/transcripts/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - 218 targets, aggregate 4126 passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `a7594b124...` |

The shot.sh leg required the delegated wave-C golden refresh (UI-C1/C2/C5 recorded "the golden refresh is the wave-C integrate card's job"): the merged-head default frame `a7594b124...` (byte-identical to UI-C2's `empty.png`) was set as the new golden via the sanctioned `scripts/ui/shot.sh` path, two byte-identical launches each, and the superseded wave-B golden preserved as `proof/ui-a2/ui-a2-golden-before-ui-c7.png`.

## Register rows

The card rule allows `docs/scope-register.json` changes only for rows the wave's tests actually prove. No wave-C test maps to a scope-register row (wave-B already moved the input-contract rows its tests proved; wave-C tests prove editor-panel behaviour that has no register row), so the register is byte-identical and `scope_register_check` stays green (23/23).

## Screenshots (acceptance #4)

Three real-binary screenshots of the merged-head binary captured under `xvfb-run` via the XTEST injector (`proof/ui-c7/ui_c7_inject.c` + `capture.sh`), two independent launches byte-identical each:

| Shot | sha256 | Shows |
|---|---|---|
| tree | `a7594b124ac8ea49c95ef49d2c25106bf37cf309ca808d652e9943bb417586ef` | default merged-head frame: live document tree + C2 empty state |
| properties | `a1473024020db22c8e00ddaa2fa5825c5c218c0cf117f65f89a447f8df5ec97e` | ImportStep selected, honest "No editable parameters for ImportStep", status "1 selected" |
| measure | `c02c76b30af87455123653f61b392a54ab0804b7066f47ddba52b198046d6ce3` | measure surface: "distance point p0 -> point p1 = 40.000 mm" |

**Variables screenshot: evidence is the UI-C3 kittest harness (orchestrator decision, option b, 2026-11-23).** No real-binary variables frame is producible: the `variables.rs` panel is `#[path]`-includable and NOT wired into a shell region (no `variables::show` call site in `src/`), UI-C3 and UI-C6 both documented that a wiring step is required before a "variables" screenshot, and UI-C7's owned files exclude all crates while the card forbids editing a crate. UI-C7 blocked on that single bullet rather than faking a frame. The orchestrator accepted the UI-C3 kittest harness as the wave-C "variables" evidence (`cargo test -p oep-app --test variable_table`: 133 passed / 0 failed on the merged head, real `show` driven through `egui_kittest` with a real `app::App`), declined a sibling wiring card for this wave and declined a crate exception. Hosting the panel is a known gap for a later wave; the three wired panels (tree / properties / measure) are real-binary frames.

## What is not done

- The variables table and the kernel-features dialogs (UI-C3/UI-C6) remain `#[path]`-includable and unhosted: hosting is a later card that owns the shell, and until then no real-binary "variables" frame can exist. Per the orchestrator decision recorded above, the UI-C3 kittest harness (133 tests incl. the real `show` driven through egui_kittest with a real `app::App`) is the accepted "variables" evidence on the merged head.
- Assembly, drafting, PDM and GPU viewport work are later waves (E, F, G, D) per the plan.
- The `wave/ui-c` branch is left in place after the checkpoint so the wave history and record are preserved.
- No UI tests were weakened, skipped or deleted to reach green; the single gate difference (golden drift) was the INTENTIONAL wave-C frame the C1/C2/C5 cards delegated to the integrate card, refreshed via the sanctioned path.
