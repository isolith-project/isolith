//! Injectable clock for deterministic lease/redelivery semantics.
//!
//! The node-loss drill must advance time deterministically (a test cannot
//! sleep or race the wall clock). [`Clock`] is the seam: production uses
//! [`SystemClock`]; tests use [`ManualClock`].

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_advances_monotonically() {
        let c = ManualClock::new(10);
        assert_eq!(c.now(), 10);
        c.advance(5);
        assert_eq!(c.now(), 15);
    }
}
