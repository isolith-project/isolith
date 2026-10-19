//! P2-M5-T01 acceptance test (TDD, RED first) — `oep-cgr2gltf` CLI with the
//! documented exit-code contract.
//!
//! Written BEFORE the `oep-cgr2gltf` binary existed (RED: `CARGO_BIN_EXE_`
//! resolves to nothing and the assertions fail). The contract, once GREEN, is:
//!
//! 1. The CLI is a real binary named `oep-cgr2gltf` living in the `oep-cgr`
//!    crate (bin target). It parses `INPUT.cgr [-o OUTPUT.glb|.gltf]` plus
//!    `--format`, `--verbose`, `--version`, `--help`.
//! 2. Exit codes are documented and enforced:
//!
//!    ```text
//!    0 = success (a glTF/GLB file was written)
//!    1 = usage error (bad/unknown arguments)
//!    2 = input I/O error (file missing or unreadable)
//!    3 = not a CGR container (header spine mismatch)
//!    4 = classified skip (valid CGR but no usable tessellation found)
//!    5 = output I/O error (cannot write output file)
//!    6 = internal error (extractor produced an invalid mesh)
//!    ```
//!
//!    The CLI NEVER panics/crashes on any input; it prints diagnostics to
//!    stderr and exits with the mapped code.
//! 3. Successful conversion writes a structurally valid GLB (magic `glTF`,
//!    version 2, non-empty JSON chunk with a mesh primitive).
//! 4. Real-material leg: every real sample supplied via `OEP_CGR_SAMPLES`
//!    reaches a *classified* CLI outcome — either success (valid output) or
//!    one of the documented non-zero codes — never a crash/signal. SKIPs
//!    cleanly when no samples are supplied so CI stays green before the
//!    (never-committed, clean-room) corpus sync.
//!
//! Clean-room rule: no proprietary CGR bytes are committed. The deterministic
//! success case uses the SAME synthetic payload pattern the P2-M2-T01 golden
//! test builds (observed header facts + a simple indexed triangle stream), so
//! the identical code path exercised on real corpus material is proven.
//!
//! Scope note: the crate-level glTF/GLB writer card is P2-M2-T03 (in flight).
//! This CLI card commits a minimal, self-contained GLB writer **inside the CLI
//! bin** (`cli_gltf` module) so the CLI is independently verifiable today; when
//! P2-M2-T03 lands its `oep_cgr::gltf` module the CLI should be switched to it
//! (one-line seam change), documented in `docs/re-notes/oep-cgr2gltf-cli.md`.

use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled CLI binary (Cargo provides this env var for bin
/// targets of the same package).
fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_oep-cgr2gltf")
}

/// Locations searched for real samples: `$OEP_CGR_SAMPLES` (colon/comma
/// separated dirs), else `samples`, else `../samples` relative to the crate.
fn sample_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(PathBuf::from(chunk));
            }
        }
    }
    dirs.push(PathBuf::from("samples"));
    dirs.push(PathBuf::from("../samples"));
    dirs
}

fn find_samples() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in sample_dirs() {
        if !d.is_dir() {
            continue;
        }
        let mut entries: Vec<_> = std::fs::read_dir(&d)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "cgr" || x == "CGR"))
                    .collect()
            })
            .unwrap_or_default();
        out.append(&mut entries);
    }
    out.sort();
    out.dedup();
    out
}

/// Run the CLI with the given args; returns (exit_code, stdout, stderr).
fn run_cli(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(cli_bin())
        .args(args)
        .output()
        .expect("spawn CLI");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Build a synthetic (license-clean, observed-facts) CGR-shaped payload that
/// the P2-M2-T01 extractor CAN latch: header spine + a 5x6 f32 grid vertex
/// pool + separator + u16 index stream. Mirrors P2-M2-T01's golden test so the
/// same extraction code path exercises the CLI success path.
fn synthetic_indexed_mesh_payload() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // magic @0x00
    b.extend_from_slice(&[0u8; 0x60]); // zero preamble 0x04..0x63
    b.extend_from_slice(&0x10CD_3600u32.to_le_bytes()); // landscape @0x64
    let template: [u8; 16] = [
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ];
    b.extend_from_slice(&template); // invariant template @0x68
    let cols = 6usize;
    let rows = 5usize;
    for y in 0..rows {
        for x in 0..cols {
            b.extend_from_slice(&(x as f32).to_le_bytes());
            b.extend_from_slice(&(y as f32).to_le_bytes());
            b.extend_from_slice(&0.0_f32.to_le_bytes());
        }
    }
    b.extend_from_slice(&[0xFFu8; 64]); // separator (NaN as f32, out-of-range as u16)
    let mut indices: Vec<u16> = Vec::new();
    for y in 0..(rows - 1) {
        for x in 0..(cols - 1) {
            let a = (y * cols + x) as u16;
            let b_ = ((y + 1) * cols + x) as u16;
            let c = (y * cols + x + 1) as u16;
            indices.extend_from_slice(&[a, b_, c]);
            let c2 = ((y + 1) * cols + x + 1) as u16;
            indices.extend_from_slice(&[b_, c2, c]);
        }
    }
    for i in &indices {
        b.extend_from_slice(&i.to_le_bytes());
    }
    b
}

// --- Part 1: deterministic CLI contract (no real bytes needed) ---

#[test]
fn cli_version_and_help_exit_clean() {
    let (code0, stdout, _) = run_cli(&["--version"]);
    assert_eq!(code0, 0, "--version exits 0; stdout: {stdout}");
    assert!(
        stdout.contains("oep-cgr2gltf"),
        "version names the program: {stdout}"
    );

    let (code1, stdout, _) = run_cli(&["--help"]);
    assert_eq!(code1, 0, "--help exits 0");
    assert!(
        stdout.to_lowercase().contains("usage"),
        "--help prints usage: {stdout}"
    );
    // The exit-code contract must be documented in --help.
    assert!(
        stdout.to_lowercase().contains("exit code"),
        "--help documents exit codes: {stdout}"
    );
}

#[test]
fn cli_usage_error_is_exit_1() {
    for bad in [
        &["a.cgr", "b.glb", "c.glb"][..], // too many positionals
        &["--nonsense"][..],              // unknown flag
        &[][..],                          // no args at all
    ] {
        let (code, _so, se) = run_cli(bad);
        assert_eq!(code, 1, "usage error exit 1 for {bad:?}; stderr: {se}");
    }
}

#[test]
fn cli_missing_input_is_exit_2() {
    let tmp = std::env::temp_dir().join("oep-cgr2gltf-definitely-missing.cgr");
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    let (code, _so, se) = run_cli(&[tmp.to_str().unwrap()]);
    assert_eq!(code, 2, "missing input -> exit 2; stderr: {se}");
}

#[test]
fn cli_non_cgr_input_is_exit_3() {
    let dir = std::env::temp_dir().join("oep-cgr2gltf-t3");
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("not_a_cgr.bin");
    std::fs::write(&path, [0xde, 0xad, 0xbe, 0xef, 0x00, 0x01, 0x02]).expect("write junk");
    let (code, _so, se) = run_cli(&[path.to_str().unwrap()]);
    assert_eq!(code, 3, "non-CGR bytes -> exit 3; stderr: {se}");
}

#[test]
fn cli_success_writes_valid_glb_from_synthetic_mesh() {
    let dir = std::env::temp_dir().join("oep-cgr2gltf-ok");
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let input = dir.join("synthetic.cgr");
    let output = dir.join("out.glb");
    std::fs::write(&input, synthetic_indexed_mesh_payload()).expect("write synthetic");
    let (code, _so, se) = run_cli(&[input.to_str().unwrap(), "-o", output.to_str().unwrap()]);
    assert_eq!(code, 0, "synthetic mesh -> exit 0; stderr: {se}");
    assert!(output.exists(), "output file written");
    let bytes = std::fs::read(&output).expect("read glb");
    assert!(bytes.starts_with(b"glTF"), "GLB magic present");
    assert_eq!(&bytes[4..8], &2u32.to_le_bytes(), "GLB version 2");
    // A minimal structural check: JSON chunk type 0x4E4F534A ('JSON') appears.
    let json_chunk_present = bytes.windows(4).any(|w| w == 0x4E4F534Au32.to_le_bytes());
    assert!(json_chunk_present, "JSON chunk present in glb");
}

// --- Part 2: real material (CI-safe skip when corpus absent) ---

#[test]
fn real_material_every_sample_classified_outcome() {
    let samples = find_samples();
    if samples.is_empty() {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    }
    let dir = std::env::temp_dir().join("oep-cgr2gltf-real");
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let mut n_ok = 0usize;
    let mut n_classified = 0usize;
    for p in &samples {
        let out = dir.join(format!(
            "out_{}.glb",
            p.file_stem().unwrap_or_default().to_string_lossy()
        ));
        let (code, _so, se) = run_cli(&[p.to_str().unwrap(), "-o", out.to_str().unwrap()]);
        assert!(
            code == 0 || (1..=6).contains(&code),
            "CLI must never crash; got exit {} on {:?}; stderr: {se}",
            code,
            p
        );
        match code {
            0 => {
                n_ok += 1;
                assert!(out.exists(), "success writes output for {:?}", p);
                let b = std::fs::read(&out).expect("read output");
                assert!(b.starts_with(b"glTF"), "output is a GLB for {:?}", p);
            }
            // 2 input IO, 3 not-a-CGR, 4 classified skip, 5 output IO, 6 internal:
            // any of these is a *classified* outcome; the real corpus may legitimately
            // produce skip code 4 while the extractor under test stays honest.
            2..=6 => {
                n_classified += 1;
                eprintln!(
                    "  classified (exit {code}) {:?}: {}",
                    p.file_name().unwrap_or_default(),
                    se.trim()
                );
            }
            _ => panic!("unexpected exit code {code}"),
        }
    }
    assert_eq!(
        n_ok + n_classified,
        samples.len(),
        "every sample reaches a classified outcome (no hangs/crashes)"
    );
    eprintln!(
        "  summary: {} success, {} classified of {} samples",
        n_ok,
        n_classified,
        samples.len()
    );
}
