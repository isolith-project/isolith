//! Update-check configuration resolution for the surfaces (decision #20).
//!
//! One seam every surface uses to obtain its [`UpdateConfig`] - the workbench
//! startup check, the oep-web banner, and the `oep update` CLI all call
//! [`resolve`] (or [`from_path`] with an explicit file) and never construct
//! configs by hand.
//!
//! Resolution order (first hit wins):
//!
//! 1. `OEP_UPDATE_CONFIG`: a path to a JSON config file (the
//!    `update-check.sample.json` shape). Missing or unparsable falls through
//!    with a logged reason - never a launch failure.
//! 2. `OEP_UPDATE_CHECK=on` plus the air-gap `.env` source variables
//!    (`OEP_UPDATE_MIRROR_URL` for an on-site mirror or
//!    `OEP_UPDATE_OFFLINE_BUNDLE` for an offline bundle; `OEP_UPDATE_PUBKEY`
//!    for the 64-hex Ed25519 verifying key). This is the same variable
//!    contract `scripts/airgap/*` and the stack `.env` already generate.
//! 3. The default [`UpdateConfig`]: `enabled=false`, source `Off`, zero I/O.
//!
//! Hard rule (decision #11, beyond this module): a config with `enabled=false`
//! or source `Off` performs NO I/O of any kind - proven by the gating tests in
//! `crates/oep-update/tests/update_surface_gating.rs`.

use std::path::{Path, PathBuf};

use crate::{UpdateConfig, UpdateSource};

/// Env var naming a JSON config file (the `update-check.sample.json` shape).
pub const ENV_CONFIG: &str = "OEP_UPDATE_CONFIG";
/// Env var master switch: `on` enables checking against the source vars below.
pub const ENV_CHECK: &str = "OEP_UPDATE_CHECK";
/// Env var: on-site mirror DIRECTORY containing `update.bundle.json`.
pub const ENV_MIRROR_URL: &str = "OEP_UPDATE_MIRROR_URL";
/// Env var: offline signed bundle FILE placed by an admin (air-gap).
pub const ENV_OFFLINE_BUNDLE: &str = "OEP_UPDATE_OFFLINE_BUNDLE";
/// Env var: 64-hex Ed25519 verifying public key.
pub const ENV_PUBKEY: &str = "OEP_UPDATE_PUBKEY";

/// Resolve the update config from the process environment (see module docs).
pub fn resolve() -> UpdateConfig {
    resolve_from(&|k| std::env::var(k).ok())
}

/// Resolve from an injectable environment source (tests / embedders). A
/// missing or bad `OEP_UPDATE_CONFIG` falls through to the env-var path and
/// then to the non-phoning default, exactly like [`resolve`].
pub fn resolve_from(get: &dyn Fn(&str) -> Option<String>) -> UpdateConfig {
    if let Some(path) = get(ENV_CONFIG) {
        match from_path(Path::new(&path)) {
            Ok(cfg) => return cfg,
            Err(e) => eprintln!(
                "oep-update: config file {} could not be loaded ({e}); running OFF",
                path
            ),
        }
    }
    from_env_source(get)
}

/// Build a config from the `OEP_UPDATE_*` environment variables alone.
/// `OEP_UPDATE_CHECK` different from `on`/`1`/`true` yields the non-phoning
/// default. "On" with no source still yields source `Off` (zero I/O): the
/// master switch and a source are separate conditions, and only a real source
/// can produce I/O.
pub fn from_env_source(get: &dyn Fn(&str) -> Option<String>) -> UpdateConfig {
    let on = match get(ENV_CHECK).as_deref() {
        Some(v) => {
            let v = v.trim();
            v.eq_ignore_ascii_case("on") || v == "1" || v.eq_ignore_ascii_case("true")
        }
        None => false,
    };
    if !on {
        return UpdateConfig::default();
    }
    let source = match (
        get(ENV_OFFLINE_BUNDLE).map(|s| s.trim().to_string()),
        get(ENV_MIRROR_URL).map(|s| s.trim().to_string()),
    ) {
        (Some(b), _) if !b.is_empty() => UpdateSource::Bundle(PathBuf::from(b)),
        (_, Some(m)) if !m.is_empty() => UpdateSource::Mirror(PathBuf::from(m)),
        // On but no source configured: still Off, still zero I/O.
        _ => UpdateSource::Off,
    };
    let public_key = get(ENV_PUBKEY).and_then(|h| hex32(&h));
    UpdateConfig {
        enabled: true,
        source,
        public_key,
        ..UpdateConfig::default()
    }
}

/// Load a config from a JSON file in the `update-check.sample.json` shape.
/// A missing/unreadable/unparsable file returns `Err` - callers decide how to
/// fall back (surfaces log and run OFF; the CLI reports and exits 0).
pub fn from_path(path: &Path) -> Result<UpdateConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: ConfigFile =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let source = match file.source {
        None => UpdateSource::Off,
        Some(ConfigSource::Mirror { mirror }) => UpdateSource::Mirror(PathBuf::from(mirror)),
        Some(ConfigSource::Remote { url }) => UpdateSource::RemoteUrl(url),
        Some(ConfigSource::Bundle { bundle }) => UpdateSource::Bundle(PathBuf::from(bundle)),
    };
    let public_key = file.public_key.as_ref().and_then(|h| hex32(h));
    if file.public_key.is_some() && public_key.is_none() {
        eprintln!("oep-update: public_key must be 64 hex chars; source untrusted");
    }
    Ok(UpdateConfig {
        enabled: file.enabled,
        source,
        public_key,
        current_version: file
            .current_version
            .unwrap_or_else(|| UpdateConfig::default().current_version),
        timeout_ms: file.timeout_ms.unwrap_or(3000),
    })
}

/// Override the installed current version (used by the CLI `--version` flag
/// and the proof tooling when the real build version differs from the config).
pub fn with_current_version(mut cfg: UpdateConfig, version: Option<&str>) -> UpdateConfig {
    if let Some(v) = version {
        cfg.current_version = v.to_string();
    }
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

/// Parse a 64-hex-char Ed25519 public key.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake environment map (avoids mutating the real process env, so unit
    /// tests never race each other).
    fn fake(entries: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let mut map = Vec::new();
        for (k, v) in entries {
            map.push((k.to_string(), v.to_string()));
        }
        move |k: &str| map.iter().find(|(mk, _)| mk == k).map(|(_, v)| v.clone())
    }

    #[test]
    fn default_env_resolves_off_non_phoning() {
        let cfg = resolve_from(&fake(&[]));
        assert!(!cfg.enabled);
        assert!(matches!(cfg.source, UpdateSource::Off));
        assert!(matches!(crate::check(&cfg), crate::UpdateStatus::Off));
    }

    #[test]
    fn env_on_without_a_source_stays_off() {
        let cfg = resolve_from(&fake(&[(ENV_CHECK, "on")]));
        assert!(cfg.enabled, "master switch reflects OEP_UPDATE_CHECK=on");
        assert!(
            matches!(cfg.source, UpdateSource::Off),
            "on without a source is still Off (zero I/O)"
        );
        assert!(matches!(crate::check(&cfg), crate::UpdateStatus::Off));
    }

    #[test]
    fn env_off_is_never_on() {
        for v in ["off", "0", "false", ""] {
            let cfg = resolve_from(&fake(&[(ENV_CHECK, v)]));
            assert!(!cfg.enabled, "OEP_UPDATE_CHECK={v:?} must stay off");
            assert!(matches!(crate::check(&cfg), crate::UpdateStatus::Off));
        }
    }

    #[test]
    fn env_on_with_bundle_source_builds_config() {
        let pubkey = "ab".repeat(32);
        let cfg = resolve_from(&fake(&[
            (ENV_CHECK, "on"),
            (ENV_OFFLINE_BUNDLE, "/srv/updates/update.bundle.json"),
            (ENV_PUBKEY, &pubkey),
        ]));
        assert!(cfg.enabled);
        assert!(matches!(
            cfg.source,
            UpdateSource::Bundle(p) if p == Path::new("/srv/updates/update.bundle.json")
        ));
        assert_eq!(cfg.public_key, Some([0xAB; 32]));
    }

    #[test]
    fn explicit_config_file_beats_env() {
        let dir = std::env::temp_dir().join(format!(
            "oep-update-config-{}-{}",
            std::process::id(),
            "unit"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("update-check.json");
        std::fs::write(
            &file,
            r#"{"enabled":true,"source":{"mirror":"/opt/isolith/updates"},"public_key":"ab","current_version":"1.2.3"}"#,
        )
        .unwrap();
        // The env says OFF; the file says ON - the file wins.
        let cfg = resolve_from(&fake(&[(ENV_CONFIG, file.to_str().unwrap())]));
        assert!(cfg.enabled);
        assert!(matches!(
            cfg.source,
            UpdateSource::Mirror(p) if p == Path::new("/opt/isolith/updates")
        ));
        // Bad pubkey hex -> None key -> the source can never be trusted.
        assert_eq!(cfg.public_key, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_config_file_falls_back_off_without_failing() {
        // A config path that does not exist: logs and runs OFF (never a launch
        // failure), and returns the non-phoning default.
        let cfg = resolve_from(&fake(&[(
            ENV_CONFIG,
            "/definitely/not/here/update-check.json",
        )]));
        assert!(!cfg.enabled);
        assert!(matches!(cfg.source, UpdateSource::Off));
    }
}
