# UI-E3 UI-ASSY-DIAGNOSTICS: dof readout and interference panel  -  proof

Wave E card. The assembly reports its degrees of freedom and the pairs that
interfere, per pair, with a jump-to that selects both components.

## Files owned by this card

- `crates/oep-app/src/panels/assy_diagnostics.rs` (new: dof line, interference
  table, jump-to) - a `#[path]` child module of `assy_tree`, pulled in with
  one include and ONE call, exactly like the mate tool.
- `crates/oep-app/src/panels/assy_tree.rs` (ADDITIVE: one `#[path]` include
  (`mod assy_diagnostics;`) + ONE call (`assy_diagnostics::show`)  -  both
  sides kept, +21/-0).
- `crates/oep-app/tests/assy_diagnostics.rs` (new: acceptance harness).
- `crates/oep-app/Cargo.toml` (+`oep-analysis` path dep - SEE NOTE BELOW).
- `workstreams/modeler/ui/proof/UI-E3/` (this proof).
- `workstreams/modeler/ui/README.md` (UI-E3 row, this wave).

## One dependency, and why (deliberate, card-mandated)

`crates/oep-app/Cargo.toml` gains one line: `oep-analysis = { path =
"../oep-analysis" }`. This is the ONLY file touched outside the owned list.
Reason: the card's instruction is to USE the existing interference check
("Use the existing DOF and collision path (`AssemblySession::solve` report and
the assembly crate's interference check). Do not write a second collision
test"). The workspace has exactly ONE assembly interference check -
`oep_analysis::clash::detect_in_assembly` (`crates/oep-analysis/src/clash.rs`,
the check the analysis crate's own `tests/mass_clash_explode.rs` commits
against the clash fixture). oep-app previously did not depend on oep-analysis,
so reaching that check requires a direct path dependency. There is no other
interference check in the workspace (oep-assem has none), and writing a second
one is exactly what the card forbids, so this line is the only faithful
reading of the card's own instruction. `oep-analysis` is already a workspace
member and already counted in the licensing boundary (34 crates unchanged); the
licensing gate stays 6/6. Cargo.lock changes by exactly the one `oep-analysis`
edge under `oep-app`'s dependencies.

The panel uses the check READ-ONLY on the live canonical document every frame;
it never writes the document (mutation guard green - see gates).

## Acceptance

```
cargo test -p oep-app --test assy_diagnostics   -> 143 passed / 0 failed, exit 0
xvfb-run -a cargo run -p oep-app -- --smoke      -> frames=8 smoke ok, exit 0
```

Transcripts below. The diagnostics panel publishes a plain-`String` oracle
every frame (`dof`, `dof_source`, `tree_dof`, `mate_dof`, `pairs`, one
`pair_i=` per interfering pair, `selection`, `err`) plus the jump-to selection
id list under its own key, so every clause below is proven with numbers read
back from the running panel.

### 1. The committed interference fixture reports EXACTLY the known overlapping pair

The committed `clash-fixture` (the analysis crate's golden: base plate
40x30x10 at the origin + interfering block 20x10x10 at (25,20,5)) is seeded
into the panel; the known overlapping pair is `i-base` x `i-block` with a
750 mm3 overlap at (32.5, 25, 7.5). Both the fixture's expected pair and the
reported list are printed, and the panel's result is re-derived independently
through the SAME check:

```
fixture expected pair = i-base x i-block
reported list:
  i-base x i-block
pairs=1
pair_0=i-base x i-block:vol=750.0000:loc=32.5,25,7.5
```

Asserted: `pairs == 1`, `pair_0` starts with `i-base x i-block`, and the
independent re-derivation equals exactly `[(i-base, i-block)]` - one pair,
nothing else.

### 2. A non-overlapping pair reports zero interference (no false positive)

The committed two-part fixture (`base` + `cap` - cap placed well above base,
positive-volume overlap on every axis is impossible):

```
non-overlap fixture: base + cap (cap well above base)
reported pairs = 0
```

Asserted `pairs == 0` and the independent re-derivation is empty.

### 3. Jump-to selects both components of the reported pair

Clicking the pair row's `Jump i-base x i-block` button (asserted present in
the AccessKit tree) selects BOTH components:

```
jump-to selection id list = i-base,i-block
selection in report = i-base,i-block
tree highlight (jump focus) = i-base
```

The selection id list is asserted equal to the pair (`i-base,i-block`), the
report's `selection=` matches it, and the tree's single highlight (the jump
focus) is the pair's first component. The `interference: 1 pair` summary is
visible in the UI.

### 4. The dof readout equals the solver's report for the same assembly

Base grounded (cap the only moving body, no user mate yet): the panel's
readout and an independent `AssemblySession::solve` over the SAME document
and constraint set are both printed and asserted equal:

```
panel dof readout = 0  (from tree solver report)
solver report dof  = 0  (AssemblySession::solve, same assembly)
```

Note the seam's honest semantics (documented in the test): the solver reports
`dof = 7 per moving body minus rank(J)` where J includes one unit-quaternion
boundedness row per moving body, and `solve_least_squares` short-circuits to
`free_dof = 0` when the starting state already satisfies every residual - the
same `0` UI-E2 printed for a free cap after a drag. The clause is equality
with the report (never a fabricated number), so the panel shows exactly what
the solver reports for the same assembly.

### 5. The dof readout updates when a mate is added or removed

Through the real mate tool (coincident `cap:origin == base:top`):

```
dof before mate (cap free, base grounded) = 0
solver report dof (same state)            = 0
dof after mate added = 3
solver report dof (same assembly, same start state) = 3
dof readout updated on mate add: 0 -> 3
dof after mate removed = 0
solver report dof (same state)   = 0
dof readout updated on mate remove: 3 -> 0
```

Each transition is printed and independently verified through the same solver
over the same document + constraint state the tool solved: the independent
add check uses the document AS IT WAS (cap at [28,14,20], so the solver
iterates and reports 3 rotational dof left); the remove check uses the
post-delete document. Asserted: `after_add == 3 == independent_add`,
`after_remove == independent_remove`, and the readout moved on both edges.

## Layout note (three wave-E surfaces in one fixed-height region)

The assembly region is the fixed lower 48% of the model-tree column and must
now host THREE wave-E surfaces (tree rows, diagnostics, mate tool). The
diagnostics panel therefore renders as a normal in-flow SECTION immediately
after the tree's toolbar and BEFORE the mate tool: every section lays out in
order with no overlap in the shipped 1280x800 window (verified by the real
binary - the screenshot below shows tree rows + toolbar, then Diagnostics with
the live dof + interference readout, then Mates, nothing overlapping). Cost: a
mate-op's dof enters the readout one repaint after the op (the diagnostics
section renders before the mate tool in the same frame), which the acceptance
accounts for with one extra `harness.step()` and documents. `assy_mates.rs`
and `assy_tree.rs` were not otherwise touched.

## Gates (on the committed tree)

- `--test assy_diagnostics`: **143 passed / 0 failed**, exit 0 -
  `transcripts/10-acceptance-nocapture.txt` (all numbers printed, single
  thread).
- `--test assy_tree` (UI-E1 regression): **142 / 0** and `--test assy_mates`
  (UI-E2 regression): **143 / 0** - `transcripts/15-regressions.txt`.
- `cargo test -p oep-app` (all targets): **29 targets, 4377 passed / 0
  failed**, exit 0 - `transcripts/30-oep-app-full.txt`. One pre-existing
  egui_kittest exit-time SIGSEGV flake (kittest-heavy targets under full
  parallel load, the documented UI-D6/D12 flake) was hit on the first run and
  re-ran clean; the committed transcript is the green re-run.
- `cargo test --workspace`: **229 suites, 5651 passed / 0 failed**, exit 0 -
  `transcripts/31-workspace.txt` (the +1 suite / +143 tests vs UI-E2's 228 /
  5508 is exactly this card's new target). No segv in the committed run.
- Mutation guard green: `no_module_mutates_the_document_outside_doc_io_rs`
  (the diagnostics panel only READS `oep_analysis::clash::detect_in_assembly`
  and solves a throwaway `AssemblySession` copy; no document mutation
  signature appears in `src/`).
- clippy `-D warnings` all targets: exit 0 - `transcripts/40-clippy.txt`.
  fmt `--check`: exit 0 - `transcripts/41-fmt.txt`.
  Licensing: 6/6, 34 crates / 535 deps unchanged - `transcripts/42-licensing.txt`.

## Real-binary screenshot (deterministic: two independent captures byte-identical)

- `dof.png` - the shipped 1280x800 binary, base selected + grounded through
  real clicks; the Diagnostics section renders between the tree toolbar and
  the Mates section with the LIVE readout `degrees of freedom: 0 (from tree
  solver report)` and `interference: none (2)`.

sha256 per image in `dof.png.sha256` (byte-identical across two independent
launches each, verified by `capture_e3.sh`):

```
dof.png  df5647fa0470076fbcaa7c5fe7b5a5b58aa7752075944ac4430240ac2a866055
```

Captures are driven by `capture_e3.sh` + `ui_e3_inject.c` (XTEST gestures at
coordinates derived from a kittest geometry probe of the same code at the same
window size, window at origin on a 1600x1000 Xvfb screen, no WM).

Scope note on the interference pair in the real binary: the committed
interference fixture is not openable through the current real-binary UI (the
assembly panel opens the two-part fixture; there is no fixture-mechanism
surface yet), and the flow layout places the mate tool's distance input within
a few pixels of the 800px window bottom, so a real-binary interference-state
shot would be unreliable. The interference pair + jump-to are therefore proven
deterministically by the kittest acceptance above (byte-exact oracle numbers);
the merged-head interference SCREENSHOT belongs to the UI-E5 integrate card as
its own acceptance states.

## SHELL STATE NOTE

STEP 0 was run (`git fetch github; git merge github/wave/ui-e`): the merge
brought UI-E2 (mate tool), UI-E6 (nav cube) and the wave-E tree. Not an empty
merge.
