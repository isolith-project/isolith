---
title: P7-M3-T02 Mates/kinematics demo - own DMU-like semantics wedge, clean-room (recorded)
decision: mates_kinematics_demo_is_an_own_dependency_free_semantics_wedge_production_solver_follows_P7-M3-T01_ADR
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks program governance, P7 modeler lane (P7-M3-T02)
applies_to: the P7-M3-T02 mates/kinematics demo, the oep-kinematics crate, and the P7-M3-T01 solver ADR boundary
supersedes: nothing
---

# P7-M3-T02 Mates / Kinematics Demo (own DMU-like semantics)

## 1. Decision

P7-M3-T02 is delivered as a **clean-room semantics wedge**: the card's own
instruction ("reuse DMU-like semantics, own implementation, no vendor
libraries") governs. We implement a DMU-like mate/joint model (revolute,
prismatic, spherical, rigid), a dependency-free constrained-assembly solver
(damped Gauss-Newton least squares), a kinematic command driver, and a
dependency-free software renderer, all inside a new crate `oep-kinematics`,
over the `oep-geo` canonical model. The deliverable is a **recorded demo** of
a constrained crank-slider assembly moving, plus the executable clean-room
invariant test (no vendor bytes anywhere in the crate).

Two scopes are deliberately kept apart:

1. **This card (P7-M3-T02):** a demo / prototype solver written by us, zero
   vendor and zero third-party geometry dependencies. It proves and pins the
   mate *semantics* (joint vocabulary, DOF analysis, closed-loop position
   analysis, driven commands) in code now, and is verified against a
   closed-form analytic solution and real `oep-geo` corpus material.
2. **P7-M3-T01:** the *production* assembly constraint solver, which per the
   ratified P7-M1-T01 record is to be **adopted as OSS, not rebuilt**. That
   ADR is owned by P7-M3-T01 and is untouched by this card. The own solver in
   `oep-kinematics` is a reference implementation behind a seam; when T01's
   solver lands, it swaps in without rewriting the mate model or the demo.

This construction honors both the P7-M1-T01 "adopt, do not rebuild" rule (for
production) and this card's "own implementation, no vendor libraries" (for the
demo) -- they are different lanes, and this record makes the boundary explicit
instead of letting the two readings collide.

## 2. Why

- The card requires a demo whose recorded bytes contain no vendor code; the
  only way to prove that is to ship the semantics as our own implementation
  and gate it with an executable invariant test, which we did
  (`tests/cleanroom.rs` + pytest mirror).
- DMU-like behavior (1-DOF mechanism, driven revolute command, closed loop on
  the slider) is a small, well-understood numerical problem; a from-scratch
  solver is days of work, keeps the crate dependency-free, and makes the
  semantics legible -- the right trade for a *demo* lane.
- Analytic ground truth for the crank-slider makes acceptance objective: the
  solver must match the closed-form solution to ~1e-10, which it does
  (worst error 7e-11 mm over 90 frames).

## 3. Scope notes and boundaries

- The own solver is explicitly NOT a claim to build a production solver; that
  remains P7-M3-T01's ADR (OSS adoption).
- Display meshes are visualization-only approximations from `oep-geo` body
  bounding boxes (the model carries bbox only at this program wave); mechanics
  act only on instance transforms.
- ffmpeg is used purely as a recording-host packaging step for PPM frames; it
  is not a crate or CI dependency.

## 4. Verification

- `cargo test -p oep-kinematics` (lib + integration) 23 tests green.
- `cargo clippy --workspace --all-targets` -D warnings clean; `cargo fmt` clean.
- `pytest tests/test_p7_m3_t02_acceptance.py` green (independent stdlib voice).
- Recorded demo: 90 frames, piston stroke exactly 60 mm = 2R, rod length
  70 mm held closed-loop, worst residual 3e-11, worst analytic error 7e-11 mm.
- Clean-room invariant test green: runtime deps exactly `{oep-geo}`, no vendor
  references in code, no prebuilt artifacts / vendored trees in the crate.

## 5. Change log

- 1.0.0 (2026-10-16): initial ratification.
