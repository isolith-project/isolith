# PMI Fidelity Audit Runner: GD&T Types x Reference Apps (Honest Matrix)

P4-M2-T03 · Keelworks / OEP P4 STEP/PMI lane
Status: measured on real material 2026-10-16 (NIST MBE PMI AP242 corpus, 17 files; step-io 0.2.5 column PASS/FAIL with per-file evidence; OCCT / FreeCAD / commercial readers honestly SKIP on this host)
Version: 1.0

## 1. What this is

A fidelity AUDIT RUNNER for STEP PMI, not a correctness claim. For every GD&T
characteristic carried in a corpus STEP file it asks one question per reference
app: "does the app preserve and classify the instances the file actually
carries?" — and reports the answer in a closed-verdict honest capability table
(P13-T03 style): PASS / FAIL / SKIP, each with an evidence string.

It is re-derivable from real bytes:
`python3 -m oep_tooling.pmi_fidelity --corpus <dir> --matrix-out <dir>`
backed by the probe `cargo build -p oep-step --example pmi_probe`
(`crates/oep-step/examples/pmi_probe.rs`), which reports per-file PMI
ground-truth through the wrapped OSS STEP library (step-io 0.2.5, per the
P4-M1 step-library ADR). Acceptance test: `tests/test_pmi_fidelity_acceptance.py`
(TDD: RED without the runner, GREEN on real material). Published artifact:
`docs/pmi-fidelity/{pmi-fidelity-matrix.json, pmi-fidelity-matrix.md}`.

## 2. Honesty contract

- **Scope is presence + classification fidelity only.** Tolerance-value,
  datum-anchoring and feature-association correctness are P4-M2-T01/T02
  (semantic import / re-export) scope. The matrix does not claim them.
- **Ground truth is independent.** "What the file carries" comes from a
  lexical scan of the DATA section (raw bytes, closed PMI-type token set,
  simple and complex tuple forms) — not from the library being audited.
- **No fabricated success.** PASS requires the app to have surfaced >=1
  instance on EVERY corpus file that carries the type. A type that the corpus
  carries but the app never surfaces is FAIL, never PASS. An unavailable app
  is SKIP with the reason recorded.
- **Reproducible.** The matrix carries runner metadata (probe sha256,
  step-io pin, generated_at, corpus count) so a rerun is auditable.

## 3. Matrix axes

- Rows: the closed 14-value ASME Y14.5 / ISO 1101 characteristic set (the
  same set the `oep-pmi` model uses), plus two supporting rows — Datums &
  datum reference frames, and tolerance modifiers (MMC/LMC/RFS,
  unequally-disposed).
- Columns (reference apps): step-io 0.2.5 (in-stack pure-Rust OSS reader),
  Open CASCADE Technology via pythonocc/OCP (the P4-M1 ADR designates OCCT as
  the AP242 semantic-PMI fidelity path), FreeCAD (independent OSS app), and
  commercial readers (policy SKIP, P1-M3-T02). The runner auto-detects
  availability; unavailable columns are honest SKIP, not assumed.

## 4. Findings on the NIST MBE PMI corpus (2026-10-16)

Corpus: `ref/nist/NIST-PMI-STEP-Files` (17 license-clean AP242 files; P1/P4
dev-only reference material per decision #7 — never shipped). Every file
parses; the step-io 0.2.5 column is fully probed; OCCT / FreeCAD / commercial
columns are SKIP on this aarch64 host (OCP publishes no aarch64 wheel; FreeCAD
has no apt candidate; commercial readers are not licensed).

| row | step-io 0.2.5 | evidence anchor |
|-----|---------------|-----------------|
| straightness | PASS | 3 carrying files, all surfaced |
| flatness | PASS | 14 files, all surfaced |
| circularity | PASS | 2 files, all surfaced |
| cylindricity | PASS | 2 files, all surfaced |
| profile-of-line | PASS | 1 file, surfaced |
| profile-of-surface | PASS | 15 files, all surfaced |
| angularity | PASS | 1 file, surfaced |
| perpendicularity | PASS | 14 files, all surfaced |
| parallelism | PASS | 4 files, all surfaced |
| position | PASS | 14 files, all surfaced (11+ in ftc_06, complex-instance form) |
| concentricity | PASS | 1 file, surfaced |
| symmetry | PASS | 2 files, all surfaced |
| **circular-runout** | **FAIL** | nist_ctc_05 carries 3 CIRCULAR_RUNOUT_TOLERANCE at #946-#948; step-io surfaces 0 (12 entities dropped on that file). ftc_11 surfaces its 1. |
| total-runout | PASS | 1 file, surfaced |
| datums | PASS | 16 files carry datums; scene surfaces them all |
| **modifiers** | **FAIL** | UNEQUALLY_DISPOSED etc. present (ctc_02 carries 3); all three step-io modifier arenas are 0 on every file and the scene exposes no modifier semantics |

Two honest FAILs surfaced by this audit, both useful:

1. **Circular runout is not reliably preserved by step-io 0.2.5.** A file
   (and edition) dependent drop: some circular-runout instances parse cleanly
   (nist_ftc_11) while others (nist_ctc_05, old AP242 e1 style) are dropped.
   This is exactly the fidelity signal P4-M2-T01/T02 must not paper over.
2. **Tolerance-modifier semantics are lost.** step-io retains the base
   characteristic of a modified tolerance (its complex-unit leaf facet) but no
   modifier state (MMC/LMC, unequally-disposed, statistical). The internal
   `oep-pmi` model has a MaterialCondition slot; today it can only be filled
   from a semantic-PMI reader (P4-M2-T01), not from step-io's surface.

## 5. Beyond the axis (not scored)

- PLUS_MINUS_TOLERANCE (size/location callouts, not one of the 14
  characteristics) is carried widely and retained by step-io in its dedicated
  arena (e.g. 19 in nist_ftc_06); reported here, not given a row.
- STATISTICAL_TOLERANCE appears in 2 corpus files; preservation unscored
  (follow-on probe work).

## 6. Boundary / follow-on work

- This card owns the runner + matrix ONLY. The semantic PMI import model,
  value-level correctness and re-export are P4-M2-T01/T02 cards; the GA-gate
  publication is P4-M5-T03 (STEP fidelity matrix published).
- The OCCT column becomes probeable when an OSS OCCT binding with an aarch64
  wheel exists (or OCCT is built for this host); until then it stays honest
  SKIP. Commercial columns stay SKIP by policy.
