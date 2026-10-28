//! `oep` -- the Isolith operator command tree (decision #20 surfacing of the
//! P10-M5-T01 update checker, UPDATE-CHANNEL config toggle).
//!
//! Today it exposes the update-check surface on top of the `oep-update` crate;
//! further operator subcommands can hang off the same tree. Advisory rule from
//! decision #11 is encoded here and in the crate: an update status NEVER makes
//! a command exit non-zero. Only command-line misuse (unknown subcommand or
//! bad arguments) exits 2; a config-write failure exits 1.
//!
//! Usage:
//!   oep update status [--config PATH] [--version X.Y.Z]
//!   oep update check [--config PATH] [--version X.Y.Z] [--json]
//!   oep update config [--status] [--settings PATH]
//!   oep update config --enable [--source SPEC] [--settings PATH]
//!   oep update config --disable [--settings PATH]
//!   oep update config --source SPEC [--settings PATH]
//!   oep update config --accept-offer COND [--settings PATH] [--config PATH]
//!   oep update config --decline-offer COND [--settings PATH]
//!
//! `SPEC` is one of `off`, `url:<URL>`, `mirror:<PATH>`, `bundle:<PATH>`
//! (a bare `http(s)://` is treated as `url:`).
//!
//! Both status/check run the same live advisory check; `status` prints the
//! human line, `check` prints the human line unless `--json` asks for the
//! serialized [`oep_update::UpdateStatus`]. Config resolution follows
//! [`oep_update::config::resolve`] (env contract + persisted settings + build
//! default) unless `--config` names a file. `config` edits the persisted
//! settings file (UPDATE-CHANNEL), which is exactly what the tri-state
//! settings toggle in oep-web writes too.

use std::path::Path;

use oep_update::config;
use oep_update::config::{write_settings, UpdateChannelState};
use oep_update::offer::{offer_path_from, OfferAnswer, OfferCondition, OfferLedger};
use oep_update::{check, UpdateConfig, UpdateSource};

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
        eprintln!("oep update: expected 'status', 'check' or 'config'");
        print_update_usage();
        return 2;
    };
    match sub.as_str() {
        "status" => run_check(rest, false),
        "check" => run_check(rest, false),
        "config" => run_config(rest),
        "--help" | "-h" | "help" => {
            print_update_usage();
            0
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
    // UPDATE-CHANNEL one-off failure offer: when the live check returns
    // Unreachable or Untrusted and the condition has no stored answer, the
    // program offers ONCE to turn update checking off (see offer.rs). Still
    // never exits non-zero.
    if !emit_json {
        if let Some(cond) = OfferCondition::from_status(&status) {
            let offers = offer_path_from(&|k| std::env::var(k).ok());
            if let Some(p) = &offers {
                let ledger = OfferLedger::load(p);
                if ledger.should_offer(cond) {
                    println!(
                        "update check: offer: turn update checking off for '{}'? run 'oep update config --disable' to accept, or 'oep update config --decline-offer {}'",
                        cond.key(),
                        cond.key()
                    );
                }
            }
        }
    }
    // Advisory: never exit non-zero on a status (decision #11).
    0
}

/// `oep update config` - the UPDATE-CHANNEL settings toggle. Reads/writes the
/// persisted settings file (the same one the oep-web settings toggle uses).
fn run_config(args: &[String]) -> i32 {
    let mut status_only = false;
    let mut enable = false;
    let mut disable = false;
    let mut source_spec: Option<String> = None;
    let mut settings_path: Option<String> = None;
    let mut explicit_config: Option<String> = None;
    let mut accept_offer: Option<String> = None;
    let mut decline_offer: Option<String> = None;
    let get = |k: &str| std::env::var(k).ok();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--status" => status_only = true,
            "--enable" => enable = true,
            "--disable" => disable = true,
            "--source" => {
                i += 1;
                source_spec = args.get(i).cloned();
            }
            "--settings" => {
                i += 1;
                settings_path = args.get(i).cloned();
            }
            "--config" => {
                i += 1;
                explicit_config = args.get(i).cloned();
            }
            "--accept-offer" => {
                i += 1;
                accept_offer = args.get(i).cloned();
            }
            "--decline-offer" => {
                i += 1;
                decline_offer = args.get(i).cloned();
            }
            "--help" | "-h" => {
                print_update_usage();
                return 0;
            }
            other if other.starts_with("--") => {
                eprintln!("oep update config: unknown option '{other}'");
                print_update_usage();
                return 2;
            }
            other => {
                eprintln!("oep update config: unexpected argument '{other}'");
                print_update_usage();
                return 2;
            }
        }
        i += 1;
    }

    // Resolve the settings file path: explicit --settings wins, else the
    // resolved config seam (OEP_UPDATE_SETTINGS, XDG/HOME default).
    let settings = match &settings_path {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => config::settings_path_from(&get),
    };
    // For --accept-offer / --decline-offer the offer ledger path.
    let offers = offer_path_from(&get);

    // No mutation requested: status.
    if status_only
        || (source_spec.is_none()
            && !enable
            && !disable
            && accept_offer.is_none()
            && decline_offer.is_none())
    {
        let resolved = if let Some(cfgp) = &explicit_config {
            match config::from_path(Path::new(cfgp)) {
                Ok(c) => config::ResolvedConfig {
                    state: state_for(&c),
                    config: c,
                },
                Err(e) => {
                    eprintln!("oep update: config error ({e}); reporting OFF");
                    config::ResolvedConfig {
                        state: UpdateChannelState::OffNeverConfigured,
                        config: UpdateConfig::default(),
                    }
                }
            }
        } else {
            config::resolve_full(&get)
        };
        print_state(&resolved.state, &resolved.config);
        return 0;
    }

    // Validate: enable and disable are mutually exclusive.
    if enable && disable {
        eprintln!("oep update config: --enable and --disable are mutually exclusive");
        return 2;
    }

    // Offer answers: accept = disable + ledger accept; decline = ledger only.
    if let Some(cond_key) = accept_offer {
        return accept_offer_cmd(&cond_key, &settings, &offers, &get);
    }
    if let Some(cond_key) = decline_offer {
        return decline_offer_cmd(&cond_key, &offers, &get);
    }

    if source_spec.is_none() && !enable && !disable {
        eprintln!("oep update config: expected an action (--status/--enable/--disable/--source/--accept-offer/--decline-offer)");
        return 2;
    }

    // Read current effective config so enable-without-source keeps the
    // existing source (or falls to the build default).
    let current = if let Some(cfgp) = &explicit_config {
        config::from_path(Path::new(cfgp)).unwrap_or_default()
    } else {
        config::resolve()
    };

    let source = match &source_spec {
        // explicit source: parse it.
        Some(spec) => match parse_source(spec) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("oep update config: {e}");
                return 2;
            }
        },
        // enable without a source: keep the current source if any, else the
        // build default (public URL), else Off.
        None => match &current.source {
            UpdateSource::Off => config::build_default_config()
                .map(|c| c.source)
                .unwrap_or(UpdateSource::Off),
            s => s.clone(),
        },
    };

    let path = match &settings {
        Some(p) => p.clone(),
        None => {
            eprintln!("oep update config: no settings path resolved (set OEP_UPDATE_SETTINGS or HOME/XDG_CONFIG_HOME)");
            return 1;
        }
    };

    if let Err(e) = write_settings(&path, !disable, &source) {
        eprintln!(
            "oep update config: cannot write settings {}: {e}",
            path.display()
        );
        return 1;
    }
    println!(
        "update check: {} (settings {})",
        if disable { "disabled" } else { "enabled" },
        path.display()
    );
    0
}

/// Build-default-aware state (used when reporting an explicit config file).
fn state_for(cfg: &UpdateConfig) -> UpdateChannelState {
    if cfg.enabled {
        match &cfg.source {
            UpdateSource::Off => UpdateChannelState::On {
                source: UpdateSource::Off,
            },
            src => UpdateChannelState::On {
                source: src.clone(),
            },
        }
    } else {
        UpdateChannelState::OffByUser
    }
}

fn accept_offer_cmd(
    cond_key: &str,
    settings: &Option<std::path::PathBuf>,
    offers: &Option<std::path::PathBuf>,
    get: &dyn Fn(&str) -> Option<String>,
) -> i32 {
    let Some(cond) = parse_condition(cond_key) else {
        eprintln!("oep update config: unknown offer condition '{cond_key}' (expected unreachable|untrusted)");
        return 2;
    };
    let ledger_path = match offers {
        Some(p) => p.clone(),
        None => match config::settings_path_from(get) {
            Some(p) => p.with_file_name("update-offers.json"),
            None => {
                eprintln!("oep update config: no offers path resolved");
                return 1;
            }
        },
    };
    let mut ledger = OfferLedger::load(&ledger_path);
    ledger.record(cond, OfferAnswer::Accepted);

    // Disable update checking (persist enabled:false, keep the source so a
    // later re-enable remembers it).
    let path = match settings {
        Some(p) => p.clone(),
        None => match config::settings_path_from(get) {
            Some(p) => p,
            None => {
                eprintln!("oep update config: no settings path resolved");
                return 1;
            }
        },
    };
    let current = config::resolve();
    if let Err(e) = write_settings(&path, false, &current.source) {
        eprintln!(
            "oep update config: cannot write settings {}: {e}",
            path.display()
        );
        return 1;
    }
    println!(
        "update check: disabled (offer accepted for '{}')",
        cond.key()
    );
    0
}

fn decline_offer_cmd(
    cond_key: &str,
    offers: &Option<std::path::PathBuf>,
    get: &dyn Fn(&str) -> Option<String>,
) -> i32 {
    let Some(cond) = parse_condition(cond_key) else {
        eprintln!("oep update config: unknown offer condition '{cond_key}' (expected unreachable|untrusted)");
        return 2;
    };
    let ledger_path = match offers {
        Some(p) => p.clone(),
        None => match config::settings_path_from(get) {
            Some(p) => p.with_file_name("update-offers.json"),
            None => {
                eprintln!("oep update config: no offers path resolved");
                return 1;
            }
        },
    };
    let mut ledger = OfferLedger::load(&ledger_path);
    ledger.record(cond, OfferAnswer::Declined);
    println!(
        "update check: offer declined for '{}' (will not re-ask for this condition)",
        cond.key()
    );
    0
}

fn parse_condition(s: &str) -> Option<OfferCondition> {
    match s.trim() {
        "unreachable" => Some(OfferCondition::Unreachable),
        "untrusted" => Some(OfferCondition::Untrusted),
        _ => None,
    }
}

/// Print the tri-state config status (exit-independent human text).
fn print_state(state: &UpdateChannelState, cfg: &UpdateConfig) {
    match state {
        UpdateChannelState::On { source } => match source {
            UpdateSource::Off => println!("update check: ON (no source configured)"),
            UpdateSource::Mirror(p) => {
                println!("update check: ON (source: mirror {})", p.display())
            }
            UpdateSource::RemoteUrl(u) => println!("update check: ON (source: {u})"),
            UpdateSource::Bundle(p) => {
                println!("update check: ON (source: bundle {})", p.display())
            }
        },
        UpdateChannelState::OffByUser => println!("update check: OFF (disabled by user)"),
        UpdateChannelState::OffNeverConfigured => {
            let _ = cfg;
            println!("update check: OFF (never configured)");
        }
    }
}

/// Parse a `--source` spec into an [`UpdateSource`].
fn parse_source(spec: &str) -> Result<UpdateSource, String> {
    let s = spec.trim();
    if s == "off" || s == "Off" || s == "OFF" || s.is_empty() {
        return Ok(UpdateSource::Off);
    }
    if let Some(url) = s.strip_prefix("url:") {
        let url = url.trim();
        if url.is_empty() {
            return Err("--source url:<URL> needs a URL".to_string());
        }
        return Ok(UpdateSource::RemoteUrl(url.to_string()));
    }
    if let Some(p) = s.strip_prefix("mirror:") {
        let p = p.trim();
        if p.is_empty() {
            return Err("--source mirror:<PATH> needs a path".to_string());
        }
        return Ok(UpdateSource::Mirror(Path::new(p).to_path_buf()));
    }
    if let Some(p) = s.strip_prefix("bundle:") {
        let p = p.trim();
        if p.is_empty() {
            return Err("--source bundle:<PATH> needs a path".to_string());
        }
        return Ok(UpdateSource::Bundle(Path::new(p).to_path_buf()));
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Ok(UpdateSource::RemoteUrl(s.to_string()));
    }
    Err(format!(
        "--source spec '{spec}' must be off|url:URL|mirror:PATH|bundle:PATH|http(s)://URL"
    ))
}

fn print_usage() {
    println!(
        "oep - Isolith operator command tree\n\
         \n\
         usage: oep <command>\n\
         \n\
         commands:\n\
         \x20 update   update-check surface (status / check / config)\n\
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
         \x20 oep update config [--status] [--settings PATH]\n\
         \x20 oep update config --enable [--source SPEC] [--settings PATH]\n\
         \x20 oep update config --disable [--settings PATH]\n\
         \x20 oep update config --source SPEC [--settings PATH]\n\
         \x20 oep update config --accept-offer unreachable|untrusted\n\
         \x20 oep update config --decline-offer unreachable|untrusted\n\
         \n\
         An update status never changes the exit code: every status is advisory.\n\
         SPEC: off | url:URL | mirror:PATH | bundle:PATH | http(s)://URL"
    );
}

fn print_check_usage() {
    println!("usage: oep update check [--config PATH] [--version X.Y.Z] [--json]");
}
