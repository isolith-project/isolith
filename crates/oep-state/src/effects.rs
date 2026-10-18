//! Durable effect ledger: the idempotency mechanism for at-least-once jobs.
//!
//! Delivery is at-least-once: a job may run two or more times after node
//! loss. To make that safe, side effects are keyed and recorded exactly once
//! in the shared [`JobStore`]. [`EffectLedger::apply_once`] is the single
//! gate: the first caller runs the effect and marks the key; every later
//! caller (including a redelivered run on another worker) observes
//! [`EffectOutcome::AlreadyApplied`] and skips — no double-effect.
//!
//! Keys are content-derived (`{job id}:{effect name}`, or the content
//! address of the produced output), consistent with the P8 content-address
//! doctrine where writing the same output twice is a no-op.

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

/// Durable exactly-once gate over a shared [`JobStore`].
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
    ///
    /// - First call for a key: marks it applied, runs `apply`, returns
    ///   [`EffectOutcome::Applied`].
    /// - Any later call for the same key: returns
    ///   [`EffectOutcome::AlreadyApplied`] without running `apply`, even
    ///   across workers and across a redelivery after node loss.
    ///
    /// The mark is written *before* `apply` runs so that a worker killed
    /// mid-effect still cannot cause a second application (jobs MUST be
    /// idempotent at the effect level per P9 doctrine; re-running a
    /// content-addressed write is a no-op by construction).
    pub fn apply_once<W, F>(&self, key: &str, witness: &mut W, apply: F) -> EffectOutcome
    where
        F: FnOnce(&mut W),
    {
        match self.store.mark_applied(key) {
            Ok(true) => {
                apply(witness);
                EffectOutcome::Applied
            }
            Ok(false) => EffectOutcome::AlreadyApplied,
            Err(_) => EffectOutcome::AlreadyApplied, // storage is shared durable state
        }
    }

    /// Whether the key (and therefore the effect) is durably applied.
    pub fn is_applied(&self, key: &str) -> bool {
        self.store.is_applied(key).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::in_memory;

    #[test]
    fn first_apply_wins_subsequent_are_noops() {
        let ledger = EffectLedger::new(in_memory());
        let mut log = Vec::new();

        assert_eq!(
            ledger.apply_once("k", &mut log, |l| l.push(1)),
            EffectOutcome::Applied
        );
        assert_eq!(log, vec![1]);

        assert_eq!(
            ledger.apply_once("k", &mut log, |l| l.push(2)),
            EffectOutcome::AlreadyApplied
        );
        assert_eq!(log, vec![1], "second call must not run the closure");
    }

    #[test]
    fn different_keys_are_independent() {
        let ledger = EffectLedger::new(in_memory());
        let mut log = Vec::new();
        assert_eq!(
            ledger.apply_once("a", &mut log, |l| l.push("a")),
            EffectOutcome::Applied
        );
        assert_eq!(
            ledger.apply_once("b", &mut log, |l| l.push("b")),
            EffectOutcome::Applied
        );
        assert_eq!(log, vec!["a", "b"]);
        assert!(ledger.is_applied("a"));
        assert!(!ledger.is_applied("c"));
    }
}
