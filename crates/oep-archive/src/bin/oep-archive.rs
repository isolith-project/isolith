//! `oep-archive` command-line entry: the P10-M3-T02 export-all / verify-import
//! / retention-policy tool.
//!
//! Usage:
//!
//! ```text
//! oep-archive export-all --vault <store-dir> --out <archive-dir> [--owner <name>]
//! oep-archive verify-import --archive <archive-dir> [--out <report>]
//! oep-archive retention-policy --write <dir>            # emit the defaults
//! oep-archive --help
//! ```
//!
//! Exit codes: `0` success (verify-import only exits 0 when the archive
//! re-imports cleanly), `1` failure, `2` usage error.

use std::path::PathBuf;
use std::process::ExitCode;

use oep_archive::retention::RetentionPolicy;

const USAGE: &str = "\
oep-archive  (P10-M3-T02 export-all interop escape hatch)

USAGE:
  oep-archive export-all --vault <store-dir> --out <archive-dir> [--owner <name>]
  oep-archive verify-import --archive <archive-dir> [--out <report-path>]
  oep-archive retention-policy --write <dir>
  oep-archive --help

EXIT CODES:
  0  success (verify-import: archive re-imports cleanly)
  1  failure
  2  usage error
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    match args[0].as_str() {
        "export-all" => match run_export_all(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "verify-import" => match run_verify_import(&args[1..]) {
            Ok(ok) => {
                if ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "retention-policy" => match run_retention(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("error: unknown subcommand {other:?}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_export_all(args: &[String]) -> Result<(), String> {
    let mut vault: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut owner: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--vault" => vault = Some(next_value(&mut it, "--vault")?),
            "--out" => out = Some(next_value(&mut it, "--out")?),
            "--owner" => owner = Some(next_str(&mut it, "--owner")?),
            other => return Err(format!("unexpected argument {other:?} (see --help)")),
        }
    }
    let vault = vault.ok_or_else(|| "--vault <store-dir> is required (see --help)".to_string())?;
    let out = out.ok_or_else(|| "--out <archive-dir> is required (see --help)".to_string())?;

    let store = oep_pdm::store::VaultStore::load(&vault)
        .map_err(|e| format!("cannot load vault {0}: {e}", vault.display()))?;
    let manifest = oep_archive::export_all(&store, &out, owner.as_deref())
        .map_err(|e| format!("export-all failed: {e}"))?;

    let s = &manifest.summary;
    println!("archive written: {}", out.display());
    println!(
        "  items={} revisions={} files={}",
        s.items, s.revisions, s.files
    );
    println!(
        "  geometry artifacts={} model artifacts={} raw artifacts={} skipped={}",
        s.geometry_artifacts, s.model_artifacts, s.raw_artifacts, s.skipped
    );
    println!("  manifest: {}", out.join("manifest.json").display());
    println!(
        "  vault metadata: {}",
        out.join("vault-metadata.json").display()
    );
    println!(
        "  retention policy: {}",
        out.join("retention-policy.json").display()
    );
    println!("  checksums: {}", out.join("checksums.sha256").display());
    Ok(())
}

fn run_verify_import(args: &[String]) -> Result<bool, String> {
    let mut archive: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--archive" => archive = Some(next_value(&mut it, "--archive")?),
            "--out" => out = Some(next_value(&mut it, "--out")?),
            other => return Err(format!("unexpected argument {other:?} (see --help)")),
        }
    }
    let archive = archive.ok_or_else(|| "--archive <dir> is required (see --help)".to_string())?;

    let report = oep_archive::verify_import(&archive, out.as_deref())
        .map_err(|e| format!("verify-import failed: {e}"))?;
    println!("archive verified: {}", report.summary.ok);
    println!(
        "  vault rebuilt: items={} revisions={} files={}",
        report.vault.items_rebuilt, report.vault.revisions_rebuilt, report.vault.files
    );
    println!(
        "  oep-geo re-import: step={} reimported={} skipped={} failed={}",
        report.summary.total_step,
        report.summary.reimported,
        report.summary.skipped,
        report.summary.failed
    );
    Ok(report.summary.ok)
}

fn run_retention(args: &[String]) -> Result<(), String> {
    let mut dir: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--write" => dir = Some(next_value(&mut it, "--write")?),
            other => return Err(format!("unexpected argument {other:?} (see --help)")),
        }
    }
    let dir = dir.ok_or_else(|| "--write <dir> is required (see --help)".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let policy = RetentionPolicy::defaults();
    let json = policy
        .to_json_pretty()
        .map_err(|e| format!("serialize policy: {e}"))?;
    let path = dir.join("retention-policy.json");
    std::fs::write(&path, json).map_err(|e| format!("write {}: {e}", path.display()))?;
    println!("default retention policy written: {}", path.display());
    println!("  (doc: docs/P10-M3/retention-policy.md -- edit the JSON to override;");
    println!("   deletion is NEVER automatic, even after an override)");
    Ok(())
}

fn next_value<'a>(
    it: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<PathBuf, String> {
    it.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} expects a value"))
}

fn next_str<'a>(it: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<String, String> {
    it.next()
        .cloned()
        .ok_or_else(|| format!("{flag} expects a value"))
}
