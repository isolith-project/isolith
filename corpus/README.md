# Isolith Test Corpus

The shared test-data registry plus golden-file runner (P0-M4-T02). Downstream
consumers: P1 corpus build (manifest + sync), P2/P3 regression runners
(classified-skip matrix), P12 QA golden corpus (runner in CI).

## Layout

    corpus/
      manifest.json        the registry: one entry per corpus file
      data/                repo-backed fixtures (backend=repo), committed
      object/              (mirror layout for object-store blobs; not committed)
      golden/              golden outputs for golden-file tests
      golden_tests.json    golden-file test spec consumed by the runner
      cache/               local object-store blob cache (gitignored)

    python/oep_tooling/corpus/
      registry.py          load manifest, resolve/verify entries
      store.py             NAS object-store backend (Gitea generic packages)
      runner.py            golden-file runner (text/bytes/numeric compare)
      __main__.py          CLI: list | verify | fetch | golden

## Registry (corpus/manifest.json), schema v1

One JSON object with `schema_version` and `entries[]`. Each entry:

    id             unique slug
    path           path relative to corpus/ (local layout)
    format         machine format tag (stl-ascii, cgr, 3dxml, step, ...)
    geometry_class prism | surface | assembly | dataset | text
    source         provenance (clean-room invariant: must be redistributable)
    license        SPDX id
    size           bytes
    sha256         64-hex content hash  (integrity source of truth)
    backend        "repo" (committed) | "object-store" (NAS blob)
    object_store   {owner, package, version, object}  (only backend=object-store)

Adding a repo-backed file: put the file under `corpus/data/`, copy its real
sha256 into the entry, and let `python -m oep_tooling.corpus verify` prove it.

## Backend decision (ADR, P0-M4-T02)

- Large corpus blobs use the **NAS object store**: the NAS Gitea generic package
  registry (`/api/packages/<owner>/generic/<pkg>/<version>/<object>`),
  content-addressed by sha256 in the object name. Rationale:
    - No client-side git-lfs binary is installed anywhere on U1 or the CI
      containers (`git-lfs` absent); generic packages need only the don token
      already present in `~/.git-credentials`.
    - Blobs never bloat git history or CI clone time.
    - Integrity is explicit: fetch is verified against the manifest sha256.
    - The gitea skill records this endpoint as proven for 10-200 MB blobs.
- Small fixtures and golden outputs are committed directly (backend=repo) so the
  acceptance test and CI stay hermetic and off-NAS.
- git-lfs remains an option for a future fully-in-git corpus; the manifest
  `backend` field is the seam, swapping stores is a registry edit only.

## Golden-file runner

    python -m oep_tooling.corpus list
    python -m oep_tooling.corpus verify
    python -m oep_tooling.corpus fetch
    python -m oep_tooling.corpus golden --spec corpus/golden_tests.json

Spec (corpus/golden_tests.json): per test `{id, corpus_id, command[], golden,
compare, tolerance}`. `command` runs with `{input}` replaced by the resolved
corpus path; stdout is compared against `golden` (path relative to repo root),
mode `text` (normalized) / `bytes` / `numeric` (tolerance = max abs diff).

Classification (tolerant per P2-M1-T04, report matrix per P2-M5-T03):
  PASS       output matches golden
  FAIL       output differs beyond tolerance  -> run exits non-zero
  SKIP       corpus entry/golden/command missing or object-store fetch failed
             -> warning only, run still exits 0 (a classified skip, not a failure)

## Object-store mirror (P1-M4-T01)

The whole shared corpus is mirrored to the NAS object store: every
`backend=object-store` entry of `corpus/manifest.json` is present as a blob at
`don/isolith-corpus` v1 under the exact object name its `object_store` pointer
names (`<sha256>.<ext>`), and GET-back verifies against the manifest sha256.

    # materialize (upload missing blobs), write index, verify all — live NAS
    python3 corpus/tools/mirror_object_store.py --mirror \
        --data-root <local-source-dir>...
    # read-only live verify of every blob
    python3 corpus/tools/mirror_object_store.py --verify

The P2/P3 regression runner resolves object-store entries end to end:

    python3 -m oep_tooling.regression --manifest corpus/manifest.json \
        --cache <cache-dir> --fetch --out corpus/
    # -> 139/139 classified; row + summary carry `backend` + `by_backend`

