# oep-job: the P9 job/task spec (P9-M1-T01)

Version 1.0, 2026-10-17. Milestone: P9-M1-T01 (job/task spec: *input vault
refs*, *executor*, *resource caps*, *output refs*). Program: Keelworks /
Open Engineering Platform (OEP), P9 P2P compute lane (wave 2).
Clean-room, our own format — nothing here is a vendor or cloud schema.

Reference implementation: `crates/oep-job` (Rust, serde), modules
`spec` (wire types + vocabularies) / `validate` (structural rules) /
`p8` (the P8-M6-T01 REST vertex mapping).
Machine contract: `docs/compute/spec/job-spec-v1.schema.json` (JSON Schema); shared
real material: `docs/compute/spec/examples/render-job.json` and
`convert-job.json`, which the crate's integration test deserializes and
validates (`crates/oep-job/tests/job_spec_contract.rs`, `include_str!`) and
which `tests/test_p9_job_spec.py` validates against the schema — both
languages peer at the same bytes, so the schema and the crate cannot drift.

---

## 1. Shape of a job

A `JobSpec` is the versioned unit of work the P9 compute lane accepts. It has
a header (`format = "oep-job"`, `version = 1`) and exactly four contract
groups, the four named by the card:

| group       | module field | role |
|-------------|--------------|------|
| Input refs  | `input`      | the P8 vault objects the job consumes — **the input currency** |
| Executor    | `executor`   | which worker function runs the job (P9-M3-T01/T02) |
| Resource caps | `resources` | what the worker sandbox must enforce (P9-M3-T03, P9-M4-T01) |
| Output refs | `output`     | declared outputs landing back in the P8 vault (P9-M5-T02) |

```json
{ "format": "oep-job", "version": 1,
  "input":  [ { "name": "mesh",
                "ref": { "kind": "revision",
                         "item_id": "cowl-panel", "revision": "3" } } ],
  "executor": { "kind": "render", "function": "oep-geo-render" },
  "resources": { "cpu_millicores": 2000, "memory_mib": 4096 },
  "output": [ { "name": "thumbnail", "format": "png" } ] }
```

Header rules (mirror oep-vault): a reader rejects any `format` other than
`oep-job` (`bad-format`) and any `version` it does not understand
(`unsupported-version`). Deserialization is tolerant — unknown fields are
ignored so version-1 documents can gain optional fields without a bump.

## 2. Input refs: P8 object refs are the input currency

The card's contract note points at **P8-M6-T01, the REST API v1 vertex**.
This card honors it literally: a job's inputs are P8 vault object refs and
nothing else. There are exactly three shapes, mirroring the P8-M1-T02 vault
classes and the P8-M6-T01 OpenAPI resources that carry them (`crates/
oep-job/src/p8.rs` is the single source of the mapping):

| ref kind | shape | P8-M6-T01 REST v1 resource (base `/api/v1`) |
|----------|-------|----------------------------------------------|
| `item`     | `{ item_id }` | `GET /items/{item_id}` |
| `revision` | `{ item_id, revision }` | `GET /items/{item_id}/revisions/{rev}` (+ `.../{rev}/file`) |
| `file`     | `{ blob_sha256 }` | content address, verified by re-hash (`X-Blob-Sha256` header / blob store) |

`revision` is a positive decimal string (`"1"`, `"2"`, ...) — the same shape
as the P8-M6 `Rev` path parameter. `blob_sha256` is exactly 64 lowercase hex,
the P8-M1-T02 content-address rule (`invalid-blob-address`), so a job input
can be a content-addressed file whose bytes were never inside any single
revision — dedup and verify-by-hash still find them.

`JobSpec::p8_input_resources()` returns the exact REST resources a job needs;
the scheduler (P9-M2-T01) preflights those before enqueue. Resolving refs
against the *live* vault is the scheduler's job — `validate()` only checks
that refs have a legal shape.

## 3. Executor

`executor.kind` is a closed vocabulary aligned with the P9-M3 worker set:
`render`, `convert`, `import`, `export`, `validate`, `custom`. `function`
names the specific worker function within the kind (e.g. `oep-geo-render`,
`cgr2gltf`, `step2mesh`); `args` carries worker parameterization (render
resolution/fidelity options, P9-M3-T01). `image` is optional except for
`custom`, which has no built-in function and therefore MUST carry an image
(`custom-executor-needs-image`).

## 4. Resource caps

`resources` is what the worker sandbox enforces (P9-M3-T03). Semantics owned
by the downstream cards, bounded here:

| field | bounds | consumed by |
|-------|--------|-------------|
| `cpu_millicores` | 1..=64000 (required) | sandbox CPU cap |
| `memory_mib` | 1..=1048576 (required) | sandbox RAM cap |
| `disk_mib` | 1..=8388608 | scratch disk |
| `gpu` | count 1..=8, `required` bool, optional `model`; absent = CPU-only | P9-M4-T01 GPU scheduling (strict vs CPU fallback) |
| `network_egress` | bool, default `false` | P9-M3-T03 (blocked by default) |
| `timeout_seconds` | 1..=604800 | sandbox kill + scheduler retry |
| `checkpoint_every_seconds` | 60..=360000 | P9-M4-T02 checkpoint/resume |

`ResourceCaps::conservative_default()` (1000 millicores, 2048 MiB, no GPU,
egress off, 1 h timeout) is the conservative cap the sandbox falls back to
(P9-M2-T02: defaults are conservative).

## 5. Output refs: verified in PDM

Declared outputs land back in the P8 vault (P9-M5-T02 GA gate: "outputs
verified in PDM"). Each output has `name`, `format`, optional `content_type`
and `expected_sha256` (a 64-hex address the gate re-checks by re-hash), and a
`target`:

| target | meaning | GA-gate verify |
|--------|---------|----------------|
| `blob` (default) | content-addressed blob, no item | by address + re-hash |
| `new_item { item_name, item_kind?, context_id? }` | worker creates a new vault item | item exists, output file present on it |
| `attach_item { item_id }` | new revision/file on an existing item | `P8RestResource::Item(item_id)` — the item gained the revision |

`required` (default `true`) means the job is not done unless the output is
produced; a failed `required` output poisons the run in the scheduler's dead
letter.

## 6. Validation rules

`JobSpec::validate()` returns every violation as `{code, message}`, never
panics, rejects nothing at load time (P8-M1-T02 doctrine).

| code | condition |
|------|-----------|
| `bad-format` / `unsupported-version` | header is not `oep-job` version 1 |
| `invalid-job-id` / `invalid-job-name` / `invalid-timestamp` | optional envelope fields malformed (`created_at` shape-checked RFC 3339) |
| `invalid-input-name` / `duplicate-input-name` | input names empty / not unique |
| `invalid-item-id` | `item_id` not an opaque url-safe id |
| `invalid-revision` | `revision` not a positive decimal string |
| `invalid-blob-address` | `blob_sha256` / `expected_sha256` not 64 lowercase hex |
| `unknown-executor-kind` | `executor.kind` outside the closed vocabulary |
| `empty-executor-function` | `function` names no worker |
| `custom-executor-needs-image` | `custom` executor with no image |
| `invalid-image-tag` | `image` has an illegal tag shape |
| `invalid-cpu-caps` / `invalid-memory-caps` / `invalid-disk-caps` | caps out of bounds |
| `invalid-gpu-request` | `gpu.count` out of bounds or `gpu.model` empty |
| `invalid-timeout` / `invalid-checkpoint-interval` | out of bounds |
| `invalid-output-name` / `duplicate-output-name` / `empty-output-format` | output envelope malformed |
| `new-item-needs-name` / `invalid-item-kind` / `invalid-context-id` | `new_item` target malformed (`item_kind` mirrors the P8 item vocabulary) |
| `attach-item-needs-item-id` | `attach_item` target without a valid `item_id` |

Closed vocabularies are declared once in `crates/oep-job/src/spec.rs` and
re-stated as enums in the JSON Schema; `tests/test_p9_job_spec.py` DRIFT test
parses the Rust constants and asserts the Schema enums match, so the two
committed contracts cannot diverge.

## 7. Ownership boundaries (what this card does NOT decide)

Scheduler concerns are intentionally **absent** from the spec so downstream
cards own their semantics without renegotiating the format:

- **Priority, retries, DAG dependencies, dead-letter policy, queue identity**
  — the P9-M2-T01 scheduler envelope. A `JobSpec` is the unit; the scheduler
  wraps it with scheduling fields.
- **Message bus choice + topic taxonomy** — P9-M1-T02 (NATS self-hosted
  default ADR). The spec is payload-agnostic.
- **Worker sandbox mechanics, worker registry/function resolution** —
  P9-M3-T01/T02/T03. The spec declares *what* (`kind`/`function`) and *how
  much* (`resources`); the registry resolves *how*.
- **Node inventory / quota pricing** — P9-M2-T02. Caps are requests; quota
  enforcement compares them to per-team/node budgets.
- **Job identity/audit wiring to the P8 identity/RBAC layers** — later P9/P10
  cards; `job_id` is present and opaque.

## 8. Handoff notes

- P9-M2-T01 (scheduler): enqueues specs by scheduler fields; preflights
  `p8_input_resources()`; wraps each spec with priority/retries/DAG.
- P9-M3-T01/T02 (workers): `executor.function` is the registry key; worker
  returns bytes + metadata that the job broker maps onto `output` refs.
- P9-M3-T03 (sandbox): enforces `resources`; `network_egress` default-off is
  the testable boundary ("escaping the sandbox fails a test").
- P9-M4-T01 (GPU): `resources.gpu` `required:false` = clean CPU fallback;
  `required:true` = park until a matching GPU is free.
- P9-M4-T02 (checkpoint): `checkpoint_every_seconds` triggers worker
  checkpoint emission; resume must be byte-equivalent (verified via
  `expected_sha256` where declared).
- P9-M5-T02 (GA gate): every `required` output verified in the P8 vault by
  address + re-hash; the 100-job wave ships these specs.

See also `docs/layout.md` (crate map) and the P8 contract docs
(`docs/pdm/service/vault-object-model.md`, `workstreams/pdm/api/openapi-v1.yaml`, both on the
unmerged P8 PR branches at the time of writing — `oep-job` pins the same ref
shapes by value, so it stacks on the P8 wave regardless of merge order).
