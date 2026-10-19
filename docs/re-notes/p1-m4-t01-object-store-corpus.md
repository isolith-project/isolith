# P1-M4-T01 — Object-Store Corpus Mirror Materialized + Verified (wave-2 prep, flag 3)

Status: DONE. Closes waveform-analysis flag 3 (wave-1: 122/139 shared corpus
entries resolved `SKIP object-store-unresolved` — the blobs were never on the
store). Every backend=object-store entry of `corpus/manifest.json` is now
materialized on the NAS object store, content-addressed by sha256, and the
golden runner + P2/P3 regression runner resolve entries from it end to end.

## What changed

- **Mirror materialized (122/122 blobs on the NAS object store).** Walks
  `corpus/manifest.json` (P1-M1-T04 registry, the single source of truth);
  every `backend=object-store` entry is uploaded to the NAS Gitea generic
  package registry `don/keelworks-corpus` v1 under the exact object name its
  manifest `object_store` pointer names (`<sha256>.<ext>`), source bytes
  matched by sha256 (never by filename), and GET-back verified against the
  manifest hash. 121 upstream MIT CATPart/CATProduct blobs
  (github.com/mohanevs/solid-modeling-catia-v6) + the box.stl fixture mirror
  (`box.stl.object`). Idempotent: re-running dedups by object name (409-safe).
- **`python/oep_tooling/objectstore.py`** gains `mirror()` (materialize the
  whole manifest), `verify_manifest_mirror()` (fetch-every-blob, hash vs
  manifest), and makes `upload_file()` idempotent on 409 (prior upload present
  → verify hash-match instead of failing), so acceptance re-runs stay green.
- **`corpus/tools/mirror_object_store.py`** new operator CLI:
  `--mirror --data-root <dir>...`, `--verify`, `--check` (deterministic rebuild
  diff).
- **`python/oep_tooling/regression.py`** (P2/P3-shared corpus runner) now
  resolves object-store entries from the NAS object store end to end: a
  `--fetch` mode materializes a missing cache blob via the golden-runner store
  backend before classification (tolerant: store down → `object-store-unresolved`
  skip, never a crash). Rows and the report matrix now carry `backend`
  (repo/object-store) and the summary publishes `by_backend` counts.
- **`corpus/object-store-index.json`** regenerated: commits the full 122-entry
  mirror (status=uploaded) as proof, preserving the wave-1 P1-M1-T05 sync/index
  entries by spec id and the reference-only records.
- **`tests/corpus_object_mirror_test.py`** new acceptance test (TDD): pointer
  wellformedness, index covers all 122, live NAS checksum verify, live
  end-to-end `--fetch` sweep of all 139 entries. Live-NAS sections classify
  SKIP (exit 0) when the store is unreachable — tolerant doctrine.
- **P1-M1-T06 privacy/ITAR sweep kept**: re-run over all 139 full-corpus bytes
  (121 upstream + repo data) on 2026-10-16 → 139/139 clean, 0 HIGH, gate PASS.
  Evidence: `workstreams/P1-M1/privacy-itar-sweep-report-full-2026-10-16.json`.

## How to re-run

    # hermetic determinism of the committed index (no network)
    python3 corpus/tools/mirror_object_store.py --check \
        --data-root <local-source-dir>...

    # live: upload missing blobs, write the index, GET-verify every blob
    python3 corpus/tools/mirror_object_store.py --mirror \
        --data-root <local-source-dir>...

    # live, read-only: verify every blob GETs back and re-hashes to the manifest
    python3 corpus/tools/mirror_object_store.py --verify

    # full regression matrix, object store end-to-end, counts by backend
    python3 -m oep_tooling.regression --manifest corpus/manifest.json \
        --cache <cache-dir> --fetch --out corpus/

    # golden runner resolves the same mirror
    python3 -m oep_tooling.corpus fetch --cache <cache-dir>

## Results (real material, 2026-10-16, U1 → live NAS)

- Object store mirror: 122/122 blobs uploaded and GET-back verified; every
  checksum equals the manifest sha256 (0 pending, 0 mismatch).
- Regression sweep of the shared 139-entry manifest with `--fetch`
  (object store end-to-end): 139/139 rows classified; `object-store-unresolved`
  = 0, `missing-on-disk` = 0.
  - by_backend: repo 17 (7 OPEN, 10 classified SKIP native/P5-scope);
    object-store 122 (1 OPEN = box.stl.object, 121 classified SKIP
    native/P5-scope on real V5_CFV2-magic bytes).
  - total OPEN 8 (box.stl, 6 repo 3DXML, box.stl.object); SKIP 131 (native
    CATPart/CATProduct — real reason: V5_CFV2 container, native reader is P5
    scope).
- No NEW skips vs wave 1: the wave-1 OPEN set (box.stl + the 6 repo 3DXML)
  is still OPEN; the only new OPEN is box.stl.object (was 122-unresolved).
- Privacy/ITAR sweep over all 139 full-corpus bytes: gate PASS, 0 findings.

## Files touched

- python/oep_tooling/objectstore.py (+mirror/verify_manifest_mirror, 409-safe upload)
- python/oep_tooling/regression.py (--fetch, backend field, by_backend matrix)
- corpus/tools/mirror_object_store.py (new operator CLI)
- corpus/object-store-index.json (full 122-entry mirror proof)
- corpus/regression-matrix.json + .md (published matrix with backend counts)
- tests/corpus_object_mirror_test.py (new acceptance test)
- workstreams/P1-M1/privacy-itar-sweep-report-full-2026-10-16.json (+ md note)
- docs/re-notes/p1-m4-t01-object-store-corpus.md (this note), corpus/README.md,
  docs/layout.md (seam note)
