//! Pure dependency graph (DAG) for the scheduler (P9-M2-T01).
//!
//! The scheduler's job dependencies form a **DAG**: an edge `A -> B` reads
//! "B depends on A" (A is a dependency of B), so B becomes runnable only
//! after A has succeeded. The graph is deliberately *pure* (no wall clock, no
//! job payloads): it answers structural questions -- dependents of a node,
//! whether a node's dependencies are satisfied, and whether an added edge
//! would create a cycle.

use std::collections::HashMap;

use super::JobId;

/// A `JobId` referenced by a job's dependency list that was never enqueued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownDependency(pub JobId);

/// A dependency edge that would create a cycle in the job graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleError {
    /// The job whose deps seeding caused the cycle.
    pub job: JobId,
    /// The dependency edge (from, to) that closed the loop: `from -> to`.
    pub from: JobId,
    /// The dependency edge that closed the loop: `from -> to`.
    pub to: JobId,
}

/// A job's resolved dependency set: jobs that must succeed before it runs.
pub type DepSet = Vec<JobId>;

/// The pure dependency graph.
///
/// Convention: `deps[child] = [p1, p2, ...]` means child waits on every `pn`.
/// The inverse `dependents[parent]` is derived so cascade queries (which jobs
/// are blocked behind a dead-lettered parent?) are O(1).
#[derive(Default, Debug)]
pub struct DepGraph {
    deps: HashMap<JobId, DepSet>,
    dependents: HashMap<JobId, Vec<JobId>>,
    /// Jobs known but not yet wired with deps (for validation).
    known: std::collections::HashSet<JobId>,
}

impl DepGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a job id so dependency lists may reference it.
    pub fn add_job(&mut self, id: JobId) {
        self.known.insert(id);
        self.deps.entry(id).or_default();
    }

    /// All dependencies of `job`, in the order declared.
    pub fn deps_of(&self, job: JobId) -> &[JobId] {
        self.deps.get(&job).map(Vec::as_slice).unwrap_or(&[])
    }

    /// All jobs that directly depend on `parent`.
    pub fn dependents_of(&self, parent: JobId) -> &[JobId] {
        self.dependents
            .get(&parent)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Whether `job` has zero dependencies declared (a root / source job).
    pub fn is_root(&self, job: JobId) -> bool {
        self.deps_of(job).is_empty()
    }

    /// Whether every dependency of `job` is in the provided *succeeded* set.
    ///
    /// This is the runnable gate: the scheduler calls it with the set of jobs
    /// that have reached the terminal success state, and only jobs returning
    /// `true` may dispatch.
    pub fn deps_satisfied(&self, job: JobId, succeeded: &std::collections::HashSet<JobId>) -> bool {
        self.deps_of(job).iter().all(|d| succeeded.contains(d))
    }

    /// Declare that `child` depends on `parent`.
    ///
    /// Errors:
    /// - [`UnknownDependency`] if `child` or `parent` was never
    ///   [`add_job`](DepGraph::add_job)'d.
    /// - [`CycleError`] if the edge would close a directed cycle.
    ///
    /// On cycle error the graph is left in its prior state (the edge is not
    /// inserted).
    pub fn add_dependency(&mut self, child: JobId, parent: JobId) -> Result<(), DepError> {
        if !self.known.contains(&child) || !self.known.contains(&parent) {
            return Err(DepError::Unknown(UnknownDependency(parent)));
        }
        if !self
            .deps
            .get(&child)
            .map(|d| d.contains(&parent))
            .unwrap_or(false)
        {
            // Adding "child depends on parent" closes a cycle when `parent`
            // already (transitively) depends on `child` -- i.e. a path
            // `parent -> ... -> child` exists in the depends-on orientation.
            if self.reaches(parent, child) {
                return Err(DepError::Cycle(CycleError {
                    job: child,
                    from: parent,
                    to: child,
                }));
            }
            self.deps.entry(child).or_default().push(parent);
            self.dependents.entry(parent).or_default().push(child);
        }
        Ok(())
    }

    /// Whether a directed path `from -> ... -> to` exists in the *depends-on*
    /// orientation (following each node's dependencies).
    fn reaches(&self, from: JobId, to: JobId) -> bool {
        if from == to {
            return true;
        }
        let mut stack = vec![from];
        let mut seen = std::collections::HashSet::new();
        while let Some(node) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            if node == to {
                return true;
            }
            for &d in self.deps_of(node) {
                stack.push(d);
            }
        }
        false
    }
}

/// Errors from [`DepGraph::add_dependency`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepError {
    /// A referenced job id was never registered.
    Unknown(UnknownDependency),
    /// The edge would create a cycle.
    Cycle(CycleError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[u64]) -> Vec<JobId> {
        v.iter().copied().map(JobId::new).collect()
    }

    fn set(v: &[u64]) -> std::collections::HashSet<JobId> {
        ids(v).into_iter().collect()
    }

    #[test]
    fn unknown_dependency_rejected() {
        let mut g = DepGraph::new();
        g.add_job(JobId::new(1));
        assert_eq!(
            g.add_dependency(JobId::new(1), JobId::new(99)),
            Err(DepError::Unknown(UnknownDependency(JobId::new(99))))
        );
    }

    #[test]
    fn root_jobs_have_no_deps() {
        let mut g = DepGraph::new();
        let a = JobId::new(1);
        g.add_job(a);
        assert!(g.is_root(a));
        assert!(g.deps_satisfied(a, &std::collections::HashSet::new()));
    }

    #[test]
    fn dependency_gates_runnability() {
        let mut g = DepGraph::new();
        let a = JobId::new(1);
        let b = JobId::new(2);
        g.add_job(a);
        g.add_job(b);
        g.add_dependency(b, a).unwrap();

        assert!(g.deps_satisfied(b, &set(&[1])), "b runs after a succeeds");
        assert!(!g.deps_satisfied(b, &set(&[])));
        assert!(g.is_root(a));
        assert!(!g.is_root(b));
        assert_eq!(g.dependents_of(a), &ids(&[2]));
    }

    #[test]
    fn depends_on_both_parents() {
        let mut g = DepGraph::new();
        let a = JobId::new(1);
        let b = JobId::new(2);
        let c = JobId::new(3);
        for j in [a, b, c] {
            g.add_job(j);
        }
        g.add_dependency(c, a).unwrap();
        g.add_dependency(c, b).unwrap();
        assert!(!g.deps_satisfied(c, &set(&[1])));
        assert!(!g.deps_satisfied(c, &set(&[2])));
        assert!(g.deps_satisfied(c, &set(&[1, 2])));
    }

    #[test]
    fn direct_cycle_rejected() {
        let mut g = DepGraph::new();
        let a = JobId::new(1);
        let b = JobId::new(2);
        g.add_job(a);
        g.add_job(b);
        g.add_dependency(b, a).unwrap();
        // a -> b would close the loop a -> b -> a.
        let err = g.add_dependency(a, b);
        assert!(matches!(err, Err(DepError::Cycle(_))));
        // State unchanged: a still has no deps.
        assert!(g.is_root(a));
    }

    #[test]
    fn indirect_cycle_rejected() {
        let mut g = DepGraph::new();
        let a = JobId::new(1);
        let b = JobId::new(2);
        let c = JobId::new(3);
        for j in [a, b, c] {
            g.add_job(j);
        }
        g.add_dependency(b, a).unwrap();
        g.add_dependency(c, b).unwrap();
        // c -> a would close c -> b -> a -> c.
        assert!(matches!(g.add_dependency(a, c), Err(DepError::Cycle(_))));
    }

    #[test]
    fn long_chain_preserved_after_cycle_reject() {
        let mut g = DepGraph::new();
        let (a, b, c, d) = (JobId::new(1), JobId::new(2), JobId::new(3), JobId::new(4));
        for j in [a, b, c, d] {
            g.add_job(j);
        }
        g.add_dependency(b, a).unwrap();
        g.add_dependency(c, b).unwrap();
        g.add_dependency(d, c).unwrap();
        assert_eq!(g.dependents_of(a), &[b]);
        assert_eq!(g.dependents_of(b), &[c]);
        assert_eq!(g.dependents_of(c), &[d]);
    }
}
