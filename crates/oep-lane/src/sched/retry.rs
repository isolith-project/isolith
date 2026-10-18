//! Retry policy with exponential backoff (P9-M2-T01).

/// Per-job retry policy: how many attempts a job may make and how long the
/// scheduler waits between a failure and the next attempt.
///
/// Backoff is **exponential**: after the `n`-th failure the scheduler waits
/// `base_backoff_ms * backoff_factor^(n-1)`, clamped to `max_backoff_ms`.
/// The cap makes long-lived retry chains bounded (a poison job cannot spin
/// forever at exponentially growing gaps; it lands in the dead-letter log
/// when `max_attempts` is exhausted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total number of execution attempts allowed (1 = no retry).
    pub max_attempts: u32,
    /// Delay before the first retry, in ms.
    pub base_backoff_ms: u64,
    /// Backoff multiplier (2 = classic exponential).
    pub backoff_factor: u32,
    /// Upper bound on any single backoff delay, in ms.
    pub max_backoff_ms: u64,
}

impl RetryPolicy {
    /// A policy that never retries (a single attempt).
    pub const fn once() -> Self {
        RetryPolicy {
            max_attempts: 1,
            base_backoff_ms: 0,
            backoff_factor: 2,
            max_backoff_ms: 0,
        }
    }

    /// Backoff delay, in ms, to apply *before* the attempt that follows the
    /// given number of already-consumed failures.
    ///
    /// `failures_already` is the count of completed failed attempts. The
    /// first failure yields `base_backoff_ms`, each further failure multiplies
    /// by `backoff_factor`, and the result never exceeds `max_backoff_ms`.
    pub const fn backoff_ms(&self, failures_already: u32) -> u64 {
        if self.max_attempts <= 1 || failures_already == 0 {
            return 0;
        }
        let mut delay = self.base_backoff_ms;
        let mut i = 1;
        // failures_already >= 1 here; scale (n-1) times.
        loop {
            if i >= failures_already {
                break;
            }
            delay = delay.saturating_mul(self.backoff_factor as u64);
            if delay >= self.max_backoff_ms {
                return self.max_backoff_ms;
            }
            i += 1;
        }
        if delay > self.max_backoff_ms {
            self.max_backoff_ms
        } else {
            delay
        }
    }

    /// Whether another attempt is permitted after `failures` failed attempts.
    pub const fn can_retry_after(&self, failures: u32) -> bool {
        failures < self.max_attempts
    }
}

impl Default for RetryPolicy {
    /// Default: 3 attempts, 100ms base, factor 2, 5s cap. Small values match
    /// the deterministic test clock; production callers set their own.
    fn default() -> Self {
        RetryPolicy {
            max_attempts: 3,
            base_backoff_ms: 100,
            backoff_factor: 2,
            max_backoff_ms: 5_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_never_retries() {
        let p = RetryPolicy::once();
        assert_eq!(p.max_attempts, 1);
        assert!(!p.can_retry_after(1));
        assert!(p.can_retry_after(0));
        assert_eq!(p.backoff_ms(1), 0);
    }

    #[test]
    fn exponential_backoff_shape() {
        let p = RetryPolicy {
            max_attempts: 10,
            base_backoff_ms: 100,
            backoff_factor: 2,
            max_backoff_ms: 10_000,
        };
        assert_eq!(p.backoff_ms(0), 0);
        assert_eq!(p.backoff_ms(1), 100);
        assert_eq!(p.backoff_ms(2), 200);
        assert_eq!(p.backoff_ms(3), 400);
        assert_eq!(p.backoff_ms(4), 800);
    }

    #[test]
    fn backoff_capped_at_max() {
        let p = RetryPolicy {
            max_attempts: 100,
            base_backoff_ms: 100,
            backoff_factor: 10,
            max_backoff_ms: 1_000,
        };
        assert_eq!(p.backoff_ms(1), 100);
        assert_eq!(p.backoff_ms(2), 1_000); // 100*10 = 1000, at cap
        assert_eq!(p.backoff_ms(50), 1_000); // stays at cap
    }

    #[test]
    fn can_retry_after_respects_budget() {
        let p = RetryPolicy {
            max_attempts: 3,
            base_backoff_ms: 100,
            backoff_factor: 2,
            max_backoff_ms: 1_000,
        };
        assert!(p.can_retry_after(0));
        assert!(p.can_retry_after(1));
        assert!(p.can_retry_after(2));
        assert!(!p.can_retry_after(3)); // budget exhausted
    }
}
