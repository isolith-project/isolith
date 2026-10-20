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
//! "off".

use std::path::PathBuf;

use oep_update::{check, UpdateConfig, UpdateSource};

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
        println!("{}", status_line(&status));
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
        Some(p) => match ConfigFile::from_path(p) {
            Ok(c) => c.into_update_config(opts.version.as_deref()),
            Err(e) => {
                // A bad/missing config must not block launch either: fall back
                // to off and report why, still exit 0.
                eprintln!("oep-update-check: config error ({e}); running OFF");
                UpdateConfig::default()
            }
        },
    };
    cfg
}

/// Mirrors workstreams/delivery/update-checker/update-check.sample.json.
#[derive(serde::Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    source: Option<ConfigSource>,
    #[serde(default)]
    public_key: Option<String>,
    #[serde(default)]
    current_version: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ConfigSource {
    Mirror { mirror: String },
    Remote { url: String },
    Bundle { bundle: String },
}

impl ConfigFile {
    fn from_path(p: &std::path::Path) -> Result<ConfigFile, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))
    }

    fn into_update_config(self, version_override: Option<&str>) -> UpdateConfig {
        let source = match self.source {
            None => UpdateSource::Off,
            Some(ConfigSource::Mirror { mirror }) => UpdateSource::Mirror(PathBuf::from(mirror)),
            Some(ConfigSource::Remote { url }) => UpdateSource::RemoteUrl(url),
            Some(ConfigSource::Bundle { bundle }) => UpdateSource::Bundle(PathBuf::from(bundle)),
        };
        let public_key = self.public_key.and_then(|h| hex32(&h)).or_else(|| {
            eprintln!("oep-update-check: public_key must be 64 hex chars; source untrusted");
            None::<[u8; 32]>
        });
        UpdateConfig {
            enabled: self.enabled,
            source,
            public_key,
            current_version: version_override
                .map(|s| s.to_string())
                .or(self.current_version)
                .unwrap_or_else(|| "0.0.0".to_string()),
            timeout_ms: self.timeout_ms.unwrap_or(3000),
        }
    }
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16)? as u8;
        let lo = (chunk[1] as char).to_digit(16)? as u8;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn status_line(s: &oep_update::UpdateStatus) -> String {
    match s {
        oep_update::UpdateStatus::Off => {
            "update check: OFF (no source configured; non-phoning default)".to_string()
        }
        oep_update::UpdateStatus::Unreachable { detail } => {
            format!("update check: unavailable ({detail})")
        }
        oep_update::UpdateStatus::Untrusted { detail } => {
            format!("update check: source NOT TRUSTED - no payloads consumed ({detail})")
        }
        oep_update::UpdateStatus::UpToDate => "update check: up to date".to_string(),
        oep_update::UpdateStatus::UpdateAvailable { version, .. } => {
            format!("update check: update available -> {version} (advisory; install is a separate operator action)")
        }
    }
}
