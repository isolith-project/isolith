//! Shared durable job-state store + effect-ledger backend (RED API surface).

use crate::record::{JobRecord, JobState, StoreError};
use std::sync::Arc;

/// The durable state backend contract (RED: trait declared, behavior GREEN).
pub trait JobStore: Send + Sync {
    /// Persist a job record (insert or overwrite by `id`).
    fn put(&self, record: &JobRecord) -> Result<(), StoreError>;

    /// Fetch a job by id.
    fn get(&self, id: &str) -> Result<Option<JobRecord>, StoreError>;

    /// Atomically transition a job's state (CAS on `expected`).
    fn transition(
        &self,
        id: &str,
        expected: &JobState,
        next: JobState,
        bump_attempt: bool,
    ) -> Result<bool, StoreError>;

    /// Enumerate all records.
    fn all(&self) -> Result<Vec<JobRecord>, StoreError>;

    /// Durable exactly-once effect mark; `Ok(true)` = newly marked.
    fn mark_applied(&self, key: &str) -> Result<bool, StoreError>;

    /// Whether an effect key has already been applied.
    fn is_applied(&self, key: &str) -> Result<bool, StoreError>;
}

/// In-memory stub (RED): state is empty until GREEN supplies the backend.
#[derive(Debug, Default)]
pub struct MemoryStore;

impl MemoryStore {
    /// A new, empty store (RED stub).
    pub fn new() -> Self {
        MemoryStore
    }

    /// Wrap in an `Arc<dyn JobStore>` (RED stub).
    pub fn shared() -> Arc<dyn JobStore> {
        Arc::new(MemoryStore)
    }
}

impl JobStore for MemoryStore {
    fn put(&self, _record: &JobRecord) -> Result<(), StoreError> {
        unimplemented!("P9-M2-T03 store put lands in GREEN")
    }
    fn get(&self, _id: &str) -> Result<Option<JobRecord>, StoreError> {
        unimplemented!("P9-M2-T03 store get lands in GREEN")
    }
    fn transition(
        &self,
        _id: &str,
        _expected: &JobState,
        _next: JobState,
        _bump_attempt: bool,
    ) -> Result<bool, StoreError> {
        unimplemented!("P9-M2-T03 store transition lands in GREEN")
    }
    fn all(&self) -> Result<Vec<JobRecord>, StoreError> {
        Ok(Vec::new())
    }
    fn mark_applied(&self, _key: &str) -> Result<bool, StoreError> {
        unimplemented!("P9-M2-T03 store mark_applied lands in GREEN")
    }
    fn is_applied(&self, _key: &str) -> Result<bool, StoreError> {
        unimplemented!("P9-M2-T03 store is_applied lands in GREEN")
    }
}

/// Convenience: a fresh shared in-memory store (RED stub).
pub fn in_memory() -> Arc<dyn JobStore> {
    MemoryStore::shared()
}
