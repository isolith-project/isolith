# UI-B6 UI-CONSTRAINTS: apply and remove constraints

## Behaviour (card)
Constraints are applied and removed from the UI through the solver; the
degrees of freedom are reported honestly; an over-constrained sketch says so
instead of silently solving.

## Acceptance

| Command | Result |
|---|---|
| `cargo test -p oep-app --test constraints` | 99 passed (4 acceptance tests + 95 module/guard) |
| `cargo test -p oep-app` | 13 targets, all green |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | exit 0, no warnings |
| fmt | my 3 files clean; workspace check flags only the pre-existing UI-B1 `input.rs:504` blank |

Real transcripts: `transcripts/0X-*.txt`. Deterministic screenshot:
`constraints.png` (sha256 `e17098049f5c80dba6c27fa5742b695140cd2ab572b4ad6269b877e4e71e115d`),
captured twice under xvfb (XTEST injector, same as UI-B3/B4 proof), byte-identical.

## What the kittest acceptance proves
Drives the real `app::App` (the same binary as production) through
`egui_kittest`:

1. **H / V / Coincident / Length apply AND remove from the on-screen toolbar**
   - draws a two-point line (`p0 p1 l0`) with the Line tool;
   - clicks the toolbar `H`, `V`, `Len`, `Co` buttons at the exact rects the
     renderer exposes (`constraint_toolbar_layout`), asserts the oracle reports
     the applied `constraint=<kind>:<target>` and a dof drop;
   - clicks `Rm` after each, asserts dof returns to the prior value.
2. **dof is reported honestly and first-class, never floored**
   - a free line reports `dof=4`; four identical lengths drive it
     `4 -> 3 -> 2 -> 1 -> 0` according to the adopted solver;
   - the report oracle carries `dof=/solve=/constraint_status=` and the
     toolbar paints the same readout (`dof 3` in the screenshot) - a real UI
     value, not a debug print.
3. **Over-constrained is a distinct, named state, never a silent solve**
   - a 5th redundant length pushes the solver's dof NEGATIVE; the report
     switches to `solve=over` and `constraint_status=over-constrained
     (dof -1)`, and the negative dof is surfaced rather than hidden;
   - removing the redundancy heals back to `solve=fully` / dof 0.
4. **Context menu routes through the SAME apply path as the toolbar**
   - right-click in the viewport opens the constraint menu (oracle
     `context_menu=1`), adds no geometry, and clicking the menu's `H` applies
     the same `constraints::apply` as the toolbar.

## Solver seam ("use the existing solver, do not write a second solver")
`src/sketch/constraints.rs` translates the app's committed `SketchDoc`
(points/lines with the `p0..`/`l0..` ids the B4 journal commits) plus the
applied `UiConstraint`s into `oep_sketch::sketch::Sketch` and calls
`oep_sketch::solve::solve_sketch` + an empty `VariableSet`. There is no
constraint math in this crate - the adopted vcad solver owns it. The honesty
rule is applied to the solver's OWN output:
- `dof < 0` (the vcad contract for over-constrained, even when the redundant
  set converges) maps to the named `ConstraintError::OverConstrained`, never a
  fabricated "dof 0 / solved";
- a non-converged inconsistent set maps to the typed `Conflict`;
- `oep-sketch` was added to `crates/oep-app/Cargo.toml` as the card-named seam
  (DETAIL: "Use the existing solver seam (oep-assem / oep-sketch)"), the same
  precedent as UI-A6 (oep-shell) and UI-A7 (oep-import).

## Files
- `crates/oep-app/src/sketch/constraints.rs` (new) - solver-facing layer,
  apply/remove driver, `ConstraintPanel` (context-store resident), named
  verdicts.
- `crates/oep-app/src/panels/sketch.rs` - constraint toolbar + context menu
  (deterministic `constraint_toolbar_layout` / `constraint_menu_layout`), dof
  status readout, oracle fields, right-click handling; draw clicks skip the
  toolbar rect so buttons and tool placement never fight.
- `crates/oep-app/tests/constraints.rs` (new) - kittest acceptance (4 tests).
- `crates/oep-app/Cargo.toml` - `oep-sketch` path dep (card-named seam).
