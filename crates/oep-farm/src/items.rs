//! Deterministic work items for a long compute job.
//!
//! A long job in the farm model is an *ordered sequence of work items*. An
//! item with a given index must produce the **same output bytes on every
//! run**, because that determinism is what makes a resumed run
//! byte-equivalent to a clean run: the items after the checkpoint are
//! exactly determined by their indices. The [ItemSource] trait is that
//! contract; [SeededItemSource] is a reproducible implementation for drills
//! and proofs (its bytes look like real per-item work output without being
//! a mock of any specific engine).

/// The deterministic work-item contract for a long P9 job.
pub trait ItemSource {
    /// Total number of items in the job (>= 1 for a real job).
    fn item_count(&self) -> u64;
    /// Deterministic output bytes for the item at `index`.
    ///
    /// Invariants (the byte-equivalence basis):
    /// - `item_bytes(i)` returns the same bytes every call, on every host;
    /// - the sequence for `0..item_count()` never changes across runs.
    fn item_bytes(&self, index: u64) -> Vec<u8>;
}

/// A deterministic, host-independent item source for drills and proofs.
///
/// Item `i` emits `item_size` bytes derived from `splitmix64(seed ^ i)`
/// expanded, so the stream is reproducible everywhere and clearly differs per
/// item (looks like real per-item output, e.g. a rendered tile or a
/// converted-file chunk) without depending on any graphics or CAD engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededItemSource {
    seed: u64,
    count: u64,
    item_size: usize,
}

impl SeededItemSource {
    /// A job of `count` items, each yielding `item_size` deterministic bytes,
    /// seeded by `seed` (changing the seed changes the whole stream; the same
    /// seed reproduces it exactly).
    pub fn new(seed: u64, count: u64, item_size: usize) -> Self {
        assert!(count > 0, "a farm job needs at least one work item");
        assert!(item_size > 0, "a work item must produce at least one byte");
        SeededItemSource {
            seed,
            count,
            item_size,
        }
    }

    /// The seed this source was built from.
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl ItemSource for SeededItemSource {
    fn item_count(&self) -> u64 {
        self.count
    }

    fn item_bytes(&self, index: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.item_size);
        let mut state = crate::splitmix64(self.seed ^ index);
        while out.len() < self.item_size {
            state = crate::splitmix64(state);
            out.extend_from_slice(&state.to_le_bytes());
        }
        out.truncate(self.item_size);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_are_deterministic_per_index() {
        let a = SeededItemSource::new(7, 32, 64);
        let b = SeededItemSource::new(7, 32, 64);
        for i in 0..32 {
            assert_eq!(a.item_bytes(i), b.item_bytes(i), "item {i} reproducible");
            if i > 0 {
                assert_ne!(a.item_bytes(i), a.item_bytes(i - 1), "distinct per item");
            }
        }
        assert_eq!(
            a.item_bytes(0),
            SeededItemSource::new(7, 1, 64).item_bytes(0)
        );
    }

    #[test]
    fn item_bytes_size_is_exact() {
        let s = SeededItemSource::new(3, 5, 257);
        for i in 0..5 {
            assert_eq!(s.item_bytes(i).len(), 257);
        }
    }

    #[test]
    #[should_panic]
    fn needs_at_least_one_item() {
        let _ = SeededItemSource::new(1, 0, 8);
    }
}
