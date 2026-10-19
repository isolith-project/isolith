# P3-M5-T01 — Regression runner shared with P2 corpus infra: 3DXML open path

> RE / tooling note. Clean-room: every fact below is a published observation
> (P1-M2-T04 hex notebook §3/§6 and the P3-M1-T01 golden map) or a public
> ZIP/XML standard fact. No proprietary bytes are present in this repo.

## What this is

P2-M5-T03 delivered `python/oep_tooling/regression.py` as the P2/P3-shared
corpus regression runner and explicitly reserved it as the infra that
P3-M5-T01 reuses. This task makes that sharing real for 3DXML: instead of a
blanket `unsupported-format` skip ("P3 scope"), every 3DXML corpus file now
gets a real open check with BOM coherence — **OPEN** when the container is
coherent, or a classified skip (`truncated` / `unknown-format`) otherwise.

No new runner, no Rust, no new format dispatcher. The same module, the same
closed taxonomy, the same matrix output. The 3DXML dispatch is a pluggable
branch beside the CGR/STL ones, exactly as the P2 re-notes promised.

## 3DXML container probe (deterministic, no vendor code)

`probe_3dxml_container(data)` classifies the container from its first bytes:

- **Standalone ASCII model document** — first 4 bytes `3C 3F 78 6D` (`<?xm`).
  Observed on `quad.3dxml` (P1-M2-T04 §6). → `OPEN`.
- **ZIP-archive 3DXML** — first 4 bytes `50 4B 03 04` (`PK\x03\x04`, public
  PKWARE APPNOTE local-file header). Opens the archive (stdlib `zipfile`) and
  requires **BOM coherence**:
  1. the archive contains a member named exactly `Manifest.xml`;
  2. that manifest declares `<Root>NAME.3dxml</Root>` (regex-scanned);
  3. the declared root names a member that is actually present in the archive.
  All three hold → `OPEN`. Any failure → classified skip:
  - invalid/truncated ZIP (`zipfile` error) → `unknown-format`;
  - no `Manifest.xml` member (a ZIP, but not a 3DXML) → `unknown-format`;
  - `<Root>` missing or its value not an archive member (BOM incoherent) →
    `unknown-format`.
- **Neither magic under 4 bytes** → `truncated`; anything else →
  `unknown-format`. Never raises (tolerant model, P2-M1-T04).

Observed on the real corpus 2026-10-15: all 5 committed ZIP-archive samples
(`blocks.3dxml` 3 members/@`Abaqus_Model.3dxml`, `cube1.3dxml` 7/@
`Abaqus_Model.3dxml`, `machinist.3DXML` 9/@`NameSetTree.3dxml`,
`cycloidal.3DXML` 28/@`NameSetTree.3dxml`, `FM1_satellite.3DXML`
69/@`NameSetTree.3dxml`) open with BOM coherence; `quad.3dxml` (standalone)
opens; the reference-only Dassault gallery sample `press_rink_axe_fta.3dxml`
(not committed) also opens as a coherent ZIP-archive.

## External reader escalation (OEP_3DXML_READER)

Mirrors `OEP_CGR_READER`: when an external P3 3DXML open CLI is configured via
`OEP_3DXML_READER` (or an `oep-3dxml*` CLI appears on PATH/target/release) it is
run as the authoritative open check and its exit code is mapped per the
documented contract:

| exit | meaning | row |
|---|---|---|
| 0 | opens | `OPEN` (reader=cli) |
| 3 | not a 3DXML container | `SKIP unknown-format` |
| 4 | valid container, no mesh | `SKIP no-mesh-found` |
| 2 | IO failure | `SKIP io-error` |
| other | unexpected | `SKIP unknown-format` (detail = stderr) |

Today no `oep-3dxml` CLI ships (it is a P3-M4/M5 follow-on), so the probe is
authoritative and the matrix reports `reader=probe`. The hook is live and
tested via a fake reader (exit 0 → OPEN reader=cli; exit 3 → SKIP reader=cli).

## CLI

`python3 -m oep_tooling.regression` — the same command as P2-M5-T03, now with
the 3DXML branch wired in. Sample dirs come from `--samples`,
`OEP_CGR_SAMPLES`, or `OEP_3DXML_SAMPLES` (any `.3dxml`/`.3drep`/`.stl` file in
a sample dir is dispatched by extension, not forced to CGR).

## Verification transcript (real material, 2026-10-15)

Full manifest sweep (139 entries) + 3DXML sample dirs → **141 rows, all
classified**:

- 6 repo 3DXML entries OPEN (`FM1_satellite.3DXML`, `blocks.3dxml`,
  `cube1.3dxml`, `cycloidal.3DXML`, `machinist.3DXML` zip-archive +
  `quad.3dxml` standalone) — previously `unsupported-format`, now OPEN.
- Reference sample `press_rink_axe_fta.3dxml` OPEN (via `OEP_3DXML_SAMPLES`).
- `box.stl` OPEN (unchanged STL path).
- 114 catpart-v5 + 7 catproduct-v5 + 1 stl object-store entries →
  `object-store-unresolved` (unchanged, tolerant).
- 9 `catpart-v5` + 1 `catproduct-v5` → `unsupported-format` (P5 scope,
  unchanged); 1 `.md` inside a sample dir → `not-a-cgr-container` (tolerant
  fallback: sample dir files not `.3dxml`/`.stl` still class, never crash).

Acceptance: `tests/corpus_regression_test.py` extended (30 pass: 22 P2 + 8 new
3DXML legs — probe opens, BOM incoherence, hostile inputs, CLI escalation,
real material). Matrix at `reports/3dxml-sweep/regression-matrix.{json,md}`.

## Boundary / anti-collision

This card touches only `python/oep_tooling/regression.py` (the P2-M5-T03 file,
merged in as upstream), its acceptance test, and this re-notes doc. The
`oep-3dxml` Rust crate (outer ZIP/BOM parse) is P3-M1 scope on a parallel card;
the mesh/tessellation and writer are P3-M2/M3/M4 cards. This runner intentionally
does NOT depend on any of those crates — the probe is a deterministic,
dependency-free stand-in that stays in the shared Python infra.
