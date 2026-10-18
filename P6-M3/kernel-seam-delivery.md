# P6-M3-T04: Kernel seam delivery (oep-engine-occ, OCCT 8.0.1)

Status: implemented and verified on real material (U1/GB-10, ARM64, 2026-10-18).

## What was delivered

`crates/oep-engine-occ` -- the ADR v1.1.0 geometry-kernel seam behind the
oep-geo `kernel` feature:

- The pinned OCCT **8.0.1** system dependency, fetched + built as a dependency
  by `scripts/kernel/fetch-occt.sh` (SHA-256 pinned) and
  `scripts/kernel/build-occt.sh` (static archives by default, so every binary in
  the workspace links hermetically -- no rpath / LD_LIBRARY_PATH). The repo
  vendors no OCCT source (guardrail, see CI scan).
- The cxx.rs bridge (`src/lib.rs`) and the single C++ translation unit
  `cpp/glue.cc/hh` -- the ONLY place OCCT identifiers appear.
- Exact B-Rep booleans (union / difference / intersection) via
  BRepAlgoAPI_Fuse / Cut / Common.
- Fillet / chamfer via BRepFilletAPI_MakeFillet / MakeChamfer.
- Primitives box / cylinder / sphere via BRepPrimAPI.
- Typed `EngineError` mapped from OCCT IsDone()/status across the seam; never a
  panic (BRepAlgoAPI status code -> `EngineError`).

`crates/oep-geo` gains `src/kernel.rs` -- the clean, kernel-neutral seam
(`solid_boolean`, `solid_fillet`, `solid_chamfer`, `make_box`, `make_cylinder`,
`make_sphere`, `sys_new`, `sys_drop`, `solid_edges`, typed `BooleanError` /
`KernelError`, `Solid` + `SolidMeasures`, `Tolerance` passed by the caller per
P6-M1-T03). No OCCT identifier appears in any oep-geo public item (seam denial,
CI-enforced red-on-leak).

The engine is feature-gated (`build-engine`, enabled by oep-geo `kernel`) so the
workspace builds with no C++/OCCT toolchain unless asked; without the feature
the seam returns `EngineUnavailable`/`BooleanError::EngineUnavailable`.

## Measured numbers (pinned OCCT 8.0.1, through the seam)

Golden assertions in `crates/oep-geo/tests/kernel_seam.rs`:

| Case | Volume (tau) | Faces |
| --- | --- | --- |
| box 10x20x30 | 6000 | 6 |
| cylinder r5 h10 | pi*250 = 785.3981633974482 | 3 |
| sphere r7 | (4/3)pi*343 = 1436.755040241732 | 1 |
| union of 2 cubes offset 5 in x | 1500 | 14 |
| difference | 500 | 6 |
| intersection | 500 | 6 |
| fillet r2 on a 10-cube edge | 1000 - 10*(4 - pi) = 991.4159265... | 7 |
| chamfer d2 on a 10-cube edge | 980 | 7 |

Two OCCT 8.0.1 behaviors are locked as the kernel's real answers (a version bump
must be reviewed against them):

1. A union of two interpenetrating axis-aligned cubes keeps the coplanar
   interior walls: 14 faces, not a resewn 6. Volume is exact (1500).
2. TopExp_Explorer reports edges/vertices per incident face occurrence (a cube
   reports 24 edge rows / 48 vertex rows, not 12/8). The seam reports the
   explorer's per-occurrence counts.

Known-failing geometry returns typed errors (verified, never a panic):
zero-radius sphere / zero-height box -> `KernelError::Degenerate`; boolean with
fewer than two operands -> `BooleanError::InvalidOperand`; a difference whose
tool fully contains the base -> `BooleanError::EmptyResult`; empty edge
selection -> `KernelError::InvalidArgument`.

## Build + verify

```
# one time (or let build.rs do it): pinned OCCT 8.0.1 static
bash scripts/kernel/build-occt.sh
# typeless stub build (default)
cargo build --workspace
# through-the-seam acceptance on real OCCT
cargo test -p oep-geo --features kernel --test kernel_seam
cargo test -p oep-engine-occ --features build-engine
# guardrails (seam denial, no vendored OCCT, ADR contract)
python3 tests/test_kernel_seam_guardrails.py
# CI wiring
.gitea/workflows/kernel-seam.yaml  (guardrails + acceptance jobs)
```

ARM64 is a first-class target: this was built and tested on the U1/GB-10 host.

## Scope notes / limits

- The oep-geo `Solid` carries kernel-measured properties plus a session-local
  token that references engine state for chaining operations in one session.
  Tokens are documented as session-local and not meaningful across sessions
  (ADR section 5: the engine is replaceable behind an unchanged interface).
  A full kernel-neutral B-Rep serialization in oep-geo is future work (P6-M4
  lane); nothing in this milestone requires it.
- Tolerance is passed by the caller and threaded to the OCCT algorithm
  (boolean fuzzy value); fillet/chamfer use the radius/distance as the primary
  parameter.
- License: OCCT is LGPL-2.1 + OCCT Exception 1.0, kept a separate dependency
  (our crates stay Apache-2.0). The P0-M2 legal pass writes the required OCCT
  notice before any release (ADR section 7).
