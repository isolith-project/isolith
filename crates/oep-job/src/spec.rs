//! The P9 job/task spec: wire format types, closed vocabularies, caps bounds.
//!
//! A [`JobSpec`] is the unit of work the P9 compute lane accepts. It is
//! serialized in our own format (`format = "oep-job"`, `version = 1`) and is
//! the shared substrate every downstream P9 card consumes: the scheduler
//! (P9-M2-T01) queues specs by the fields below, the workers (P9-M3-T01/T02)
//! run the executor, the sandbox (P9-M3-T03) enforces [`ResourceCaps`], the
//! GPU scheduler (P9-M4-T01) honors the GPU request, and the GA gate
//! (P9-M5-T02) verifies declared [`OutputRef`]s in the P8 vault.
//!
//! Serialization conventions (matching P8-M1-T02): deterministic key order,
//! no `HashMap` in serialized types, optional fields are
//! `#[serde(skip_serializing_if)]`, closed vocabularies are declared as `&str`
//! constants and re-stated as enums in the committed JSON Schema so code and
//! contract cannot drift. Deserialization is tolerant: unknown fields are
//! ignored so future optional fields do not break version-1 documents.

use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying the P9 job/task spec.
pub const FORMAT: &str = "oep-job";
/// The current schema version of the spec format.
pub const VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Closed vocabularies
// ---------------------------------------------------------------------------

/// Executor kind: `render` -- headless oep-geo renderer worker (P9-M3-T01).
pub const EXECUTOR_RENDER: &str = "render";
/// Executor kind: `convert` -- format conversion worker, e.g. CGR->glTF,
/// STEP->mesh (P9-M3-T02).
pub const EXECUTOR_CONVERT: &str = "convert";
/// Executor kind: `import` -- ingest a file into the vault or oep-geo model.
pub const EXECUTOR_IMPORT: &str = "import";
/// Executor kind: `export` -- emit a derived format to an output ref.
pub const EXECUTOR_EXPORT: &str = "export";
/// Executor kind: `validate` -- run a validator/QA worker (e.g. MBD, GA rules).
pub const EXECUTOR_VALIDATE: &str = "validate";
/// Executor kind: `custom` -- an arbitrary worker not in the built-in set;
/// MUST carry an `image` (there is no built-in function to run).
pub const EXECUTOR_CUSTOM: &str = "custom";
/// The closed vocabulary of [`ExecutorRef::kind`]; mirrors the P9-M3 worker
/// set so the spec lists the same executors the scheduler can place.
pub const EXECUTOR_KINDS: [&str; 6] = [
    EXECUTOR_RENDER,
    EXECUTOR_CONVERT,
    EXECUTOR_IMPORT,
    EXECUTOR_EXPORT,
    EXECUTOR_VALIDATE,
    EXECUTOR_CUSTOM,
];

/// Vault ref kind: `item` -- a whole P8 vault item (latest revision).
pub const REF_KIND_ITEM: &str = "item";
/// Vault ref kind: `revision` -- a specific revision of a P8 vault item.
pub const REF_KIND_REVISION: &str = "revision";
/// Vault ref kind: `file` -- a content-addressed P8 vault file blob.
pub const REF_KIND_FILE: &str = "file";
/// The closed vocabulary of [`VaultObjectRef::kind`]; the three P8-M1-T02
/// classes a job may consume by id/address.
pub const REF_KINDS: [&str; 3] = [REF_KIND_ITEM, REF_KIND_REVISION, REF_KIND_FILE];

/// Output target kind: `blob` -- a content-addressed vault blob with no item
/// (default; intermediate artifacts).
pub const TARGET_BLOB: &str = "blob";
/// Output target kind: `new_item` -- the worker's output creates a new P8
/// vault item.
pub const TARGET_NEW_ITEM: &str = "new_item";
/// Output target kind: `attach_item` -- the worker's output becomes a new
/// revision/file of an existing P8 vault item.
pub const TARGET_ATTACH_ITEM: &str = "attach_item";
/// The closed vocabulary of [`OutputTarget::kind`].
pub const TARGET_KINDS: [&str; 3] = [TARGET_BLOB, TARGET_NEW_ITEM, TARGET_ATTACH_ITEM];

// P8 item-kind vocabulary, mirrored from P8-M1-T02 (the storage-side schema)
// and the P8-M6-T01 OpenAPI `CreateItemRequest.kind` enum so an output new_item
// can only name the same kinds the REST vertex accepts.
/// Item kind: `part` -- a single component artifact.
pub const ITEM_KIND_PART: &str = "part";
/// Item kind: `assembly` -- an ordered collection of components.
pub const ITEM_KIND_ASSEMBLY: &str = "assembly";
/// Item kind: `document` -- a non-geometry artifact (notes, specs, images).
pub const ITEM_KIND_DOCUMENT: &str = "document";
/// The closed vocabulary of the `OutputTarget::NewItem` `item_kind` field.
pub const ITEM_KINDS: [&str; 3] = [ITEM_KIND_PART, ITEM_KIND_ASSEMBLY, ITEM_KIND_DOCUMENT];

// ---------------------------------------------------------------------------
// Resource-cap bounds (conservative; see docs/compute/spec/job-spec.md)
// ---------------------------------------------------------------------------

/// Upper bound on [`ResourceCaps::cpu_millicores`] (64 CPUs).
pub const MAX_CPU_MILLICORES: u64 = 64_000;
/// Upper bound on [`ResourceCaps::memory_mib`] (1 TiB).
pub const MAX_MEMORY_MIB: u64 = 1_048_576;
/// Upper bound on [`ResourceCaps::disk_mib`] (8 TiB scratch).
pub const MAX_DISK_MIB: u64 = 8_388_608;
/// Upper bound on [`ResourceCaps::timeout_seconds`] (7 days).
pub const MAX_TIMEOUT_SECONDS: u64 = 604_800;
/// Upper bound on [`GpuRequest::count`] per job.
pub const MAX_GPU_COUNT: u32 = 8;
/// Minimum [`ResourceCaps::checkpoint_every_seconds`] (below this a checkpoint
/// is meaningless).
pub const CHECKPOINT_MIN_SECONDS: u64 = 60;
/// Upper bound on [`ResourceCaps::checkpoint_every_seconds`].
pub const CHECKPOINT_MAX_SECONDS: u64 = 360_000;
/// Canonical length of a sha-256 content address (64 lowercase hex).
pub const BLOB_HEX_LEN: usize = 64;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A reference to a P8 vault object, the **input currency** of a job.
///
/// There are exactly three shapes, mirroring the P8-M1-T02 vault classes and
/// the P8-M6-T01 REST v1 resources that carry them:
///
/// - `item`     -> `GET /api/v1/items/{item_id}`
/// - `revision` -> `GET /api/v1/items/{item_id}/revisions/{rev}`
///   (+ `.../{rev}/file` for the bytes)
/// - `file`     -> a content address `GET`able and verifiable by re-hash
///   (`X-Blob-Sha256` header / blob store `GET <root>/blobs/<sha256>`).
///
/// See [`crate::p8`] for the exact resource mapping a scheduler preflights.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VaultObjectRef {
    /// A whole vault item (resolved to its latest released/available state by
    /// the scheduler at enqueue time).
    Item {
        /// The vault item id (opaque, url-safe, non-empty).
        item_id: String,
    },
    /// A specific revision of a vault item.
    Revision {
        /// The owning vault item id.
        item_id: String,
        /// The revision number as a string (`"1"`, `"2"`, ...), matching the
        /// P8-M6-T01 `Rev` path parameter.
        revision: String,
    },
    /// A content-addressed vault file blob, referenced by its sha-256.
    File {
        /// The 64-lowercase-hex sha-256 content address of the bytes.
        blob_sha256: String,
    },
}

/// One declared input of a job: a bound name plus a P8 vault object ref.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InputRef {
    /// Logical name the worker uses to address this input inside the job
    /// (unique across the job's inputs; e.g. `mesh`, `source_step`).
    pub name: String,
    /// The P8 vault object ref (the input currency).
    #[serde(rename = "ref")]
    pub reference: VaultObjectRef,
    /// Optional human note about the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl InputRef {
    /// An input named `name` referencing the given P8 vault object.
    pub fn new(name: impl Into<String>, reference: VaultObjectRef) -> Self {
        InputRef {
            name: name.into(),
            reference,
            note: None,
        }
    }
}

/// Which worker executes the job.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecutorRef {
    /// Executor family; closed vocabulary of [`EXECUTOR_KINDS`].
    pub kind: String,
    /// Specific worker function within the family (e.g. `oep-geo-render`,
    /// `cgr2gltf`, `step2mesh`). Non-empty; worker registry (P9-M3) resolves
    /// it.
    pub function: String,
    /// Optional container image / artifact tag, e.g. `oep-workers/render:v1`.
    /// REQUIRED when `kind == custom` (no built-in function exists then).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Worker arguments (parameterization, e.g. render resolution/fidelity
    /// options — P9-M3-T01).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl ExecutorRef {
    /// An executor with the given family `kind` and worker `function`.
    pub fn new(kind: impl Into<String>, function: impl Into<String>) -> Self {
        ExecutorRef {
            kind: kind.into(),
            function: function.into(),
            image: None,
            args: Vec::new(),
        }
    }

    /// `true` when this executor requires a container image of its own.
    pub fn needs_image(&self) -> bool {
        self.kind == EXECUTOR_CUSTOM
    }
}

/// A GPU request for a job (P9-M4-T01: GPU discovery + scheduling).
///
/// Absent `gpu` on [`ResourceCaps`] means the job is CPU-only. When present,
/// `required` decides whether the scheduler may fall back to CPU
/// (`required: false`) or must park the job until a matching GPU is free
/// (`required: true`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GpuRequest {
    /// Number of devices requested (1..=[`MAX_GPU_COUNT`]).
    pub count: u32,
    /// `true` = strict (no CPU fallback); `false` = preferred (fall back to
    /// CPU cleanly, P9-M4-T01).
    #[serde(default)]
    pub required: bool,
    /// Optional hardware/model hint (e.g. `dgx-a100`); free string, non-empty
    /// when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Resource caps a job needs and the worker sandbox MUST enforce
/// (P9-M3-T03) — the conservative defaults doctrine (P9-M2-T02).
///
/// `cpu_millicores` / `memory_mib` are required; everything else is optional
/// with conservative semantics (no GPU, egress blocked by default).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResourceCaps {
    /// CPU request in millicores (thousandths of a core), 1..=MAX.
    pub cpu_millicores: u64,
    /// Memory request in MiB, 1..=MAX.
    pub memory_mib: u64,
    /// Optional scratch-disk request in MiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_mib: Option<u64>,
    /// Optional GPU request; `None` = CPU-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuRequest>,
    /// `true` allows network egress from the worker sandbox. Default `false`:
    /// no network egress unless a job explicitly asks (P9-M3-T03).
    #[serde(default)]
    pub network_egress: bool,
    /// Optional hard timeout; the sandbox kills the job at this many seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Optional checkpoint interval for long jobs (P9-M4-T02);
    /// 60..=CHECKPOINT_MAX_SECONDS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_every_seconds: Option<u64>,
}

impl ResourceCaps {
    /// A CPU-only, egress-blocked, one-hour-timeout cap of 1000 millicores /
    /// 2 GiB — the conservative default for jobs that do not state otherwise.
    /// This is what the P9 worker sandbox enforces when no cap is negotiated.
    pub fn conservative_default() -> Self {
        ResourceCaps {
            cpu_millicores: 1000,
            memory_mib: 2048,
            disk_mib: None,
            gpu: None,
            network_egress: false,
            timeout_seconds: Some(3600),
            checkpoint_every_seconds: None,
        }
    }
}

/// Where a declared output lands in the P8 vault.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputTarget {
    /// A content-addressed blob with no owning item (intermediate artifacts).
    Blob {},
    /// The worker creates a brand-new P8 vault item for this output.
    NewItem {
        /// Name of the new item.
        item_name: String,
        /// Optional item kind from the closed [`ITEM_KINDS`] vocabulary.
        #[serde(skip_serializing_if = "Option::is_none")]
        item_kind: Option<String>,
        /// Optional home context id for the new item.
        #[serde(skip_serializing_if = "Option::is_none")]
        context_id: Option<String>,
    },
    /// The worker attaches this output as a new file (in a new revision) of an
    /// existing vault item.
    AttachItem {
        /// The existing vault item id to attach to.
        item_id: String,
    },
}

/// One declared output of a job.
///
/// The P9-M5-T02 GA gate verifies every `required` output is present and
/// hash-correct (`expected_sha256`) in the P8 vault after the job completes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OutputRef {
    /// Logical output name (unique across the job; e.g. `thumbnail`, `mesh`).
    pub name: String,
    /// Output routing tag / format (e.g. `png`, `gltf`, `stl`, `step`). The
    /// worker tags its produced artifact with this. Non-empty.
    pub format: String,
    /// Optional media type of the output bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Where the output lands; default `blob` when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<OutputTarget>,
    /// Optional expected sha-256 of the output bytes (64 lowercase hex); when
    /// set, completion verification compares against it (P9-M5-T02).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
    /// `true` (default): the job is not done unless this output is produced.
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

impl OutputRef {
    /// An output named `name` in `format`, required by default, blob target.
    pub fn blob(name: impl Into<String>, format: impl Into<String>) -> Self {
        OutputRef {
            name: name.into(),
            format: format.into(),
            content_type: None,
            target: None,
            expected_sha256: None,
            required: true,
        }
    }
}

/// The P9 job/task spec: the versioned unit of work for the compute lane.
///
/// Header (`format`/`version`) plus the four contract groups: `input` (P8
/// vault object refs), `executor` (worker), `resources` (caps), `output`
/// (vault targets). Scheduler concerns that are NOT part of the spec --
/// priority, retries, DAG dependencies, dead-letter policy, queue identity --
/// live in the P9-M2-T01 scheduler envelope and are intentionally absent here
/// (one card, one owner).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    /// Must equal [`FORMAT`] (`"oep-job"`); a reader rejects anything else
    /// (`bad-format`).
    pub format: String,
    /// Must equal [`VERSION`]; a reader rejects newer versions it does not
    /// understand (`unsupported-version`).
    pub version: u32,
    /// Optional stable job id (assigned by the scheduler when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Optional human label for the job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional human description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional RFC 3339 UTC creation timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Declared inputs; P8 vault object refs are the input currency.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<InputRef>,
    /// The worker that executes the job.
    pub executor: ExecutorRef,
    /// Resource caps the worker sandbox must enforce.
    pub resources: ResourceCaps,
    /// Declared outputs, each landing in the P8 vault.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<OutputRef>,
}

impl JobSpec {
    /// A job spec with the default header and no inputs/outputs yet. Callers
    /// push [`InputRef`]s and [`OutputRef`]s onto the returned spec.
    pub fn new(executor: ExecutorRef, resources: ResourceCaps) -> Self {
        JobSpec {
            format: FORMAT.to_string(),
            version: VERSION,
            job_id: None,
            name: None,
            description: None,
            created_at: None,
            input: Vec::new(),
            executor,
            resources,
            output: Vec::new(),
        }
    }
}
