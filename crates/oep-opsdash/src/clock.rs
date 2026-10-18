//! Injected clock (the `oep-lane` time doctrine): every duration and
//! timestamp the dashboard records comes from a [`Clock`] the caller
//! supplies, so the live binary uses wall time and tests use a deterministic
//! manual clock.

use std::sync::atomic::{AtomicU64, Ordering};

/// A source of "now" in milliseconds since a caller-defined epoch.
///
/// Mirrors `oep-lane`'s injected-time model: the dashboard never reads the
/// wall clock itself, so every test is deterministic and the live binary
/// simply feeds it [`SystemClock`]. Clocks are shareable behind `Arc` for the
/// HTTP server, so the underlying time is stored atomically.
pub trait Clock: Send + Sync {
    /// Current time in ms since an opaque epoch.
    fn now_ms(&self) -> u64;
}

/// Wall-clock implementation backed by `std::time::SystemTime`, captured
/// relative to a fixed epoch captured at construction.
#[derive(Debug, Default)]
pub struct SystemClock {
    start_at: AtomicU64,
    initialized: std::sync::atomic::AtomicBool,
}

impl SystemClock {
    /// A wall clock whose epoch starts at construction.
    pub fn new() -> Self {
        let c = SystemClock::default();
        let epoch = unix_now_ms();
        c.start_at.store(epoch, Ordering::Relaxed);
        c.initialized.store(true, Ordering::Relaxed);
        c
    }
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        let epoch = self.start_at.load(Ordering::Relaxed);
        if !self.initialized.load(Ordering::Relaxed) || epoch == 0 {
            return 0;
        }
        unix_now_ms().saturating_sub(epoch)
    }
}

/// Deterministic manual clock for tests: the test advances an atomic counter
/// explicitly, so retry backoff windows, lease expiries and runtimes are all
/// exact even when the clock is shared behind an `Arc`.
#[derive(Debug, Default)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    /// A clock starting at `t0_ms`.
    pub fn new(t0_ms: u64) -> Self {
        ManualClock {
            now: AtomicU64::new(t0_ms),
        }
    }

    /// Advance the clock by `ms`.
    pub fn advance(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::Relaxed);
    }

    /// Set the clock to an absolute value (for exact leases in tests).
    pub fn set(&self, ms: u64) {
        self.now.store(ms, Ordering::Relaxed);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_advances_exactly() {
        let c = ManualClock::new(100);
        assert_eq!(c.now_ms(), 100);
        c.advance(250);
        assert_eq!(c.now_ms(), 350);
    }

    #[test]
    fn manual_clock_shared_advances() {
        let c = std::sync::Arc::new(ManualClock::new(0));
        let c2 = std::sync::Arc::clone(&c);
        c2.advance(42);
        assert_eq!(c.now_ms(), 42);
    }

    #[test]
    fn system_clock_moves_forward() {
        let c = SystemClock::new();
        let a = c.now_ms();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = c.now_ms();
        assert!(b >= a);
    }
}
