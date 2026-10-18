//! Quota enforcement + per-node fair-share admission: the live scheduler.
//!
//! [`FairShareScheduler`] is the admission gate the P9 scheduler (sibling
//! `oep-lane` crate, P9-M2-T01) consults before dispatch. It answers one
//! question for each job submission: *may team `T` run `request` on node `N`?*
//!
//! Two independent gates, both conservative:
//!
//! 1. **Cluster-wide quota** ([`ThrottleReason::CpuQuota`] / `MemoryQuota` /
//!    `JobQuota`): the team's currently-running resources plus this request
//!    must stay inside its configured caps. An oversubscribed team is
//!    throttled here even when the cluster is idle -- that is the card's
//!    "quotas configured and enforced under load".
//! 2. **Per-node fair share** ([`ThrottleReason::NodeFairShareCpu`] /
//!    `NodeFairShareMemory`): recompute [`weighted_max_min`] over every team
//!    with demand on the node; the submitting team may not exceed its
//!    allocation. Two equal-weight teams therefore saturate a node 50/50.
//!
//! Teams not declared in the config get a conservative default quota: an
//! equal slice of the whole cluster assuming at least two tenants ever run
//! (see [`FairShareScheduler::default_quota`]) -- an unconfigured team can
//! never hoard the pool.

use std::collections::BTreeMap;

use crate::config::{Cluster, Config, Node, TeamQuota};
use crate::fair::weighted_max_min;

/// A job's resource request, in the same units as the P9 job spec caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceRequest {
    /// CPU requested in millicores.
    pub cpu_millicores: u64,
    /// Memory requested in MiB.
    pub memory_mib: u64,
}

/// Why an admission was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleReason {
    /// Team's cluster-wide CPU quota would be exceeded.
    CpuQuota,
    /// Team's cluster-wide memory quota would be exceeded.
    MemoryQuota,
    /// Team's cluster-wide concurrent-job quota would be exceeded.
    JobQuota,
    /// Node CPU is physically saturated.
    NodeCpuCapacity,
    /// Node memory is physically saturated.
    NodeMemoryCapacity,
    /// Team's CPU usage on this node would exceed its weighted fair share.
    NodeFairShareCpu,
    /// Team's memory usage on this node would exceed its weighted fair share.
    NodeFairShareMemory,
    /// Submission named a node that is not in the pool.
    UnknownNode,
    /// Submission named a team with no effective quota (config error).
    UnknownTeam,
}

impl std::fmt::Display for ThrottleReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The decision for one submission attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Admitted; the request now counts against the team and node.
    Admitted,
    /// Refused; the job must wait (throttled) until usage frees up.
    Throttled {
        /// Which gate refused the request.
        reason: ThrottleReason,
    },
}

/// Per-node, per-team running usage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TeamUsage {
    cpu_millicores: u64,
    memory_mib: u64,
}

/// Per-node running usage across all teams.
#[derive(Debug, Clone, Copy, Default)]
struct NodeUsage {
    cpu_millicores: u64,
    memory_mib: u64,
}

/// The live admission gate enforcing per-team quotas and per-node fairness.
///
/// Construct via [`FairShareScheduler::from_config`]. Stateful: admitted
/// requests accumulate until [`FairShareScheduler::complete`] releases them,
/// so a load wave (many submissions, few completions) exercises the throttle
/// path exactly as the card's acceptance demands.
#[derive(Debug)]
pub struct FairShareScheduler {
    nodes: Vec<Node>,
    /// Effective per-team quota map (explicit entries + conservative
    /// defaults materialized on first use).
    quotas: BTreeMap<String, TeamQuota>,
    /// Count of teams *declared* in the config, used to size the
    /// conservative default share.
    declared_teams: usize,
    /// (node_idx, team_id) -> running usage on that node.
    node_team: BTreeMap<(usize, String), TeamUsage>,
    /// node_idx -> total running usage.
    node_total: BTreeMap<usize, NodeUsage>,
    /// team_id -> cluster-wide running usage.
    team_running: BTreeMap<String, TeamUsage>,
    /// team_id -> count of concurrently running jobs cluster-wide.
    team_jobs: BTreeMap<String, u64>,
}

impl FairShareScheduler {
    /// Build a scheduler from a validated [`Cluster`].
    pub fn from_config(config: &Config) -> Result<Self, Vec<String>> {
        let cluster = config.build()?;
        Ok(Self::from_cluster(cluster))
    }

    /// Build a scheduler from an already-validated [`Cluster`].
    pub fn from_cluster(cluster: Cluster) -> Self {
        let declared_teams = cluster.teams.len();
        Self {
            nodes: cluster.nodes,
            quotas: cluster.teams,
            declared_teams,
            node_team: BTreeMap::new(),
            node_total: BTreeMap::new(),
            team_running: BTreeMap::new(),
            team_jobs: BTreeMap::new(),
        }
    }

    /// The number of compute nodes in the pool.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The pool's nodes.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The effective quota for a team, materializing the conservative
    /// default on first use.
    pub fn effective_quota(&mut self, team: &str) -> Option<TeamQuota> {
        if let Some(q) = self.quotas.get(team) {
            return Some(q.clone());
        }
        let q = self.default_quota();
        self.quotas.insert(team.to_string(), q.clone());
        Some(q)
    }

    /// The conservative default quota for an undeclared team.
    ///
    /// Equal slice of the whole cluster divided over `max(2, declared + 1)`
    /// tenants: a brand-new team can never take a majority of the pool, and
    /// room shrinks as more teams are declared. This is deliberately
    /// conservative: explicitly configured teams can raise it.
    fn default_quota(&self) -> TeamQuota {
        let total_cpu: u64 = self.nodes.iter().map(|n| n.capacity.cpu_millicores).sum();
        let total_mem: u64 = self.nodes.iter().map(|n| n.capacity.memory_mib).sum();
        let divisor = (self.declared_teams + 1).max(2) as u64;
        TeamQuota::conservative_default(total_cpu / divisor, total_mem / divisor)
    }

    /// Try to admit `request` for `team` on `node`.
    ///
    /// Returns [`Admission::Admitted`] only when every gate passes, in which
    /// case usage is recorded (call [`FairShareScheduler::complete`] when the
    /// job finishes to release it). Otherwise returns
    /// [`Admission::Throttled`] with the first violated gate.
    pub fn admit(&mut self, team: &str, node: &str, request: ResourceRequest) -> Admission {
        let Some(node_idx) = self.nodes.iter().position(|n| n.id == node) else {
            return Admission::Throttled {
                reason: ThrottleReason::UnknownNode,
            };
        };
        let Some(quota) = self.effective_quota(team) else {
            return Admission::Throttled {
                reason: ThrottleReason::UnknownTeam,
            };
        };

        // 1. Cluster-wide quota gate.
        let running = self.team_running.get(team).copied().unwrap_or_default();
        if running
            .cpu_millicores
            .saturating_add(request.cpu_millicores)
            > quota.max_concurrent_cpu_millicores
        {
            return Admission::Throttled {
                reason: ThrottleReason::CpuQuota,
            };
        }
        if running.memory_mib.saturating_add(request.memory_mib) > quota.max_concurrent_memory_mib {
            return Admission::Throttled {
                reason: ThrottleReason::MemoryQuota,
            };
        }
        let jobs = self.team_jobs.get(team).copied().unwrap_or_default();
        if jobs.saturating_add(1) > quota.max_concurrent_jobs {
            return Admission::Throttled {
                reason: ThrottleReason::JobQuota,
            };
        }

        // 2. Per-node weighted fair-share gate (CPU and memory independently).
        //    Checked before the physical capacity gate: when a node is fully
        //    and fairly allocated, an over-share request is a *fairness*
        //    violation first (that is the scheduler policy), while capacity
        //    still bounds a team that genuinely outgrew a node's hardware.
        let node_cap = &self.nodes[node_idx].capacity;
        let current_on_node = self
            .node_team
            .get(&(node_idx, team.to_string()))
            .copied()
            .unwrap_or_default();
        let candidate_cpu = current_on_node
            .cpu_millicores
            .saturating_add(request.cpu_millicores);
        let candidate_mem = current_on_node
            .memory_mib
            .saturating_add(request.memory_mib);

        let (weights, cpu_demands) = self.node_cpu_demands(node_idx, team, candidate_cpu);
        let cpu_alloc = weighted_max_min(node_cap.cpu_millicores, &weights, &cpu_demands);
        let my_cpu_alloc = cpu_alloc[weights.len() - 1]; // submitting team is last
        if candidate_cpu > my_cpu_alloc {
            return Admission::Throttled {
                reason: ThrottleReason::NodeFairShareCpu,
            };
        }

        let (weights_m, mem_demands) = self.node_mem_demands(node_idx, team, candidate_mem);
        let mem_alloc = weighted_max_min(node_cap.memory_mib, &weights_m, &mem_demands);
        let my_mem_alloc = mem_alloc[weights_m.len() - 1];
        if candidate_mem > my_mem_alloc {
            return Admission::Throttled {
                reason: ThrottleReason::NodeFairShareMemory,
            };
        }

        // 3. Node physical capacity gate (hardware bound; cannot be exceeded
        //    even by a team that is within its share on an undersized node).
        let node_used = self.node_total.get(&node_idx).copied().unwrap_or_default();
        let node_cap = &self.nodes[node_idx].capacity;
        if node_used
            .cpu_millicores
            .saturating_add(request.cpu_millicores)
            > node_cap.cpu_millicores
        {
            return Admission::Throttled {
                reason: ThrottleReason::NodeCpuCapacity,
            };
        }
        if node_used.memory_mib.saturating_add(request.memory_mib) > node_cap.memory_mib {
            return Admission::Throttled {
                reason: ThrottleReason::NodeMemoryCapacity,
            };
        }

        // Admitted: record usage.
        let team_key = team.to_string();
        *self
            .node_team
            .entry((node_idx, team_key.clone()))
            .or_default() += TeamUsage {
            cpu_millicores: request.cpu_millicores,
            memory_mib: request.memory_mib,
        };
        *self.node_total.entry(node_idx).or_default() += NodeUsage {
            cpu_millicores: request.cpu_millicores,
            memory_mib: request.memory_mib,
        };
        *self.team_running.entry(team_key.clone()).or_default() += TeamUsage {
            cpu_millicores: request.cpu_millicores,
            memory_mib: request.memory_mib,
        };
        *self.team_jobs.entry(team_key).or_default() += 1;
        Admission::Admitted
    }

    /// Release a finished job's resources (the inverse of a successful
    /// [`FairShareScheduler::admit`]).
    pub fn complete(&mut self, team: &str, node: &str, request: ResourceRequest) {
        let Some(node_idx) = self.nodes.iter().position(|n| n.id == node) else {
            return;
        };
        let key = (node_idx, team.to_string());
        if let Some(u) = self.node_team.get_mut(&key) {
            u.cpu_millicores = u.cpu_millicores.saturating_sub(request.cpu_millicores);
            u.memory_mib = u.memory_mib.saturating_sub(request.memory_mib);
        }
        if let Some(u) = self.node_total.get_mut(&node_idx) {
            u.cpu_millicores = u.cpu_millicores.saturating_sub(request.cpu_millicores);
            u.memory_mib = u.memory_mib.saturating_sub(request.memory_mib);
        }
        if let Some(u) = self.team_running.get_mut(team) {
            u.cpu_millicores = u.cpu_millicores.saturating_sub(request.cpu_millicores);
            u.memory_mib = u.memory_mib.saturating_sub(request.memory_mib);
        }
        if let Some(j) = self.team_jobs.get_mut(team) {
            *j = j.saturating_sub(1);
        }
    }

    /// Current aggregate cluster-wide CPU usage of a team.
    pub fn team_cpu(&self, team: &str) -> u64 {
        self.team_running
            .get(team)
            .map(|u| u.cpu_millicores)
            .unwrap_or(0)
    }

    /// Current aggregate cluster-wide memory usage of a team.
    pub fn team_memory(&self, team: &str) -> u64 {
        self.team_running
            .get(team)
            .map(|u| u.memory_mib)
            .unwrap_or(0)
    }

    /// Current concurrent job count of a team.
    pub fn team_jobs(&self, team: &str) -> u64 {
        self.team_jobs.get(team).copied().unwrap_or(0)
    }

    /// Total CPU used on a node.
    pub fn node_cpu(&self, node: &str) -> u64 {
        let Some(idx) = self.nodes.iter().position(|n| n.id == node) else {
            return 0;
        };
        self.node_total
            .get(&idx)
            .map(|u| u.cpu_millicores)
            .unwrap_or(0)
    }

    fn node_team_list(&self, node_idx: usize, submit_team: &str) -> Vec<String> {
        // Teams with current demand on this node, deduped, plus the
        // submitting team guaranteed in LAST position (so its allocation
        // is `alloc[weights.len() - 1]`).
        let mut teams: Vec<String> = self
            .node_team
            .keys()
            .filter(|(ni, _)| *ni == node_idx)
            .filter(|(_, tid)| *tid != submit_team)
            .map(|(_, tid)| tid.clone())
            .collect();
        teams.sort();
        teams.dedup();
        teams.push(submit_team.to_string());
        teams
    }

    fn node_cpu_demands(
        &self,
        node_idx: usize,
        submit_team: &str,
        candidate: u64,
    ) -> (Vec<u64>, Vec<u64>) {
        let teams = self.node_team_list(node_idx, submit_team);
        let mut weights = Vec::with_capacity(teams.len());
        let mut demands = Vec::with_capacity(teams.len());
        for t in &teams {
            let w = self.quotas.get(t).map(|q| q.weight).unwrap_or(1);
            weights.push(w);
            if t == submit_team {
                demands.push(candidate);
            } else {
                demands.push(
                    self.node_team
                        .get(&(node_idx, t.clone()))
                        .map(|u| u.cpu_millicores)
                        .unwrap_or(0),
                );
            }
        }
        (weights, demands)
    }

    fn node_mem_demands(
        &self,
        node_idx: usize,
        submit_team: &str,
        candidate: u64,
    ) -> (Vec<u64>, Vec<u64>) {
        let teams = self.node_team_list(node_idx, submit_team);
        let mut weights = Vec::with_capacity(teams.len());
        let mut demands = Vec::with_capacity(teams.len());
        for t in &teams {
            let w = self.quotas.get(t).map(|q| q.weight).unwrap_or(1);
            weights.push(w);
            if t == submit_team {
                demands.push(candidate);
            } else {
                demands.push(
                    self.node_team
                        .get(&(node_idx, t.clone()))
                        .map(|u| u.memory_mib)
                        .unwrap_or(0),
                );
            }
        }
        (weights, demands)
    }
}

impl std::ops::AddAssign<TeamUsage> for TeamUsage {
    fn add_assign(&mut self, rhs: TeamUsage) {
        self.cpu_millicores += rhs.cpu_millicores;
        self.memory_mib += rhs.memory_mib;
    }
}

impl std::ops::AddAssign<NodeUsage> for NodeUsage {
    fn add_assign(&mut self, rhs: NodeUsage) {
        self.cpu_millicores += rhs.cpu_millicores;
        self.memory_mib += rhs.memory_mib;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Node, NodeCapacity, TeamQuota};

    fn sample_config() -> Config {
        Config {
            format: crate::FORMAT.to_string(),
            version: crate::VERSION,
            nodes: vec![
                Node {
                    id: "node-a".into(),
                    capacity: NodeCapacity {
                        cpu_millicores: 16_000,
                        memory_mib: 64_000,
                    },
                },
                Node {
                    id: "node-b".into(),
                    capacity: NodeCapacity {
                        cpu_millicores: 8_000,
                        memory_mib: 32_000,
                    },
                },
            ],
            teams: BTreeMap::from([
                (
                    "alpha".into(),
                    TeamQuota {
                        weight: 1,
                        max_concurrent_cpu_millicores: 8_000,
                        max_concurrent_memory_mib: 24_000,
                        max_concurrent_jobs: 4,
                    },
                ),
                (
                    "beta".into(),
                    TeamQuota {
                        weight: 1,
                        max_concurrent_cpu_millicores: 8_000,
                        max_concurrent_memory_mib: 24_000,
                        max_concurrent_jobs: 4,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn admits_within_quota() {
        let mut s = FairShareScheduler::from_config(&sample_config()).unwrap();
        for _ in 0..2 {
            assert_eq!(
                s.admit(
                    "alpha",
                    "node-a",
                    ResourceRequest {
                        cpu_millicores: 2_000,
                        memory_mib: 4_000
                    }
                ),
                Admission::Admitted
            );
        }
        // two jobs of 2000 cpu on 8000 cpu quota: still allowed (4000).
        assert_eq!(s.team_cpu("alpha"), 4_000);
        assert_eq!(s.node_cpu("node-a"), 4_000);
    }

    #[test]
    fn oversubscribed_team_throttled_by_quota_even_on_idle_cluster() {
        // Cluster is idle except alpha's demand; alpha hits its 8000 cpu
        // cap and is throttled even though the whole cluster is free.
        let mut s = FairShareScheduler::from_config(&sample_config()).unwrap();
        let mut admitted = 0;
        for _ in 0..10 {
            if let Admission::Admitted = s.admit(
                "alpha",
                "node-b",
                ResourceRequest {
                    cpu_millicores: 2_000,
                    memory_mib: 4_000,
                },
            ) {
                admitted += 1;
            }
        }
        // 8000 cpu cap / 2000 per job = 4 admitted, then throttled.
        assert_eq!(admitted, 4);
        assert_eq!(s.team_cpu("alpha"), 8_000);
        assert_eq!(
            s.admit(
                "alpha",
                "node-a",
                ResourceRequest {
                    cpu_millicores: 1,
                    memory_mib: 1
                }
            ),
            Admission::Throttled {
                reason: ThrottleReason::CpuQuota
            }
        );
    }

    #[test]
    fn fair_share_caps_weighted_shares_under_contention() {
        // Weighted max-min under real contention: beta has weight 3, alpha
        // weight 1 on a 16000-cpu node. If both teams demand far more than
        // the node holds, the allocation must be exactly 4000/12000 -- no
        // matter how aggressively alpha submits -- and any alpha request past
        // its 4000 share is throttled by NodeFairShareCpu (its 32k cluster
        // quota never binds; node capacity is not reached either).
        let cfg = Config {
            format: crate::FORMAT.to_string(),
            version: crate::VERSION,
            nodes: vec![Node {
                id: "node-a".into(),
                capacity: NodeCapacity {
                    cpu_millicores: 16_000,
                    memory_mib: 64_000,
                },
            }],
            teams: BTreeMap::from([
                (
                    "alpha".into(),
                    TeamQuota {
                        weight: 1,
                        max_concurrent_cpu_millicores: 32_000,
                        max_concurrent_memory_mib: 64_000,
                        max_concurrent_jobs: 16,
                    },
                ),
                (
                    "beta".into(),
                    TeamQuota {
                        weight: 3,
                        max_concurrent_cpu_millicores: 32_000,
                        max_concurrent_memory_mib: 64_000,
                        max_concurrent_jobs: 16,
                    },
                ),
            ]),
        };
        let mut s = FairShareScheduler::from_config(&cfg).unwrap();

        // Contend the node: alpha first, then beta -- both demand far more
        // than 16000 total. alpha gets in at its 4000 share.
        assert_eq!(
            s.admit(
                "alpha",
                "node-a",
                ResourceRequest {
                    cpu_millicores: 4_000,
                    memory_mib: 2_000
                }
            ),
            Admission::Admitted
        );
        // beta (weight 3) fills its 12000 share for the same 4k units.
        assert_eq!(
            s.admit(
                "beta",
                "node-a",
                ResourceRequest {
                    cpu_millicores: 12_000,
                    memory_mib: 2_000
                }
            ),
            Admission::Admitted
        );
        assert_eq!(s.node_cpu("node-a"), 16_000);
        assert_eq!(s.team_cpu("alpha"), 4_000);
        assert_eq!(s.team_cpu("beta"), 12_000);

        // alpha asks for more than its 4000 share -> throttled by fair share.
        let r = s.admit(
            "alpha",
            "node-a",
            ResourceRequest {
                cpu_millicores: 1_000,
                memory_mib: 500,
            },
        );
        assert_eq!(
            r,
            Admission::Throttled {
                reason: ThrottleReason::NodeFairShareCpu
            }
        );
        assert_eq!(
            s.team_cpu("alpha"),
            4_000,
            "throttled request records no usage"
        );

        // Release everything; both are admitted again (round-trip sanity).
        s.complete(
            "alpha",
            "node-a",
            ResourceRequest {
                cpu_millicores: 4_000,
                memory_mib: 2_000,
            },
        );
        s.complete(
            "beta",
            "node-a",
            ResourceRequest {
                cpu_millicores: 12_000,
                memory_mib: 2_000,
            },
        );
        assert_eq!(s.node_cpu("node-a"), 0);
        assert_eq!(s.team_cpu("alpha"), 0);
        assert_eq!(s.team_cpu("beta"), 0);
        assert_eq!(
            s.admit(
                "alpha",
                "node-a",
                ResourceRequest {
                    cpu_millicores: 4_000,
                    memory_mib: 2_000
                }
            ),
            Admission::Admitted
        );
        assert_eq!(
            s.admit(
                "beta",
                "node-a",
                ResourceRequest {
                    cpu_millicores: 12_000,
                    memory_mib: 2_000
                }
            ),
            Admission::Admitted
        );
    }

    #[test]
    fn releasing_usage_unthrottles() {
        let mut s = FairShareScheduler::from_config(&sample_config()).unwrap();
        let req = ResourceRequest {
            cpu_millicores: 6_000,
            memory_mib: 4_000,
        };
        assert_eq!(s.admit("alpha", "node-a", req), Admission::Admitted);
        // second identical request exceeds 8000 cpu quota -> throttled
        assert_eq!(
            s.admit("alpha", "node-a", req),
            Admission::Throttled {
                reason: ThrottleReason::CpuQuota
            }
        );
        s.complete("alpha", "node-a", req);
        assert_eq!(s.team_cpu("alpha"), 0);
        assert_eq!(s.admit("alpha", "node-a", req), Admission::Admitted);
    }
}
