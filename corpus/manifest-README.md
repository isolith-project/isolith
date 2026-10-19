# Keelworks Corpus Manifest (P1-M1-T04)

**Task:** build the corpus manifest: source, license, format, size, geometry
class for every located sample — CGR, 3DXML, CATPart/CATProduct, fixtures.

**Files in this module:**

| file | what it is |
|---|---|
| `corpus/manifest.json` | canonical shared-corpus registry, schema v1 |
| `corpus/inventory.json` | complete inventory (shared + reference-only), machine readable |
| `corpus/inventory.md` | same, human readable summary |
| `corpus/tools/build_manifest.py` | deterministic builder (single source of truth) |
| `corpus/tools/3dxml-catalog.json` | machine 3DXML sample catalog (P1-M1-T02 evidence) |
| `tests/manifest_test.py` | acceptance test (TDD; runs standalone + pytest) |

## Schema

`corpus/manifest.json` keeps the schema v1 inherited from the P0-M4-T02 corpus
harness (its `python/oep_tooling/corpus/registry.py` reads it unchanged).
Per entry:

    id             unique slug (repo entries: the on-disk filename)
    path           path relative to corpus/ ("data/..." repo, "object/..." store)
    format         catpart-v5 | catproduct-v5 | 3dxml-zip | 3dxml-ascii |
                   cgr-v5 | stl-ascii
    geometry_class prism | surface | assembly | dataset | text
    source         provenance (clean-room invariant)
    license        SPDX or explicit verdict
    size           bytes
    sha256         64-hex content hash (integrity source of truth)
    backend        "repo" (committed) | "object-store" (NAS Gitea generic pkg)
    object_store   {owner, package, version, object} where backend=object-store

## Geometry classes (controlled vocabulary)

| class | meaning | examples here |
|---|---|---|
| `prism` | single solid, volume-modeled component | box.stl, 9 CATPart, DMU mechanical CGRs, cube/blocks/machinist 3DXML, 114 upstream CATParts |
| `surface` | surfacic / shell / freeform geometry | (none yet — piston CGR carries surfacic rep records) |
| `assembly` | multiple placed parts (hierarchy + transforms) | knuckle-joint.CATProduct, cycloidal + FM1_satellite 3DXML, 7 upstream |
| `dataset` | non-geometric / structural data | quad.3dxml (ProductStructure + materials only) |
| `text` | text payloads | (unused) |

Vocabulary is shared with P0-M4-T02 so the harness geometry filter and the
P2/P3 regression matrix can select targets by class.

## Backend split (redistribution discipline)

- **repo (17):** box.stl fixture + 10 CATPart/CATProduct + 6 committed 3DXML.
  Verified on real bytes here (size + sha256 recomputed from `corpus/data/`).
- **object-store (122):** box.stl.object mirror + the 121 MIT CATPart blobs
  from P1-M1-T03's full registry. Content-addressed by sha256; uploaded to the
  NAS object store by P1-M1-T05. Until then the harness resolves them as a
  classified SKIP (tolerant, never a crash).
- **reference-only (13):** 11 CGR + 2 3DXML samples have NO redistribution
  grant. They are NOT manifest entries and are **never committed or synced**;
  they live in `corpus/inventory.json` (role=reference) with source URL, license
  verdict, size and sha256. Developers fetch them into a private cache; the
  acceptance test re-verifies their hashes against those real copies when
  present (no silent drift).

## Regeneration (deterministic, doctrine #3)

    python3 corpus/tools/build_manifest.py             # regenerate outputs
    python3 corpus/tools/build_manifest.py --check     # prove committed = fresh
    CGR_CACHE_DIR=/path/with/cgr/files python3 corpus/tools/build_manifest.py

House rule: never hand-edit `manifest.json` / `inventory.*`. Change an input
registry, re-run the builder, commit builder + outputs together. The acceptance
test fails if a committed output no longer matches a fresh build.

## Provenance

Inputs folded here from sibling deliveries: `P1-M1/cgr-sample-registry.json`
(P1-M1-T01), `corpus/catpart-samples.json` + `corpus/catpart-source-full-registry.json`
(P1-M1-T03), `corpus/tools/3dxml-catalog.json` (P1-M1-T02, from its catalog +
real-material rep inspection), `corpus/data/*` real bytes, and the box.stl
fixture (P0-M4-T02). Clean-room invariant held: no vendor binary is committed
here beyond the license-clean corpus files the sibling tasks already cleared.
