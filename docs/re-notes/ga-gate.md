# P2-M6-T03 — P2 GA gate: all corpus meshes load, 100% golden files match within epsilon

> Task: t_e8c603e4 (Keelworks wave 1, P2 capstone). Status: implemented +
> verified on real material 2026-10-15. Version 1.0.
> Repo: keelworks/keelworks @ NAS Gitea; branch task/ga-gate-1015a.
> Clean-room: no vendor library reads any CGR; no proprietary bytes committed;
> every constant is a published clean-room observation (docs/re-notes
> oep-cgr-header-parse / P1-M2-T01 hex notebook).

## 1. What this card delivers

The P2 General-Availability gate: a deterministic acceptance harness that runs
BOTH hard legs of the P2 milestone contract over the whole corpus:

| Leg | Contract | Exit |
|-----|----------|------|
| LOAD  | every corpus CGR mesh reaches a CLASSIFIED outcome — `OPEN` (reader header/mesh opens) or a `SKIP` from the closed taxonomy — zero panics, no silent drop | part of gate |
| GOLDEN | 100% of golden-file tests PASS within their numeric tolerance (epsilon); zero `FAIL` rows | part of gate |

Deliverables:

- `python/oep_tooling/ga_gate.py` — pure-stdlib gate runner:
  `load_leg()`, `golden_leg()`, `run_gate()`, `build_report()`, `exit_code()`,
  CLI `python -m oep_tooling.ga_gate`.
- `tests/ga_gate_test.py` — TDD acceptance test (written FIRST, RED on the
  P0-only base, now GREEN).
- `docs/re-notes/ga-gate.md` — this record.
- CI: `gate` job added to `.gitea/workflows/ci.yaml`.

Composition (reuse, not rewrite): LOAD leg escalates to the sibling P2-M5-T03
regression sweep (`oep_tooling.regression`) when importable and falls back to a
self-contained probe built from the same published constants otherwise. GOLDEN
leg escalates to the P0-M4-T02 corpus-harness golden runner
(`oep_tooling.corpus.runner`) when importable and falls back to a self-contained
text|bytes|numeric-with-tolerance comparator otherwise.

## 2. Honesty boundary (why SKIP rows are contract, not weakness)

The clean-room P2 reader on the OLD-ERA reference corpus (R11 DMU tutorial set
and the 2007 piston) documents an RE wall: the mesh extractor returns a *typed,
reasoned* `MeshNotFound` skip for those payloads because the triangle-stream
offset cannot yet be locked by structural scan (see docs/re-notes
oep-cgr-mesh-extraction.md §3, §6). The GA gate therefore locks the honest,
non-fabricating contract: every corpus mesh is CLASSIFIED (OPEN or a closed-
taxonomy SKIP), never a crash, never invented geometry. A golden row whose
spec/input/golden is absent is a classified SKIP, never a FAIL and never a
fabricated PASS — so CI stays green on main before the corpus PRs
(#30 corpus-manifest, #14 corpus-harness) merge, exactly like the sibling
P2-M5-T03 regression card convention.

## 3. Real-material verification transcript (2026-10-15)

Run:  PYTHONPATH=$PWD/python python3 -m oep_tooling.ga_gate \
         --corpus-root <corpus-manifest>/corpus \
         --spec <corpus-manifest>/corpus/golden_tests.json \
         --samples /home/r1a/.hermes/cache/scratch/cfysa \
         --samples /home/r1a/.hermes/cache/scratch/piston

```
GA gate: total=24 pass=1 open=11 fail=0 skip=12 | exit=0
```

- LOAD leg (22 rows): 11 real CGR meshes (ATOMIZER, BODY1, BODY2, LOCK,
  NOZZLE1, NOZZLE2, REGULATION_COMMAND, REGULATOR, TRIGGER, VALVE@cfysa +
  piston@piston) → OPEN. 11 `_head_*` 24-byte fragments → SKIP `truncated`
  (they are head dumps, not complete CGRs — correct classification).
- GOLDEN leg (2 rows): `box-triangle-count` → PASS within tolerance
  (12 facets / 36 vertices match the committed golden); `box-domain-object` →
  SKIP (its object-store fixture `box.stl.object` is not materialized locally;
  absent input, not a golden failure).
- Zero FAIL rows, zero panics, no dropped files.

Epsilon proof (independent numeric leg, tests/ga_gate_test.py): matching
geometry PASSes; moving one box vertex 1.0→1.5 produces
`FAIL token 1 off by 0.5 (tolerance 1e-06)` — the within-epsilon gate rejects
geometry drift at 1e-6.

CI-safe degraded run (fresh main, no corpus): `total=1 skip=1 fail=0 exit=0`
— the gate degrades to a classified SKIP instead of crashing.

## 4. Exit code / report contract

- Exit 0 iff no `FAIL` row anywhere (`SKIP` allowed) — identical semantics to
  the P0-M4-T02 golden runner and the P2-M5-T03 regression sweep.
- Report `ga-gate-matrix.{json,md}` is self-consistent: summary totals roll up
  to the emitted rows; schema_version 1.
- CLI flags: `--manifest`, `--corpus-root`, `--spec`, `--samples` (repeatable),
  `--out`, `--reader`.

## 5. Boundary / anti-collision

This card owns the GA gate harness only. The CGR mesh reader, glTF/STEP writers
and CLIs are separate P2-M2/M5 cards on their own branches; when those binaries
land, the gate's LOAD leg escalates automatically via `OEP_CGR_READER`/PATH and
the matrix gains `reader=cli` rows. Sibling P2-M5-T03 owns the regression sweep;
P0-M4-T02 owns the corpus harness. New files only added here (ga_gate.py, its
test, RE note, CI job) — no edits to sibling-owned paths.
