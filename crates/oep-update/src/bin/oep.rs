//! `oep` -- the Isolith operator command tree (decision #20 surfacing of the
//! P10-M5-T01 update checker).
//!
//! Today it exposes the update-check surface on top of the `oep-update` crate;
//! further operator subcommands can hang off the same tree. Advisory rule from
//! decision #11 is encoded here and in the crate: an update status NEVER makes
//! a command exit non-zero. Only command-line misuse (unknown subcommand or
//! bad arguments) exits 2.
//!
//! Usage:
//!   oep update status [--config PATH] [--version X.Y.Z]
//!   oep update check [--config PATH] [--version X.Y.Z] [--json]
//!
//! Both run the same live advisory check; `status` prints the human line,
//! `check` prints the human line unless `--json` asks for the serialized
//! [`oep_update::UpdateStatus`]. Config resolution follows
//! [`oep_update::config::resolve`] (env contract) unless `--config` names a
//! file.

use std::path::Path;

use oep_update::config;
use oep_update::{check, UpdateConfig};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    match args.split_first() {
        Some((first, rest)) if first.as_str() == "update" => run_update(rest),
        Some((first, _))
            if first.as_str() == "--help" || first.as_str() == "-h" || first.as_str() == "help" =>
        {
            print_usage();
            0
        }
        Some((other, _)) => {
            eprintln!("oep: unknown command '{other}'");
            print_usage();
            2
        }
        None => {
            print_usage();
            2
        }
    }
}

fn run_update(args: &[String]) -> i32 {
    let Some((sub, rest)) = args.split_first() else {
        eprintln!("oep update: expected 'status' or 'check'");
        print_update_usage();
        return 2;
    };
    match sub.as_str() {
        "status" => run_check(rest, false),
        "check" => run_check(rest, false),
        "--help" | "-h" | "help" => {
            print_update_usage();
            0
        }
        "config" => {
            eprintln!("oep update: 'config' (the settings toggle) is delivered with the update-channel card");
            print_update_usage();
            2
        }
        other => {
            eprintln!("oep update: unknown subcommand '{other}'");
            print_update_usage();
            2
        }
    }
}

/// Shared implementation of `oep update status` and `oep update check`.
/// `--json` forces the serialized status output (only meaningful for check,
/// but harmless either way). Always exits 0 on a status.
fn run_check(args: &[String], json: bool) -> i32 {
    let mut config_path: Option<String> = None;
    let mut version: Option<String> = None;
    let mut emit_json = json;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--version" => {
                i += 1;
                version = args.get(i).cloned();
            }
            "--json" => emit_json = true,
            "--help" | "-h" => {
                print_check_usage();
                return 0;
            }
            other if other.starts_with("--") => {
                eprintln!("oep update: unknown option '{other}'");
                print_check_usage();
                return 2;
            }
            other => {
                eprintln!("oep update: unexpected argument '{other}' (use --config PATH)");
                return 2;
            }
        }
        i += 1;
    }

    let cfg = match &config_path {
        Some(p) => match config::from_path(Path::new(p)) {
            Ok(c) => c,
            Err(e) => {
                // A bad config never blocks: report and run OFF, still exit 0.
                eprintln!("oep update: config error ({e}); running OFF");
                UpdateConfig::default()
            }
        },
        None => config::resolve(),
    };
    let cfg = config::with_current_version(cfg, version.as_deref());

    let status = check(&cfg);
    if emit_json {
        println!("{}", serde_json::to_string_pretty(&status).unwrap());
    } else {
        println!("{}", status.advisory_line());
    }
    // Advisory: never exit non-zero on a status (decision #11).
    0
}

fn print_usage() {
    println!(
        "oep - Isolith operator command tree\n\
         \n\
         usage: oep <command>\n\
         \n\
         commands:\n\
         \x20 update   update-check surface (status / check)\n\
         \n\
         run 'oep update --help' for the update subcommands."
    );
}

fn print_update_usage() {
    println!(
        "oep update - advisory update check (decision #11 / #20)\n\
         \n\
         usage:\n\
         \x20 oep update status [--config PATH] [--version X.Y.Z]\n\
         \x20 oep update check [--config PATH] [--version X.Y.Z] [--json]\n\
         \n\
         An update status never changes the exit code: every status is advisory."
    );
}

fn print_check_usage() {
    println!("usage: oep update check [--config PATH] [--version X.Y.Z] [--json]");
}
