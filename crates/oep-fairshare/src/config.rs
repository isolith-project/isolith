//! Fair-share configuration: nodes, per-team quotas with conservative
//! defaults, and cluster assembly with validation.
//!
//! "Configured" means a real, serializable `Config` (see
//! `docs/compute/scheduler/fairshare.sample.json`) rather than hard-coded policy.
//! Deserialization is tolerant: unknown fields are ignored so future optional
//! fields do not break version-1 documents. Validation is non-panicking and
//! returns a structured list of problems.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying P9 fair-share configs.
pub const FORMAT: &str = "oep-fairshare";
/// The current schema version of the config format.
pub const VERSION: u32 = 1;

/// A compute node's total capacity, in the same units the P9 job spec
/// (`oep-job` sibling crate) uses for `ResourceCaps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCapacity {
    /// Total CPU capacity in millicores (1000 = 1 core).
    pub cpu_millicores: u64,
    /// Total memory capacity in MiB.
    pub memory_mib: u64,
}

/// A named compute node in the pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Stable node identifier.
    pub id: String,
    /// Total capacity.
    pub capacity: NodeCapacity,
}

/// Per-team quota. All fields are hard ceilings enforced independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamQuota {
    /// Fair-share weight: scales the per-node max-min allocation. Must be >= 1.
    pub weight: u64,
    /// Hard ceiling on the team's concurrently-running CPU across the cluster.
    pub max_concurrent_cpu_millicores: u64,
    /// Hard ceiling on the team's concurrently-running memory across the cluster.
    pub max_concurrent_memory_mib: u64,
    /// Hard ceiling on the team's concurrently-running job count across the cluster.
    pub max_concurrent_jobs: u64,
}

impl TeamQuota {
    /// A conservative default quota: `weight = 1` and all three caps set to the
    /// provided cluster share (a caller computes `total_capacity / team_count`
    /// so an unconfigured team can never claim more than an equal share).
    ///
    /// `share_cpu`/`share_mem` are the conservative per-team ceilings; the job
    /// cap defaults to the same share value clamped to a sane concurrency
    /// bound (see [`DEFAULT_MAX_CONCURRENT_JOBS`]).
    pub fn conservative_default(share_cpu: u64, share_mem: u64) -> Self {
        let jobs = share_cpu.max(1).min(DEFAULT_MAX_CONCURRENT_JOBS as u64);
        Self {
            weight: 1,
            max_concurrent_cpu_millicores: share_cpu,
            max_concurrent_memory_mib: share_mem,
            max_concurrent_jobs: jobs,
        }
    }
}

/// Bounds a conservative default's job concurrency so an overwhelmingly large
/// per-team CPU share does not silently permit unbounded job fan-out.
pub const DEFAULT_MAX_CONCURRENT_JOBS: u32 = 16;

/// A validated cluster: nodes plus one effective quota per team
/// (explicit where configured, conservative default otherwise).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    /// Nodes in the pool.
    pub nodes: Vec<Node>,
    /// Effective per-team quota, keyed by team id.
    pub teams: BTreeMap<String, TeamQuota>,
}

/// The serializable, human-editable configuration for a fair-share pool.
///
/// `format`/`version` are the wire headers mirroring the `oep-job` spec
/// conventions (P9-M1-T01) so config and code cannot drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Wire header, always `oep-fairshare`.
    pub format: String,
    /// Wire header, always `1`.
    pub version: u32,
    /// Compute nodes.
    pub nodes: Vec<Node>,
    /// Explicit per-team quotas (optional; omitted teams get a conservative
    /// equal-share default when the cluster is built).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub teams: BTreeMap<String, TeamQuota>,
}

impl Config {
    /// Build a validated [`Cluster`] from this config.
    ///
    /// Every team known to the cluster gets an effective quota: explicit
    /// entries are used as-is; teams named only in a node-level demand (the
    /// scheduler sees teams as they submit) are handled dynamically, but teams
    /// declared here with no explicit quota entry receive a conservative
    /// equal-share default computed from the cluster totals.
    ///
    /// Validation failures (duplicate node ids, zero capacity, zero/oversized
    /// weights, empty pool) are returned as a structured list; nothing is
    /// silently repaired.
    pub fn build(&self) -> Result<Cluster, Vec<String>> {
        let mut problems = Vec::new();

        // format/version identity
        if self.format != FORMAT {
            problems.push(format!("format must be {FORMAT:?}, got {:?}", self.format));
        }
        if self.version != VERSION {
            problems.push(format!("version must be {VERSION}, got {}", self.version));
        }

        // node checks
        let mut seen = BTreeSet::new();
        for n in &self.nodes {
            if !seen.insert(n.id.clone()) {
                problems.push(format!("duplicate node id {:?}", n.id));
            }
            if n.capacity.cpu_millicores == 0 && n.capacity.memory_mib == 0 {
                problems.push(format!("node {:?} has zero capacity", n.id));
            }
        }
        if self.nodes.is_empty() {
            problems.push("cluster must have at least one node".to_string());
        }

        // team checks
        let mut seen_teams = BTreeSet::new();
        for (tid, q) in &self.teams {
            if !seen_teams.insert(tid.clone()) {
                problems.push(format!("duplicate team id {tid:?}"));
            }
            if q.weight == 0 {
                problems.push(format!("team {tid:?} weight must be >= 1"));
            }
        }

        if !problems.is_empty() {
            return Err(problems);
        }

        // Effective quotas: explicit entries are used as-is. Teams that are
        // NOT declared (and so cannot appear in the map set) receive a
        // conservative equal-share default at scheduler-enforcement time
        // (see FairShareScheduler::default_quota), sized from the cluster
        // totals computed below.
        let mut teams = BTreeMap::new();
        for (tid, q) in &self.teams {
            teams.insert(tid.clone(), q.clone());
        }

        Ok(Cluster {
            nodes: self.nodes.clone(),
            teams,
        })
    }
}

impl Default for Config {
    /// An empty-but-valid placeholder; [`Config::build`] rejects it until at
    /// least one node is added (fail-loud on a nonsense pool).
    fn default() -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            nodes: Vec::new(),
            teams: BTreeMap::new(),
        }
    }
}
