# Ground-truth inputs for the P13-T03 format-matrix spot-check

These four files are byte-faithful copies of the committed P2/P3/P4 GA matrices
that the docs-site format matrix is checked against ("no overstated claims").
They live under the P13-T03 proof dir so the committed proof is self-contained
(it does not depend on which branches have merged).

Provenance (source of truth is the keelworks/keelworks GA-gate branch files):

- ga-gate-p4-matrix.json   <- corpus/ga-gate-p4-matrix.json on
                              task/p4m5t03-ga-gate-1016a (P4-M5-T03 gate)
- pmi-fidelity-matrix.md   <- docs/p4-m2-t03-pmi-fidelity/pmi-fidelity-matrix.md
                              on task/p4m2t03-pmi-fidelity-1016a (P4-M2-T03)
- p2-m6-t03-ga-gate.md     <- docs/re-notes/p2-m6-t03-ga-gate.md on
                              task/ga-gate-1015a (P2-M6-T03 gate)
- p3-m5-t02-ga-gate.md     <- docs/re-notes/p3-m5-t02-ga-gate.md on
                              task/3dxml-ga-gate-1016a (P3-M5-T02 gate)

When wave-3 integration merges those branches onto main, the checker's default
paths (corpus/ga-gate-p4-matrix.json, docs/re-notes/...,
docs/p4-m2-t03-pmi-fidelity/...) become the canonical sources; these copies are
kept as the recorded run's inputs.
