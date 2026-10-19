# P3-M5-T02 — P3 GA gate: corpus of 3DXML files opens; surfaces tessellate; round-trip stable

> Task: t_2e96ec9c (Keelworks wave 1, P3 capstone). Status: implemented +
> verified on real material 2026-10-16. Version 1.0.
> Repo: keelworks/keelworks @ NAS Gitea; branch task/3dxml-ga-gate-1016a.
> Clean-room: no vendor library reads any 3DXML/CGR; no proprietary bytes
> committed; every constant is a published clean-room observation (docs/re-notes
> 3dxml-hex-notebook / gregory-patch-decode / p3-m5-t01-3dxml-regression) or a
> public ZIP/XML/Web3D standard fact.

## 1. What this card delivers

The P3 General-Availability gate: a deterministic acceptance harness that runs
ALL THREE hard legs of the P3 milestone contract over the workstream:

| Leg | Contract (plan P3-M5-T02) | Classified outcome |
|-----|---------------------------|--------------------|
| OPEN       | corpus of 3DXML files opens | every corpus 3DXML reaches `OPEN` (coherent ZIP container BOM / standalone ASCII model) or a closed-taxonomy `SKIP`; zero panics, no silent drop |
| TESSELLATE | surfaces tessellate        | every surface-rep file (XvlShell lattice) decodes to a finite, in-range, non-degenerate triangle mesh with quality control; real-model topology goldens where recorded |
| ROUNDTRIP  | round-trip stable          | write -> read-back -> re-write is a byte-class fixed point (strict byte equality for OUR encoder; content-level fixed point honored for a foreign-writer archive) |

Deliverables:

- `python/oep_tooling/ga_gate_3dxml.py` — pure-stdlib gate runner:
  `open_leg()` / `tessellate_leg()` / `roundtrip_leg()` / `run_gate()` /
  `build_report()` / `exit_code()` / `write_report()`, CLI
  `python -m oep_tooling.ga_gate_3dxml`.
- `tests/ga_gate_3dxml_test.py` — TDD acceptance test (written FIRST, RED on
  the module-less base as `ModuleNotFoundError`, now GREEN; 9 checks).
- `.gitea/workflows/ga-gate-3dxml.yaml` — P3 gate CI job (separate workflow
  file; the P2-M6-T03 gate job edits `ci.yaml` on its own open branch, so a
  NEW workflow avoids that collision).
- `docs/re-notes/p3-m5-t02-ga-gate.md` — this record.

Composition (reuse, not rewrite): OPEN leg escalates to the sibling P3-M5-T01
shared regression sweep (`oep_tooling.regression`) when importable and falls
back to a self-contained probe built from the same published constants
otherwise. TESSELLATE/ROUNDTRIP legs are self-contained; ROUNDTRIP reads a
foreign-writer archive (e.g. the sibling P3-M4-T01 Rust golden) back and
distinguishes true geometry drift from writer byte-layout.

## 2. Honesty boundary (why SKIP rows are contract, not weakness)

- OPEN: a ZIP that is a coherent 3DXML container OPENs; a ZIP with an incoherent
  or absent `Manifest.xml` BOM is `unknown-format`; a truncated blob is
  `truncated`. A surface-rep `.x3d` sample is `unsupported-format` in OPEN
  (it is the TESSELLATE leg's material). Never a crash, never a fabricated OPEN.
- TESSELLATE: quality control rejects out-of-range face indices and non-finite
  coordinates as classified SKIP (never a fabricated PASS); topology goldens
  (vertex/face/triangle counts) turn any count drift into a hard FAIL.
  `XvlShellProtoDeclare.x3d` is SKIP `no-mesh-found` — it is a prototype
  declaration with no concrete lattice, an honest, reasoned skip.
- ROUNDTRIP: OUR deterministic encoder is proven to be a strict byte-class fixed
  point (P1 == encode(decode(P1))). A foreign-writer archive whose MEMBER
  CONTENTS decode and re-encode to identical bytes (verified member-by-member:
  Manifest.xml, model doc, rep) but whose low-level ZIP framing differs is a
  classified SKIP `writer byte-layout differs` — valid, not a FAIL, and never a
  fabricated PASS. A drifted/corrupted archive is never given a PASS.

## 3. Real-material verification transcript (2026-10-16)

Material: corpus/data/3dxml samples (P1-M1-T02 branch), the Web3D LatticeXvl
surface corpus (P3-M2-T01 branch, BSD-style license committed there), and the
P3-M4-T01 writer's byte-locked golden `box-reference.3dxml`.

```
P3 GA gate: total=18 pass=9 open=7 fail=0 skip=2 | exit=0
```

- OPEN leg (7 rows): all 6 real corpus files OPEN —
  `FM1_satellite.3DXML`, `blocks.3dxml`, `cube1.3dxml`, `cycloidal.3DXML`,
  `machinist.3DXML` (coherent ZIP BOM) + `quad.3dxml` (standalone ASCII) —
  and the writer's `box-reference.3dxml` OPEN. This closes the previous
  P2-era `unsupported-format (P3 scope)` gap: the corpus now opens.
- TESSELLATE leg (10 rows): 9 real XvlShell models tessellate with the recorded
  topology goldens (Star 78v/80f/152t, Dolphin 199v/193f/394t, Baby
  570v/528f/1112t, Hand 440v/422f/872t, Octopus 239v/229f/458t, AttributeAll
  184v/138f/276t, + 3 attribute demos 24v/18f/36t) = PASS. ProtoDeclare -> SKIP
  `no-mesh-found` (prototype declaration, no concrete lattice).
- ROUNDTRIP leg (1 row): the sibling Rust writer's golden read back decodes to
  the exact 8-vert/12-tri unit box, and re-encoding reproduces all three member
  contents byte-for-byte (`Manifest.xml`, `box.3dxml`, `box.3DRep` identical;
  only the ZIP frame layout differs) = SKIP `writer byte-layout differs`
  (honest — not a fabricated PASS, not a FAIL). OUR canonical encoder's own
  write->read->write cycle is a strict byte-identical PASS (test 4).
- Zero FAIL rows, zero panics, no dropped files.

CI-safe degraded run (fresh main-no-corpus base): `total=2 skip=2 fail=0
exit=0` — the gate degrades to classified SKIP instead of crashing.

## 4. Exit code / report contract

- Exit 0 iff no FAIL row anywhere (`SKIP` allowed) — identical semantics to the
  P0-M4-T02 golden runner, P2-M5-T03 regression sweep, and P2-M6-T03 GA gate.
- Report `ga-gate-3dxml-matrix.{json,md}` is self-consistent (summary totals
  roll up to the emitted rows; schema_version 1).
- CLI flags: `--manifest`, `--corpus-root`, `--samples` (repeatable),
  `--surface` (repeatable), `--golden-pkg`, `--out`.

## 5. Boundary / anti-collision

This card owns the P3 GA gate harness only (new file paths: ga_gate_3dxml.py,
its test, this RE note, a NEW workflow file). It does NOT edit sibling-owned
paths: `regression.py` (P3-M5-T01), `ga_gate.py`/`ga_gate_test.py`/`ci.yaml`
(P2-M6-T03), the `oep-3dxml` crate sources (P3-M1/M2/M4 cards), or
`corpus/data` (P1 corpus cards). Git ls-tree scan of every open task branch for
the four delivered paths returned no hits.
