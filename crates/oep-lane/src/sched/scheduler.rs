//! The scheduler itself (P9-M2-T01): priority queue + DAG gating + retries +
//! dead-letter, with a lease/ack contract that carries at-least-once
//! semantics (the seam P9-M2-T03 builds node-loss survival on).
//!
//! Mental model:
//!
//! - Every job is enqueued with [`JobSpec`] → a unique [`JobId`] and an
//!   optional **idempotency key** ([`JobSpec::idempotency_key`]).
//! - Dependencies are wired at enqueue time and validated for cycles.
//! - A job becomes *ready* only when every dependency has succeeded and any
//!   retry backoff window has elapsed; ready jobs live in a binary heap
//!   ordered by (priority desc, enqueue-seq asc).
//! - [`dispatch`](Scheduler::dispatch) hands out the highest-priority ready
//!   job, marking it `Running` and starting a **lease**.
//! - The worker must `ack` (success) or `fail` (transient error → schedule a
//!   retry) or `poison_report` (terminal). If the lease expires without a
//!   decision, [`reclaim_expired`](Scheduler::reclaim_expired) moves the job
//!   back to ready and it is **redelivered with the same idempotency key**
//!   (at-least-once).
//! - Jobs that exhaust retries, are reported poison, or whose dependency
//!   dead-lettered land in the dead-letter log and are never dispatched
//!   again.

use std::collections::{BinaryHeap, HashMap, HashSet};

use super::{
    CycleError, DeadLetter, DeadLetterReason, DepGraph, Priority, RetryPolicy, UnknownDependency,
};
use crate::gpu::NodeResources;
use crate::placement::{GpuRequest, NodePool, Placement, PlacementPolicy};

/// Default nominal CPU node id used when a placement policy is asked where a
/// CPU job goes and no node has been registered in the pool yet. The real
/// fleet wiring (P9-M1-T02 node heartbeats) replaces this at runtime.
const DEFAULT_CPU_NODE: &str = "cpu-default";

/// A unique job identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(u64);

impl JobId {
    /// Build an id from a raw `u64`.
    pub const fn new(raw: u64) -> Self {
        JobId(raw)
    }

    /// The raw `u64` backing this id.
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "job {}", self.0)
    }
}

/// A job's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    /// Enqueued; one or more dependencies have not succeeded yet.
    Waiting,
    /// Dependencies satisfied; eligible for dispatch.
    Ready,
    /// Handed to a worker; waiting on ack/fail/lease expiry.
    Running,
    /// In the retry backoff window after a failure.
    Retrying,
    /// Acked as successful; terminal.
    Succeeded,
    /// Moved to the dead-letter log; terminal, never dispatched again.
    DeadLettered,
}

/// Everything the scheduler needs to know about a job up front.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSpec {
    /// Scheduling priority (higher dispatches first).
    pub priority: Priority,
    /// Dependency job ids that must succeed before this job may run.
    pub deps: Vec<JobId>,
    /// The retry policy (attempt budget + backoff). Defaults to
    /// [`RetryPolicy::default`] when the field is omitted.
    pub retry: RetryPolicy,
    /// **Idempotency key**: stable across redeliveries. A worker executing a
    /// job with this key can dedupe against the same key, which is how
    /// at-least-once delivery (P9-M2-T03) becomes exactly-once effect.
    pub idempotency_key: Option<String>,
    /// Optional GPU request (P9-M4-T01). `None` = CPU-only job. When
    /// `Some`, the placement policy sends the job to a GPU node holding a
    /// matching free device; `required: false` requests fall back to a CPU
    /// node cleanly when no GPU is free.
    pub gpu: Option<GpuRequest>,
    /// Opaque payload. P9-M1-T01 defines the real job payload (vault refs,
    /// executor, resource caps, output refs); the scheduler treats it as an
    /// opaque string so the two crates stay decoupled until the importer-hub
    /// milestone (P6-M4-T02) opens cross-crate deps.
    pub payload: String,
}

impl JobSpec {
    /// A minimal spec with all defaults and no payload.
    pub fn new(payload: impl Into<String>) -> Self {
        JobSpec {
            priority: Priority::default(),
            deps: Vec::new(),
            retry: RetryPolicy::default(),
            idempotency_key: None,
            gpu: None,
            payload: payload.into(),
        }
    }

    /// Chainable priority setter.
    pub fn with_priority(mut self, p: Priority) -> Self {
        self.priority = p;
        self
    }

    /// Chainable dependency setter.
    pub fn with_deps(mut self, deps: Vec<JobId>) -> Self {
        self.deps = deps;
        self
    }

    /// Chainable retry policy setter.
    pub fn with_retry(mut self, r: RetryPolicy) -> Self {
        self.retry = r;
        self
    }

    /// Chainable idempotency key setter.
    pub fn with_idempotency(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Chainable GPU-request setter (P9-M4-T01). Set the layout this job needs
    /// before enqueue; the placement policy maps it onto a real node pool.
    pub fn with_gpu(mut self, gpu: GpuRequest) -> Self {
        self.gpu = Some(gpu);
        self
    }
}

/// Internal full job record.
#[derive(Debug)]
struct Job {
    spec: JobSpec,
    status: JobStatus,
    /// Enqueue sequence; the FIFO tiebreaker within a priority.
    seq: u64,
    /// Failed attempts consumed so far (0 until the first failure).
    failures: u32,
    /// Wall-clock ms when the current retry window ends.
    ready_at_ms: u64,
    /// Lease deadline for a `Running` job; None when not running.
    lease_deadline_ms: Option<u64>,
    /// Dispatch generation: bumped on every handoff, so a stale ack from a
    /// previous lease can be detected (see [`AckError::StaleLease`]).
    generation: u64,
}

/// A job handed to a worker by [`Scheduler::dispatch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispatch {
    /// The job id.
    pub job: JobId,
    /// The job's opaque payload.
    pub payload: String,
    /// The job's idempotency key (None if the job set none).
    pub idempotency_key: Option<String>,
    /// Dispatch generation: each handoff (including redelivery after a lease
    /// expiry) carries a fresh generation.
    pub generation: u64,
    /// Placement decision (P9-M4-T01): which node the job must run on, and
    /// which GPU devices (if any) are reserved for it. For a scheduler built
    /// without a node pool this is [`Placement::unplaced`] (the P9-M2-T01
    /// baseline: no node assignment).
    pub placement: Placement,
}

impl Dispatch {
    /// Whether the job carries an idempotency key.
    pub fn is_idempotent(&self) -> bool {
        self.idempotency_key.is_some()
    }
}

/// Errors from [`Scheduler::enqueue`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnqueueError {
    /// A dependency referenced a job id that is not known to the scheduler.
    UnknownDependency(UnknownDependency),
    /// The dependency set would create a cycle.
    Cycle(CycleError),
    /// `ops` was passed >0 but the scheduler does not auto-increment
    /// (defensive; ids are caller-supplied and must be unique).
    DuplicateId(JobId),
}

/// Errors from [`Scheduler::ack`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AckError {
    /// No such job.
    NotFound(JobId),
    /// The job is not currently running.
    NotRunning(JobId, JobStatus),
    /// The ack references a stale lease generation (a previous dispatch of
    /// the same job already resolved). At-least-once: the current lease still
    /// owns the job.
    StaleLease {
        /// The job being acked.
        job: JobId,
        /// The generation the scheduler currently expects.
        expected: u64,
        /// The generation the caller supplied.
        got: u64,
    },
}

/// Errors from [`Scheduler::fail`] / [`Scheduler::poison_report`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailError {
    /// No such job.
    NotFound(JobId),
    /// The job is not currently running.
    NotRunning(JobId, JobStatus),
}

/// Heap entry: ready jobs ordered by (priority desc, seq asc).
#[derive(Debug)]
struct HeapEntry {
    priority: Priority,
    seq: u64,
    id: JobId,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // BinaryHeap is a max-heap. We want the *highest* priority "greatest"
        // (popped first) and, within a priority, the *lowest* seq "greatest"
        // (earliest enqueued first).
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for HeapEntry {}

/// Delayed entry: a retrying job waiting until `ready_at_ms` to re-enter the
/// ready heap. Ordered by earliest ready time, then seq.
#[derive(Debug)]
struct DelayedEntry {
    ready_at_ms: u64,
    seq: u64,
    id: JobId,
}

impl Ord for DelayedEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .ready_at_ms
            .cmp(&self.ready_at_ms)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for DelayedEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DelayedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for DelayedEntry {}

/// The scheduler.
#[derive(Debug)]
pub struct Scheduler {
    jobs: HashMap<JobId, Job>,
    graph: DepGraph,
    succeeded: HashSet<JobId>,
    ready: BinaryHeap<HeapEntry>,
    delayed: BinaryHeap<DelayedEntry>,
    dead_letters: Vec<DeadLetter>,
    /// Lease duration for a dispatched job, in ms.
    lease_ms: u64,
    next_seq: u64,
    next_id: u64,
    /// Node pool for GPU-aware placement (P9-M4-T01). `None` = the P9-M2-T01
    /// baseline (no placement decisions; every dispatch returns
    /// [`Placement::unplaced`]).
    pool: Option<NodePool>,
    /// Placement policy used when `pool` is attached.
    policy: PlacementPolicy,
    /// Jobs that are ready (deps satisfied, no retry backoff) but *parked*
    /// because they require a GPU device that no node currently has free.
    /// They are kept out of the ready heap so they cannot head-of-line block
    /// other work, and are re-promoted when a device is released or a node
    /// registers.
    parked: Vec<JobId>,
    /// GPU devices currently reserved per dispatched job (node + indices),
    /// so a release on ack/fail/poison/reclaim hands them back to the pool.
    reservations: HashMap<JobId, Placement>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// A scheduler with default a 60-second dispatch lease.
    pub fn new() -> Self {
        Scheduler::with_lease(60_000)
    }

    /// A scheduler with a custom dispatch-lease duration (ms).
    pub fn with_lease(lease_ms: u64) -> Self {
        Scheduler {
            jobs: HashMap::new(),
            graph: DepGraph::new(),
            succeeded: HashSet::new(),
            ready: BinaryHeap::new(),
            delayed: BinaryHeap::new(),
            dead_letters: Vec::new(),
            lease_ms,
            next_seq: 0,
            next_id: 0,
            pool: None,
            policy: PlacementPolicy::default(),
            parked: Vec::new(),
            reservations: HashMap::new(),
        }
    }

    /// A scheduler with a node pool attached (P9-M4-T01), so dispatch makes
    /// GPU-aware placement decisions.
    ///
    /// When a pool is attached, a job carrying a GPU request dispatches only
    /// onto a node that has a matching free device; a required request parks
    /// when no device is free (without blocking other ready work) and a
    /// preferred request falls back to a CPU node.
    pub fn with_pool(pool: NodePool) -> Self {
        Scheduler {
            pool: Some(pool),
            ..Scheduler::new()
        }
    }

    /// Register discovered nodes into the attached node pool (replacing any
    /// earlier registration of the same node ids), then re-promote any parked
    /// GPU jobs that now have capacity. No-op when no pool is attached.
    pub fn add_nodes(&mut self, nodes: Vec<NodeResources>) {
        let Some(pool) = self.pool.as_mut() else {
            return;
        };
        for n in nodes {
            pool.add(n);
        }
        self.promote_parked();
    }

    /// Number of jobs currently parked on missing GPU capacity (P9-M4-T01).
    /// These are `Ready` in status but intentionally kept out of the ready
    /// heap so they cannot head-of-line block other work.
    pub fn parked_count(&self) -> usize {
        self.parked.len()
    }

    /// Attach a node pool to a scheduler created without one (use before
    /// enqueueing GPU-tagged work).
    pub fn attach_pool(&mut self, pool: NodePool) {
        self.pool = Some(pool);
    }

    /// Enqueue a job with an auto-assigned id (monotonically increasing from
    /// 1). Returns the assigned [`JobId`].
    ///
    /// Validates:
    /// - every dependency is a known job ([`EnqueueError::UnknownDependency`]);
    /// - the dependency set introduces no cycle ([`EnqueueError::Cycle`]).
    pub fn enqueue(&mut self, spec: JobSpec) -> Result<JobId, EnqueueError> {
        let id = JobId::new(self.next_id);
        self.next_id += 1;
        self.enqueue_with_id(id, spec)?;
        Ok(id)
    }

    /// Enqueue a job with an explicit caller-supplied id.
    pub fn enqueue_with_id(&mut self, id: JobId, spec: JobSpec) -> Result<(), EnqueueError> {
        if self.jobs.contains_key(&id) {
            return Err(EnqueueError::DuplicateId(id));
        }
        // Validate + wire the dependency graph first (all-or-nothing).
        for &dep in &spec.deps {
            if !self.jobs.contains_key(&dep) {
                return Err(EnqueueError::UnknownDependency(UnknownDependency(dep)));
            }
        }
        self.graph.add_job(id);
        for &dep in &spec.deps {
            self.graph.add_dependency(id, dep).map_err(|e| match e {
                super::dag::DepError::Unknown(u) => EnqueueError::UnknownDependency(u),
                super::dag::DepError::Cycle(c) => EnqueueError::Cycle(c),
            })?;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let root = spec.deps.is_empty();
        let job = Job {
            spec,
            status: if root {
                JobStatus::Ready
            } else {
                JobStatus::Waiting
            },
            seq,
            failures: 0,
            ready_at_ms: 0,
            lease_deadline_ms: None,
            generation: 0,
        };
        self.jobs.insert(id, job);
        if root {
            self.push_ready(id);
        }
        Ok(())
    }

    /// Dispatch the next runnable job at `now_ms`, or `None` if nothing is
    /// ready. The returned job is marked `Running` with a fresh lease and a
    /// new dispatch generation, and carries the placement decision
    /// (P9-M4-T01: which node + reserved GPU devices, CPU fallback flag).
    ///
    /// When a node pool is attached, a *required* GPU job that currently has
    /// no matching free device is parked (kept `Ready` but moved out of the
    /// ready heap so it cannot head-of-line block other jobs) and dispatch
    /// continues with the next ready job; a *preferred* GPU job dispatches to
    /// a CPU node when no GPU is free.
    pub fn dispatch(&mut self, now_ms: u64) -> Option<Dispatch> {
        self.promote_delayed(now_ms);
        // If a node registered or a device was released since the last call
        // (and no ack/fail/reclaim ran), re-admit parked jobs that now fit.
        self.promote_parked();
        while let Some(entry) = self.ready.pop() {
            // Snapshot the fields we need without holding a &mut into
            // `self.jobs` across the placement call (which borrows self).
            let (status, spec) = {
                let job = self.jobs.get_mut(&entry.id)?;
                (job.status, job.spec.clone())
            };
            if status != JobStatus::Ready {
                continue; // stale heap entry (already dispatched/dead-lettered)
            }

            // Placement decision (P9-M4-T01). Pool-less schedulers keep the
            // P9-M2-T01 baseline: unplaced dispatch.
            let placement = match self.pool.as_mut() {
                None => Placement::unplaced(),
                Some(pool) => {
                    match self.policy.place(pool, spec.gpu.as_ref(), DEFAULT_CPU_NODE) {
                        Some(p) => p,
                        None => {
                            // Required GPU job, no matching free device:
                            // park it (out of the heap) and keep looking.
                            self.parked.push(entry.id);
                            continue;
                        }
                    }
                }
            };

            let id = entry.id;
            let job = self.jobs.get_mut(&id)?;
            if job.status != JobStatus::Ready {
                continue;
            }
            job.status = JobStatus::Running;
            job.generation += 1;
            job.lease_deadline_ms = Some(now_ms + self.lease_ms);

            // Record the reservation so releasing on ack/fail/reclaim hands
            // devices back to the pool (recorded only once the job is truly
            // Running, so a stale entry cannot leak a device).
            if !placement.gpu_devices.is_empty() {
                self.reservations.insert(id, placement.clone());
            }

            return Some(Dispatch {
                job: id,
                payload: spec.payload,
                idempotency_key: spec.idempotency_key,
                generation: job.generation,
                placement,
            });
        }
        None
    }

    /// Ack a dispatched job as successful. On success the job is terminal and
    /// its dependents are released into the ready heap.
    pub fn ack(&mut self, job: JobId, generation: u64) -> Result<(), AckError> {
        let job_rec = self.jobs.get_mut(&job).ok_or(AckError::NotFound(job))?;
        if job_rec.status != JobStatus::Running {
            return Err(AckError::NotRunning(job, job_rec.status));
        }
        if job_rec.generation != generation {
            let expected = job_rec.generation;
            return Err(AckError::StaleLease {
                job,
                expected,
                got: generation,
            });
        }
        job_rec.status = JobStatus::Succeeded;
        job_rec.lease_deadline_ms = None;
        self.succeeded.insert(job);

        // A GPU device a running job held is now free (P9-M4-T01).
        self.release_reservation(job);

        // Release dependents whose deps are now satisfied.
        let dependents: Vec<JobId> = self.graph.dependents_of(job).to_vec();
        for dep in dependents {
            let ready = self.graph.deps_satisfied(dep, &self.succeeded);
            let d = self.jobs.get_mut(&dep);
            if let Some(d) = d {
                if d.status == JobStatus::Waiting && ready {
                    d.status = JobStatus::Ready;
                    self.push_ready(dep);
                }
            }
        }
        Ok(())
    }

    /// Fail a dispatched job. If the retry budget allows, the job moves to the
    /// backoff window (it re-dispatches after [`RetryPolicy::backoff_ms`]);
    /// otherwise it dead-letters and its dependents cascade.
    pub fn fail(&mut self, job: JobId, generation: u64, now_ms: u64) -> Result<(), FailError> {
        self.fail_inner(job, generation, now_ms)
    }

    /// Report a job as poison (terminal, will never succeed). Dead-letters
    /// immediately without consuming the retry budget, and cascades to
    /// dependents.
    pub fn poison_report(
        &mut self,
        job: JobId,
        generation: u64,
        detail: &str,
        now_ms: u64,
    ) -> Result<(), FailError> {
        self.resolve_failure(
            job,
            generation,
            now_ms,
            DeadLetterReason::PoisonReported(detail),
        )
    }

    /// Move a job straight to the dead-letter log by manual request
    /// (regardless of running state), cascading to dependents.
    pub fn dead_letter_manually(
        &mut self,
        job: JobId,
        detail: &str,
        now_ms: u64,
    ) -> Result<(), FailError> {
        if !self.jobs.contains_key(&job) {
            return Err(FailError::NotFound(job));
        }
        self.resolve_failure(
            job,
            /* allow regardless of running state */ u64::MAX,
            now_ms,
            DeadLetterReason::Manual(detail),
        )
    }

    /// Reclaim any `Running` job whose lease has expired at `now_ms`,
    /// returning the reclaimed ids. The job is moved back to `Ready` and
    /// **redelivered with the same idempotency key** — the at-least-once
    /// seam (a worker that died mid-job does not swallow the work).
    pub fn reclaim_expired(&mut self, now_ms: u64) -> Vec<JobId> {
        let mut reclaimed = Vec::new();
        let expired: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|(_, j)| {
                j.status == JobStatus::Running && j.lease_deadline_ms.is_some_and(|d| d < now_ms)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            {
                let j = self.jobs.get_mut(&id);
                if let Some(j) = j {
                    j.status = JobStatus::Ready;
                    j.lease_deadline_ms = None;
                }
            }
            // A lost worker's GPU device returns to the pool (P9-M4-T01);
            // the redelivered job re-reserves on its next dispatch.
            self.release_reservation(id);
            self.push_ready(id);
            reclaimed.push(id);
        }
        reclaimed
    }

    /// Dependencies of a job, as enqueued.
    pub fn deps_of(&self, job: JobId) -> Vec<JobId> {
        self.graph.deps_of(job).to_vec()
    }

    /// Current status of a job, if it exists.
    pub fn status(&self, job: JobId) -> Option<JobStatus> {
        self.jobs.get(&job).map(|j| j.status)
    }

    /// The dead-letter log (chronological death order).
    pub fn dead_letters(&self) -> &[DeadLetter] {
        &self.dead_letters
    }

    /// Whether any jobs are waiting on dependencies (blocked).
    pub fn pending_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| j.status == JobStatus::Waiting)
            .count()
    }

    /// Number of jobs currently ready to dispatch.
    pub fn ready_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| j.status == JobStatus::Ready)
            .count()
    }

    /// Number of jobs currently handed to workers.
    pub fn running_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| j.status == JobStatus::Running)
            .count()
    }

    /// Total number of known jobs of any status.
    pub fn total_count(&self) -> usize {
        self.jobs.len()
    }

    /// How many jobs have been dispatched so far (any generation).
    pub fn dispatch_generation_of(&self, job: JobId) -> Option<u64> {
        self.jobs.get(&job).map(|j| j.generation)
    }

    // --- internals -----------------------------------------------------

    /// Push a `Ready` job's heap entry (status already transitioned).
    fn push_ready(&mut self, id: JobId) {
        let entry = self.jobs.get(&id).expect("job exists when pushing");
        self.ready.push(HeapEntry {
            priority: entry.spec.priority,
            seq: entry.seq,
            id,
        });
    }

    /// Move retrying jobs whose backoff window has elapsed into ready.
    fn promote_delayed(&mut self, now_ms: u64) {
        while let Some(entry) = self.delayed.pop() {
            if entry.ready_at_ms > now_ms {
                // The entry is not due yet; put it back and stop (the heap is
                // ordered by ready_at, so nothing later is due either).
                self.delayed.push(entry);
                break;
            }
            let job = self.jobs.get_mut(&entry.id);
            if let Some(job) = job {
                if job.status == JobStatus::Retrying {
                    job.status = JobStatus::Ready;
                    self.push_ready(entry.id);
                }
            }
        }
    }

    /// Re-examine parked GPU jobs: any that now have a matching free device
    /// (a device was released or a node registered) move back into the ready
    /// heap. Jobs that still have no capacity stay parked.
    fn promote_parked(&mut self) {
        if self.parked.is_empty() || self.pool.is_none() {
            return;
        }
        let mut still_parked = Vec::new();
        let mut now_ready = Vec::new();
        for &id in &self.parked {
            let can_run = {
                let j = match self.jobs.get(&id) {
                    Some(j) => j,
                    None => continue, // unknown; drop from parked
                };
                let pool = self.pool.as_ref().expect("checked above");
                let req = j.spec.gpu.as_ref();
                let placeable = match req {
                    None => true, // a CPU job never parks; defensive only
                    Some(r) => pool.has_capacity(r.count, r.model.as_deref()),
                };
                placeable
            };
            if can_run {
                now_ready.push(id);
            } else {
                still_parked.push(id);
            }
        }
        self.parked = still_parked;
        for id in now_ready {
            self.push_ready(id);
        }
    }

    /// Release any device reservation a job currently holds back into the
    /// pool (on ack, fail, poison, dead-letter, or lease reclaim).
    fn release_reservation(&mut self, id: JobId) {
        if let Some(p) = self.reservations.remove(&id) {
            if let Some(pool) = self.pool.as_mut() {
                pool.release(&p.node_id, &p.gpu_devices);
            }
        }
        self.promote_parked();
    }

    /// Shared failure path. `generation == u64::MAX` bypasses the lease check
    /// (manual dead-lettering).
    fn fail_inner(&mut self, job: JobId, generation: u64, now_ms: u64) -> Result<(), FailError> {
        let job_rec = self.jobs.get_mut(&job).ok_or(FailError::NotFound(job))?;
        if job_rec.status != JobStatus::Running {
            return Err(FailError::NotRunning(job, job_rec.status));
        }
        let failures = job_rec.failures + 1;
        let policy = job_rec.spec.retry;
        if policy.can_retry_after(failures) {
            // Schedule a retry.
            let delay = policy.backoff_ms(failures);
            job_rec.status = JobStatus::Retrying;
            job_rec.failures = failures;
            job_rec.ready_at_ms = now_ms + delay;
            job_rec.lease_deadline_ms = None;
            let seq = job_rec.seq;
            // The running job no longer owns its GPU device during backoff
            // (P9-M4-T01); it re-reserves when it next dispatches.
            self.release_reservation(job);
            self.delayed.push(DelayedEntry {
                ready_at_ms: now_ms + delay,
                seq,
                id: job,
            });
            Ok(())
        } else {
            // resolve_failure releases any held reservation inside.
            self.resolve_failure(
                job,
                generation,
                now_ms,
                DeadLetterReason::RetriesExhausted { attempts: failures },
            )
        }
    }

    /// Move the job to the dead-letter log and cascade to its dependents.
    fn resolve_failure(
        &mut self,
        job: JobId,
        generation: u64,
        now_ms: u64,
        reason: DeadLetterReason<'_>,
    ) -> Result<(), FailError> {
        let message = reason.message();
        // Snapshot policy/priority before mutation for the record.
        let (policy, priority, failures, running_gen, status) = {
            let j = self.jobs.get(&job).ok_or(FailError::NotFound(job))?;
            (
                j.spec.retry,
                j.spec.priority,
                j.failures,
                j.generation,
                j.status,
            )
        };
        if generation != u64::MAX && (status != JobStatus::Running || generation != running_gen) {
            // Stale or not-running: the job is not owned by this lease.
            // At-least-once means we must not resolve the job twice.
            return Err(FailError::NotRunning(job, status));
        }
        let rec = self.jobs.get_mut(&job).expect("checked above");
        rec.status = JobStatus::DeadLettered;
        rec.lease_deadline_ms = None;
        self.dead_letters.push(DeadLetter {
            job,
            reason: message,
            retry: policy,
            priority,
            dead_at_ms: now_ms,
            attempts_used: failures,
        });

        // The dead job no longer owns its GPU device (P9-M4-T01).
        self.release_reservation(job);

        // Cascade **transitively** to every dependent that is still Waiting:
        // a job whose (transitive) dependency dead-lettered can never be
        // satisfied, so it dead-letters too and every job downstream of it as
        // well. This is a pure dependent-walk; direct and indirect dependents
        // are both covered because each newly dead-lettered node is pushed to
        // the stack and its own dependents are walked next.
        let mut stack: Vec<JobId> = self.graph.dependents_of(job).to_vec();
        while let Some(dep) = stack.pop() {
            let dead = {
                let d = self.jobs.get_mut(&dep);
                match d {
                    Some(d) if d.status == JobStatus::Waiting => {
                        d.status = JobStatus::DeadLettered;
                        let (dp, dr, df) = (d.spec.priority, d.spec.retry, d.failures);
                        self.dead_letters.push(DeadLetter {
                            job: dep,
                            reason: DeadLetterReason::DependencyDeadLettered { dependency: job }
                                .message(),
                            retry: dr,
                            priority: dp,
                            dead_at_ms: now_ms,
                            attempts_used: df,
                        });
                        true
                    }
                    _ => false,
                }
            };
            if dead {
                for &next in self.graph.dependents_of(dep) {
                    stack.push(next);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(payload: &str) -> JobSpec {
        JobSpec::new(payload)
    }

    #[test]
    fn enqueue_assigns_monotonic_ids() {
        let mut s = Scheduler::new();
        let a = s.enqueue(spec("a")).unwrap();
        let b = s.enqueue(spec("b")).unwrap();
        assert_eq!(a, JobId::new(0));
        assert_eq!(b, JobId::new(1));
        assert_eq!(s.total_count(), 2);
    }

    #[test]
    fn priority_order_and_fifo() {
        let mut s = Scheduler::new();
        let low = s
            .enqueue(spec("low").with_priority(Priority::new(1)))
            .unwrap();
        let high = s
            .enqueue(spec("high").with_priority(Priority::new(10)))
            .unwrap();
        let mid = s
            .enqueue(spec("mid").with_priority(Priority::new(5)))
            .unwrap();
        assert_eq!(s.dispatch(0).unwrap().job, high);
        assert_eq!(s.dispatch(0).unwrap().job, mid);
        assert_eq!(s.dispatch(0).unwrap().job, low);
        assert!(s.dispatch(0).is_none());
    }

    #[test]
    fn fifo_tiebreak_within_priority() {
        let mut s = Scheduler::new();
        let a = s
            .enqueue(spec("a").with_priority(Priority::new(5)))
            .unwrap();
        let b = s
            .enqueue(spec("b").with_priority(Priority::new(5)))
            .unwrap();
        let c = s
            .enqueue(spec("c").with_priority(Priority::new(5)))
            .unwrap();
        assert_eq!(s.dispatch(0).unwrap().job, a);
        assert_eq!(s.dispatch(0).unwrap().job, b);
        assert_eq!(s.dispatch(0).unwrap().job, c);
    }

    #[test]
    fn dependency_gates_dispatch() {
        let mut s = Scheduler::new();
        let parent = s.enqueue(spec("parent")).unwrap();
        let child = s.enqueue(spec("child").with_deps(vec![parent])).unwrap();

        // Only parent is dispatchable; child waits.
        assert_eq!(s.dispatch(0).unwrap().job, parent);
        assert!(s.dispatch(0).is_none());

        // Parent acks -> child releases.
        s.ack(parent, 1).unwrap();
        assert_eq!(s.dispatch(0).unwrap().job, child);
    }

    #[test]
    fn unknown_dependency_rejected() {
        let mut s = Scheduler::new();
        let err = s.enqueue(spec("x").with_deps(vec![JobId::new(99)]));
        assert!(matches!(err, Err(EnqueueError::UnknownDependency(_))));
    }

    #[test]
    fn duplicate_id_rejected_at_enqueue() {
        let mut s = Scheduler::new();
        let a = s.enqueue(spec("a")).unwrap();
        // The scheduler owns a monotonically-increasing id space and rejects
        // re-enqueueing an existing id. This is also what structurally
        // prevents cycles via the public API: deps must reference known jobs
        // and no known job's identity can be reused, so no edge can point
        // backwards into a cycle. Cycle detection itself is unit-tested on
        // the DepGraph (sched::dag::tests) where it is reachable.
        let err = s.enqueue_with_id(a, spec("dupe"));
        assert_eq!(err, Err(EnqueueError::DuplicateId(a)));
        assert_eq!(s.total_count(), 1);
    }

    #[test]
    fn retry_with_backoff_then_succeed() {
        let mut s = Scheduler::new();
        let retry = RetryPolicy {
            max_attempts: 3,
            base_backoff_ms: 100,
            backoff_factor: 2,
            max_backoff_ms: 10_000,
        };
        let job = s.enqueue(spec("j").with_retry(retry)).unwrap();

        // Attempt 1: fail -> backoff 100ms.
        let d1 = s.dispatch(0).unwrap();
        assert_eq!(d1.job, job);
        s.fail(job, d1.generation, 0).unwrap();
        assert_eq!(s.status(job), Some(JobStatus::Retrying));
        assert!(s.dispatch(50).is_none(), "not due until 100ms");

        // Attempt 2: fail -> backoff 200ms (100ms base scaled once).
        let d2 = s.dispatch(100).unwrap();
        assert_eq!(d2.job, job);
        s.fail(job, d2.generation, 100).unwrap();
        assert!(s.dispatch(299).is_none(), "not due until 300ms");

        // Attempt 3: success.
        let d3 = s.dispatch(300).unwrap();
        assert_eq!(d3.job, job);
        s.ack(job, d3.generation).unwrap();
        assert_eq!(s.status(job), Some(JobStatus::Succeeded));
        assert!(s.dispatch(1000).is_none());
        assert!(s.dead_letters().is_empty());
    }

    #[test]
    fn dead_letters_poison_after_budget() {
        let mut s = Scheduler::new();
        let retry = RetryPolicy {
            max_attempts: 2,
            base_backoff_ms: 0,
            backoff_factor: 2,
            max_backoff_ms: 1000,
        };
        let job = s.enqueue(spec("poison").with_retry(retry)).unwrap();
        let d1 = s.dispatch(0).unwrap();
        s.fail(job, d1.generation, 0).unwrap();
        let d2 = s.dispatch(0).unwrap();
        s.fail(job, d2.generation, 0).unwrap();

        assert_eq!(s.status(job), Some(JobStatus::DeadLettered));
        assert_eq!(s.dead_letters().len(), 1);
        assert!(s.dead_letters()[0].reason.starts_with("retries exhausted"));
        assert!(
            s.dispatch(1000).is_none(),
            "dead-lettered jobs never dispatch"
        );
    }

    #[test]
    fn poison_report_dead_letters_immediately() {
        let mut s = Scheduler::new();
        let job = s.enqueue(spec("bad")).unwrap();
        let d = s.dispatch(0).unwrap();
        s.poison_report(job, d.generation, "corrupt header", 0)
            .unwrap();
        assert_eq!(s.status(job), Some(JobStatus::DeadLettered));
        assert!(s.dead_letters()[0].reason.starts_with("poison reported"));
    }

    #[test]
    fn at_least_once_reclaim_expired_redelivers() {
        let mut s = Scheduler::with_lease(50);
        let job = s
            .enqueue(spec("node-loss").with_idempotency("nk-1"))
            .unwrap();
        let d1 = s.dispatch(0).unwrap();
        assert!(d1.is_idempotent());
        assert_eq!(d1.idempotency_key.as_deref(), Some("nk-1"));

        // No ack: worker lost. Lease expires at now+50. Reclaim at t=60.
        assert!(s.reclaim_expired(60).contains(&job));
        assert_eq!(s.status(job), Some(JobStatus::Ready));

        // Redelivered with the SAME idempotency key and a fresh generation.
        let d2 = s.dispatch(60).unwrap();
        assert_eq!(d2.job, job);
        assert_eq!(d2.idempotency_key.as_deref(), Some("nk-1"));
        assert!(d2.generation > d1.generation);
    }

    #[test]
    fn stale_ack_after_redelivery_is_rejected() {
        let mut s = Scheduler::with_lease(50);
        let job = s.enqueue(spec("j")).unwrap();
        let d1 = s.dispatch(0).unwrap();
        s.reclaim_expired(60);
        let d2 = s.dispatch(60).unwrap();

        // The worker from lease 1 tries to ack late: must be rejected.
        assert!(matches!(
            s.ack(job, d1.generation),
            Err(AckError::StaleLease { .. }) | Err(AckError::NotRunning(..))
        ));
        // The current lease acks fine.
        s.ack(job, d2.generation).unwrap();
        assert_eq!(s.status(job), Some(JobStatus::Succeeded));
    }

    #[test]
    fn dependency_dead_letter_cascades() {
        let mut s = Scheduler::new();
        let a = s.enqueue(spec("a")).unwrap();
        let b = s.enqueue(spec("b").with_deps(vec![a])).unwrap();
        let da = s.dispatch(0).unwrap();
        assert_eq!(da.job, a);
        s.poison_report(a, da.generation, "poison", 0).unwrap();

        assert_eq!(s.status(a), Some(JobStatus::DeadLettered));
        assert_eq!(s.status(b), Some(JobStatus::DeadLettered));
        assert_eq!(s.dead_letters().len(), 2);
        assert!(s.dead_letters()[1]
            .reason
            .starts_with("dependency dead-lettered"));
    }

    #[test]
    fn counts_reflect_lifecycle() {
        let mut s = Scheduler::new();
        let a = s.enqueue(spec("a")).unwrap();
        let b = s.enqueue(spec("b").with_deps(vec![a])).unwrap();
        assert_eq!(s.ready_count(), 1);
        assert_eq!(s.pending_count(), 1);
        s.dispatch(0).unwrap();
        assert_eq!(s.running_count(), 1);
        s.ack(a, 1).unwrap();
        assert_eq!(s.running_count(), 0);
        assert_eq!(s.ready_count(), 1);
        let _ = b;
    }
}
