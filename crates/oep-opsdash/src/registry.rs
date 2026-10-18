//! Real node registry: node health derived ONLY from scheduler lifecycle
//! events, never from fabricated status.
//!
//! A node comes online when it first claims a dispatch (the scheduler handed
//! it a job), is marked `Degraded` when one of the jobs it holds is reclaimed
//! by the scheduler (`reclaim_expired` -- the lease expired without an ack,
//! i.e. the worker split), and is marked `Down` when its heartbeat goes stale
//! past a threshold. All transitions are driven by real events recorded by
//! the dashboard, so the dashboard's health column is a projection of the
//! real scheduler's lease state.

use std::collections::BTreeMap;

use oep_lane::sched::{JobId, Priority};

/// Node health states shown by the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// No job has ever been claimed by this node (registered but idle).
    Unknown,
    /// Online and had a fresh heartbeat within the stale threshold.
    Online,
    /// A held job was reclaimed by the scheduler (lease expired, no ack).
    Degraded,
    /// Heartbeat went stale past the threshold (no lifecycle event for a while).
    Down,
}

/// Static node identity: what the operator configured about the machine.
///
/// The capacity vocabulary (cpu_millicores, memory_mib) intentionally matches
/// P9-M2-T02 (oep-fairshare) so the wave merges cleanly; this card owns the
/// *health projection* (a live view), not the quota policy.
#[derive(Debug, Clone)]
pub struct NodeSpec {
    /// Human node id (matches `oep.nodes.{node-id}` in the taxonomy).
    pub id: String,
    /// Logical CPU capacity in millicores.
    pub cpu_millicores: u64,
    /// Memory capacity in MiB.
    pub memory_mib: u64,
    /// Heartbeat stale threshold, ms. After this long with no heartbeat the
    /// node is marked `Down` on a tick.
    pub heartbeat_stale_ms: u64,
    /// Declared electrical draw at load, watts (used by the energy model).
    pub power_w: u64,
    /// Declared energy rate, US-cents per kWh (used by the cost view).
    pub cents_per_kwh: u64,
}

/// Live per-node state maintained by the dashboard.
#[derive(Debug, Clone)]
pub struct NodeState {
    /// The static spec.
    pub spec: NodeSpec,
    /// Current health.
    pub health: Health,
    /// Last heartbeat time (ms) by the dashboard clock.
    pub last_heartbeat_ms: u64,
    /// Clock time (ms) the node first claimed a dispatch.
    pub first_seen_ms: u64,
    /// Jobs currently running on this node (dispatched, not yet acked) with
    /// their scheduling priority.
    pub running: Vec<(JobId, Priority)>,
    /// Jobs this node has acked (succeeded) since registration.
    pub jobs_succeeded: u64,
    /// Jobs this node lost (reclaimed) since registration.
    pub jobs_reclaimed: u64,
}

impl NodeState {
    /// Register a fresh node with a heartbeat at `now_ms`.
    pub fn new(spec: NodeSpec, now_ms: u64) -> Self {
        NodeState {
            spec,
            health: Health::Online,
            last_heartbeat_ms: now_ms,
            first_seen_ms: now_ms,
            running: Vec::new(),
            jobs_succeeded: 0,
            jobs_reclaimed: 0,
        }
    }

    /// Apply clock-advance: mark `Down` if the heartbeat has gone stale.
    pub fn tick(&mut self, now_ms: u64) {
        if self.health != Health::Down
            && now_ms.saturating_sub(self.last_heartbeat_ms) > self.spec.heartbeat_stale_ms
        {
            self.health = Health::Down;
        }
    }

    /// Recompute the running-set cache after a mutation.
    fn refresh(&mut self) {
        // (running is stored verbatim; recompute is a no-op kept for clarity)
    }
}

/// The node registry. All updates come from dashboard lifecycle events.
#[derive(Debug, Default, Clone)]
pub struct NodeRegistry {
    /// Node id -> live state.
    nodes: BTreeMap<String, NodeState>,
}

impl NodeRegistry {
    /// A registry with no nodes.
    pub fn new() -> Self {
        NodeRegistry::default()
    }

    /// Register (or refresh) a node with a heartbeat at `now_ms`.
    pub fn register(&mut self, spec: NodeSpec, now_ms: u64) {
        self.nodes
            .insert(spec.id.clone(), NodeState::new(spec, now_ms));
    }

    /// Record that a node claimed a dispatch at `now_ms` (real scheduler
    /// handoff). Marks the node online and adds the job to its running set.
    pub fn claim(&mut self, node_id: &str, job: JobId, priority: Priority, now_ms: u64) {
        if let Some(n) = self.nodes.get_mut(node_id) {
            n.health = Health::Online;
            n.last_heartbeat_ms = now_ms;
            n.running.push((job, priority));
            n.refresh();
        }
    }

    /// Record that a node acked a job at `now_ms` (real scheduler ack). The
    /// job leaves the running set.
    pub fn ack(&mut self, node_id: &str, job: JobId, now_ms: u64) {
        if let Some(n) = self.nodes.get_mut(node_id) {
            n.health = Health::Online;
            n.last_heartbeat_ms = now_ms;
            n.running.retain(|&(j, _)| j != job);
            n.jobs_succeeded += 1;
            n.refresh();
        }
    }

    /// Remove a job from a node's running set without changing health (used
    /// when a transient failure ends the run; the node is still online).
    pub fn remove_running(&mut self, node_id: &str, job: JobId) {
        if let Some(n) = self.nodes.get_mut(node_id) {
            n.running.retain(|&(j, _)| j != job);
            n.refresh();
        }
    }

    /// Record that a job held by this node was reclaimed by the scheduler
    /// (lease expired, no ack). Marks the node `Degraded` and removes the job.
    pub fn reclaim(&mut self, node_id: &str, job: JobId, now_ms: u64) {
        if let Some(n) = self.nodes.get_mut(node_id) {
            n.health = Health::Degraded;
            n.last_heartbeat_ms = now_ms;
            n.running.retain(|&(j, _)| j != job);
            n.jobs_reclaimed += 1;
            n.refresh();
        }
    }

    /// Advance every node's staleness clock; marks stale nodes `Down`.
    pub fn tick_all(&mut self, now_ms: u64) {
        for n in self.nodes.values_mut() {
            n.tick(now_ms);
        }
    }

    /// Live node states in id order.
    pub fn nodes(&self) -> impl Iterator<Item = &NodeState> {
        self.nodes.values()
    }

    /// A specific node's state.
    pub fn get(&self, node_id: &str) -> Option<&NodeState> {
        self.nodes.get(node_id)
    }

    /// Number of registered nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the registry has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
