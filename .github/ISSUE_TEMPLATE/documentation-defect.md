---
name: Documentation defect
about: A page, note, or guide claims something the project does not back
title: "[docs] "
labels: documentation
---

## Documentation defect

Point at the claim, the document that makes it, and the place the claim is
(or is not) verified. Do not rewrite the document in this issue; a defect
report names the gap.

### The document

- Page or file: (e.g. `docs-site/getting-started.html`, `docs/geometry-math.md`, `README.md`)
- Section or quote: (the claim as written, quoted)

### The claim

What the document says, restated in one sentence.

### What backs the claim

Where the behavior is verified: an acceptance test, a GA gate report, a
format-matrix row, a recorded decision, or "nothing in the tree verifies
this". If you tested it and observed something different, say so with the
build and steps.

### Suggested fix (optional)

A one-line suggestion for the correction. The maintainer will make the change
with a green test behind it if one is needed.

---

Honesty rule: the documentation must not claim behavior that a green test does not prove. Point the defect at the claim and its source; do not insert a new claim that no test supports, and do not soften an honest statement into vagueness.
