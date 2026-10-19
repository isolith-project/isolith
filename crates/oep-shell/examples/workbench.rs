//! Machine-callable OEP interchange workbench CLI (P7-M2-T01, help surface
//! P7-M5-T02).
//!
//! Drives the same [`oep_shell::workbench::Workbench`] the shell panel will
//! host, one command per argument so the surface is scriptable and
//! headless-testable (the P7-M5-T01 CLI-parity target). A PDM client can be
//! bound with `--pdm <json-file>` later; by default each process gets a fresh
//! in-memory local stub, so `save-pdm` followed by `open-pdm` in the same
//! invocation round-trips (persistence is a P8 deliverable).
//!
//! The help surface is GENERATED from the command metadata
//! ([`oep_shell::commands`], single source of truth): `help`, `help:NAME`,
//! `help:--json` and `--markdown` all render from the registry, and
//! `--write-help-index <path>` regenerates the committed markdown index. No
//! hand-written usage block lives here, so the help cannot drift from the
//! surface.
//!
//! Usage:
//!
//! ```text
//! oep-workbench <command> [<command> ...]
//!
//!   load:cgr:PATH        load a CGR file (classic .cgr or modern V5_CFV2)
//!   load:3dxml:PATH      load a 3DXML file (ZIP or standalone XML)
//!   load:step:PATH       load an ISO 10303-21 STEP file
//!   open-pdm:ITEM:REV    open a PDM vault revision into the workbench
//!   save-pdm:ITEM:REV[:SUMMARY]
//!                        publish the current document to the PDM vault
//!   save:cgr|3dxml|step  request an interchange save lane (honest pending)
//!   journal              print the operation journal
//!   help                 print the generated command index
//!   help:NAME            print the full help for one command
//!   help:--json          print the machine-readable help index (JSON)
//!   --markdown           print the generated markdown help index
//!   --write-help-index:PATH
//!                        regenerate the committed markdown help index
//! ```
//!
//! Example:
//!
//! ```text
//! oep-workbench load:step:box-reference.step save-pdm:BOX-01:A:initial open-pdm:BOX-01:A journal
//! ```
//!
//! Exit code: 0 when every command succeeded, 1 when any command Failed or
//! the invocation was empty.

use oep_shell::commands;
use oep_shell::workbench::{Interchange, Workbench, WorkbenchAction};

fn print_outcome(outcome: &oep_shell::workbench::WorkbenchOutcome) -> bool {
    match outcome {
        oep_shell::workbench::WorkbenchOutcome::Loaded {
            format,
            stats,
            note,
            ..
        } => {
            println!(
                "loaded {format}: parts={} bodies={} features={} assemblies={}{}",
                stats.parts,
                stats.bodies,
                stats.features,
                stats.assemblies,
                note.as_deref()
                    .map(|n| format!(" [{n}]"))
                    .unwrap_or_default()
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::OpenedFromPdm { stats, .. } => {
            println!(
                "opened from PDM: parts={} bodies={} features={}",
                stats.parts, stats.bodies, stats.features
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::SavedToPdm { revision } => {
            println!(
                "saved to PDM: {}@{} state={} digest={}",
                revision.item_id,
                revision.revision,
                revision.state,
                revision.blob_digest.as_deref().unwrap_or("none")
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::InterchangeSavePending { format, reason } => {
            println!("{format} save pending: {reason}");
            true
        }
        oep_shell::workbench::WorkbenchOutcome::Skipped { format, reason } => {
            println!("{format} load skipped: {reason}");
            true
        }
        oep_shell::workbench::WorkbenchOutcome::Failed { message } => {
            println!("failed: {message}");
            false
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print!("{}", commands::render_index());
        std::process::exit(1);
    }

    let mut workbench = Workbench::with_local_vault();
    let mut ok = true;

    for arg in &args {
        // Pure help-surface tokens: all generated from the command metadata.
        if arg == "help" || arg == "--help" || arg == "-h" {
            print!("{}", commands::render_index());
            continue;
        }
        if let Some(name) = arg.strip_prefix("help:") {
            if name == "--json" {
                println!("{}", commands::render_index_json());
            } else if let Some(doc) = commands::render_help(name) {
                print!("{doc}");
            } else {
                println!(
                    "failed: unknown command to document '{name}'; run 'help' for the full index"
                );
                ok = false;
            }
            continue;
        }
        if arg == "--markdown" {
            print!("{}", commands::render_index_markdown());
            continue;
        }
        if let Some(path) = arg.strip_prefix("--write-help-index:") {
            match std::fs::write(path, commands::render_index_markdown()) {
                Ok(()) => println!("wrote help index to {path}"),
                Err(e) => {
                    println!("failed: cannot write {path}: {e}");
                    ok = false;
                }
            }
            continue;
        }

        // Split on the FIRST ':' only; summaries may contain ':'.
        let (cmd, rest) = match arg.split_once(':') {
            Some((c, r)) => (c, Some(r)),
            None => (arg.as_str(), None),
        };
        // Validate the token against the metadata before dispatch so an
        // unknown command fails with an actionable message naming the token.
        if let Err(message) = commands::validate_token(cmd) {
            println!("failed: {message}");
            ok = false;
            continue;
        }
        match (cmd, rest) {
            ("load", Some(arg)) => {
                let fmt = arg.split(':').next().unwrap_or_default();
                let path = arg.get(fmt.len().saturating_add(1)..).unwrap_or_default();
                let format = match fmt {
                    "cgr" => Interchange::Cgr,
                    "3dxml" => Interchange::ThreeDXml,
                    "step" => Interchange::Step,
                    other => {
                        println!(
                            "failed: unknown load format '{other}'; expected cgr, 3dxml or step"
                        );
                        ok = false;
                        continue;
                    }
                };
                let bytes = match std::fs::read(path) {
                    Ok(b) => b,
                    Err(e) => {
                        println!("failed: cannot read {path}: {e}");
                        ok = false;
                        continue;
                    }
                };
                let out = workbench.run(WorkbenchAction::Load { format, bytes });
                ok &= print_outcome(&out);
            }
            ("save-pdm", Some(arg)) => {
                let mut parts = arg.splitn(2, ':');
                let item = parts.next().unwrap_or_default().to_string();
                let rest = parts.next().unwrap_or_default();
                let (revision, summary) = rest
                    .split_once(':')
                    .map(|(r, s)| (r.to_string(), s.to_string()))
                    .unwrap_or((rest.to_string(), String::new()));
                let out = workbench.run(WorkbenchAction::SaveToPdm {
                    item_id: item,
                    revision,
                    summary,
                });
                ok &= print_outcome(&out);
            }
            ("save", Some(format)) => {
                let out = workbench.run(WorkbenchAction::Save {
                    format: match format {
                        "cgr" => Interchange::Cgr,
                        "3dxml" => Interchange::ThreeDXml,
                        "step" => Interchange::Step,
                        other => {
                            println!(
                                "failed: unknown save format '{other}'; expected cgr, 3dxml or step"
                            );
                            ok = false;
                            continue;
                        }
                    },
                });
                ok &= print_outcome(&out);
            }
            ("open-pdm", Some(arg)) => {
                let mut parts = arg.splitn(2, ':');
                let item = parts.next().unwrap_or_default().to_string();
                let revision = parts.next().unwrap_or_default().to_string();
                let out = workbench.run(WorkbenchAction::OpenFromPdm {
                    item_id: item,
                    revision,
                });
                ok &= print_outcome(&out);
            }
            ("journal", None) => {
                for line in workbench.journal() {
                    println!("{line}");
                }
            }
            (other, _) => {
                // Unreachable for unknown tokens (validated above); kept so a
                // known token can never silently fall through as success.
                println!("failed: unsupported command '{other}'; run 'help' for the full index");
                ok = false;
            }
        }
    }

    std::process::exit(if ok { 0 } else { 1 });
}
