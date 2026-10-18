//! `oep-batch-convert` — P9-M3-T02 batch convert driver CLI.
//!
//! Two verbs:
//!
//! ```text
//!   oep-batch-convert seed <seed.json> [--vault URL]
//!   oep-batch-convert run  <batch.json> [--vault URL] [--report PATH]
//! ```
//!
//! `seed` uploads local real-material files (paths in the seed manifest, dev
//! only) into the P8 vault as revision-1 items. `run` executes every convert
//! job in the batch manifest across the seeded corpus and writes (and prints)
//! the hash-verified [`BatchReport`].
//!
//! Exit codes:
//! ```text
//!   0  ok (or only classified skips)
//!   1  usage / vault / batch error, or at least one job errored
//! ```
//! `seed` and `run` target the same vault: `run --vault` may be omitted when
//! the manifest's `vault` field was supplied at construction.

use std::process::ExitCode;

use oep_pdm::adapter::PdmAdapter;
use oep_worker::batch::{run_batch, seed_from_manifest, BatchManifest, SeedManifest};

const HELP: &str = "\
oep-batch-convert — P9-M3-T02 batch convert driver (dogfood P2/P4 into the P8 vault).

USAGE:
    oep-batch-convert seed <seed.json>  [--vault URL]
    oep-batch-convert run  <batch.json> [--vault URL] [--report PATH]

VERBS:
    seed   Upload the local real-material files named in the seed manifest
           (format 'oep-seed', version 1) into the vault as revision-1 items.
    run    Execute every JobSpec in the batch manifest (format 'oep-batch',
           version 1) across the seeded corpus, store outputs in the vault,
           and emit a hash-verified report (format 'oep-batch-report').

OPTIONS:
    --vault URL   PDM base URL (default: http://127.0.0.1:8787).
    --report PATH Write the run report JSON here (default: ./batch-report.json).

EXIT CODES:
    0  ok (or only classified skips)
    1  usage / vault / batch error, or at least one job errored
";

fn default_vault() -> String {
    "http://127.0.0.1:8787".to_string()
}

fn parse_args(args: &[String]) -> (Vec<String>, std::collections::HashMap<String, String>) {
    let mut opts = std::collections::HashMap::new();
    let mut positionals = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--vault" | "--report" => {
                let key = args[i].clone();
                if i + 1 < args.len() {
                    opts.insert(key, args[i + 1].clone());
                    i += 2;
                } else {
                    opts.insert(key, String::new());
                    i += 1;
                }
            }
            a if a.starts_with("--") => {
                opts.insert(a.to_string(), String::new());
                i += 1;
            }
            a => {
                positionals.push(a.to_string());
                i += 1;
            }
        }
    }
    (positionals, opts)
}

fn opt_value(opts: &std::collections::HashMap<String, String>, key: &str, default: &str) -> String {
    opts.get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return ExitCode::from(0);
    }
    if args.iter().any(|a| a == "--version") {
        println!("oep-batch-convert {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::from(0);
    }
    let (positionals, opts) = parse_args(&args);
    if positionals.is_empty() {
        eprintln!("oep-batch-convert: missing verb (see --help)");
        return ExitCode::from(1);
    }
    let verb = positionals[0].clone();
    let vault_url = opt_value(&opts, "--vault", &default_vault());
    let match_report = opt_value(&opts, "--report", "batch-report.json");

    let mut vault = match PdmAdapter::new(&vault_url) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("oep-batch-convert: cannot bind vault adapter: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = vault.health() {
        eprintln!("oep-batch-convert: vault not healthy at {vault_url}: {e}");
        return ExitCode::from(1);
    }

    match verb.as_str() {
        "seed" => {
            if positionals.len() < 2 {
                eprintln!("oep-batch-convert: seed needs a seed manifest path (see --help)");
                return ExitCode::from(1);
            }
            let seed = match SeedManifest::load(&positionals[1]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("oep-batch-convert: {e}");
                    return ExitCode::from(1);
                }
            };
            match seed_from_manifest(&seed, &mut vault) {
                Ok(rows) => {
                    for (item, rev) in rows {
                        println!("seeded {item} as revision {rev}");
                    }
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("oep-batch-convert: seed failed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "run" => {
            if positionals.len() < 2 {
                eprintln!("oep-batch-convert: run needs a batch manifest path (see --help)");
                return ExitCode::from(1);
            }
            let manifest = match BatchManifest::load(&positionals[1]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("oep-batch-convert: {e}");
                    return ExitCode::from(1);
                }
            };
            match run_batch(&manifest, &mut vault, &vault_url) {
                Ok(report) => {
                    let path = match_report.clone();
                    if let Err(e) = report.save(&path) {
                        eprintln!("oep-batch-convert: could not save report: {e}");
                        return ExitCode::from(1);
                    }
                    print!("{report}");
                    if report.summary.failed == 0 {
                        println!("report written to {path}");
                        ExitCode::from(0)
                    } else {
                        eprintln!(
                            "report written to {path}; {} job(s) errored",
                            report.summary.failed
                        );
                        ExitCode::from(1)
                    }
                }
                Err(e) => {
                    eprintln!("oep-batch-convert: batch failed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("oep-batch-convert: unknown verb '{other}' (see --help)");
            ExitCode::from(1)
        }
    }
}
