//! Durable effect ledger: the idempotency mechanism (RED API surface).

use crate::store::JobStore;
use std::sync::Arc;

/// Result of attempting to apply an effect exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOutcome {
    /// This call ran the effect and durably marked the key.
    Applied,
    /// The effect was already applied by an earlier run (this run skips).
    AlreadyApplied,
}

/// Durable exactly-once gate over a shared [`JobStore`] (RED: behavior in
/// GREEN).
#[derive(Clone)]
pub struct EffectLedger {
    store: Arc<dyn JobStore>,
}

impl EffectLedger {
    /// A ledger backed by the same durable store that holds job records.
    pub fn new(store: Arc<dyn JobStore>) -> Self {
        EffectLedger { store }
    }

    /// Apply `apply(witness)` exactly once per `key`, durably.
    pub fn apply_once<W, F>(&self, _key: &str, _witness: &mut W, _apply: F) -> EffectOutcome
    where
        F: FnOnce(&mut W),
    {
        unimplemented!("P9-M2-T03 effect ledger lands in GREEN")
    }

    /// Whether the key (and therefore the effect) is durably applied.
    pub fn is_applied(&self, _key: &str) -> bool {
        unimplemented!("P9-M2-T03 effect ledger is_applied lands in GREEN")
    }
}
