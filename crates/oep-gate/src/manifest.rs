//! Wave manifest for the P9 GA gate: the 100 render/convert jobs the driver
//! enqueues, plus the seed manifest that loads the real corpus into the vault.
//!
//! ## Corpus / seed
//!
//! The wave eats real material, exactly like the P9-M3-T02 dogfood proof:
//!
//! - **STEP fixtures** (committed under `crates/oep-step/tests/golden/` and
//!   `crates/oep-worker/tests/fixtures/`) -- `box-reference.step`,
//!   `box-part.step`, `assembly.step`, `box-mini.step` -- feed the `step2mesh`
//!   convert lane (dogfooding the P4 reader);
//! - **CGR material** (real CATPart corpus, dev-only paths in the seed
//!   manifest) feeds the `cgr2gltf` convert lane (dogfooding the P2 reader);
//! - **an oep-geo model** (the `example-bracket.oepgeo.json` golden) feeds the
//!   `oep-geo-render` render lane.
//!
//! The generated manifest builds 100 deterministic jobs: a fixed mix of
//! render (resolution/fidelity-parameterized over the golden bracket model)
//! and convert (step2mesh over the STEP fixtures, cgr2gltf over the seeded
//! CGR). Render outputs are always distinct (different pixels); convert
//! outputs may repeat only if the same fixture is re-seeded under a different
//! output item (the wave is 100 *jobs*, each with its own vault output item).

use oep_job::spec::{
    ExecutorRef, InputRef, JobSpec, OutputRef, OutputTarget, ResourceCaps, VaultObjectRef,
};
use oep_worker::batch::SeedManifest;
use serde::{Deserialize, Serialize};

/// Default wave size: 100 jobs (the GA gate acceptance).
pub const DEFAULT_JOBS: usize = 100;

/// One wave job: the job spec plus the seed info it depends on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveJob {
    /// The job spec the scheduler/worker executes.
    pub spec: JobSpec,
}

impl WaveJob {
    /// The job's display name.
    pub fn name(&self) -> &str {
        self.spec.name.as_deref().unwrap_or("job")
    }
}

/// The wave manifest (wire format).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobManifest {
    /// Must equal `oep-gate-wave`.
    pub format: String,
    /// Must equal 1.
    pub version: u64,
    /// Human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Seed files: local real-material paths -> vault items (dev-only).
    #[serde(default)]
    pub seed: Vec<SeedFile>,
    /// The jobs.
    pub jobs: Vec<WaveJob>,
}

/// One seed entry (subset of the P9-M3-T02 seed shape, same fields).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeedFile {
    /// Vault item (name) the file becomes revision 1 of.
    pub item_name: String,
    /// Local filesystem path to the real material (dev-only).
    pub path: String,
    /// Format tag the vault records (`cgr`, `step`, `oepgeo`).
    pub format: String,
}

impl JobManifest {
    /// Load a manifest from disk.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
        let m: JobManifest =
            serde_json::from_str(&text).map_err(|e| format!("parse {path:?}: {e}"))?;
        if m.format != "oep-gate-wave" {
            return Err(format!("bad wave format: {}", m.format));
        }
        Ok(m)
    }

    /// Save a manifest (reproducibility).
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| format!("encode: {e}"))?;
        std::fs::write(path, bytes).map_err(|e| format!("write {path:?}: {e}"))
    }

    /// Produce the seed manifest the driver feeds to `seed_from_manifest`.
    pub fn seed_manifest(&self) -> Result<SeedManifest, String> {
        let files: Vec<_> = self
            .seed
            .iter()
            .map(|s| oep_worker::batch::SeedFile {
                item_name: s.item_name.clone(),
                path: s.path.clone(),
                format: s.format.clone(),
            })
            .collect();
        if files.is_empty() {
            return Err("wave declares no seed files; the vault has no inputs".to_string());
        }
        Ok(SeedManifest {
            format: oep_worker::batch::SEED_FORMAT.to_string(),
            version: 1,
            description: Some("P9-M5-T02 GA gate wave corpus".to_string()),
            files,
        })
    }

    /// Generate a deterministic N-job wave over the canonical corpus paths
    /// used by the live proof. The STEP fixtures and the oep-geo golden model
    /// are committed, so a fresh CI checkout always produces real work; the
    /// real CGR CATPart is dev-only material (pending-corpus/, gitignored), so
    /// when it is absent the wave runs the committed corpus only and says so.
    pub fn generate_deterministic(count: usize) -> Self {
        let mut seed = Vec::new();
        // STEP fixtures (committed, relocatable to the repo root).
        let step = |name: &str| -> String {
            format!("{}/crates/oep-worker/tests/fixtures/{}", repo_root(), name)
        };
        // Real CGR material (dev-only; the live proof seeds these).
        let cgr_path = format!(
            "{}/pending-corpus/nist_stc_06_asme1_V56R2023.CATPart",
            repo_root()
        );
        let oepgeo = format!(
            "{}/crates/oep-geo/tests/golden/example-bracket.oepgeo.json",
            repo_root()
        );
        let cgr_present = std::path::Path::new(&cgr_path).exists();

        seed.push(SeedFile {
            item_name: "corpus-box-ref".to_string(),
            path: step("box-reference.step"),
            format: "step".to_string(),
        });
        seed.push(SeedFile {
            item_name: "corpus-box-part".to_string(),
            path: step("box-part.step"),
            format: "step".to_string(),
        });
        seed.push(SeedFile {
            item_name: "corpus-assembly".to_string(),
            path: step("assembly.step"),
            format: "step".to_string(),
        });
        seed.push(SeedFile {
            item_name: "corpus-box-mini".to_string(),
            path: step("box-mini.step"),
            format: "step".to_string(),
        });
        if cgr_present {
            seed.push(SeedFile {
                item_name: "corpus-stc06".to_string(),
                path: cgr_path,
                format: "cgr".to_string(),
            });
        }
        seed.push(SeedFile {
            item_name: "corpus-bracket".to_string(),
            path: oepgeo,
            format: "oepgeo".to_string(),
        });

        let mut jobs = Vec::new();
        for i in 0..count {
            // cgr2gltf only when the real corpus is staged.
            if cgr_present {
                jobs.push(if i % 3 == 2 {
                    render_job(i)
                } else if i % 3 == 0 {
                    convert_step_job(i)
                } else {
                    convert_cgr_job(i)
                });
            } else {
                jobs.push(if i % 2 == 0 {
                    convert_step_job(i)
                } else {
                    render_job(i)
                });
            }
        }

        JobManifest {
            format: "oep-gate-wave".to_string(),
            version: 1,
            description: Some(format!(
                "P9-M5-T02 GA gate wave: {count} render/convert jobs across the fleet \
                 (cgr_corpus_present={})",
                cgr_present
            )),
            seed,
            jobs: jobs.into_iter().map(|spec| WaveJob { spec }).collect(),
        }
    }
}

/// `step2mesh` convert job #`i` cycling the three mesh-producing STEP
/// fixtures (box-mini is a known classified-skip fixture, seeded but not cycled
/// so a full 100-job wave is dominated by verified outputs).
fn convert_step_job(i: usize) -> JobSpec {
    let item = match i % 3 {
        0 => "corpus-box-ref",
        1 => "corpus-box-part",
        _ => "corpus-assembly",
    };
    let mut spec = JobSpec::new(
        ExecutorRef::new("convert", "step2mesh"),
        ResourceCaps::conservative_default(),
    );
    spec.name = Some(format!("step-{item}-{i}"));
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::Item {
            item_id: item.to_string(),
        },
    ));
    spec.output.push(OutputRef {
        name: "mesh".to_string(),
        format: "oep-mesh".to_string(),
        content_type: None,
        target: Some(OutputTarget::NewItem {
            item_name: format!("gate-out-step-{i}"),
            item_kind: Some("part".to_string()),
            context_id: None,
        }),
        expected_sha256: None,
        required: true,
    });
    spec
}

/// `cgr2gltf` convert job #`i`.
fn convert_cgr_job(i: usize) -> JobSpec {
    let mut spec = JobSpec::new(
        ExecutorRef::new("convert", "cgr2gltf"),
        ResourceCaps::conservative_default(),
    );
    spec.name = Some(format!("cgr-stc06-{i}"));
    spec.input.push(InputRef::new(
        "source",
        VaultObjectRef::Item {
            item_id: "corpus-stc06".to_string(),
        },
    ));
    spec.output.push(OutputRef {
        name: "glb".to_string(),
        format: "gltf".to_string(),
        content_type: None,
        target: Some(OutputTarget::NewItem {
            item_name: format!("gate-out-cgr-{i}"),
            item_kind: Some("part".to_string()),
            context_id: None,
        }),
        expected_sha256: None,
        required: true,
    });
    spec
}

/// `oep-geo-render` job #`i` (resolution/fidelity cycle).
fn render_job(i: usize) -> JobSpec {
    let res = match i % 4 {
        0 => "320x240",
        1 => "640x480",
        2 => "800x600",
        _ => "1280x720",
    };
    let fid = match (i / 4) % 3 {
        0 => "low",
        1 => "medium",
        _ => "high",
    };
    let mut args = vec![
        "--resolution".to_string(),
        res.to_string(),
        "--fidelity".to_string(),
        fid.to_string(),
    ];
    let mut spec = JobSpec::new(
        ExecutorRef::new("render", "oep-geo-render"),
        ResourceCaps::conservative_default(),
    );
    spec.executor.args = args.clone();
    spec.name = Some(format!("render-bracket-{i}"));
    spec.input.push(InputRef::new(
        "model",
        VaultObjectRef::Item {
            item_id: "corpus-bracket".to_string(),
        },
    ));
    args.dedup();
    let _ = args;
    spec.output.push(OutputRef {
        name: "image".to_string(),
        format: "png".to_string(),
        content_type: None,
        target: Some(OutputTarget::NewItem {
            item_name: format!("gate-out-render-{i}"),
            item_kind: Some("document".to_string()),
            context_id: None,
        }),
        expected_sha256: None,
        required: true,
    });
    spec
}

/// The repo root, resolved robustly from the current worktree at build/run
/// time (the gate always runs from a isolith checkout).
fn repo_root() -> String {
    // Allow an explicit env override (the proof script sets it), else walk up
    // from CWD until we see Cargo.toml.
    if let Ok(p) = std::env::var("ISOLITH_ROOT") {
        return std::path::PathBuf::from(p).to_string_lossy().into_owned();
    }
    let mut dir = std::env::current_dir().unwrap_or_default();
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir.to_string_lossy().into_owned();
        }
        if !dir.pop() {
            break;
        }
    }
    ".".to_string()
}
