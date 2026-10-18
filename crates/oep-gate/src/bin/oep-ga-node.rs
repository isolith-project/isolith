//! `oep-ga-node` -- the P9 GA gate node agent.
//!
//! The uniform worker agent deployed (or run locally) on every node of the
//! wave: reads one `oep-job` JobSpec from stdin, executes the real worker
//! function (convert: `cgr2gltf`/`step2mesh` via `oep_worker`; render:
//! `oep-geo-render` via `oep_gate::render`) against the **shared** P8 vault,
//! stores the output into the vault, hash-verifies it, and prints a single
//! JSON line with the outcome.
//!
//! ```text
//!   oep-ga-node run --vault <base/api/v1> < job.json
//! ```
//!
//! The output JSON carries the exact fields the gate driver's report needs:
//! `status` (`ok`/`skip`/`error`), `input_ref`, `artifact_sha256`,
//! `vault_blob_sha256`, `output_item`, `note`. Exit 0 = the job ran to a
//! typed outcome (ok or skip); exit 1 = precondition/worker error.

use std::io::{Read, Write};

use oep_gate::render::{run_render_job, RenderStatus};
use oep_job::JobSpec;
use oep_worker::worker::{run_job, JobStatus};

fn usage_and_exit() -> ! {
    eprintln!("usage: oep-ga-node run --vault <base/api/v1> < job.json");
    std::process::exit(2);
}

fn emit(status: &str, skipped: bool, obj: &serde_json::Map<String, serde_json::Value>) {
    let mut o = obj.clone();
    o.insert(
        "status".to_string(),
        serde_json::Value::String(status.to_string()),
    );
    o.insert("skipped".to_string(), serde_json::Value::Bool(skipped));
    println!("{}", serde_json::Value::Object(o));
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        usage_and_exit();
    }
    let mut vault: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "run" => {}
            "--vault" => {
                if i + 1 < argv.len() {
                    vault = Some(argv[i + 1].clone());
                    i += 1;
                }
            }
            other => {
                eprintln!("unknown arg: {other}");
                usage_and_exit();
            }
        }
        i += 1;
    }
    let vault = vault.unwrap_or_else(|| usage_and_exit());

    // Read the job spec from stdin.
    let mut input = Vec::new();
    std::io::stdin()
        .read_to_end(&mut input)
        .expect("read stdin");
    let spec: JobSpec = match serde_json::from_slice(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bad job spec: {e}");
            std::process::exit(1);
        }
    };

    let mut adapter = match oep_pdm::adapter::PdmAdapter::new(&vault) {
        Ok(a) => a,
        Err(e) => {
            let mut m = serde_json::Map::new();
            m.insert(
                "note".to_string(),
                serde_json::Value::String(format!("vault: {e}")),
            );
            emit("error", false, &m);
            std::process::exit(1);
        }
    };

    let job_name = spec
        .name
        .clone()
        .unwrap_or_else(|| spec.executor.function.clone());

    match spec.executor.kind.as_str() {
        "convert" => match run_job(&spec, &mut adapter, &job_name) {
            Ok(o) => {
                let mut m = serde_json::Map::new();
                m.insert(
                    "input_ref".to_string(),
                    serde_json::Value::String(o.input_ref.clone()),
                );
                m.insert(
                    "artifact_sha256".to_string(),
                    serde_json::Value::String(o.artifact_sha256.clone()),
                );
                m.insert(
                    "vault_blob_sha256".to_string(),
                    serde_json::Value::String(o.vault_blob_sha256.clone()),
                );
                m.insert(
                    "note".to_string(),
                    serde_json::Value::String(o.note.clone()),
                );
                m.insert(
                    "output_item".to_string(),
                    serde_json::Value::String(o.output_item.clone()),
                );
                match o.status {
                    JobStatus::Ok => emit("ok", false, &m),
                    JobStatus::Skipped(_) => emit("skip", true, &m),
                    JobStatus::Failed(msg) => {
                        m.insert("note".to_string(), serde_json::Value::String(msg));
                        emit("error", false, &m);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                let mut m = serde_json::Map::new();
                m.insert("note".to_string(), serde_json::Value::String(e.to_string()));
                emit("error", false, &m);
                std::process::exit(1);
            }
        },
        "render" => match run_render_job(&spec, &mut adapter, &job_name) {
            Ok(o) => {
                let mut m = serde_json::Map::new();
                m.insert(
                    "input_ref".to_string(),
                    serde_json::Value::String(o.input_ref),
                );
                m.insert(
                    "artifact_sha256".to_string(),
                    serde_json::Value::String(o.artifact_sha256),
                );
                m.insert(
                    "vault_blob_sha256".to_string(),
                    serde_json::Value::String(o.vault_blob_sha256),
                );
                m.insert(
                    "output_item".to_string(),
                    serde_json::Value::String(o.output_item),
                );
                m.insert("note".to_string(), serde_json::Value::String(o.note));
                match o.status {
                    RenderStatus::Ok => emit("ok", false, &m),
                    RenderStatus::Skipped(note) => {
                        m.insert("note".to_string(), serde_json::Value::String(note));
                        emit("skip", true, &m);
                    }
                    RenderStatus::Failed(msg) => {
                        m.insert("note".to_string(), serde_json::Value::String(msg));
                        emit("error", false, &m);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                let mut m = serde_json::Map::new();
                m.insert("note".to_string(), serde_json::Value::String(e.to_string()));
                emit("error", false, &m);
                std::process::exit(1);
            }
        },
        other => {
            let mut m = serde_json::Map::new();
            m.insert(
                "note".to_string(),
                serde_json::Value::String(format!("unknown executor kind: {other}")),
            );
            emit("error", false, &m);
            std::process::exit(1);
        }
    }
    let _ = std::io::stdout().flush();
}
