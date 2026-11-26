# Wave ui-d status record

Wave D of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`, section 5): GPU viewport, shading, picking, navigation, and the render-seam performance fix. Branch `wave/ui-d` cut from main at `6275787`.

Written by card UI-D6 (wave integration) at wave close.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-D1 | UI-GPU-BACKEND: wgpu viewport behind the seam, shared device with egui | done |
| UI-D2 | UI-GPU-SHADING: studio lighting, edges, flat/smooth, transparency | done |
| UI-D3 | UI-PICK: id buffer picking, hover pre-highlight, box select | done |
| UI-D7 | UI-RENDER-FIX: correct the clip_w projection and guard empty face buffers | done |
| UI-D4 | UI-NAV: view cube, standard views, section planes | done |
| UI-D8 | UI-VEC4-PIPELINE: one homogeneous clip-space path, transform in the vertex stage | done |
| UI-D9 | UI-PERF-PROFILE: per-stage frame cost split and the render-seam performance ADR | done |
| UI-D10 | UI-RESIDENT-SCENE: keep geometry on the GPU, cull per primitive, re-upload only dirty meshes | done |
| UI-D5 | UI-PERF: interactive frame budget on U1, recorded numbers | done |
| UI-D12 | UI-GATE-FIX: make the wave-D head pass its own gate | done |
| UI-D6 | UI-WAVE-D-INTEGRATE: merge, gate, checkpoint, screenshots, tag | done |

Every wave-D card branch (the 9 feature cards plus the UI-D12 gate-fix branch)
is an ancestor of the wave head (audit in
`proof/UI-D6/transcripts/08-ancestors.txt`, 10/10), and every card's proof dir
is present on the wave head.

## Head sha

Final wave-D state (what `main` carries after the close; the landing is recorded
verbatim in `proof/UI-D6/transcripts/00-landing.txt`):

- Wave head (merged, final): `b6176aa` on `wave/ui-d`, containing all wave-D
  cards UI-D1..UI-D10 plus UI-D5's merged-head re-prove, the UI-D12 gate fix,
  and the UI-D6 wave-close record (this file, the README row, `proof/UI-D6/`).
- Checkpoint merge commit: `16e58f4`, a single no-ff merge with parents
  `c1e37ec` (`github/main`, which carries PR #40 CI-GPU-ICD) and `b6176aa`
  (`wave/ui-d` final head). `main` moved after the first checkpoint attempt
  (CI-GPU-ICD landed), so the checkpoint was rebuilt on the new main; the
  checkpoint tree differs from the wave head only by main's two post-wave-C
  docs PRs and the two CI workflow files, none of which the six-leg gate
  touches. The checkpoint was amended (53a29a9 -> 16e58f4, docs-only, parents
  unchanged) to fold the fresh checkpoint-head gate transcripts
  (`proof/UI-D6/transcripts/checkpoint-merged-head-53a29a9/`). The full
  six-leg gate was run on the checkpoint merged head and passed.
- Landing: PR #39 (head checklist c2948c7) closed as superseded; PR #41 from
  `checkpoint/wave-ui-d-16e58f4`, with the DCO sign-off line in the body, went
  green (dco-sign-off PASS; integration and quality-gate PASS with the software
  Vulkan adapter from PR #40, no gpu lock PoisonError) and was merged via
  `gh pr merge 41 --merge` -> main `a4cd2db`. Main's tree == checkpoint
  `16e58f4` tree. Annotated tag `wave-ui-d` -> `c5deb5d` (peels to `16e58f4`),
  pushed to github and gitea. Gitea mirror fast-forwarded to `a4cd2db`.
  `github/wave/ui-e` created at `a4cd2db`.
- STEP 0 of the close card fast-forwarded nothing (the card worktree merged the
  new head `39425ee`); attempt 1's block on the pre-fix head `1097f76` was
  resolved by UI-D12 (see `proof/UI-D6/PROOF.md` for the full history).
- The UI-D6 record (this file, the README row, `proof/UI-D6/`) is folded into
  the single checkpoint commit and re-committed onto the wave branch, per the
  wave-close convention.

## Full gate on the checkpoint merged head

Transcripts in `workstreams/modeler/ui/proof/UI-D6/transcripts/checkpoint-merged-head/`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0) |
| `cargo test --workspace` | PASS (0) - 225 suites, aggregate 4608 passed / 0 failed |
| `python3 scripts/licensing-check.py` | PASS (0) - 6/6, 34 crates, 535 deps |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green (three earlier runs flaked on the documented egui_kittest exit-time SIGSEGV class and one on a pytest interpreter miss - all preserved as `05-precommit-flake{1,2,3}.txt`, clean run in `05-precommit.txt`) |
| `bash scripts/ui/shot.sh` | PASS (0) - frame `56afced6...`, byte-identical x2, NO drift (golden already refreshed by UI-D12) |

`scope_register_check`: 23/23, `docs/scope-register.json` and `.md`
byte-identical (`07-scope-register.txt`) - no wave-D test proves a register row.

### Re-gate on the merged head `f628f99`

After the checkpoint gate, the wave head moved once more (the integrate merge
`f628f99` folded the UI-D6 record, README row, screenshots and prior transcripts
onto the wave branch). The full six-leg gate was re-run on that final merged
head and is green, real output committed alongside this record as
`proof/UI-D6/gate-{fmt,clippy,test,licensing,precommit,shot}.txt`:

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | exit 0 - 4608 passed / 0 failed (one documented egui_kittest SIGSEGV flake in `feature_from_sketch`, isolated 181/0 + full re-run green, noted in `gate-test.txt`) |
| `python3 scripts/licensing-check.py` | exit 0 - 6/6 pass |
| `bash scripts/gate/precommit-gate.sh` | exit 0 - `gate PASS`, pytest 559 passed / 28 skipped, cargo test ok (ran with the sanctioned venv for the pytest interpreter) |
| `bash scripts/ui/shot.sh` | exit 0 - frame `56afced64733...` byte-identical to the committed golden, NO drift |

The re-gate transcripts are the proof this wave record stands on: the same six
commands, on the commit this record is committed beside.

## Gate-fix facts (the reusable lesson, UI-D12)

The wave head did not reach its own gate in one shot - four legs were red on
the pre-fix head and each failure was mechanical, not behavioural (full block
report in `proof/UI-D6/PROOF.md`; fixed by the UI-D12 card, `91a07a5`):

- red on `cargo fmt --all -- --check`: rustfmt whitespace in `pick_gpu.rs` and
  `perf_budget.rs`;
- red on clippy `-D warnings`: only `clippy::assertions-on-constants` in
  `perf_budget.rs` - clippy needed `--keep-going` to report EVERY failing
  target in one run (without it, one error hides the rest);
- red on the em-dash text guard: the guard walks `viewer/tests` as well as
  `viewer/src` - 63 em dashes across 10 viewer source and test files were
  replaced with plain punctuation, comment/string text only;
- red on two committed artifacts (`demos/editor-sketch-extrude/present-1080p-{before,after}.png`),
  regenerated byte-identically through the committed workbench generator.

None of these were test weakenings; the fixes plus the golden refresh are
documented in `proof/UI-D12/`.

## The numbers the wave carries (from the cards' own proofs)

Interactive seam - 1,002,528 triangles at 1600x1000 on the real NVIDIA GB10
(Vulkan, driver 580.178.04; adversarial scene = P6-M5-T03 heightfield grid):

- p95 434.94 ms BEFORE (UI-D5 measurement on the pre-D10 seam) -> 6.366 ms AFTER
  UI-D10 (UI-D10 proof; UI-D5's merged-head re-measure 6.408 ms agrees), against
  the unchanged 16.6 ms bar (bar never lowered; the historical ~26x miss is
  recorded in UI-D5's proof and README row as history, fixed by UI-D10).
- Steady-state upload per frame: 120,303,360 B before -> 64 B after (MVP
  uniform only).
- Device buffer allocations per frame: 0 (created once). Draw calls per frame: 2.
- Per-primitive world-AABB frustum cull: 78.4% of a 1,331-primitive framed
  assembly dropped (3,444 of 15,972 triangles submitted).
- Pixel oracle: 21/21 proof frames sha256-identical across the change; CPU
  golden 12597229237775278722 and GPU echo 9287941873092625150 unchanged,
  picking parity 123/0, transparent-50 snapshot byte-identical.
- Decision record:
  `workstreams/modeler/ui/decisions/render-seam-performance.md` v1.0.1, with the
  prior-art brief committed at
  `workstreams/modeler/ui/proof/UI-D9/prior-art.md` (36 cited sources).

What the render-seam fix was, in one line (the lesson for wave E): geometry is
uploaded ONCE into device-resident buffers and referenced per frame,
per-primitive world-AABB culling, dirty-only re-uploads, painter order
preserved - the painter-sorted streams stay in resident GPU buffers created
once, so the steady-state frame is a 64-byte uniform write plus two draws.

## Register rows

No wave-D test maps to a `docs/scope-register.json` row, so the register is
byte-identical and `scope_register_check` stays green (23/23).

## Screenshots (acceptance)

Three real-binary screenshots of the merged-head binary, captured under
`xvfb-run` via the XTEST injector (`proof/UI-D6/ui_d6_inject.c` + `capture.sh`),
two independent launches byte-identical each (sha256 in proof):

| Shot | sha256 | Shows |
|---|---|---|
| shaded | `7fdf02ec...d41` | default studio launch: GPU-shaded demo solid + UI-D2 Shading card (Shaded selected) |
| view-cube | `29e8fab7...1eb3` | UI-D4 Orientation card (cube faces + Front/Top/Right/Iso + Section + `Triangles: 12`) after the Iso click landed the standard Iso view |
| picking | `5678b384...c84e` | UI-D3 GPU box-select mid Alt+left-drag: rubber band framing exactly the drag rect over the shaded solid |

## What is not done / deferred

- **UI-D11 (UI-DEPTH-SORT, reverse-Z depth buffer) is deliberately NOT part of
  wave D and is deferred (parked in `scheduled` for Don's decision).** It is a
  correctness upgrade that changes the GPU output contract (per-pixel depth
  ordering, GPU regolden, id-parity and transparency re-verification), NOT a
  budget fix. The premise that auto-cut it - that the painter sort could not
  reach the budget - is false: UI-D10 measured p95 6.366 ms at the full
  1,002,528-triangle / 1600x1000 scene against the 16.6 ms bar, so the budget is
  met with the painter sort retained. Its branch `ui/d11-depth-sort` was never
  merged into `wave/ui-d` (no remote branch exists) and the ancestry check does
  not fail on it.
- The wave-C variables/ kernel-features hosting gap (panels delivered
  `#[path]`-includable and unhosted) is unchanged by wave D; the UI-C3 kittest
  harness remains the accepted variables evidence per the wave-C orchestrator
  decision.
- The `wave/ui-d` branch is left in place after the checkpoint so the wave
  history and record are preserved.
- No UI test was weakened, skipped or deleted to reach green; the only gate-run
  disturbances were the documented egui_kittest teardown flake (re-run clean,
  flake transcripts preserved) and the throwaway venv used to give the precommit
  gate a pytest-capable interpreter.
