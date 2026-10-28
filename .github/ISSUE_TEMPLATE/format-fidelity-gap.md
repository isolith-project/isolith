---
name: Format fidelity gap
about: A reader does not match what the format matrix claims for a file
title: "[fidelity] "
labels: format-fidelity
---

## Format fidelity gap

The format matrix (`docs-site/format-matrix.json`) is the honesty surface for
what each reader opens. This template is for cases where the reader did not
behave the way the matrix row says, or where the row itself needs to become
more precise. Name the file, the reader, and the exact row.

### The file

- Path or file name and extension: (e.g. `example.3dxml`)
- Source / provenance: (where the file came from; vendor example, NIST corpus, customer-supplied. Do not attach vendor CAD files that you may not redistribute.)
- Size: (bytes, if known)

### The reader

- Module and version: (e.g. `oep-3dxml` 0.2.5, or `main @ <commit sha>`)
- How it was invoked: (CLI, GUI, convert worker, integration)

### The expected row

- Row id from `docs-site/format-matrix.json`: (e.g. `3dxml-read`)
- What the row claims (`opens` and `fidelity`): (quote the row)
- The evidence gate the row cites: (e.g. `P3-M5-T02`)

### Observed versus expected

- What the reader did: (opened, listed a mesh, skipped, failed, crashed, produced geometry outside the stated fidelity)
- What the matrix row says should happen: (quote or restate)
- Diff between them: (one or two sentences; include the reader's own output line if it classified a SKIP or FAIL)

### Repro

The shortest file, command, or test that shows the gap. An acceptance test comparing observed behavior to the matrix row is the strongest form of repro.

---

Honesty rule: the format matrix may only claim a fidelity its committed GA report proves (`docs-site/format-matrix.json` rows and `evidence.gate`). Give the row id and the observed behavior exactly; do not claim an open, a skip, or a pass beyond what the reader produced, and do not relabel a SKIP as a bug or a FAIL as a missing feature without the row in front of you.
