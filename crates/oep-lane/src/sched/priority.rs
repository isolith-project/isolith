//! Priority for scheduling (P9-M2-T01).

/// Scheduling priority. Higher values dispatch first; equal priorities fall
/// back to strictly FIFO enqueue order (handled by the scheduler's sequence,
/// not by this type).
///
/// Stored as an 8-bit value on purpose: priorities are a *hint*, not a
/// resource dimension (quotas are P9-M2-T02's concern), and `u8` keeps the
/// invariant "no silent wrap when maxing out" trivially checkable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(u8);

impl Priority {
    /// The lowest expressible priority (0).
    pub const MIN: Priority = Priority(0);
    /// The highest expressible priority (255).
    pub const MAX: Priority = Priority(255);

    /// Build a priority from a raw 8-bit value (0..=255).
    pub const fn new(raw: u8) -> Self {
        Priority(raw)
    }

    /// The raw 8-bit value.
    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_priority_sorts_first() {
        assert!(Priority::new(10) > Priority::new(5));
        assert!(Priority::new(0) < Priority::new(255));
    }

    #[test]
    fn min_max_bounds() {
        assert_eq!(Priority::MIN.raw(), 0);
        assert_eq!(Priority::MAX.raw(), 255);
        assert!(Priority::MIN < Priority::MAX);
    }
}
