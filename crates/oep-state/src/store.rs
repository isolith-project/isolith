//! Shared durable job-state store + effect-ledger backend.
//!
//! The [`JobStore`] trait is the seam where distributed state lives: job
//! records are durable and addressable by id, and the backend must provide
//! compare-and-set transitions so two schedulers/workers cannot race a claim
//! or a completion. [`MemoryStore`] is the test/proof backend:
//! single-process, mutex-serialized — perfect for the deterministic node-loss
//! drill and for a scheduler + workers embedded in one process. A durable
//! cluster backend (JetStream KV bucket, P8 vault blob store) implements the
//! same trait.

use crate::record::{JobRecord, JobState, StoreError};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// The durable state backend contract.
///
/// All methods are internally synchronized; a real backend is a JetStream KV
/// bucket or the P8 vault blob store.
pub trait JobStore: Send + Sync {
    /// Persist a job record (insert or overwrite by `id`).
    fn put(&self, record: &JobRecord) -> Result<(), StoreError>;

    /// Fetch a job by id.
    fn get(&self, id: &str) -> Result<Option<JobRecord>, StoreError>;

    /// Atomically transition a job's state and (optionally) bump its attempt
    /// counter. Returns `Ok(true)` only when `expected` matched; `Ok(false)`
    /// when the record is absent or its state differed.
    fn transition(
        &self,
        id: &str,
        expected: &JobState,
        next: JobState,
        bump_attempt: bool,
    ) -> Result<bool, StoreError>;

    /// Enumerate all records (scheduler scans for runnable jobs per queue).
    fn all(&self) -> Result<Vec<JobRecord>, StoreError>;

    /// Record that an effect key has been applied (content-addressed
    /// idempotency mark). Returns `Ok(true)` when newly marked, `Ok(false)`
    /// when the key was already marked — the atomic dedup primitive.
    fn mark_applied(&self, key: &str) -> Result<bool, StoreError>;

    /// Whether an effect key has already been applied.
    fn is_applied(&self, key: &str) -> Result<bool, StoreError>;
}

/// A simple in-process, mutex-serialized [`JobStore`].
///
/// Used by the acceptance drill and as the embedded (single-node) backend.
/// Node loss is proven against it because records survive independently of
/// any worker — the same invariant a durable cluster store provides.
#[derive(Debug, Default)]
pub struct MemoryStore {
    inner: Mutex<HashMap<String, JobRecord>>,
    applied: Mutex<HashSet<String>>,
}

impl MemoryStore {
    /// A new, empty store.
    pub fn new() -> Self {
        MemoryStore::default()
    }

    /// Wrap in an `Arc<dyn JobStore>` for sharing across workers/scheduler.
    pub fn shared() -> Arc<dyn JobStore> {
        Arc::new(MemoryStore::new())
    }
}

impl JobStore for MemoryStore {
    fn put(&self, record: &JobRecord) -> Result<(), StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Storage("store lock poisoned".into()))?
            .insert(record.id.clone(), record.clone());
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<JobRecord>, StoreError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| StoreError::Storage("store lock poisoned".into()))?
            .get(id)
            .cloned())
    }

    fn transition(
        &self,
        id: &str,
        expected: &JobState,
        next: JobState,
        bump_attempt: bool,
    ) -> Result<bool, StoreError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| StoreError::Storage("store lock poisoned".into()))?;
        match guard.get_mut(id) {
            None => Ok(false),
            Some(rec) if &rec.state != expected => Ok(false),
            Some(rec) => {
                if bump_attempt {
                    rec.attempt = rec.attempt.saturating_add(1);
                }
                rec.state = next;
                Ok(true)
            }
        }
    }

    fn all(&self) -> Result<Vec<JobRecord>, StoreError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| StoreError::Storage("store lock poisoned".into()))?
            .values()
            .cloned()
            .collect())
    }

    fn mark_applied(&self, key: &str) -> Result<bool, StoreError> {
        let mut guard = self
            .applied
            .lock()
            .map_err(|_| StoreError::Storage("applied lock poisoned".into()))?;
        Ok(guard.insert(key.to_string()))
    }

    fn is_applied(&self, key: &str) -> Result<bool, StoreError> {
        Ok(self
            .applied
            .lock()
            .map_err(|_| StoreError::Storage("applied lock poisoned".into()))?
            .contains(key))
    }
}

/// Convenience: a fresh shared in-memory store.
pub fn in_memory() -> Arc<dyn JobStore> {
    MemoryStore::shared()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_is_cas_guarded_and_bumps_attempt() {
        let store = in_memory();
        let rec = JobRecord::new("j", "q", None, 0);
        store.put(&rec).unwrap();
        let running = JobState::Running {
            worker: "w".into(),
            lease_until_ms: 10,
        };
        assert!(store
            .transition("j", &JobState::Queued, running.clone(), true)
            .unwrap());
        // Second transition from a stale expected state must fail.
        assert!(!store
            .transition("j", &JobState::Queued, running.clone(), true)
            .unwrap());
        assert!(store
            .transition("j", &running, JobState::Succeeded, false)
            .unwrap());
        let got = store.get("j").unwrap().unwrap();
        assert_eq!(got.state, JobState::Succeeded);
        assert_eq!(got.attempt, 1, "claim bumped attempt exactly once");
    }

    #[test]
    fn applied_marks_are_atomic_dedup() {
        let store = in_memory();
        assert!(store.mark_applied("k").unwrap());
        assert!(!store.mark_applied("k").unwrap(), "second mark is a no-op");
        assert!(store.is_applied("k").unwrap());
        assert!(!store.is_applied("other").unwrap());
    }

    #[test]
    fn get_absent_is_none() {
        let store = in_memory();
        assert!(store.get("nope").unwrap().is_none());
    }
}
