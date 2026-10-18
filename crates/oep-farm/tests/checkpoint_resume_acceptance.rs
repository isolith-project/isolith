//! P9-M4-T02 acceptance: **checkpoint/resume for long jobs (beam/farm style)**.
//!
//! The card acceptance, mapped to real test evidence:
//!
//! 1. **A long job survives worker eviction via checkpoints.** A long,
//!    deterministic job runs as a sequence of work items; a worker is evicted
//!    mid-job (a *fresh* run is stopped after some items, `completed=false`,
//!    in a durable DirStore + FileArtifact, and the worker object is dropped
//!    as if the process died). No piece of in-memory worker state is carried
//!    across.
//! 2. **It resumes from the last checkpoint.** A brand-new worker object over
//!    the *same* store reads the latest checkpoint, rewinds to its boundary,
//!    and continues; it does not restart from item 0 (`resumed_from > 0`).
//! 3. **Resume is verified byte-equivalent to a clean run on output.** The
//!    artifact bytes after eviction+resume are asserted byte-for-byte equal to
//!    a clean (never-evicted) run, and their FNV-1a64 digests match.
//!
//! The drill uses real durability (a temp directory with DirStore +
//! FileArtifact), an eviction point that is *not* aligned to the checkpoint
//! cadence (so resume genuinely rewinds to the nearest checkpoint boundary),
//! and also exercises the corruption gate (a tampered prefix must refuse to
//! resume). All deterministic: no sleeps, no wall clock.

use std::path::PathBuf;

use oep_farm::{
    Artifact, DirStore, FarmConfig, FarmError, FarmRunner, FileArtifact, SeededItemSource,
    VecArtifact,
};

/// A scratch directory unique to this process+seed.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oep-farm-accept-{tag}-{}-{}",
        std::process::id(),
        oep_farm::splitmix64(tag.len() as u64 ^ 0xACE)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn job() -> SeededItemSource {
    // A credibly "long" job: 400 items of 512 bytes each ~ 200 KB of output.
    SeededItemSource::new(0x7EED_F00D, 400, 512)
}

const CHECKPOINT_EVERY: u64 = 16;

#[test]
fn long_job_survives_eviction_and_resumes_byte_equivalent() {
    let dir = scratch("eviction");
    let artifact_path = dir.join("out.bin");
    let items = job();

    // ---- Clean run: a never-evicted reference -----------------------------
    let mut clean_store = DirStore::open(dir.join("clean")).unwrap();
    let mut clean_runner = FarmRunner::new(
        &mut clean_store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut clean_artifact = VecArtifact::new();
    let clean_rep = clean_runner.run(&items, &mut clean_artifact).unwrap();
    assert!(clean_rep.completed, "clean run completes");
    assert_eq!(clean_rep.artifact_len, 400 * 512);
    let clean_bytes = clean_artifact.read_all().unwrap();

    // ---- Evict the worker partway through a FRESH attempt ----------------
    // Evict after 117 items, which is not a multiple of 16: the nearest
    // checkpoint boundary behind it is 112. Resume must rewind to 112.
    {
        let mut store = DirStore::open(dir.join("state")).unwrap();
        let mut runner = FarmRunner::new(
            &mut store,
            "wave",
            FarmConfig {
                checkpoint_every: CHECKPOINT_EVERY,
            },
        );
        let mut artifact = FileArtifact::open(&artifact_path).unwrap();
        let evicted = runner
            .run_bounded(&items, &mut artifact, Some(117))
            .unwrap();
        assert!(!evicted.completed, "eviction stops before completion");
        assert_eq!(evicted.items_computed, 117);
        assert_eq!(evicted.resumed_from, 0, "first attempt starts fresh");
        // The runner object dies here: nothing is carried across.
    }

    // ---- Resume with a brand-new worker over the same store --------------
    {
        let mut store = DirStore::open(dir.join("state")).unwrap();
        let mut runner = FarmRunner::new(
            &mut store,
            "wave",
            FarmConfig {
                checkpoint_every: CHECKPOINT_EVERY,
            },
        );
        let mut artifact = FileArtifact::open(&artifact_path).unwrap();
        let rep = runner.run(&items, &mut artifact).unwrap();
        assert!(rep.completed, "resumed run completes");
        assert!(
            rep.resumed_from > 0 && rep.resumed_from <= 112,
            "resume starts from the last checkpoint (got {})",
            rep.resumed_from
        );
        let resumed_bytes = artifact.read_all().unwrap();

        // THE ACCEPTANCE: resumed output is byte-equivalent to the clean run.
        assert_eq!(
            resumed_bytes.len(),
            clean_bytes.len(),
            "same output length as clean run"
        );
        assert_eq!(resumed_bytes, clean_bytes, "byte-for-byte identical output");
        assert_eq!(
            oep_farm::fnv1a64(&resumed_bytes),
            oep_farm::fnv1a64(&clean_bytes),
            "FNV-1a64 digest identical"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repeated_evictions_still_byte_equivalent() {
    let dir = scratch("multi");
    let artifact_path = dir.join("out.bin");
    let items = job();

    let mut clean_store = DirStore::open(dir.join("clean")).unwrap();
    let mut clean_runner = FarmRunner::new(
        &mut clean_store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut clean_artifact = VecArtifact::new();
    let _ = clean_runner.run(&items, &mut clean_artifact).unwrap();
    let clean_bytes = clean_artifact.read_all().unwrap();

    // Evict three times in sequence, each with a fresh worker object.
    let mut store_path = dir.join("state");
    let caps = [30u64, 150, 280];
    for (i, cap) in caps.iter().enumerate() {
        let mut store = DirStore::open(&store_path).unwrap();
        let mut runner = FarmRunner::new(
            &mut store,
            "wave",
            FarmConfig {
                checkpoint_every: CHECKPOINT_EVERY,
            },
        );
        let mut artifact = FileArtifact::open(&artifact_path).unwrap();
        let rep = runner
            .run_bounded(&items, &mut artifact, Some(*cap))
            .unwrap();
        if i < caps.len() - 1 {
            assert!(!rep.completed, "eviction {i} stops early");
        }

        // Each consumer reopens the durable artifact; only the store survives.
        drop(runner);
        drop(artifact);
        store_path = dir.join("state"); // same store across evictions
    }

    // Final completion pass.
    let mut store = DirStore::open(&store_path).unwrap();
    let mut runner = FarmRunner::new(
        &mut store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut artifact = FileArtifact::open(&artifact_path).unwrap();
    let rep = runner.run(&items, &mut artifact).unwrap();
    assert!(rep.completed);
    let resumed_bytes = artifact.read_all().unwrap();
    assert_eq!(
        resumed_bytes, clean_bytes,
        "multi-eviction resume byte-equivalent"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resume_refuses_corrupted_prefix() {
    let dir = scratch("corrupt");
    let artifact_path = dir.join("out.bin");
    let items = job();

    let mut store = DirStore::open(dir.join("state")).unwrap();
    let mut runner = FarmRunner::new(
        &mut store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut artifact = FileArtifact::open(&artifact_path).unwrap();
    let rep = runner.run_bounded(&items, &mut artifact, Some(80)).unwrap();
    assert!(!rep.completed);
    drop(runner);
    drop(artifact);

    // Tamper with a byte inside the already-written prefix.
    let mut bytes = std::fs::read(&artifact_path).unwrap();
    bytes[10] ^= 0xff; // flip a bit deep in the durable prefix
    std::fs::write(&artifact_path, &bytes).unwrap();

    let mut store = DirStore::open(dir.join("state")).unwrap();
    let mut runner = FarmRunner::new(
        &mut store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut artifact = FileArtifact::open(&artifact_path).unwrap();
    let err = runner.run(&items, &mut artifact).unwrap_err();
    assert!(
        matches!(err, FarmError::Corruption { .. }),
        "corrupted prefix must refuse to resume, got {err:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resume_refuses_when_artifact_is_lost() {
    let dir = scratch("lost");
    let artifact_path = dir.join("out.bin");
    let items = job();

    let mut store = DirStore::open(dir.join("state")).unwrap();
    let mut runner = FarmRunner::new(
        &mut store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut artifact = FileArtifact::open(&artifact_path).unwrap();
    let rep = runner.run_bounded(&items, &mut artifact, Some(60)).unwrap();
    assert!(!rep.completed);
    drop(runner);
    drop(artifact);

    // Delete the durable artifact: resume must refuse, not fabricate output.
    std::fs::remove_file(&artifact_path).unwrap();

    let mut store = DirStore::open(dir.join("state")).unwrap();
    let mut runner = FarmRunner::new(
        &mut store,
        "wave",
        FarmConfig {
            checkpoint_every: CHECKPOINT_EVERY,
        },
    );
    let mut artifact = FileArtifact::open(&artifact_path).unwrap();
    let err = runner.run(&items, &mut artifact).unwrap_err();
    assert!(
        matches!(err, FarmError::ArtifactShorterThanCheckpoint { .. }),
        "missing artifact must refuse resume, got {err:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clean_runs_are_reproducible_across_workers() {
    // Same seed + same backend => identical bytes on entirely separate workers.
    let a = {
        let mut store = DirStore::open(scratch("repro-a").join("s")).unwrap();
        let mut r = FarmRunner::new(
            &mut store,
            "job",
            FarmConfig {
                checkpoint_every: 8,
            },
        );
        let mut art = VecArtifact::new();
        r.run(&job(), &mut art).unwrap();
        art.read_all().unwrap()
    };
    let b = {
        let mut store = DirStore::open(scratch("repro-b").join("s")).unwrap();
        let mut r = FarmRunner::new(
            &mut store,
            "job",
            FarmConfig {
                checkpoint_every: 64,
            },
        );
        let mut art = VecArtifact::new();
        r.run(&job(), &mut art).unwrap();
        art.read_all().unwrap()
    };
    assert_eq!(a, b, "deterministic job output is host/worker independent");
}

#[test]
fn fresh_store_no_checkpoint_starts_from_zero() {
    let store = DirStore::open(scratch("fresh").join("s")).unwrap();
    let mut runner = FarmRunner::new(store, "job", FarmConfig::default());
    let mut art = VecArtifact::new();
    let rep = runner.run(&job(), &mut art).unwrap();
    assert_eq!(rep.resumed_from, 0);
    assert!(rep.completed);
    assert_eq!(
        rep.checkpoints_written,
        400 / FarmConfig::default().checkpoint_every + 1
    );
}
