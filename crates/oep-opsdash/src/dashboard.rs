//! The dashboard aggregate: owns the REAL `oep-lane` [`Scheduler`], the
//! [`NodeRegistry`], the [`JobLog`] and the [`EnergyLedger`], and presents
//! all of them through one ops API plus a [`snapshot`](Dashboard::snapshot).
//!
//! Data-reality contract: every queue counter, job status and dead-letter row
//! in the snapshot is read from the scheduler itself (`.status`, `.ready_count`,
//! `.dead_letters`, ...). Node health and job logs are derived from the real
//! lifecycle events this dashboard performs (claim/ack/fail/poison/reclaim),
//! and cost/energy is accumulated from real measured runtimes. Nothing is
//! fabricated.
//!
//! Threading: a `Dashboard` is `Send`, and the live server shares one behind
//! a `Mutex`; the clock is `Arc`-shared so the driver and the snapshot agree
//! on "now".

use std::collections::BTreeMap;
use std::sync::Arc;

use oep_lane::sched::scheduler::{EnqueueError, FailError};
use oep_lane::sched::{Dispatch, JobId, JobSpec, JobStatus, Scheduler};

use crate::clock::Clock;
use crate::cost::{EnergyLedger, JobRun};
use crate::log::{JobLog, LogEntry, LogKind};
use crate::model::{
    CostTotals, CostView, DashboardSnapshot, DeadLetterView, JobView, LogView, NodeView,
    QueueCounts,
};
use crate::registry::{NodeRegistry, NodeSpec};

/// One in-flight dispatch the dashboard has handed to a node, until it
/// resolves (ack / fail / poison / reclaim). It is how the dashboard knows
/// which node owns a running job, how long the run took, and what generation
/// to resolve with -- all real bookkeeping from the actual handoff.
#[derive(Debug, Clone)]
struct HeldDispatch {
    node_id: String,
    generation: u64,
    claimed_at_ms: u64,
    node_power_w: u64,
    cents_per_kwh: u64,
}

/// Per-job metadata the dashboard tracks alongside the scheduler (the
/// scheduler owns lifecycle; the dashboard records what it enqueued).
#[derive(Debug, Clone)]
struct JobMeta {
    priority: u8,
    /// Failed attempts observed (bumped on each real `fail` decision).
    failures: u32,
}

/// The ops dashboard.
pub struct Dashboard {
    /// The real scheduler (P9-M2-T01). All state queries go through it.
    scheduler: Scheduler,
    /// Node health registry.
    nodes: NodeRegistry,
    /// Job lifecycle log.
    log: JobLog,
    /// Cost/energy ledger.
    energy: EnergyLedger,
    /// Shared clock the dashboard records against.
    clock: Arc<dyn Clock>,
    /// Job metadata the dashboard enqueued.
    jobs: BTreeMap<JobId, JobMeta>,
    /// In-flight dispatches (id -> held dispatch) until resolved.
    held: BTreeMap<JobId, HeldDispatch>,
    /// Scheduler lease duration (ms), for the UI lease column.
    lease_ms: u64,
}

impl std::fmt::Debug for Dashboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dashboard")
            .field("scheduler", &self.scheduler)
            .field("nodes", &self.nodes)
            .field("log_len", &self.log.len())
            .field("energy", &self.energy)
            .field("jobs", &self.jobs)
            .field("held", &self.held)
            .field("lease_ms", &self.lease_ms)
            .finish_non_exhaustive()
    }
}

impl Dashboard {
    /// A dashboard over a fresh scheduler with the default lease and the given
    /// node specs, sharing `clock`.
    pub fn new(clock: Arc<dyn Clock>, nodes: Vec<NodeSpec>) -> Self {
        Self::with_lease(clock, nodes, 60_000)
    }

    /// A dashboard with a custom scheduler lease duration (ms).
    pub fn with_lease(clock: Arc<dyn Clock>, nodes: Vec<NodeSpec>, lease_ms: u64) -> Self {
        let now = clock.now_ms();
        let mut registry = NodeRegistry::new();
        for spec in nodes {
            registry.register(spec, now);
        }
        Dashboard {
            scheduler: Scheduler::with_lease(lease_ms),
            nodes: registry,
            log: JobLog::with_capacity(4096),
            energy: EnergyLedger::new(),
            clock,
            jobs: BTreeMap::new(),
            held: BTreeMap::new(),
            lease_ms,
        }
    }

    /// The shared clock.
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Register an additional node (idempotent refresh).
    pub fn register_node(&mut self, spec: NodeSpec) {
        let now = self.clock.now_ms();
        self.nodes.register(spec, now);
    }

    /// Enqueue a job onto the real scheduler and log the event.
    pub fn enqueue(&mut self, spec: JobSpec) -> Result<JobId, EnqueueError> {
        let now = self.clock.now_ms();
        let priority = spec.priority.raw();
        let id = self.scheduler.enqueue(spec)?;
        self.jobs.insert(
            id,
            JobMeta {
                priority,
                failures: 0,
            },
        );
        self.log.push(LogEntry::new(
            now,
            LogKind::Enqueue,
            Some(id),
            None,
            format!("priority={priority}"),
        ));
        Ok(id)
    }

    /// Dispatch the next runnable job to `node_id`. Returns the real dispatch
    /// (or `None` if the queue is drained), recording the claim, health and
    /// energy-start for the run.
    pub fn dispatch_to(&mut self, node_id: &str) -> Option<Dispatch> {
        let now = self.clock.now_ms();
        let d = self.scheduler.dispatch(now)?;
        let meta = self.jobs.get(&d.job).cloned();
        let node_cfg = self
            .nodes
            .get(node_id)
            .map(|n| (n.spec.power_w, n.spec.cents_per_kwh));
        let (node_power_w, cents_per_kwh) = node_cfg.unwrap_or((0, 0));
        let priority = meta
            .map(|m| oep_lane::sched::Priority::new(m.priority))
            .unwrap_or_default();
        self.nodes.claim(node_id, d.job, priority, now);
        self.held.insert(
            d.job,
            HeldDispatch {
                node_id: node_id.to_string(),
                generation: d.generation,
                claimed_at_ms: now,
                node_power_w,
                cents_per_kwh,
            },
        );
        self.log.push(LogEntry::new(
            now,
            LogKind::Dispatch,
            Some(d.job),
            Some(node_id.to_string()),
            format!("gen={}", d.generation),
        ));
        Some(d)
    }

    /// Ack a held dispatch as successful (real scheduler `ack`), recording
    /// the run's energy and the node's success.
    pub fn ack(&mut self, job: JobId, generation: u64) -> Result<(), oep_lane::sched::AckError> {
        let now = self.clock.now_ms();
        self.scheduler.ack(job, generation)?;
        if let Some(held) = self.held.remove(&job) {
            // Only the current-generation holder owns the run.
            if held.generation == generation {
                self.nodes.ack(&held.node_id, job, now);
                self.record_run(&held);
            }
        }
        self.log.push(LogEntry::new(
            now,
            LogKind::Ack,
            Some(job),
            None,
            format!("gen={generation}"),
        ));
        Ok(())
    }

    /// Fail a held dispatch (real scheduler `fail`), recording the run as a
    /// consumed attempt (energy for the partial run) and the failure log.
    pub fn fail(&mut self, job: JobId, generation: u64, detail: &str) -> Result<(), FailError> {
        let now = self.clock.now_ms();
        self.scheduler.fail(job, generation, now)?;
        if let Some(meta) = self.jobs.get_mut(&job) {
            meta.failures += 1;
        }
        if let Some(held) = self.held.remove(&job) {
            if held.generation == generation {
                self.nodes.remove_running(&held.node_id, job);
                self.record_run(&held);
            }
        }
        self.log.push(LogEntry::new(
            now,
            LogKind::Fail,
            Some(job),
            None,
            detail.to_string(),
        ));
        Ok(())
    }

    /// Report a held dispatch as poison (real scheduler `poison_report`),
    /// recording the run and a poison log line.
    pub fn poison(&mut self, job: JobId, generation: u64, detail: &str) -> Result<(), FailError> {
        let now = self.clock.now_ms();
        self.scheduler.poison_report(job, generation, detail, now)?;
        if let Some(held) = self.held.remove(&job) {
            if held.generation == generation {
                self.nodes.remove_running(&held.node_id, job);
                self.record_run(&held);
            }
        }
        self.log.push(LogEntry::new(
            now,
            LogKind::Poison,
            Some(job),
            None,
            detail.to_string(),
        ));
        Ok(())
    }

    /// Reap expired leases via the real scheduler; any reclaimed job's node is
    /// marked degraded, its partial run is charged, and a reclaim log line is
    /// written. Returns the reclaimed job ids.
    pub fn reap(&mut self) -> Vec<JobId> {
        let now = self.clock.now_ms();
        let reclaimed = self.scheduler.reclaim_expired(now);
        for job in &reclaimed {
            if let Some(held) = self.held.remove(job) {
                self.nodes.reclaim(&held.node_id, *job, now);
                self.record_run(&held);
            }
            self.log.push(LogEntry::new(
                now,
                LogKind::Reclaim,
                Some(*job),
                None,
                "lease expired".to_string(),
            ));
        }
        reclaimed
    }

    /// Tick staleness on all nodes (marks stale heartbeats `Down`).
    pub fn tick_nodes(&mut self) {
        let now = self.clock.now_ms();
        self.nodes.tick_all(now);
    }

    /// The real scheduler (read access for tests / advanced callers).
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Whether a dispatch is currently held (in flight) for `job`.
    pub fn is_held(&self, job: JobId) -> bool {
        self.held.contains_key(&job)
    }

    /// Record a finished (or reclaimed) run's energy against its node.
    fn record_run(&mut self, held: &HeldDispatch) {
        let runtime_ms = self.clock.now_ms().saturating_sub(held.claimed_at_ms);
        self.energy.record(
            &held.node_id,
            JobRun {
                node_power_w: held.node_power_w,
                cents_per_kwh: held.cents_per_kwh,
                runtime_ms,
            },
        );
    }

    /// A live, fully-derived snapshot of the scheduler, nodes, jobs, log and
    /// cost/energy. Every number is read from real state at this instant.
    pub fn snapshot(&self) -> DashboardSnapshot {
        let now = self.clock.now_ms();

        // Queue counters + per-job statuses straight from the scheduler.
        let mut retrying = 0u64;
        let mut succeeded = 0u64;
        let mut jobs = Vec::with_capacity(self.jobs.len());
        for (&id, meta) in &self.jobs {
            let status = self.scheduler.status(id);
            let generation = self.scheduler.dispatch_generation_of(id).unwrap_or(0);
            jobs.push(JobView {
                job: id.raw(),
                status: status_string(status),
                priority: meta.priority,
                failures: meta.failures,
                generation,
            });
            if let Some(s) = status {
                match s {
                    JobStatus::Retrying => retrying += 1,
                    JobStatus::Succeeded => succeeded += 1,
                    _ => {}
                }
            }
        }
        let queue = QueueCounts {
            waiting: self.scheduler.pending_count() as u64,
            ready: self.scheduler.ready_count() as u64,
            running: self.scheduler.running_count() as u64,
            retrying,
            succeeded,
            dead_lettered: self.scheduler.dead_letters().len() as u64,
        };

        let dead_letters: Vec<DeadLetterView> = self
            .scheduler
            .dead_letters()
            .iter()
            .map(DeadLetterView::from)
            .collect();

        let nodes: Vec<NodeView> = self
            .nodes
            .nodes()
            .map(|n| NodeView {
                id: n.spec.id.clone(),
                health: health_string(n.health),
                cpu_millicores: n.spec.cpu_millicores,
                memory_mib: n.spec.memory_mib,
                power_w: n.spec.power_w,
                running_jobs: n.running.iter().map(|&(j, _)| j.raw()).collect(),
                jobs_succeeded: n.jobs_succeeded,
                jobs_reclaimed: n.jobs_reclaimed,
                last_heartbeat_ms: n.last_heartbeat_ms,
            })
            .collect();

        let log: Vec<LogView> = self
            .log
            .entries()
            .map(|e| LogView {
                at_ms: e.at_ms,
                job: e.job.map(|j| j.raw()),
                node: e.node.clone(),
                kind: e.kind.as_str().to_string(),
                detail: e.detail.clone(),
            })
            .collect();

        let mut cost = BTreeMap::new();
        for (node_id, node_e) in &self.energy.nodes {
            cost.insert(
                node_id.clone(),
                CostView {
                    node: node_id.clone(),
                    energy_mwh: node_e.energy_mwh,
                    cost_usd_micro: node_e.cost_usd_micro,
                    runs: node_e.runs,
                },
            );
        }
        let cost_totals = CostTotals {
            energy_mwh: self.energy.total_energy_mwh(),
            cost_usd_micro: self.energy.total_cost_usd_micro(),
            runs: self.energy.total_runs(),
        };

        let mut snap = DashboardSnapshot::with_meta(now);
        snap.queue = queue;
        snap.dead_letters = dead_letters;
        snap.nodes = nodes;
        snap.jobs = jobs;
        snap.log = log;
        snap.cost = cost;
        snap.cost_totals = cost_totals;
        snap
    }
}

/// Stable status string for the wire.
pub fn status_string(s: Option<JobStatus>) -> String {
    match s {
        None => "unknown".to_string(),
        Some(s) => match s {
            JobStatus::Waiting => "waiting".to_string(),
            JobStatus::Ready => "ready".to_string(),
            JobStatus::Running => "running".to_string(),
            JobStatus::Retrying => "retrying".to_string(),
            JobStatus::Succeeded => "succeeded".to_string(),
            JobStatus::DeadLettered => "dead-lettered".to_string(),
        },
    }
}

/// Stable node-health string for the wire.
pub fn health_string(h: crate::registry::Health) -> String {
    match h {
        crate::registry::Health::Unknown => "unknown".to_string(),
        crate::registry::Health::Online => "online".to_string(),
        crate::registry::Health::Degraded => "degraded".to_string(),
        crate::registry::Health::Down => "down".to_string(),
    }
}
