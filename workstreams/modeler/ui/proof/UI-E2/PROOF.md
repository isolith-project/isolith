# UI-E2 UI-ASSY-MATES: mate tool against the live solver  -  proof

Wave E card. Mates are created, edited and deleted against the LIVE solver
from the assembly surface, and dragging a component re-solves and reports
degrees of freedom honestly (never silently swallowed).

## Files owned by this card

- `crates/oep-app/src/panels/assy_mates.rs` (new: mate tool  -  pick two
  anchors, kind coincident / plane-distance, mate list with per-row
  edit/delete, component drag with live re-solve, undo/redo via the journal)
- `crates/oep-app/src/panels/assy_tree.rs` (ADDITIVE: one `#[path]` include
  (`mod assy_mates;`) + ONE call (`assy_mates::show`)  -  both sides kept)
- `crates/oep-app/tests/assy_mates.rs` (new: acceptance harness)
- `workstreams/modeler/ui/proof/UI-E2/` (this proof)
- `workstreams/modeler/ui/README.md` (UI-E2 row, this wave)

No second solver and no second transform path: every geometry movement goes
through the existing seam only  -  `oep_shell::assem::AssemblySession` over the
same canonical document the assembly tree already publishes, with
`add_mate`/`edit_mate`/`remove_mate` and `MateOutcome` / `SolveOutcome`
(card: "Use the existing seam only"). `oep_assem::mates::Mate` is reached
only through the shell's own `parse_mate` (coincident / plane-distance), so
`oep-app` never names an `oep-assem` type directly.

## Acceptance

```
cargo test -p oep-app --test assy_mates   -> 143 passed / 0 failed, exit 0
xvfb-run -a cargo run -p oep-app -- --smoke -> frames=8 smoke ok, exit 0
```

Transcripts below. The mate panel publishes a plain-`String` oracle
(`mates`, picks, `dof`, `moved`, `max_dist`, `err`, `hash`, `can_undo`,
`report`) every frame plus the live canonical document JSON through the
tree's existing doc key, so every clause below is proven with numbers read
back from the running panel.

### 1. A coincident mate moves the moving component onto the fixed anchor

base is grounded at identity, so `base:top` sits at `[30,25,10]` (derived
anchor: top face centre of the 60x50x10 base). Adding the default coincident
(`cap:origin == base:top`) through the real `+ Add mate` button:

```
test coincident_mate_moves_component_onto_fixed_anchor ... ok
before cap translate = [28.0, 14.0, 20.0]
after  cap translate = [30.0, 25.0, 10.0]
coincident: cap translate [28.000, 14.000, 20.000] -> [30.000, 25.000, 10.000] (analytic target [30,25,10])
```

The after transform is asserted element by element against the analytic
expectation `[30,25,10]` within 1e-6 (`translate[i]` for i in 0..3), and the
solver report agrees: `dof=3` (a point-coincidence on a body leaves its three
rotational dof) with residual ~0.

### 2. A distance mate at d=25 leaves exactly 25 mm between the anchors

`base:top` is the z=10 plane with +Z normal; `cap:origin` is the moving
point. A plane-distance mate at d=25 (default) re-solves cap z to 35:

```
test distance_mate_leaves_exactly_25_mm_between_anchors ... ok
distance: cap translate = [28.0, 14.0, 35.0] (z expected 35)
distance between anchors = 25.000000 (expected 25.000000)
```

The measured anchor distance `|cap.z - 10|` is asserted within 1e-6, and
`dof=5` (a single plane-distance constraint removes one dof).

### 3. Dragging re-solves and reports dof from the solver report (UI, not swallowed)

The cap is dragged with a real pointer gesture; the drag writes the
gesture into the journal (one undo entry) and the live solve report is
published and rendered in the panel:

```
test dragging_component_resolves_and_reports_dof_before_and_after ... ok
dof before mate (cap free, after drag) = 0
dof after mate (cap constrained) = 3
dof after re-drag with mate = 3
report line (UI) after drag = solve dof=3 moved=1 max_dist=0.000e0
cap translate after drag + re-solve = [30.0, 25.0, 10.0] (expect [30,25,10])
```

The before value (cap free) and the after value (constrained) are both
printed from the solver report, and the report text is asserted present in
the AccessKit tree (`has_label_containing "solve dof="`), so it is shown in
the UI  -  never silently swallowed. The re-solve snaps the dragged cap back
onto the fixed anchor (`[30,25,10]`).

### 4. Editing and deleting a mate restore the exact prior transform hash

Every mate op is ONE journaled command through `doc_io::EditHistory`, so
each records a before/after document hash and Undo restores byte-for-byte:

```
test edit_and_delete_restore_exact_prior_transform_hash ... ok
h0 (before any mate)  = 15168000965944691752
h1 (after add)        = 7817863590748544520  sha256 18beebce4a9df1851ad0c6ee8e21482b206fc84700c40f5fa722e311917450ea
h2 (after edit)       = 15592947043259742318  sha256 65e460743f0fddb91fc0dc81b4a2b9c354c6cc6527aebf7da775ee64f95a588d
h3 (after delete)     = 15592947043259742318
undo(delete) hash     = 15592947043259742318  (expect 15592947043259742318)
undo(edit) hash       = 7817863590748544520  (expect 7817863590748544520)
undo(add) hash        = 15168000965944691752  (expect 15168000965944691752)
```

Edit moves the mate from `base:top` (z=10) to `base:bottom` (z=0) and the
cap re-solves to z~0 (`assert |t.z| < 1e-6`). Deleting leaves the free cap
at its last solved pose (`h3 == h2`  -  free dof is not fabricated back), and
three consecutive Undos return the document hash exactly to h2, h1, h0  - 
with the restored canonical JSON asserted byte-identical (`doc_json == json`).
Each op is one journaled command.

### 5. An over-constrained set is refused with the solver's named error

After the first coincident pins cap at `base:top`, a second coincident
`cap:origin == base:origin` demands the same rigid point at z=0 too:

```
test over_constrained_set_is_refused_with_solver_named_error ... ok
after first mate: dof=3 hash=7817863590748544520
after refused second mate:
  mates=1 err=refused: solver-failed
refusal error = refused: solver-failed
```

The refusal carries the solver's NAMED error tag (`solver-failed`, from
`oep_shell::assem::workbench_error_kind`), the mate is NOT accepted
(`mates=1`), and the document hash + JSON are byte-for-byte unchanged  -  the
over-constrained set is never solved anyway.

## UI layout note (reachability in the shipped window)

The assembly region is the fixed lower 48% of the model-tree column; the
tree's rows + toolbar already occupy much of it, so the mate tool was
rendering past the region's clip rect in the real 1280x800 binary. The kind
picker, `+ Add mate` and the solve report line are PINNED above a vertical
`ScrollArea` (bounded to the remaining region height) holding the anchor
picks, the mate list, the drag handles and undo/redo  -  so the primary action
and the never-swallowed report stay always visible, and every control stays
reachable. In the tall acceptance harness nothing overflows and every widget
is clicked directly through the real input path.

## Gates (on the committed tree)

- `--test assy_mates`: **143 passed / 0 failed**, exit 0.
- `--test assy_tree` (UI-E1 regression, untouched ADDITIVE side): 142 / 0.
- Mutation guard: `no_module_mutates_the_document_outside_doc_io_rs` green
  (all document writes route through `doc_io::EditHistory`; the panel never
  uses a banned signature).
- clippy `-D warnings` all targets: exit 0. fmt `--check`: exit 0.
- `xvfb-run -a cargo run -p oep-app -- --smoke`: frames=8 smoke ok, exit 0.
- `cargo test --workspace`: **5508 passed / 0 failed, 228 suites**, exit 0  - 
  `transcripts/30-workspace.txt`.

Transcripts:
- `transcripts/10-assy-mates-nocapture.txt`  -  acceptance test output with all
  numbers printed (--nocapture, single thread).
- `transcripts/20-smoke.txt`  -  smoke run output.
- `transcripts/30-workspace.txt`  -  full workspace suite.
- `transcripts/40-clippy.txt`, `transcripts/41-fmt.txt`  -  lint gates.

## Screenshots (deterministic: two independent captures byte-identical)

- `coincident.png`  -  base grounded, coincident `cap:origin == base:top`
  created via the real `+ Add mate` click; the pinned report line reads
  `solve dof=3 moved=1 max_dist=0.000e0` (shown in the UI, never swallowed).
- `distance.png`  -  `kind distance` selected, plane-distance mate created; the
  report line and default d=25 are visible in the pinned row.
- `list.png`  -  the coincident state scrolled inside the mates region: the
  mate-list row plus the report line are visible together with the drag and
  undo/redo controls.

sha256 per image in `*.sha256` (byte-identical across two independent
launches each, verified by `capture_e2.sh`):

```
coincident.png  910009ddd3d4b666ced31a5d7cbe038ff7f9c8cdfb2b35ad71708aa6afbc0852
distance.png    657e1fc20f8cb744e9dd9980167d4d5357b5121eb48546cdb66f8a703509f615
list.png        898ca3841d6a8954daecb77af07bf065229eb2730363cf805f18c02a9ceedfbe
```

Captures are driven by `capture_e2.sh` + `ui_e2_inject.c` (XTEST gestures at
coordinates derived from a kittest geometry probe of the same code, window at
origin on a 1600x1000 Xvfb screen, no WM).

## SHELL STATE NOTE

STEP 0 was run (`git fetch github; git merge github/wave/ui-e`): the merge
brought UI-E6 (nav cube + settings.rs + nav_cube.rs tests + UI-E6 proof).
Not an empty merge.
