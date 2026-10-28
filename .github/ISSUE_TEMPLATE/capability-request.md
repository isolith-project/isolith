---
name: Capability request
about: Propose a change to a scope-register row or a new capability tied to a row
title: "[capability] "
labels: capability
---

## Capability request

The scope register (`docs/scope-register.json`) is the commitment surface.
A request that does not name the row it belongs to cannot be sequenced, so
name the row first.

### The row it belongs to

- Row id from `docs/scope-register.json`: (e.g. `sheet-metal`)
- Current status of that row: (not-scoped / planned / building / shippable)
- Cost class, if the register states one: (class-1..class-4) as a labeled guess if not

### The job you want to do

Describe the engineering task the capability unlocks, in one short paragraph.
Name the workflow, not the button: "route a tube between two flanges at an
angle" is a job; "add the routing workbench icon" is not.

### What exists today

What Isolith already does that is adjacent, plus the format matrix or
test that documents it. If nothing exists, say so.

### What "done" would look like

The acceptance test that would prove the capability works on real material.
A request that cannot name a test is a wish; this is the honest-claims
discipline from the register.

### Sequencing

What you propose changes to the register, clearly: add a new row, move an
existing row (and to what status), or re-sequence a commitment order entry.
Remember the upgrade rule: a row may only move up while a green accepted test
proves the new claim.

---

Honesty rule: a scope-register row may not claim a status higher than a green accepted test proves. This request proposes a change to the register; it is not a commitment to build it, and any acceptance is recorded by the maintainer as a decision, with a version bump on the register.
