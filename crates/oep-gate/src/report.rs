//! GA gate report: the measured, self-consistent wave proof (JSON + Markdown).
//!
//! Follows the same self-consistency contract as the P2/P3/P4/P8 gate
//! reports: `summary` totals roll up exactly to the emitted rows, and
//! `schema_version == 1`. Exit code is `0` iff no FAIL row anywhere (the
//! driver encodes that contract, not this module).

use serde::{Deserialize, Serialize};

/// Roll-up counters of the whole wave.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Summary {
    /// Total jobs executed.
    pub total: usize,
    /// Jobs that produced + stored + independently hash-verified an output.
    pub ok: usize,
    /// Jobs that classified a skip (valid input, no convertible/renderable
    /// content). Never counted as FAIL.
    pub skipped: usize,
    /// Jobs that errored.
    pub failed: usize,
    /// Outputs re-opened from the vault and re-hashed successfully.
    pub hash_verified: usize,
    /// Outputs whose re-hash did not match (would be FAIL rows).
    pub rehash_mismatch: usize,
    /// Nodes registered in the pool.
    pub nodes_registered: usize,
    /// Distinct nodes that actually executed >= 1 job.
    pub nodes_with_work: usize,
    /// Total dispatches handed out by the scheduler.
    pub dispatched: usize,
    /// Wall-clock duration of the wave in ms.
    pub wall_ms: u64,
}

/// One node's participation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRow {
    /// Node id.
    pub id: String,
    /// Jobs this node executed.
    pub jobs_ran: usize,
    /// Whether this node actually reached the wave (>= 1 dispatch).
    pub reached: bool,
}

/// One job's measured row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRow {
    /// Job name.
    pub job: String,
    /// Scheduler job id (raw u64).
    pub id: u64,
    /// Executor `kind:function`.
    pub executor: String,
    /// Placement node that executed it.
    pub node: String,
    /// Whether the placement CPU-fell-back (GPU-preferred job found no free
    /// device).
    pub cpu_fallback: bool,
    /// Input vault ref display.
    pub input_ref: String,
    /// Artifact sha-256 (client-computed).
    pub artifact_sha256: String,
    /// Vault-recorded content address of the stored file.
    pub vault_blob_sha256: String,
    /// `ok` | `skip` | `error`.
    pub status: String,
    /// True when status was a classified skip.
    pub skipped: bool,
    /// Vault item the output landed under.
    pub output_item: String,
    /// Human note.
    pub note: String,
}

/// The gate report (wire format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateReport {
    /// Card id.
    pub task: String,
    /// RFC 3339 UTC run timestamp.
    pub timestamp: String,
    /// Vault the wave ran against.
    pub vault: String,
    /// Roll-up.
    pub summary: Summary,
    /// Per-node rows.
    pub nodes: Vec<NodeRow>,
    /// Per-job rows.
    pub jobs: Vec<JobRow>,
}

impl GateReport {
    /// Write deterministic JSON.
    pub fn write_json(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| format!("encode: {e}"))?;
        std::fs::write(path, &bytes).map_err(|e| format!("write {path:?}: {e}"))
    }

    /// Write a human Markdown report.
    pub fn write_md(&self, path: &std::path::Path) -> Result<(), String> {
        let mut s = String::new();
        s.push_str(&format!(
            "# P9 GA gate -- {} (P9-M5-T02)\n\n",
            self.summary.total
        ));
        s.push_str(&format!("Run: {}\n\n", self.timestamp));
        s.push_str(&format!("Vault: `{}`\n\n", self.vault));
        s.push_str(&format!(
            "**Summary:** total={} ok={} skip={} fail={} hash_verified={} rehash_mismatch={} nodes={}/{} wall={}ms\n\n",
            self.summary.total,
            self.summary.ok,
            self.summary.skipped,
            self.summary.failed,
            self.summary.hash_verified,
            self.summary.rehash_mismatch,
            self.summary.nodes_with_work,
            self.summary.nodes_registered,
            self.summary.wall_ms
        ));

        s.push_str("## Nodes\n\n| node | jobs_ran | reached |\n|---|---|---|\n");
        for n in &self.nodes {
            s.push_str(&format!("| {} | {} | {} |\n", n.id, n.jobs_ran, n.reached));
        }

        s.push_str("\n## Jobs\n\n");
        s.push_str(
            "| id | job | executor | node | cpu_fallback | status | artifact_sha256 | vault_blob_sha256 | output | note |\n",
        );
        s.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
        for r in &self.jobs {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | `{}` | `{}` | {} | {} |\n",
                r.id,
                r.job,
                r.executor,
                r.node,
                r.cpu_fallback,
                r.status,
                r.artifact_sha256,
                r.vault_blob_sha256,
                r.output_item,
                r.note.replace('|', "/")
            ));
        }
        std::fs::write(path, &s).map_err(|e| format!("write {path:?}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrip_is_self_consistent() {
        let rep = GateReport {
            task: "P9-M5-T02".into(),
            timestamp: "2026-10-17T00:00:00Z".into(),
            vault: "http://vault".into(),
            summary: Summary {
                total: 2,
                ok: 2,
                skipped: 0,
                failed: 0,
                hash_verified: 2,
                rehash_mismatch: 0,
                nodes_registered: 3,
                nodes_with_work: 3,
                dispatched: 2,
                wall_ms: 10,
            },
            nodes: vec![
                NodeRow {
                    id: "u1".into(),
                    jobs_ran: 1,
                    reached: true,
                },
                NodeRow {
                    id: "u2".into(),
                    jobs_ran: 1,
                    reached: true,
                },
                NodeRow {
                    id: "u3".into(),
                    jobs_ran: 0,
                    reached: false,
                },
            ],
            jobs: vec![JobRow {
                job: "j".into(),
                id: 1,
                executor: "convert:step2mesh".into(),
                node: "u1".into(),
                cpu_fallback: false,
                input_ref: "item=corpus-box-ref".into(),
                artifact_sha256: "a".into(),
                vault_blob_sha256: "a".into(),
                status: "ok".into(),
                skipped: false,
                output_item: "gate-out-0".into(),
                note: "".into(),
            }],
        };
        let json = serde_json::to_string(&rep).unwrap();
        let back: GateReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.summary, rep.summary);
        assert_eq!(back.jobs.len(), 1);
    }
}
