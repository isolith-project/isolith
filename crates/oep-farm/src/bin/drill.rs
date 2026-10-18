//! `oep-farm-drill`: run the P9-M4-T02 checkpoint/resume acceptance as a
//! binary so CI can prove it on real material.
//!
//! The drill runs one deterministic long job twice and compares the artifacts
//! byte-for-byte:
//!
//! 1. **clean** - run to completion with durable DirStore + FileArtifact;
//! 2. **evicted+resumed** - a fresh worker is evicted partway (117 items,
//!    deliberately not a checkpoint boundary), then a brand-new worker object
//!    resumes from the last checkpoint and completes.
//!
//! It writes `clean.bin` and `resumed.bin` into the output directory and
//! prints a JSON report:
//!
//! ```json
//! {"clean_len":204800,"resumed_len":204800,"byte_equivalent":true,
//!  "resume_from":112,"fnv_clean":"...","fnv_resumed":"...",
//!  "out_dir":"/..."}
//! ```
//!
//! The Python acceptance test (`tests/test_p9_m4_checkpoint_resume.py`)
//! invokes this binary and additionally SHA-256s both artifacts to confirm
//! byte-equivalence independently of FNV-1a.

use std::path::PathBuf;
use std::process::ExitCode;

use oep_farm::{
    fnv1a64, Artifact, DirStore, FarmConfig, FarmRunner, FileArtifact, ItemSource, SeededItemSource,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut out_dir = std::env::temp_dir().join("oep-farm-drill");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                i += 1;
                if i < args.len() {
                    out_dir = PathBuf::from(&args[i]);
                }
            }
            other => {
                eprintln!("unknown arg {other}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    std::fs::create_dir_all(&out_dir).unwrap();
    let state_dir = out_dir.join("state");
    let _ = std::fs::remove_dir_all(&state_dir);

    let items = SeededItemSource::new(0x7EED_F00D, 400, 512);
    const CKPT_EVERY: u64 = 16;

    // ---- clean run ---------------------------------------------------------
    let clean_artifact_path = out_dir.join("clean.bin");
    let _ = std::fs::remove_file(&clean_artifact_path);
    {
        let mut store = DirStore::open(state_dir.join("clean")).unwrap();
        let mut runner = FarmRunner::new(
            &mut store,
            "wave",
            FarmConfig {
                checkpoint_every: CKPT_EVERY,
            },
        );
        let mut artifact = FileArtifact::open(&clean_artifact_path).unwrap();
        let rep = runner.run(&items, &mut artifact).unwrap();
        assert!(rep.completed, "clean run must complete");
    }

    // ---- eviction + fresh-worker resume ------------------------------------
    let resumed_artifact_path = out_dir.join("resumed.bin");
    let _ = std::fs::remove_file(&resumed_artifact_path);
    let resume_from;
    {
        // First worker: evicted after 117 items (a fresh attempt).
        {
            let mut store = DirStore::open(state_dir.join("run")).unwrap();
            let mut runner = FarmRunner::new(
                &mut store,
                "wave",
                FarmConfig {
                    checkpoint_every: CKPT_EVERY,
                },
            );
            let mut artifact = FileArtifact::open(&resumed_artifact_path).unwrap();
            let evicted = runner
                .run_bounded(&items, &mut artifact, Some(117))
                .unwrap();
            assert!(!evicted.completed, "eviction stops before completion");
        }
        // Second, brand-new worker over the same state dir.
        let mut store = DirStore::open(state_dir.join("run")).unwrap();
        let mut runner = FarmRunner::new(
            &mut store,
            "wave",
            FarmConfig {
                checkpoint_every: CKPT_EVERY,
            },
        );
        let mut artifact = FileArtifact::open(&resumed_artifact_path).unwrap();
        let rep = runner.run(&items, &mut artifact).unwrap();
        assert!(rep.completed, "resumed run must complete");
        resume_from = rep;

        // The durable bytes (post-rewind) must equal the clean run's.
        assert_eq!(
            artifact.len(),
            std::fs::metadata(&clean_artifact_path).unwrap().len()
        );
    }

    let clean_bytes = std::fs::read(&clean_artifact_path).unwrap();
    let resumed_bytes = std::fs::read(&resumed_artifact_path).unwrap();
    let byte_equivalent = clean_bytes == resumed_bytes;
    if !byte_equivalent {
        eprintln!("FATAL: resumed artifact differs from clean run");
        return ExitCode::FAILURE;
    }

    // Hand-rolled JSON (std-only crate: no serde_json dependency).
    let out_dir_json = format!("\"{}\"", out_dir.to_string_lossy().replace('\\', "\\\\"));
    let report = format!(
        "{{\n  \"clean_len\": {},\n  \"resumed_len\": {},\n  \"byte_equivalent\": {},\n  \"resume_from\": {},\n  \"items_total\": {},\n  \"fnv_clean\": \"{:016x}\",\n  \"fnv_resumed\": \"{:016x}\",\n  \"out_dir\": {}\n}}",
        clean_bytes.len(),
        resumed_bytes.len(),
        if byte_equivalent { "true" } else { "false" },
        resume_from.resumed_from,
        items.item_count(),
        fnv1a64(&clean_bytes),
        fnv1a64(&resumed_bytes),
        out_dir_json,
    );
    println!("{report}");
    ExitCode::SUCCESS
}
