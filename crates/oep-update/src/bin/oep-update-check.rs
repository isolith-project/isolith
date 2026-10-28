//! `oep-update-check` -- the P10-M5-T01 update checker CLI (decision #11).
//!
//! Advisory and NON-BLOCKING: it prints a status line and ALWAYS exits 0
//! (except command-line misuse). A launcher may call it at startup and start
//! the stack regardless of what it prints; there is no status that blocks
//! launch.
//!
//! Usage:
//!   oep-update-check [--config PATH] [--version X.Y.Z] [--json]
//!
//! Config file shape matches workstreams/delivery/update-checker/update-check.sample.json. With no config
//! file (the default deployment) the checker performs NO I/O and reports
//! "off". The richer `oep update status|check` command tree (decision #20)
//! lives in the `oep` binary in this crate.

use std::path::PathBuf;

use oep_update::config;
use oep_update::{check, UpdateConfig};

#[derive(Debug, Default)]
struct Opts {
    config: Option<PathBuf>,
    version: Option<String>,
    json: bool,
}

fn main() {
    let opts = parse_args(std::env::args().skip(1));
    let cfg = load_config(&opts);
    let status = check(&cfg);
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&status).unwrap());
    } else {
        println!("{}", status.advisory_line());
    }
    // Advisory: every status (including Unreachable/Untrusted) exits 0 so an
    // update check can never gate a launch.
}

fn parse_args<I: Iterator<Item = String>>(args: I) -> Opts {
    let mut o = Opts::default();
    let mut it = args;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => o.config = Some(PathBuf::from(it.next().unwrap_or_default())),
            "--version" => o.version = Some(it.next().unwrap_or_default()),
            "--json" => o.json = true,
            "--help" | "-h" => {
                println!("usage: oep-update-check [--config PATH] [--version X.Y.Z] [--json]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    o
}

fn load_config(opts: &Opts) -> UpdateConfig {
    let cfg = match &opts.config {
        None => UpdateConfig::default(), // default deployment: fully off
        Some(p) => match config::from_path(p) {
            Ok(c) => c,
            Err(e) => {
                // A bad/missing config must not block launch either: fall back
                // to off and report why, still exit 0.
                eprintln!("oep-update-check: config error ({e}); running OFF");
                UpdateConfig::default()
            }
        },
    };
    config::with_current_version(cfg, opts.version.as_deref())
}
