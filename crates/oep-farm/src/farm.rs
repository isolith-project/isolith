//! The farm runner: runs a long deterministic job with durable checkpoints
//! and resumes from the last checkpoint after a worker eviction.
//!
//! Mental model (beam/farm style):
//!
//! - A job is an ordered sequence of deterministic work items
//!   ([`crate::items::ItemSource`]). The worker streams every item's output
//!   bytes into an [`crate::artifact::Artifact`].
//! - Every [`FarmConfig::checkpoint_every`] completed items the runner writes
//!   a [`crate::record::Checkpoint`] to its [`crate::store::CheckpointStore`]:
//!   `next_item`, `output_len` and the running FNV-1a64 hash of the artifact
//!   prefix. That hash is the *byte-equivalence gate*.
//! - **Eviction** (worker lost mid-job) is simulated by
//!   [`FarmRunner::run_bounded`]: the runner stops after `max_items` of this
//!   run's items, `completed = false`, leaving the artifact and the latest
//!   checkpoint durable on disk (DirStore) or in memory.
//! - **Resume**: a fresh runner (new object, no leftover state) calls
//!   [`FarmRunner::run`] again. It loads the latest checkpoint, re-hashes the
//!   artifact prefix `[0..output_len)` and **refuses to continue on mismatch**
//!   ([`FarmError::Corruption`]), truncates the artifact back to the
//!   checkpoint boundary (dropping any partial tail a dying worker wrote),
//!   then resumes from `next_item`.
//! - Because item bytes are a pure function of the item index, the resumed
//!   tail is byte-identical to a clean run's tail - so the final artifact is
//!   **byte-equivalent to a clean run on output** (the acceptance; proven in
//!   `tests/checkpoint_resume_acceptance.rs`).

use std::io;

use crate::artifact::Artifact;
use crate::hash::{fnv1a64, Fnv1a64};
use crate::items::ItemSource;
use crate::record::Checkpoint;
use crate::store::{CheckpointStore, StoreError};

/// Tuning for a farm run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FarmConfig {
    /// Write a checkpoint after every `checkpoint_every` completed items.
    pub checkpoint_every: u64,
}

impl Default for FarmConfig {
    fn default() -> Self {
        FarmConfig {
            checkpoint_every: 16,
        }
    }
}

/// What a run actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// Item index this run resumed from (0 = fresh job, no checkpoint).
    pub resumed_from: u64,
    /// Work items processed during this run (computed, not skipped).
    pub items_computed: u64,
    /// Whether every item in the source was completed.
    pub completed: bool,
    /// Total artifact bytes after this run.
    pub artifact_len: u64,
    /// FNV-1a64 of the whole artifact after this run.
    pub artifact_fnv: u64,
    /// Number of checkpoints written/saved during this run.
    pub checkpoints_written: u64,
}

/// Errors from a farm run.
#[derive(Debug)]
pub enum FarmError {
    /// The checkpoint store failed.
    Store(StoreError),
    /// The artifact backend failed.
    Io(io::Error),
    /// The durable artifact prefix does not match the checkpoint's recorded
    /// hash/length, so resuming would produce output that differs from a clean
    /// run. The runner refuses instead of silently diverging.
    Corruption {
        /// Recorded prefix length in the checkpoint.
        expected_len: u64,
        /// The artifact's actual prefix length.
        got_len: u64,
        /// FNV-1a64 the checkpoint recorded for its prefix.
        expected_fnv: u64,
        /// FNV-1a64 recomputed over the artifact's prefix.
        got_fnv: u64,
    },
    /// The artifact is shorter than the checkpoint claims the prefix is
    /// (the durable artifact was lost/truncated out from under the store).
    ArtifactShorterThanCheckpoint {
        /// Length the checkpoint promised.
        checkpoint_len: u64,
        /// Actual artifact length.
        artifact_len: u64,
    },
    /// The job source names fewer items than the checkpoint claims are done
    /// (different job spec / wrong seed on resume). Never resume into that.
    CheckpointAheadOfJob {
        /// Items the checkpoint claims complete.
        checkpoint_items: u64,
        /// Total items in the job source.
        job_items: u64,
    },
    /// A fresh job with a source that has zero items.
    NoItems,
}

impl std::fmt::Display for FarmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FarmError::Store(e) => write!(f, "farm store error: {e}"),
            FarmError::Io(e) => write!(f, "farm artifact io error: {e}"),
            FarmError::Corruption {
                expected_len,
                got_len,
                expected_fnv,
                got_fnv,
            } => {
                write!(
                    f,
                    "farm checkpoint corruption: checkpoint prefix len={expected_len} fnv={expected_fnv:#x} but artifact prefix len={got_len} fnv={got_fnv:#x} (byte-equivalence gate failed; refusing to resume)"
                )
            }
            FarmError::ArtifactShorterThanCheckpoint {
                checkpoint_len,
                artifact_len,
            } => {
                write!(
                    f,
                    "farm artifact ({artifact_len} bytes) shorter than checkpoint prefix ({checkpoint_len} bytes); durable output was lost"
                )
            }
            FarmError::CheckpointAheadOfJob {
                checkpoint_items,
                job_items,
            } => {
                write!(
                    f,
                    "farm checkpoint claims {checkpoint_items} items done but job has only {job_items}; job spec changed between runs"
                )
            }
            FarmError::NoItems => write!(f, "farm job has zero work items"),
        }
    }
}

impl std::error::Error for FarmError {}

impl From<StoreError> for FarmError {
    fn from(e: StoreError) -> Self {
        FarmError::Store(e)
    }
}

impl From<io::Error> for FarmError {
    fn from(e: io::Error) -> Self {
        FarmError::Io(e)
    }
}

/// The farm runner. Generic over the checkpoint store backend.
#[derive(Debug)]
pub struct FarmRunner<S: CheckpointStore> {
    store: S,
    job_id: String,
    config: FarmConfig,
}

impl<S: CheckpointStore> FarmRunner<S> {
    /// A runner for `job_id` writing checkpoints into `store`.
    pub fn new(store: S, job_id: impl Into<String>, config: FarmConfig) -> Self {
        let job_id = job_id.into();
        assert!(!job_id.is_empty(), "farm job id must not be empty");
        assert!(config.checkpoint_every > 0, "checkpoint_every must be > 0");
        FarmRunner {
            store,
            job_id,
            config,
        }
    }

    /// The job id this runner owns.
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Run (or resume) the job to **completion** against `artifact`.
    ///
    /// Resumes transparently: if a checkpoint exists for this job it is
    /// verified (byte-equivalence gate), the artifact is truncated to the
    /// checkpoint boundary, and the run continues from `next_item`.
    pub fn run<I: ItemSource, A: Artifact>(
        &mut self,
        items: &I,
        artifact: &mut A,
    ) -> Result<RunReport, FarmError> {
        self.run_bounded(items, artifact, None)
    }

    /// Run with an optional per-attempt item cap.
    ///
    /// `Some(n)` stops after `n` items of *this* attempt (`completed =
    /// false`), simulating a worker eviction mid-job. Everything durable - the
    /// artifact and the latest checkpoint - survives, and the next [`run`](Self::run)
    /// on a fresh runner resumes from the last checkpoint.
    pub fn run_bounded<I: ItemSource, A: Artifact>(
        &mut self,
        items: &I,
        artifact: &mut A,
        max_items: Option<u64>,
    ) -> Result<RunReport, FarmError> {
        let job_items = items.item_count();
        if job_items == 0 {
            return Err(FarmError::NoItems);
        }

        // ---- Resume state from the last checkpoint (if any) --------------
        let latest = self.store.load_latest(&self.job_id)?;
        let (mut next_item, mut running) = match latest {
            Some(ck) => {
                if ck.next_item > job_items {
                    return Err(FarmError::CheckpointAheadOfJob {
                        checkpoint_items: ck.next_item,
                        job_items,
                    });
                }
                self.verify_and_rewind(&ck, artifact)?;
                // running hash continues from the verified prefix bytes.
                let prefix = self.prefix(artifact, ck.output_len)?;
                let mut hasher = Fnv1a64::new();
                hasher.update(&prefix);
                (ck.next_item, hasher)
            }
            None => (0, Fnv1a64::new()),
        };
        let resumed_from = next_item;

        // ---- Streaming execution ----------------------------------------
        let mut checkpoints_written = 0u64;
        let mut processed = 0u64;
        while next_item < job_items {
            if let Some(cap) = max_items {
                if processed >= cap {
                    break; // simulated eviction: stop mid-job, completed=false
                }
            }
            let bytes = items.item_bytes(next_item);
            artifact.append(&bytes)?;
            running.update(&bytes);
            next_item += 1;
            processed += 1;

            if next_item % self.config.checkpoint_every == 0 {
                self.save_checkpoint(next_item, artifact, &running)?;
                checkpoints_written += 1;
            }
        }

        let completed = next_item >= job_items;
        // Persist a final checkpoint when the job finishes cleanly, so a
        // restart after a successful finish also sees next_item == all items.
        if completed {
            self.save_checkpoint(next_item, artifact, &running)?;
            checkpoints_written += 1;
        }

        let artifact_len = artifact.len();
        Ok(RunReport {
            resumed_from,
            items_computed: processed,
            completed,
            artifact_len,
            artifact_fnv: running.finish(),
            checkpoints_written,
        })
    }

    /// Byte-equivalence gate + rewind before resuming.
    fn verify_and_rewind<A: Artifact>(
        &mut self,
        ck: &Checkpoint,
        artifact: &mut A,
    ) -> Result<(), FarmError> {
        let actual = artifact.len();
        if actual < ck.output_len {
            return Err(FarmError::ArtifactShorterThanCheckpoint {
                checkpoint_len: ck.output_len,
                artifact_len: actual,
            });
        }
        let prefix = self.prefix(artifact, ck.output_len)?;
        let got_fnv = fnv1a64(&prefix);
        if got_fnv != ck.output_fnv {
            return Err(FarmError::Corruption {
                expected_len: ck.output_len,
                got_len: prefix.len() as u64,
                expected_fnv: ck.output_fnv,
                got_fnv,
            });
        }
        // Discard anything past the checkpoint boundary (a partial item a
        // dying worker started after the last checkpoint was never counted).
        artifact.truncate(ck.output_len)?;
        Ok(())
    }

    /// Read `len` bytes from the artifact start as a prefix.
    fn prefix<A: Artifact>(&self, artifact: &A, len: u64) -> Result<Vec<u8>, FarmError> {
        let all = artifact.read_all()?;
        let all_len = all.len() as u64;
        if all_len < len {
            return Err(FarmError::ArtifactShorterThanCheckpoint {
                checkpoint_len: len,
                artifact_len: all_len,
            });
        }
        Ok(all[..len as usize].to_vec())
    }

    /// Persist a checkpoint at the current boundary.
    fn save_checkpoint<A: Artifact>(
        &mut self,
        next_item: u64,
        artifact: &A,
        running: &Fnv1a64,
    ) -> Result<(), FarmError> {
        let ck = Checkpoint::new(&self.job_id, next_item, artifact.len(), running.finish());
        self.store.save(&ck)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::VecArtifact;
    use crate::items::SeededItemSource;
    use crate::store::MemoryStore;

    fn runner_with_ckpt_every(n: u64) -> FarmRunner<MemoryStore> {
        FarmRunner::new(
            MemoryStore::new(),
            "test-job",
            FarmConfig {
                checkpoint_every: n,
            },
        )
    }

    #[test]
    fn clean_run_completes_and_checkpoints() {
        let mut r = runner_with_ckpt_every(16);
        let items = SeededItemSource::new(1, 100, 64);
        let mut art = VecArtifact::new();
        let rep = r.run(&items, &mut art).unwrap();
        assert!(rep.completed);
        assert_eq!(rep.resumed_from, 0);
        assert_eq!(rep.items_computed, 100);
        assert_eq!(rep.artifact_len, 100 * 64);
        // Checkpoints at every 16th item over 100 items + final.
        assert_eq!(rep.checkpoints_written, (100 / 16) + 1);
    }

    #[test]
    fn resume_from_last_checkpoint_is_byte_equivalent() {
        let mut fresh_runner = runner_with_ckpt_every(16);
        let items = SeededItemSource::new(42, 200, 32);
        let mut clean = VecArtifact::new();
        let clean_rep = fresh_runner.run(&items, &mut clean).unwrap();
        assert!(clean_rep.completed);

        // Evict the worker after 77 items of a fresh attempt (not a multiple
        // of checkpoint_every), then resume with a brand-new runner sharing the
        // same store.
        let mut store = MemoryStore::new();
        let mut doomed = FarmRunner::new(
            &mut store,
            "job-eq",
            FarmConfig {
                checkpoint_every: 16,
            },
        );
        let mut art = VecArtifact::new();
        let evicted = doomed.run_bounded(&items, &mut art, Some(77)).unwrap();
        assert!(!evicted.completed);
        assert_eq!(evicted.items_computed, 77);
        assert_eq!(evicted.resumed_from, 0);

        // A new worker object over the same store resumes.
        let mut resumed = FarmRunner::new(
            &mut store,
            "job-eq",
            FarmConfig {
                checkpoint_every: 16,
            },
        );
        let rep = resumed.run(&items, &mut art).unwrap();
        assert!(rep.completed);
        assert!(rep.resumed_from > 0);
        // Byte-equivalence: identical artifact bytes to the clean run.
        let clean_bytes = clean.read_all().unwrap();
        let resumed_bytes = art.read_all().unwrap();
        assert_eq!(resumed_bytes, clean_bytes);
        assert_eq!(
            crate::hash::fnv1a64(&resumed_bytes),
            crate::hash::fnv1a64(&clean_bytes)
        );
    }
}
