---
title: Isolith Editor Spine Decision
decision: editor_spine_full_send
version: 1.0.0
date: 2026-10-22
status: Ratified
owner: Isolith program governance, editor wave (program director)
applies_to: the 2027.1 release train and every later train; the editor wave (oep-engine-occ behind the oep-geo Evaluator seam, oep-shell and oep-viewer as the interactive editor, P14 as the editor-integration vehicle); the scope register and its per-capability statuses; the format capability matrix, release notes and README of the 2027.1 line
supersedes: decision #13 (2026-10-18) default-exclude for Class-A surfacing in the scope register (Bucket C Class 4, defer-or-exclude with a default of exclude), and the interop-first framing of the product story for the 2027.1 release
---

# Isolith Editor Spine Decision

This record is the binding product decision that makes the editor the spine of
the Isolith program. It is versioned like the charter and the other program
decisions; any material change bumps the version in the front matter and adds
a line to the change log at the bottom.

## 1. Decision

The editor is the product, not a feature. The product bar for Isolith is
same-or-greater editor functionality than CATIA and SolidWorks for the working
engineer: a program whose users cannot create and edit geometry is not a CATIA
replacement, and engineers will not adopt it. This reverses the interop-first
posture that shaped the first three waves. The interop readers stay, but as
the sovereignty door, not as the product.

Don selected the full-send option on 2026-10-22: Class-A / ICEM-class
surfacing is IN scope from day one as its own long-horizon kernel-moat
program. The longest pole and the biggest team are accepted. The
decision #13 default-exclude for Class-A in the scope register (Bucket C
Class 4) is superseded.

Concretely, this decision means:

1. 2027.1 (rc2 in canary) is retitled the INTEROP/PDM FOUNDATION release, not
   the product GA. What the 2027.1 line ships is the read/render/vault
   foundation: the interop readers, geometry core, PDM, compute, and delivery
   stack. No reader-only release may claim to be the editor; renaming reality
   is not a fix.
2. P14 (the parked reference vertical) is pulled forward as the
   editor-integration vehicle: kernel seam first, then the interactive
   surface, then the vertical.
3. The kernel is the engine. oep-engine-occ (the Open CASCADE binding,
   LGPL-2.1 with exception) lands on main behind the oep-geo Evaluator seam;
   MiniBoxEvaluator is demoted to a documented test-only reference. The OCC
   LGPL disposition is a license decision compatible with the Apache-2.0
   applications posture (no GPL-3.0). OCC is the pragmatic route to real
   boolean, fillet, chamfer, extrude, revolve, sweep, shell, split and trim
   without writing a twenty-year kernel from scratch.
4. The interactive surface: oep-shell plus oep-viewer become a mouse-driven
   interactive editor (viewer camera, render and pick paths already exist).
   The CLI stays as the agent and automation lane, accessibility contract
   intact.
5. Feature breadth is grown against an explicit CATIA/SolidWorks parity
   baseline in the scope register. The register stops being an exclusion
   list; every Bucket C row becomes a tracked capability with a status
   (not-scoped, planned, building, shippable) and a drift-guarded
   honest-claims check.
6. Honest sequencing, no fake dates: kernel first, interactive shell second,
   breadth wave by wave, Class-A as the strategy-gated long program. A parity
   calendar date would be a lie; a drivable editor on a defined sequence is
   the commitment.

## 2. Why this decision

Three facts drive it:

1. CATIA and SolidWorks are editors first. Their identity for the working
   engineer is the surface where geometry is created and changed. A platform
   that only reads, renders and vaults someone else's geometry solves the
   interop pain but does not occupy the same product slot.
2. The sovereignty wedge justifies the readers, but it does not make the
   product. Data ownership is the why; the editor is what engineers actually
   use daily. Without the editor, adoption stalls at the read-only layer.
3. The geometry kernel is the moat. Class-A surfacing needs a high-accuracy
   surface kernel (the vault research names the vendor CGM as the moat).
   Open CASCADE is not a Class-A kernel. Accepting that from day one, as a
   funded long-horizon program rather than a default exclusion, is the honest
   funding commitment the full-send option makes.

## 3. Options considered

1. Interop-first as shipped (readers, PDM, compute, no editor commitment).
   Rejected by this decision. It answers sovereignty but does not replace
   CATIA, and the product bar is set against the editor.
2. Editor-spine minimal: interactive shell over OCC, Class-A deferred.
   Rejected. Class-A is the longest pole and the kernel-moat program; parking
   it leaves the hardest and most defensible surface to an undefined future.
3. Editor spine, full-send: editor as the product bar, Class-A in scope from
   day one, kernel-first honest sequencing. Adopted. This record.

## 4. What does not change

- The clean-room invariant applies to the proprietary-format RE (CGR,
  CATPart, 3DXML observation remains the sovereignty moat). It does NOT apply
  to the public OCC kernel, which is legitimately linkable.
- The versioning, release-train, telemetry and changelog decisions stand.
- The interop readers remain first-class; they are the door the 2027.1
  foundation walks through.

## 5. Enforcement

The decision is enforced by automation and by the gate, so a future change
cannot silently re-frame the release:

- No user-facing surface of the 2027.1 line (README, docs site, format
  matrix, changelog and release notes) may claim the editor for the 2027.1
  foundation. The retitle wording "interop/PDM foundation release; the editor
  is the product spine" is asserted by this record's acceptance test.
- The scope register carries per-capability statuses and treats every Bucket C
  row as a tracked capability; Class-A is tracked as its own long-horizon
  program with its own strategy ADR (SURFACING-CLASS-A) before decomposition.
- The acceptance test in this repo, tests/test_editor_spine_decision.py,
  stays green; it exists to catch edits that weaken the record itself.

## 6. Coverage across the program

- 2027.1 / interop-PDM foundation: readers, geometry core, PDM, compute,
  delivery. Retitled, never called the product GA.
- Editor wave: oep-engine-occ (kernel seam), oep-shell, oep-viewer, P14,
  driven against the CATIA/SolidWorks parity baseline.
- Class-A surfacing: strategy-gated long-horizon kernel-moat program, in
  scope from day one.
- PDM and compute ship alongside the editor; they support it, they are not
  the product.

## 7. References

- Master plan v1.15 (2026-10-22), decision #19 and the scope-register and
  critical-path REVISED-BY-19 notes.
- Vault Topics/Isolith.md v1.23, Editor Spine Restructure section.
- workstreams/governance/decisions/release-train-versioning.md (how the
  2027.1 retitle is versioned and shipped), telemetry-decision.md,
  workstreams/release-engineering/release-changelog.md.
- docs/scope-register.{json,md} and python/oep_tooling/scope_register_check.py
  (the committed CATIA/SolidWorks parity baseline, decision #19).
- P6-M3-T04 kernel seam ADR and the scope register in the master plan.

## 8. Change log

- 1.0.0 (2026-10-22): initial ratification. The editor is the product spine;
  same-or-greater editor functionality than CATIA/SolidWorks is the product
  bar; Class-A surfacing is in scope from day one; 2027.1 is retitled the
  interop/PDM foundation release. Supersedes the decision #13 default-exclude
  for Class-A.
