//! `oep-ga-gate` -- the P9 GA gate wave driver.
//!
//! Runs a **100-job render/convert wave across 3+ real nodes** through the
//! real P9 scheduler, with every output hash-verified in the real P8 vault:
//!
//! ```text
//!   oep-ga-gate --vault http://127.0.0.1:PORT/api/v1 \
//!     --fleet fleet.json [--jobs manifest.json] [--report stem] \
//!     [--lease-ms N] [--max-dispatched N] [--local-only]
//! ```
//!
//! The driver:
//!
//! 1. connects to the live vault (integration adapter);
//! 2. seeds the corpus inputs into the vault (STEP/CGR files + an oep-geo
//!    model for the render lane) following the P9-M3-T02 seed manifest shape;
//! 3. builds the real `oep_lane` Scheduler with a `NodePool` of every node in
//!    `--fleet`;
//! 4. enqueues each job in the manifest with a GPU-preferred request so the
//!    real placement policy distributes the wave across the fleet's
//!    GPU-bearing nodes by exclusive device reservation (the honest path that
//!    sends jobs to 3+ distinct nodes; the reference renderer is CPU today,
//!    recorded in the report);
//! 5. runs the dispatch/ack loop on a wall clock -- each dispatch is executed
//!    by the placement node: locally (in-process worker functions against the
//!    shared vault) for the coordinator, or over SSH to the uniform
//!    `oep-ga-node` agent for remote nodes;
//! 6. re-opens every produced output from the vault and re-hashes it
//!    independently of the worker's own check;
//! 7. writes a self-consistent JSON + Markdown report with the GA exit-code
//!    contract (0 iff no FAIL row).
//!
//! Exit codes: `0` no gate FAIL; `1` at least one gate FAIL; `2` usage /
//! precondition error.

use std::collections::BTreeMap;
use std::path::PathBuf;

use oep_gate::render::{run_render_job, RenderOutcome, RenderStatus};
use oep_gate::report::{GateReport, JobRow, NodeRow, Summary};
use oep_lane::placement::{GpuRequest, NodePool};
use oep_lane::sched::scheduler::{Dispatch, JobId, JobSpec as SchedJobSpec, Scheduler};
use oep_worker::worker::{run_job, JobOutcome, JobStatus};

use oep_gate::fleet::Fleet;
use oep_gate::manifest::{JobManifest, WaveJob, DEFAULT_JOBS};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: oep-ga-gate --vault <base/api/v1> --fleet <fleet.json> [--jobs <manifest.json>]\n\
         \t[--report <stem>] [--lease-ms N] [--max-dispatched N] [--local-only]\n\
         \t[--jobs-count N] (generate N deterministic jobs when --jobs absent)\n\
         \t[--remote-vault <LAN base/api/v1>] (vault URL remote agents reach; \
         defaults to --vault)"
    );
    std::process::exit(2);
}

struct Args {
    vault: String,
    remote_vault: Option<String>,
    fleet: PathBuf,
    jobs: Option<PathBuf>,
    jobs_count: usize,
    report: PathBuf,
    lease_ms: u64,
    max_dispatched: usize,
    local_only: bool,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut vault: Option<String> = None;
    let mut remote_vault: Option<String> = None;
    let mut fleet: Option<PathBuf> = None;
    let mut jobs: Option<PathBuf> = None;
    let mut jobs_count = DEFAULT_JOBS;
    let mut report = PathBuf::from("./ga-gate-p9-wave");
    let mut lease_ms = 60_000u64;
    let mut max_dispatched: Option<usize> = None;
    let mut local_only = false;
    let mut i = 0;
    while i < argv.len() {
        let flag = argv[i].clone();
        let mut next_val = || -> Option<String> {
            i += 1;
            argv.get(i).cloned()
        };
        match flag.as_str() {
            "--vault" => vault = next_val(),
            "--remote-vault" => remote_vault = next_val(),
            "--fleet" => fleet = next_val().map(PathBuf::from),
            "--jobs" => jobs = next_val().map(PathBuf::from),
            "--jobs-count" => {
                let v = next_val().unwrap_or_default();
                jobs_count = v.parse().unwrap_or_else(|_| usage_and_exit());
            }
            "--report" => report = PathBuf::from(next_val().unwrap_or_default()),
            "--lease-ms" => {
                let v = next_val().unwrap_or_default();
                lease_ms = v.parse().unwrap_or_else(|_| usage_and_exit());
            }
            "--max-dispatched" => {
                let v = next_val().unwrap_or_default();
                max_dispatched = Some(v.parse().unwrap_or_else(|_| usage_and_exit()));
            }
            "--local-only" => local_only = true,
            _ => usage_and_exit(),
        }
        i += 1;
    }
    let vault = vault.unwrap_or_else(|| usage_and_exit());
    let fleet = fleet.unwrap_or_else(|| usage_and_exit());
    let max_dispatched = max_dispatched.unwrap_or(4);
    Args {
        vault,
        remote_vault,
        fleet,
        jobs,
        jobs_count,
        report,
        lease_ms,
        max_dispatched,
        local_only,
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// RFC 3339 UTC timestamp (std-only).
fn utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        sod / 3600,
        (sod / 60) % 60,
        sod % 60
    )
}

/// Howard Hinnant's `civil_from_days` (std-only, UTC calendar).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    // Preconditions first (no partial work before they are real).
    let fleet = Fleet::load(&args.fleet).unwrap_or_else(|e| {
        eprintln!("FATAL: fleet {e}");
        std::process::exit(2);
    });
    let manifest = match &args.jobs {
        Some(path) => JobManifest::load(path).unwrap_or_else(|e| {
            eprintln!("FATAL: manifest {e}");
            std::process::exit(2);
        }),
        None => JobManifest::generate_deterministic(args.jobs_count),
    };
    // With exclusive device reservation on the fleet's single-GPU nodes, the
    // honest concurrency bound is the total device capacity of the fleet: a
    // GPU-preferred job parks (never fabricates a node) when every device is
    // reserved. Never dispatch more than the pool can hold.
    let device_capacity = fleet
        .node_resources()
        .iter()
        .map(|r| r.gpus.len().max(1))
        .sum::<usize>();
    let max_dispatched = if args.local_only {
        args.max_dispatched
    } else {
        args.max_dispatched.min(device_capacity).max(1)
    };

    let mut adapter = oep_pdm::adapter::PdmAdapter::new(&args.vault).unwrap_or_else(|e| {
        eprintln!("FATAL: cannot reach vault {}: {e}", args.vault);
        std::process::exit(2);
    });
    if adapter.health().is_err() {
        eprintln!("FATAL: vault {} did not answer health", args.vault);
        std::process::exit(2);
    }

    // Seed the wave's real corpus into the vault (idempotent: re-runs refresh
    // the item's revision, which is exactly what a re-run should do).
    let seed = manifest.seed_manifest().unwrap_or_else(|e| {
        eprintln!("FATAL: seed manifest: {e}");
        std::process::exit(2);
    });
    oep_worker::batch::seed_from_manifest(&seed, &mut adapter).unwrap_or_else(|e| {
        eprintln!("FATAL: seeding corpus into the vault: {e}");
        std::process::exit(2);
    });

    // Build the real node pool from the fleet (real resources incl. GPU) and
    // attach it for a distributed run. In --local-only mode the wave is a
    // baseline unplaced scheduler: every dispatch stays on the coordinator
    // node (CI-safe, no fleet reachability assumed), matching the P9-M2-T01
    // contract exactly.
    let mut sched = Scheduler::with_lease(args.lease_ms);
    if !args.local_only {
        let pool = NodePool::from_nodes(fleet.node_resources());
        sched.attach_pool(pool);
    }

    // Enqueue every wave job (GPU-preferred so placement spreads the wave).
    let mut enqueued: Vec<(JobId, WaveJob)> = Vec::new();
    for wj in &manifest.jobs {
        let payload = serde_json::to_string(&wj.spec).unwrap();
        let spec = SchedJobSpec::new(payload).with_gpu(GpuRequest {
            count: 1,
            required: false,
            model: fleet
                .gpu_model
                .clone()
                .map(|m| m.to_uppercase())
                .or_else(|| Some("NVIDIA GB10".to_string())),
        });
        match sched.enqueue(spec) {
            Ok(id) => enqueued.push((id, wj.clone())),
            Err(e) => {
                eprintln!("FATAL: enqueue {}: {:?}", wj.name(), e);
                std::process::exit(2);
            }
        }
    }
    let total = enqueued.len();
    eprintln!("wave: enqueued {total} jobs into the real scheduler");

    // Dispatch/ack loop on a wall clock, topping up to `max_dispatched`
    // concurrent running jobs. Each dispatch executes on its placement node.
    let mut rows: Vec<JobRow> = Vec::new();
    let mut in_flight: Vec<(Dispatch, WaveJob)> = Vec::new();
    let start = now_ms();
    let budget_ms = 900_000u64; // hard wall budget 15 min

    loop {
        let t = now_ms();
        if t - start > budget_ms {
            eprintln!(
                "FATAL: wave budget expired with {} in flight",
                in_flight.len()
            );
            std::process::exit(1);
        }

        // Top up in-flight until the scheduler parks (no free device) or we
        // hit the concurrency cap.
        while in_flight.len() < max_dispatched {
            let Some(d) = sched.dispatch(t) else { break };
            let wj = enqueued
                .iter()
                .find(|(id, _)| *id == d.job)
                .map(|(_, w)| w.clone());
            match wj {
                Some(w) => in_flight.push((d, w)),
                None => {
                    eprintln!("FATAL: dispatch for unknown job {}", d.job);
                    std::process::exit(1);
                }
            }
        }

        if in_flight.is_empty() {
            let done = sched.pending_count() == 0
                && sched.ready_count() == 0
                && sched.running_count() == 0
                && sched.total_count() == total;
            if done {
                break;
            }
            // Either all jobs parked (no free device) or in a backoff window:
            // reclaim expired leases and tick.
            let _ = sched.reclaim_expired(t);
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        // Execute all in-flight dispatches on their placement nodes.
        let batch: Vec<(Dispatch, WaveJob)> = std::mem::take(&mut in_flight);
        for (d, wj) in batch {
            let node_id = if d.placement.node_id.is_empty() {
                fleet.coordinator_id()
            } else {
                d.placement.node_id.clone()
            };
            let out = execute_one(&args, &fleet, &wj, &node_id);
            let row = JobRow {
                job: wj.name().to_string(),
                id: d.job.raw(),
                executor: format!("{}:{}", wj.spec.executor.kind, wj.spec.executor.function),
                node: node_id.clone(),
                cpu_fallback: d.placement.cpu_fallback,
                input_ref: out.input_ref.clone(),
                artifact_sha256: out.artifact_sha256.clone(),
                vault_blob_sha256: out.vault_blob_sha256.clone(),
                status: out.status.clone(),
                skipped: out.skipped,
                output_item: out.output_item.clone(),
                note: out.note.clone(),
            };
            let ok = out.status == "ok" || out.skipped;
            if ok {
                let _ = sched.ack(d.job, d.generation);
            } else {
                let _ = sched.fail(d.job, d.generation, now_ms());
            }
            rows.push(row);
        }
    }

    // Independent verification: reopen every produced output from the vault
    // and re-hash, comparing to the artifact's content address.
    let mut rehashed = 0usize;
    let mut mismatch = 0usize;
    let mut fresh_adapter = oep_pdm::adapter::PdmAdapter::new(&args.vault).unwrap();
    for row in rows.iter_mut() {
        if row.artifact_sha256.is_empty() || row.skipped {
            continue;
        }
        match fresh_adapter.open_from_pdm(&row.output_item) {
            Ok(o) => {
                let hex = sha256_hex(&o.bytes);
                if hex == row.artifact_sha256 && o.blob_sha256 == row.artifact_sha256 {
                    rehashed += 1;
                } else {
                    mismatch += 1;
                    row.status = "error".to_string();
                    row.note = format!(
                        "re-hash mismatch: reopened {hex}, vault {}, artifact {}",
                        o.blob_sha256, row.artifact_sha256
                    );
                }
            }
            Err(e) => {
                mismatch += 1;
                row.status = "error".to_string();
                row.note = format!("output not reopenable from the vault: {e}");
            }
        }
    }

    // Node health rows: which fleet nodes actually ran jobs.
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rows {
        if r.node.is_empty() {
            continue;
        }
        *used.entry(r.node.clone()).or_insert(0) += 1;
    }
    let node_rows: Vec<NodeRow> = fleet
        .nodes
        .iter()
        .map(|n| NodeRow {
            id: n.id.clone(),
            jobs_ran: used.get(&n.id).copied().unwrap_or(0),
            reached: used.contains_key(&n.id),
        })
        .collect();

    let ok_jobs = rows.iter().filter(|r| r.status == "ok").count();
    let skipped_jobs = rows.iter().filter(|r| r.skipped).count();
    let failed_jobs = rows
        .iter()
        .filter(|r| r.status == "error" && !r.skipped)
        .count();
    let nodes_with_work = node_rows.iter().filter(|n| n.reached).count();

    let summary = Summary {
        total: rows.len(),
        ok: ok_jobs,
        skipped: skipped_jobs,
        failed: failed_jobs,
        hash_verified: rehashed,
        rehash_mismatch: mismatch,
        nodes_registered: fleet.nodes.len(),
        nodes_with_work,
        dispatched: total,
        wall_ms: now_ms() - start,
    };

    let report = GateReport {
        task: "P9-M5-T02".to_string(),
        timestamp: utc_now(),
        vault: args.vault.clone(),
        summary,
        nodes: node_rows,
        jobs: rows,
    };

    let fail = report.summary.failed > 0
        || report.summary.rehash_mismatch > 0
        || (report.summary.nodes_with_work < 3 && !args.local_only);
    let json_path = args.report.with_extension("json");
    let md_path = args.report.with_extension("md");
    report.write_json(&json_path).unwrap_or_else(|e| {
        eprintln!("FATAL: write report json: {e}");
        std::process::exit(2);
    });
    report.write_md(&md_path).unwrap_or_else(|e| {
        eprintln!("FATAL: write report md: {e}");
        std::process::exit(2);
    });

    println!(
        "P9 GA gate: total={} pass={} fail={} skip={} hash_verified={} nodes={}/{} | exit={}",
        report.summary.total,
        report.summary.ok,
        report.summary.failed + report.summary.rehash_mismatch,
        report.summary.skipped,
        report.summary.hash_verified,
        report.summary.nodes_with_work,
        report.summary.nodes_registered,
        if fail { 1 } else { 0 }
    );
    eprintln!("report: {}", json_path.display());
    std::process::exit(if fail { 1 } else { 0 });
}

// ---------------------------------------------------------------------------
// node-execution transport
// ---------------------------------------------------------------------------

/// The outcome fields the report needs from one executed job.
struct ExecOutcome {
    input_ref: String,
    artifact_sha256: String,
    vault_blob_sha256: String,
    status: String,
    skipped: bool,
    output_item: String,
    note: String,
}

impl ExecOutcome {
    fn error(msg: String) -> Self {
        ExecOutcome {
            input_ref: String::new(),
            artifact_sha256: String::new(),
            vault_blob_sha256: String::new(),
            status: "error".to_string(),
            skipped: false,
            output_item: String::new(),
            note: msg,
        }
    }

    fn from_convert(o: &JobOutcome) -> Self {
        ExecOutcome {
            input_ref: o.input_ref.clone(),
            artifact_sha256: o.artifact_sha256.clone(),
            vault_blob_sha256: o.vault_blob_sha256.clone(),
            status: match &o.status {
                JobStatus::Ok => "ok".to_string(),
                JobStatus::Skipped(_) => "skip".to_string(),
                JobStatus::Failed(_) => "error".to_string(),
            },
            skipped: matches!(o.status, JobStatus::Skipped(_)),
            output_item: o.output_item.clone(),
            note: o.note.clone(),
        }
    }

    fn from_render(o: &RenderOutcome) -> Self {
        ExecOutcome {
            input_ref: o.input_ref.clone(),
            artifact_sha256: o.artifact_sha256.clone(),
            vault_blob_sha256: o.vault_blob_sha256.clone(),
            status: match &o.status {
                RenderStatus::Ok => "ok".to_string(),
                RenderStatus::Skipped(_) => "skip".to_string(),
                RenderStatus::Failed(_) => "error".to_string(),
            },
            skipped: matches!(o.status, RenderStatus::Skipped(_)),
            output_item: o.output_item.clone(),
            note: o.note.clone(),
        }
    }
}

/// Execute one job on its placement node.
fn execute_one(args: &Args, fleet: &Fleet, wj: &WaveJob, node_id: &str) -> ExecOutcome {
    let endpoint = fleet.endpoint_for(node_id);
    if args.local_only || endpoint.is_local {
        // Coordinator node (or CI local-only): run the real worker functions
        // in-process against the shared vault.
        let mut adapter = oep_pdm::adapter::PdmAdapter::new(&args.vault).unwrap_or_else(|e| {
            eprintln!("FATAL: adapter re-init: {e}");
            std::process::exit(2);
        });
        return match wj.spec.executor.kind.as_str() {
            "convert" => match run_job(&wj.spec, &mut adapter, wj.name()) {
                Ok(o) => ExecOutcome::from_convert(&o),
                Err(e) => ExecOutcome::error(e.to_string()),
            },
            "render" => match run_render_job(&wj.spec, &mut adapter, wj.name()) {
                Ok(o) => ExecOutcome::from_render(&o),
                Err(e) => ExecOutcome::error(e.to_string()),
            },
            other => ExecOutcome::error(format!("unknown executor kind: {other}")),
        };
    }

    // Remote node: stream the job spec over SSH to the deployed agent. The
    // agent must reach the vault over the LAN (never 127.0.0.1 on a remote
    // host), so the remote URL is `--remote-vault` when provided, else the
    // same `--vault` (correct when the coordinator runs the vault on the LAN).
    use std::process::{Command, Stdio};
    let payload = serde_json::to_string(&wj.spec).unwrap();
    let remote_base = args.remote_vault.as_deref().unwrap_or(&args.vault);
    let agent_cmd = format!(
        "{}/oep-ga-node run --vault '{}'",
        endpoint.agent_dir, remote_base
    );
    let ssh_cmd = format!(
        "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new {}@{} \"{}\"",
        endpoint.user, endpoint.host, agent_cmd
    );
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(&ssh_cmd)
        .stdin(std::process::Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ExecOutcome::error(format!("spawn ssh: {e}")),
    };
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload.as_bytes());
        }
    }
    let out = child.wait_with_output();
    let (code, stdout, stderr) = match out {
        Ok(o) => (o.status.code().unwrap_or(1), o.stdout, o.stderr),
        Err(e) => return ExecOutcome::error(format!("ssh exec: {e}")),
    };
    if code != 0 {
        return ExecOutcome::error(format!(
            "node agent exit {code}: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&stdout);
    match serde_json_from_stdout(&text) {
        Some(v) => ExecOutcome {
            input_ref: v
                .get("input_ref")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            artifact_sha256: v
                .get("artifact_sha256")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            vault_blob_sha256: v
                .get("vault_blob_sha256")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            status: v
                .get("status")
                .and_then(|x| x.as_str())
                .unwrap_or("error")
                .to_string(),
            skipped: v.get("status").and_then(|x| x.as_str()) == Some("skip"),
            output_item: v
                .get("output_item")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            note: v
                .get("note")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        },
        None => ExecOutcome::error(format!("agent returned non-JSON: {text}")),
    }
}

/// Extract the last JSON object from the agent's stdout (robust to prefix logs).
fn serde_json_from_stdout(text: &str) -> Option<serde_json::Value> {
    for line in text.lines().rev() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if v.is_object() {
                return Some(v);
            }
        }
    }
    None
}
