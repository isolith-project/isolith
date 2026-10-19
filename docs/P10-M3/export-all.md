# export-all: the interop escape hatch (P10-M3-T02)

`oep-archive export-all` turns a P8 vault into a documented, neutral archive a
customer can always take out of Keelworks, whatever happens to the product
line. This is the "no lock-in" guarantee: your data lives on in formats any
CAD/CAE/PLM stack can open, plus a complete machine-readable vault record.

## Command

```text
oep-archive export-all --vault <store-dir> --out <archive-dir> [--owner <name>]
oep-archive verify-import --archive <archive-dir> [--out <report>]
oep-archive retention-policy --write <dir>
```

Exit codes: 0 success; 1 failure; 2 usage error. `verify-import` exits 0 only
when the archive re-imports cleanly (`import-report.json` records the proof).

`--vault` points at a persisted `oep-pdm` vault directory (the one written by
`VaultStore::set_persist_dir` / `save`: a `store.json` + `blobs/` layout).

## Archive layout (the documented contract)

```text
<archive>/
  manifest.json            format="oep-archive" v1 (tool, created, summary,
                           retention snapshot, per-artifact records incl. skips)
  vault-metadata.json      format="oep-vault-export" v1: every item, revision,
                           file — ids, names, kinds, states, timestamps, blobs
  retention-policy.json    format="oep-retention-policy" v1 (defaults snapshot)
  checksums.sha256         <sha256>  <relative-path> for every archive member
  geometry/<item>/<rev>/<stem>.step   AP242 passthrough (neutral B-Rep)
  geometry/<item>/<rev>/<stem>.glb    glTF 2.0 binary, via P2 oep-cgr
  geometry/<item>/<rev>/<stem>.stl    binary STL, this crate's writer
  geometry/<item>/<rev>/<stem>.3dxml  3DXML ASCII ZIP, via P3 oep-3dxml
  model/<item>/<rev>/<stem>.oep-geo.json   oep-geo own-format (open JSON)
  raw/<item>/<rev>/<file>  original vault blob, byte-for-byte
```

Item names become safe slugs (`Box Part A` -> `box-part-a`); the manifest
records the mapping back to the real id/name. Paths are deterministic for a
given vault; only `created_at` / `exported_at` timestamps vary between runs
(the bytes never do).

## The guarantee: no proprietary lock-in

- `geometry/` is neutral interchange only — STEP (AP242), glTF 2.0, STL,
  3DXML. Any tool that opens those opens your data.
- `model/` is the oep-geo own-format: open, documented, JSON (schema:
  `docs/p6-m1-oep-geo-model-schema.md`). It is provided for fidelity and the
  in-platform re-import, and is **not** the portability guarantee.
- `raw/` preserves the original vault bytes (which may be a proprietary
  format such as CGR). It is clearly partitioned off and the archive never
  depends on it for portability.

## Format coverage (honest capability)

| Vault file format | STEP | glTF/GLB | STL | 3DXML | oep-geo model |
|-------------------|------|----------|-----|-------|---------------|
| `step`            | passthrough | from facets | from facets | from facets | re-imported JSON |
| `cgr`             | —    | from CGR mesh | from CGR mesh | from CGR mesh | — |
| `oep-geo` (json)  | deferred* | deferred* | deferred* | deferred* | passthrough |
| other/unknown     | —    | —        | —   | —     | — (raw kept) |

*Native oep-geo documents carry no tessellation in the current schema
(P6-M2), so derived geometry is deferred until a tessellation carrier lands;
the raw open JSON is preserved regardless.

Anything a writer rejects is recorded in `manifest.json` as a **skip with a
reason** (tolerant-failure doctrine), never hidden and never fatal.

## verify-import (the acceptance)

`oep-archive verify-import --archive <dir>` performs two independent checks
and writes `import-report.json` (`format="oep-import-report"` v1):

1. **Vault metadata rebuild** — validates `vault-metadata.json` and rebuilds
   every item/revision/file into a fresh store.
2. **oep-geo re-import** — reads every neutral STEP geometry file back through
   `oep_step::parse_to_document` and validates the resulting oep-geo
   documents (parts/bodies/features counted). A genuine STEP with no faceted
   model is a classified skip (P4 doctrine), not a failure; only a corrupted
   payload fails.

Acceptance criterion (P10-M3-T02): `export-all` produces a documented archive
(geometry in interchange formats + JSON vault metadata) that re-imports into
oep-geo, with the retention policy documented and its defaults shipped — and no
data in it depends on a proprietary lock-in format. That is exactly what
`docs/P10-M3/proof/` demonstrates on real material.

## Proof

`docs/P10-M3/proof/` contains a real archive produced from a seeded vault
(committed fixtures + the actual exporters) and its `import-report.json`,
verified by `tests/test_p10_m3_export_all.py`:

- `proof/archive/` — the export-all output (manifest, vault-metadata,
  retention-policy, checksums, geometry/model/raw trees).
- `proof/import-report.json` — the verify-import result: vault rebuilt, the
  faceted STEP re-imported into oep-geo, the geometry-less STEP classified as
  a skip, zero failures.
