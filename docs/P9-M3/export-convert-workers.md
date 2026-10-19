# P9-M3-T02 — Export/convert workers: CGR->glTF, STEP->mesh (dogfood P2/P4)

Version 1.0, 2026-10-17. Milestone: P9-M3-T02 (export/convert workers) of the
P9 P2P compute lane (wave 2). Program: Keelworks / Open Engineering Platform.

Delivered as `crates/oep-worker` (layout anchor `P9 oep-worker`), which is the
P9 `convert` executor lane. The program eats its own dogfood: CGR->glTF runs
the P2 reader (`oep-cgr`), STEP->mesh runs the P4 reader (`oep-step`), and
every artifact lands back in the P8 vault (`oep-pdm`) with its sha-256 content
address verified. P2/P4 output feeds P9 input; P9 output feeds the P8 vault.

## What a worker is

Per P9-M1-T01, a job's `executor` is `{ kind, function, args }` with a closed
kind vocabulary. This lane owns `kind = convert` and registers two worker
functions (the vocabulary names the job-spec examples already used):

| executor function | worker                                                            | dogfoods |
|-------------------|-------------------------------------------------------------------|----------|
| `cgr2gltf`        | real CGR bytes -> binary glTF 2.0 (GLB)                            | P2 `oep-cgr` (mesh extract P2-M2, glTF P2-M2-T03, modern V5_CFV2 decode P2-M7) |
| `step2mesh`       | real STEP bytes -> deterministic `oep-mesh` v1 JSON (faceted mesh) | P4 `oep-step` (read lane P4-M1, MSBRep walk P4-M3) |

`oep_worker::registry::resolve(kind, function)` is the P9 worker registry — the
one seam the scheduler (P9-M2) and the batch driver ask. Adding a worker = one
registry row + one byte-in/byte-out function.

## Convert semantics (honesty contract)

Both workers are total and deterministic on any byte input — typed outcome,
never a panic, never fabricated geometry (P2-M1-T04 doctrine carried into the
compute lane):

- `cgr2gltf` tries the old-era structural extractor first, then the modern
  `V5_CFV2` decode. A valid container with no tessellation the readers map is
  a **classified skip**; bytes that carry no observed CGR container are typed
  `NotCgr`.
- `step2mesh` runs the P4 facet reader and emits vertices / triangles /
  bounding box plus the honest source report as `oep-mesh` v1 JSON. Analytic
  (non-faceted) geometry is a **classified skip**, never a partial or invented
  mesh.

Skips are recorded on the job (`status: skip` with a reason) and do not fail
the batch — the corpus is expected to contain non-convertible files.

## The worker (`oep_worker::run_job`)

The job-facing executor (`crates/oep-worker/src/worker.rs`), what the
scheduler will call and what the batch driver calls today:

1. **Validate** the `oep-job` spec (`JobSpec::validate`).
2. **Resolve input** (the P8 vault object is the input currency) through the
   P8-M6-T02 `PdmAdapter` seam — `item` and `revision` refs resolve to bytes;
   `file` (blob) refs are documented as scheduler-preflight (the v1 REST
   contract has no item-less blob GET vertex yet).
3. **Run** the registry worker.
4. **Store output** by target through the adapter's save verb: `attach_item`
   and `new_item` (part) land on their items; `blob` targets land under the
   deterministic output-catalog item `p9-blob-outputs` (documented mapping
   until P8 exposes `/blobs/{sha256}`).
5. **Hash-verify** (the P9-M5-T02 GA-gate contract, live): the vault's own
   recorded `FileRef.blob_sha256` must equal the artifact's sha-256, and a
   declared `expected_sha256` must match too.

## Batch driver (`oep-batch-convert`)

`crates/oep-worker` ships the `oep-batch-convert` CLI with two verbs:

```text
  oep-batch-convert seed <seed.json>   [--vault URL]   # upload local material
  oep-batch-convert run  <batch.json>  [--vault URL] [--report PATH]
```

Formats (committed): `oep-seed` v1 (local real-material paths; dev only,
never shipped), `oep-batch` v1 (a list of `oep-job` JobSpecs), `oep-batch-report`
v1 (durable proof: per-job outcome + `summary.hash_verified`).

## P8 seam change required by real CAD

The vault's axum service had the default 2 MiB request-body cap, which rejects
real CAD uploads (the NIST V5-6R2023 STC-06 CATPart is ~2.5 MiB, base64-in-JSON
inflates ~33%). Raised to 512 MiB in `oep_pdm::service` (`BODY_LIMIT_BYTES`),
documented as a P8-M6 seam the compute lane needs. Bodies are still decoded and
content-hashed before anything is stored.

## Proof (committed under `workstreams/P9-M3/proof/`)

A live run on 2026-10-17 against a real `oep-pdm` vault (`--persist-dir`),
seeded from the seed manifest and driven by the batch manifest:

| job           | worker   | input                         | outcome | triangles/verts | artifact sha256 (vault-verified) |
|---------------|----------|-------------------------------|---------|-----------------|----------------------------------|
| cgr-stc06     | cgr2gltf | NIST STC-06 CATPart (2.5 MiB) | ok      | 1389 / 1037     | `214156f4…b4d89` |
| cgr-valve     | cgr2gltf | VALVE.cgr (old-era native)    | skip    | — (readers map neither spine) | — |
| step-box-ref  | step2mesh| box-reference.step            | ok      | 12 / 8          | `d1eaa8fb…65e38e` |
| step-box-part | step2mesh| box-part.step (P4-M3 golden)  | ok      | 12 / 8          | committed in report |
| step-assembly | step2mesh| assembly.step (P4-M3 golden)  | ok      | 12 / 8          | committed in report |
| step-box-mini | step2mesh| box-mini.step (product-only)  | skip    | — (no faceted geometry) | — |
| step-fc-probe | step2mesh| FreeCAD probe export          | ok      | 12 / 8          | committed in report |
| step-fc-box   | step2mesh| FreeCAD box export (MSBRep)   | ok      | 12 / 8          | committed in report |

Summary: 8 jobs, 6 ok, 2 skip, 0 failed, 6/6 hash-verified. The report's
`artifact_sha256` equals the vault's `FileRef.blob_sha256` for every ok job
(re-verified by reading the vault snapshot back and by re-hashing the
downloaded artifacts — `stc06.glb` and `box-ref.oepmesh.json` reproduce the
recorded hashes byte-for-byte).

Proof files:

- `batch-corpus.json`   — the batch manifest (portable; refs vault item names)
- `seed-corpus.json`    — the dev-only seed manifest (host paths)
- `batch-report.json`   — the durable run report (hashes)
- `vault-catalogue.json` + `vault-snapshot.json` — vault read-back, all 14
  items / revisions with blob shas
- `stc06.glb`           — the CGR->glTF output artifact (sample)
- `box-ref.oepmesh.json`— the STEP->mesh output artifact (sample)
- `gen_batch.py` / `capture_snapshot.py` — the reproducers

## Acceptance mapping

- "batch convert jobs run across the corpus" — `oep-batch-convert run` across
  real CGR + STEP material, seeded + run + reported (proof above).
- "outputs hash-verified in P8 vault" — every ok job's vault
  `FileRef.blob_sha256` equals the artifact sha-256 (report + snapshot + hash
  of downloaded bytes agree); `expected_sha256` honored when declared.
- "the program eats its own dogfood (P2/P4 output feeds P9 input)" — the
  worker crate depends on `oep-cgr` and `oep-step` by crate path and calls
  their readers; it does not re-implement CGR/STEP parsing.
