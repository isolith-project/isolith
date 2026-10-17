//! Injectable clock for deterministic lease/redelivery semantics (RED API).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Abstraction over "now" in milliseconds since a caller-defined epoch.
pub trait Clock: Send + Sync {
    /// Current time in milliseconds.
    fn now(&self) -> u64;
}

/// Wall-clock implementation.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Manually advanced clock for deterministic tests.
#[derive(Debug, Default)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    /// A clock reading `now` milliseconds.
    pub fn new(now: u64) -> Self {
        ManualClock {
            now: AtomicU64::new(now),
        }
    }

    /// Advance the clock by `delta` milliseconds.
    pub fn advance(&self, delta: u64) {
        self.now.fetch_add(delta, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}
