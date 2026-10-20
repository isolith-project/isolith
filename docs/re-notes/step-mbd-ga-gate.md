# P4-M5-T03 — P4 GA gate: STEP fidelity matrix published; MBD validator demo on
# real corpus material

> Task: t_488ab698 (Keelworks wave 2, P4 capstone). Status: implemented +
> verified on real material 2026-10-16. Version 1.0.
> Repo: keelworks/keelworks @ NAS Gitea; branch task/p4m5t03-ga-gate-1016a.
> Gate classification: **PASS** — both P4 GA criteria evidenced on real corpus
> material; exit code contract + CI-safe degraded run the same as the
> P2-M6-T03 / P3-M5-T02 GA gates.
> Clean-room: no vendor library reads any STEP/CGR/3DXML; the real STEP PMI
> corpus rows are imports produced by our OWN P4-M2-T01 `oep-step-pmi`
> importer over public NIST MBE PMI AP242 fixture files already committed under
> `crates/oep-step-pmi/fixtures` (P4-M2-T01 branch). No proprietary bytes.

## 1. What this card delivers

The P4 General-Availability gate: a deterministic acceptance harness that runs
BOTH hard legs of the P4 milestone contract over the published artifacts and the
real corpus:

| Leg | Contract (plan P4-M5-T03) | Classified outcome |
|-----|---------------------------|--------------------|
| FIDELITY | STEP fidelity matrix published (P4-M2-T03 PMI fidelity matrix + P4-M1-T03 accuracy report) | matrix artifact parses, declares schema, every GD&T x app cell is a closed-set verdict with evidence, GD&T types match the cells; accuracy report measured rows within-epsilon (symmetric Hausdorff <= eps AND |volume_rel_error| <= eps) |
| MBD      | MBD validator demo on real corpus material | `oep_tooling.mbd_validate` (P4-M5-T01, re-used, not re-implemented) runs over the real STEP PMI corpus rows (NIST AP242 -> oep-pmi via P4-M2-T01) plus the committed P4-M5-T01 fixture manifest; every declared input reaches a checked report (no drop); combined report honest (rollup == granular, UNVERIFIED never PASS); JSON + Markdown written |

Deliverables:

- `python/oep_tooling/ga_gate_p4.py` — pure-stdlib gate runner:
  `fidelity_leg()` / `mbd_leg()` / `run_gate()` / `build_report()` /
  `exit_code()` / `write_report()`, CLI `python -m oep_tooling.ga_gate_p4`.
- `tests/ga_gate_p4_test.py` — TDD acceptance test (written FIRST, RED on the
  bare main as `ModuleNotFoundError`, now GREEN).
- `.gitea/workflows/ga-gate-p4.yaml` — P4 gate CI job (a NEW workflow file;
  the P2-M6-T03 gate edits `ci.yaml` on its own open branch, so a new file
  avoids that collision, same as the P3-M5-T02 gate).
- `docs/re-notes/step-mbd-ga-gate.md` — this record.
- `corpus/mbd-real-step-pmi/*.oeppmi.json` + `corpus/mbd-real-step-pmi-manifest.json`
  — the real STEP PMI corpus rows (the MBD "real corpus material" half).
- `corpus/ga-gate-p4-matrix.{json,md}` + `corpus/ga-gate-p4-mbd-report.{json,md}`
  — committed generated evidence of the real-material run.

Composition (reuse, not rewrite): FIDELITY leg reads the sibling-published
artifacts (P4-M2-T03 matrix under `docs/pmi-fidelity/`, P4-M1-T03
`corpus/accuracy-report.json`) when present on the ref. MBD leg escalates to the
sibling P4-M5-T01 engine (`oep_tooling.mbd_validate`) when importable and uses
its `run_manifest` / `build_report` / `render_markdown` unchanged; the gate
writes the combined report under gate-owned filenames (`ga-gate-p4-mbd-report.*`)
so it never collides with the sibling `corpus/mbd-validate-report.*` path when
both branches merge.

## 2. Honesty boundary (why FAIL cells are findings, not gate FAILs)

- FIDELITY: the P4-M2-T03 matrix honestly records 2 FAIL cells under step-io
  0.2.5 (circular runout dropped in nist_ctc_05; tolerance-modifier semantics
  empty on every file). Those are DOCUMENTED capability gaps of the reference
  reader — the whole point of the matrix is to record them. The gate verifies
  the matrix is published, self-consistent and within-epsilon where measured; it
  never turns an honest FAIL cell into a fabricated PASS, nor does a true
  capability finding close the gate (that is the audit's output, not its input).
- FIDELITY within-epsilon: a MEASURED accuracy row out of eps is a hard gate
  FAIL (geometry drift is real, never relabeled). Absent artifacts (sibling
  branch not merged on this ref) are classified SKIP from the closed taxonomy
  `{missing-on-disk, fidelity-matrix-unpublished, accuracy-report-unpublished,
  mbd-engine-unavailable, mbd-manifest-unavailable, no-measured-rows,
  not-faceted, io-error}` — never a crash, never a fabricated PASS.
- MBD: real STEP PMI rows surface the engine's honest findings — e.g.
  nist-ftc-06 surfaces 2 FAILs under both rule sets (datum features carry no
  designated feature/body from the STEP import seam; units `MILLIMETRE` not in
  the oep-geo accepted alias set -> WARN; model facts absent -> UNVERIFIED).
  These are the validator DEMO working, not gate failures: UNVERIFIED never
  counts as PASS, and every declared input is dropped-checked.

## 3. Real-material verification transcript (2026-10-16)

Merged view = gate module + sibling engine (P4-M5-T01 `mbd_validate.py`) on
PYTHONPATH + sibling fidelity artifacts (P4-M2-T03 matrix, P4-M1-T03 accuracy
report) + real STEP PMI corpus rows produced by the real P4-M2-T01 importer from
public NIST AP242 fixtures. Full run of the CLI:

```
PYTHONPATH=<merged python> python3 -m oep_tooling.ga_gate_p4 \
    --fidelity-dir docs/pmi-fidelity \
    --accuracy-report corpus/accuracy-report.json \
    --step-pmi-manifest corpus/mbd-real-step-pmi-manifest.json \
    --corpus-root corpus --out corpus
P4 GA gate: total=6 pass=6 fail=0 skip=0 | exit=0
```

- FIDELITY leg (2 rows): `pmi-fidelity-matrix` PASS — schema ok, 64 cells
  (PASS 14 / FAIL 2 / SKIP 48; the 2 FAIL cells are the documented step-io
  capability gaps, not gate FAILs); `keelworks-box` PASS — Hausdorff 0.0,
  volume_rel_error 0.0 vs eps 1e-9 (real P4-M1-T03 measured row). This closes
  criterion (1): the STEP fidelity matrix is published and honest.
- MBD leg (4 rows): `nist-ftc-06`, `nist-ftc-08`, `nist-stc-06` all PASS —
  each real input reached a checked report under both y4141 and prime (no
  drops); `mbd-report` PASS — writes `ga-gate-p4-mbd-report.{json,md}`, summary
  rollup == granular (inputs=6 findings=102 pass=72 fail=12 warn=6 unverified=12).
  The 12 FAIL findings are the validator surfacing real STEP-seam gaps (floating
  datums, unknown unit symbol), which is the demo working. This closes
  criterion (2): MBD validator demo on real corpus material.
- Zero gate-FAIL rows, zero panics, no dropped inputs.

CI-safe degraded run (fresh main-no-sibling base, this branch alone):

```
total=3 pass=0 fail=0 skip=3 | exit=0
```

— the gate degrades to classified SKIP (fidelity artifacts + MBD engine absent
until the sibling P4 PRs merge) instead of crashing or fabricating.

TDD acceptance (`tests/ga_gate_p4_test.py`): RED on bare main
(`ModuleNotFoundError: No module named 'oep_tooling.ga_gate_p4'`) -> GREEN after
the module lands; 7 test groups covering module API, matrix publish + within-
epsilon PASS, out-of-epsilon FAIL, absent-artifact SKIP, real STEP PMI through
the engine, run_gate report/exit-code self-consistency, bare-main degradation.

## 4. Exit code / report contract

- Exit 0 iff no gate-FAIL row anywhere (SKIP allowed) — identical semantics to
  the P2-M6-T03 / P3-M5-T02 GA gates, the P0-M4-T02 golden runner, and the
  P2/P3 regression sweeps.
- Report `ga-gate-p4-matrix.{json,md}` is self-consistent (summary totals roll
  up to the emitted rows; schema_version 1).
- CLI flags: `--fidelity-dir`, `--accuracy-report`, `--step-pmi-manifest`,
  `--corpus-root`, `--out`, `--eps`.

## 5. What blocks closure, honestly

- NOTHING currently blocks P4 GA on the two gate criteria: both the fidelity
  matrix publication and the MBD validator demo on real corpus are evidenced
  above (PASS, exit 0, CI green).
- The P4 GA gate contract does NOT require zero capability gaps in the reference
  reader (step-io 0.2.5). Two known PMI fidelity gaps (circular runout;
  MMC/LMC / unequally-disposed modifiers) and the STEP-seam datum/feature and
  unit-symbol findings surfaced by the validator are recorded explicitly in the
  published matrix and the MBD report as honest findings for follow-on cards
  (P4-M2-T02 AP242 export correctness, P4-M3 STEP export, importer seam work) —
  not hidden, not gated as failures.

## 6. Boundary / anti-collision

This card owns the P4 GA gate harness only (new file paths: ga_gate_p4.py, its
test, THIS workflow file, THIS re-note, the real STEP PMI corpus rows + manifest,
and the gate-owned reports `ga-gate-p4-matrix.*` / `ga-gate-p4-mbd-report.*`).
It does NOT edit sibling-owned paths: `mbd_validate.py` and
`corpus/mbd-fixture-manifest.json` (P4-M5-T01), `pmi_fidelity.py` +
`docs/pmi-fidelity/` (P4-M2-T03), `step_fidelity.py` +
`corpus/accuracy-report.*` (P4-M1-T03), `crates/oep-step-pmi/` (P4-M2-T01), or
`corpus/mbd-validate-report.*`. The MBD report filename was deliberately
gate-prefixed to avoid a merge collision on the sibling's
`corpus/mbd-validate-report.*` path. Git ls-tree scan of open task branches for
the delivered paths returned no hits.
