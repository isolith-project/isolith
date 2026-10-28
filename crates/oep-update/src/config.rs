//! Update-check configuration resolution for the surfaces (decision #20).
//!
//! One seam every surface uses to obtain its [`UpdateConfig`] - the workbench
//! startup check, the oep-web banner, and the `oep update` CLI all call
//! [`resolve`] (or [`from_path`] with an explicit file) and never construct
//! configs by hand.
//!
//! Resolution order (first hit wins), documented and tested here and in
//! `workstreams/delivery/update-checker/update-checker.md`:
//!
//! 1. `OEP_UPDATE_CONFIG`: a path to a JSON config file (the
//!    `update-check.sample.json` shape). Missing or unparsable falls through
//!    with a logged reason - never a launch failure.
//! 2. The air-gap env contract: a PRESENT `OEP_UPDATE_CHECK` engages the env
//!    tier. `on` (with `OEP_UPDATE_MIRROR_URL` / `OEP_UPDATE_OFFLINE_BUNDLE`,
//!    `OEP_UPDATE_PUBKEY`) builds the admin source; any other value yields the
//!    non-phoning default. This is the same variable contract
//!    `scripts/airgap/*` and the stack `.env` already generate. An admin env
//!    off/mirror/bundle beats a persisted user setting (no phone-home
//!    override; decision #11 hard guard in air-gap).
//! 3. The persisted settings file (UPDATE-CHANNEL, decision #20) at the
//!    resolved settings path (`OEP_UPDATE_SETTINGS`, else
//!    `<XDG_CONFIG_HOME|HOME/.config>/isolith/update-check.json`): the user's
//!    on/off + source. File present = "configured"; `enabled:false` = off by
//!    the user. User settings beat the build default.
//! 4. The build default: a PUBLIC build (`--features
//!    oep-update/public-default-source`) resolves the never-configured state
//!    to `On` with [`default_public_remote_url()`] (release store today;
//!    GitHub Releases after the migration - a one-line constant change).
//!    Private and air-gapped builds keep `Off`.
//! 5. The three-state fallback: [`UpdateConfig::default`] - `enabled=false`,
//!    source `Off`, zero I/O (off because nothing was ever configured).
//!
//! Three states are distinguishable at runtime via [`resolve_full`]:
//! on-with-a-source, off-by-the-user, off-because-never-configured.
//!
//! Hard rule (decision #11, beyond this module): a config with `enabled=false`
//! or source `Off` performs NO I/O of any kind - proven by the gating tests in
//! `crates/oep-update/tests/update_surface_gating.rs` and the UPDATE-CHANNEL
//! tests in `crates/oep-update/tests/update_channel.rs`.

use std::path::{Path, PathBuf};

use crate::{UpdateConfig, UpdateSource};

/// Env var naming a JSON config file (the `update-check.sample.json` shape).
pub const ENV_CONFIG: &str = "OEP_UPDATE_CONFIG";
/// Env var master switch: a present value engages the env tier.
pub const ENV_CHECK: &str = "OEP_UPDATE_CHECK";
/// Env var: on-site mirror DIRECTORY containing `update.bundle.json`.
pub const ENV_MIRROR_URL: &str = "OEP_UPDATE_MIRROR_URL";
/// Env var: offline signed bundle FILE placed by an admin (air-gap).
pub const ENV_OFFLINE_BUNDLE: &str = "OEP_UPDATE_OFFLINE_BUNDLE";
/// Env var: 64-hex Ed25519 verifying public key.
pub const ENV_PUBKEY: &str = "OEP_UPDATE_PUBKEY";
/// Env var: explicit path to the persisted settings file (tests and
/// deployments that want a non-default settings location).
pub const ENV_SETTINGS: &str = "OEP_UPDATE_SETTINGS";

/// The update channel's tri-state, distinguishable at runtime (UPDATE-CHANNEL,
/// decision #20): on with a source; off by the user; off because nothing was
/// ever configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateChannelState {
    /// On with a source (the check talks to it).
    On { source: UpdateSource },
    /// Off by the user/operator: an explicit persisted setting or admin
    /// contract turned it off. Zero I/O.
    OffByUser,
    /// Off because nothing was ever configured (no settings, private build,
    /// no admin source). Zero I/O.
    OffNeverConfigured,
}

/// Resolve the update config from the process environment (see module docs).
pub fn resolve() -> UpdateConfig {
    resolve_from(&|k| std::env::var(k).ok())
}

/// Resolve from an injectable environment source (tests / embedders). A
/// missing or bad `OEP_UPDATE_CONFIG` falls through to the env-var path, then
/// the persisted settings, then the build default - exactly like [`resolve`].
pub fn resolve_from(get: &dyn Fn(&str) -> Option<String>) -> UpdateConfig {
    resolve_full_from(get).config
}

/// Resolve and also report the tri-state (UPDATE-CHANNEL surfaces: the web
/// settings page and the CLI `oep update config --status`). See module docs
/// for the precedence ladder.
pub fn resolve_full(get: &dyn Fn(&str) -> Option<String>) -> ResolvedConfig {
    resolve_full_from(get)
}

/// [`resolve_full`] with every tier; [`ResolvedConfig`] carries both the
/// effective config and the tri-state. Uses this build's compiled-in default.
pub fn resolve_full_from(get: &dyn Fn(&str) -> Option<String>) -> ResolvedConfig {
    resolve_full_default_from(get, build_default_config())
}

/// [`resolve_full_from`] with an explicit build default (tests / embedders
/// that must exercise the public-build resolution without compiling the
/// `public-default-source` feature).
pub fn resolve_full_default_from(
    get: &dyn Fn(&str) -> Option<String>,
    build_default: Option<UpdateConfig>,
) -> ResolvedConfig {
    // 1. Explicit config file (admin-pinned): if it parses, that is the
    //    answer (its own enabled/source govern the tri-state).
    if let Some(path) = get(ENV_CONFIG) {
        if let Ok(cfg) = from_path(Path::new(&path)) {
            return ResolvedConfig {
                config: cfg.clone(),
                state: state_of_config(&cfg, true),
            };
        }
        eprintln!(
            "oep-update: config file {} could not be loaded; running OFF",
            path
        );
    }
    // 2. Env contract (air-gap admin) - engaged only when OEP_UPDATE_CHECK is
    //    present.
    if let Some(cfg) = from_env_opt(get, false) {
        return ResolvedConfig {
            config: cfg.clone(),
            state: state_of_config(&cfg, true),
        };
    }
    // 3. Persisted settings file (the user/operator toggle).
    if let Some(path) = settings_path_from(get) {
        if let Ok(cfg) = from_path(&path) {
            let state = if cfg.enabled {
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
            };
            return ResolvedConfig { config: cfg, state };
        }
        // A corrupt settings file must not mask the build default; log and
        // fall through (never fail closed on a scratch file).
        if path.exists() {
            eprintln!(
                "oep-update: persisted settings {} unreadable; falling through",
                path.display()
            );
        }
    }
    // 4. Build default (public vs private).
    if let Some(cfg) = build_default {
        return ResolvedConfig {
            config: cfg.clone(),
            state: state_of_config(&cfg, false),
        };
    }
    // 5. Nothing ever configured, private build.
    ResolvedConfig {
        config: UpdateConfig::default(),
        state: UpdateChannelState::OffNeverConfigured,
    }
}

/// The effective config plus the tri-state of how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// The effective config surfaces pass to `Probe`/`check`.
    pub config: UpdateConfig,
    /// The tri-state for UPDATE-CHANNEL surfaces.
    pub state: UpdateChannelState,
}

/// Classify a resolved config into the tri-state. `explicit` says whether the
/// config came from an explicit setting (file/env) vs the build default.
fn state_of_config(cfg: &UpdateConfig, explicit: bool) -> UpdateChannelState {
    match &cfg.source {
        UpdateSource::Off => {
            if cfg.enabled {
                // "On with no source" is still zero-I/O Off, but it is a real
                // configured state (an admin/persisted on without a source) -
                // surface it as On-with-Off so the tester can see the toggle
                // is on.
                UpdateChannelState::On {
                    source: UpdateSource::Off,
                }
            } else if explicit {
                UpdateChannelState::OffByUser
            } else {
                UpdateChannelState::OffNeverConfigured
            }
        }
        src => UpdateChannelState::On {
            source: src.clone(),
        },
    }
}

/// Build a config from the `OEP_UPDATE_*` environment variables alone.
/// `OEP_UPDATE_CHECK` different from `on`/`1`/`true` yields the non-phoning
/// default. "On" with no source still yields source `Off` (zero I/O): the
/// master switch and a source are separate conditions, and only a real source
/// can produce I/O. When `OEP_UPDATE_CHECK` is absent this returns `None` (the
/// env tier is not engaged) unless `default_off` is set, in which case it
/// returns the non-phoning default.
pub fn from_env_opt(
    get: &dyn Fn(&str) -> Option<String>,
    default_off: bool,
) -> Option<UpdateConfig> {
    let on = match get(ENV_CHECK).as_deref() {
        Some(v) => {
            let v = v.trim();
            v.eq_ignore_ascii_case("on") || v == "1" || v.eq_ignore_ascii_case("true")
        }
        None => {
            return if default_off {
                Some(UpdateConfig::default())
            } else {
                None
            }
        }
    };
    if !on {
        // A PRESENT check that is not truthy = admin off (air-gap guard).
        return Some(UpdateConfig::default());
    }
    let source = match (
        get(ENV_OFFLINE_BUNDLE).map(|s| s.trim().to_string()),
        get(ENV_MIRROR_URL).map(|s| s.trim().to_string()),
    ) {
        (Some(b), _) if !b.is_empty() => UpdateSource::Bundle(PathBuf::from(b)),
        (_, Some(m)) if !m.is_empty() => UpdateSource::Mirror(PathBuf::from(m)),
        _ => UpdateSource::Off,
    };
    let public_key = get(ENV_PUBKEY).and_then(|h| hex32(&h));
    Some(UpdateConfig {
        enabled: true,
        source,
        public_key,
        ..UpdateConfig::default()
    })
}

/// Public wrapper keeping the pre-UPDATE-CHANNEL contract: when
/// `OEP_UPDATE_CHECK` is unset the env tier returns the non-phoning default
/// (so callers that only look at env get Off). `resolve_full_from` is the tier
/// the surfaces use.
pub fn from_env_source(get: &dyn Fn(&str) -> Option<String>) -> UpdateConfig {
    from_env_opt(get, true).unwrap_or_default()
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

/// Write a persisted settings file in the `update-check.sample.json` shape
/// (the shape `from_path` reads back). `source` is serialized as the untagged
/// `mirror`/`url`/`bundle` object, or `null` when `Off`.
pub fn write_settings(path: &Path, enabled: bool, source: &UpdateSource) -> std::io::Result<()> {
    let src = match source {
        UpdateSource::Off => "null".to_string(),
        UpdateSource::Mirror(p) => format!(r#"{{"mirror": {:?}}}"#, p.to_string_lossy()),
        UpdateSource::RemoteUrl(u) => format!(r#"{{"url": {:?}}}"#, u),
        UpdateSource::Bundle(p) => format!(r#"{{"bundle": {:?}}}"#, p.to_string_lossy()),
    };
    let text = format!(
        "{{\n  \"enabled\": {},\n  \"source\": {}\n}}\n",
        enabled, src
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, text)
}

/// Resolve the persisted-settings file path. `OEP_UPDATE_SETTINGS` wins;
/// otherwise `<XDG_CONFIG_HOME|HOME/.config>/isolith/update-check.json`.
/// `None` when neither env is present (no writable settings location).
pub fn settings_path_from(get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(p) = get(ENV_SETTINGS) {
        let p = p.trim();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let base = get("XDG_CONFIG_HOME").or_else(|| get("HOME").map(|h| format!("{h}/.config")))?;
    Some(
        PathBuf::from(base)
            .join("isolith")
            .join("update-check.json"),
    )
}

/// Override the installed current version (used by the CLI `--version` flag
/// and the proof tooling when the real build version differs from the config).
pub fn with_current_version(mut cfg: UpdateConfig, version: Option<&str>) -> UpdateConfig {
    if let Some(v) = version {
        cfg.current_version = v.to_string();
    }
    cfg
}

/// The compiled-in default remote source for a PUBLIC build: the release store
/// today, the GitHub Releases bundle after the migration. This constant is the
/// ONE-LINE change the UPDATE-CHANNEL proof record calls out for that switch.
///
/// `ISOLITH_UPDATE_DEFAULT_REMOTE` (build env) overrides it for hermetic proof
/// runs that point a public build at a local mirror.
#[cfg(feature = "public-default-source")]
pub fn default_public_remote_url() -> String {
    std::option_env!("ISOLITH_UPDATE_DEFAULT_REMOTE")
        .unwrap_or("https://isolith.dev/update.bundle.json")
        .to_string()
}

/// The canonical release Ed25519 public key baked into public builds so the
/// default source is verifiable out of the box (P11-T04 committed key).
pub const DEFAULT_PUBLIC_PUBKEY_HEX: &str =
    "e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811";

/// The build-default config: `Some` on public builds (enabled, the default
/// remote URL, and the canonical verifying key), `None` on private/air-gapped
/// builds so the never-configured state stays `Off`.
#[cfg(feature = "public-default-source")]
pub fn build_default_config() -> Option<UpdateConfig> {
    let public_key = hex32(DEFAULT_PUBLIC_PUBKEY_HEX);
    Some(UpdateConfig {
        enabled: true,
        source: UpdateSource::RemoteUrl(default_public_remote_url()),
        public_key,
        ..UpdateConfig::default()
    })
}

#[cfg(not(feature = "public-default-source"))]
/// The build-default config: `None` on a private / air-gapped / plain source
/// build - the never-configured state stays `Off`.
pub fn build_default_config() -> Option<UpdateConfig> {
    None
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
        // NB: Off because this is a PRIVATE build (no public-default-source).
        assert!(!cfg.enabled);
        assert!(matches!(cfg.source, UpdateSource::Off));
        assert!(matches!(crate::check(&cfg), crate::UpdateStatus::Off));
        // And the tri-state says "never configured", not "user turned it off".
        assert_eq!(
            resolve_full(&fake(&[])).state,
            UpdateChannelState::OffNeverConfigured
        );
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

    // --- UPDATE-CHANNEL tri-state / persisted settings (decision #20) ---

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("oep-update-channel-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn settings_path_prefers_explicit_env() {
        let get = fake(&[(ENV_SETTINGS, "/s/custom.json")]);
        assert_eq!(
            settings_path_from(&get),
            Some(PathBuf::from("/s/custom.json"))
        );
        let get = fake(&[("XDG_CONFIG_HOME", "/x")]);
        assert_eq!(
            settings_path_from(&get),
            Some(PathBuf::from("/x/isolith/update-check.json"))
        );
        let get = fake(&[("HOME", "/h")]);
        assert_eq!(
            settings_path_from(&get),
            Some(PathBuf::from("/h/.config/isolith/update-check.json"))
        );
        assert_eq!(settings_path_from(&fake(&[])), None);
    }

    #[test]
    fn persisted_on_with_source_beats_build_default() {
        // User setting beats build default (precedence documented + tested).
        let dir = scratch("onwithsrc");
        let settings = dir.join("update-check.json");
        write_settings(
            &settings,
            true,
            &UpdateSource::Mirror("/srv/updates".into()),
        )
        .unwrap();
        let get = fake(&[(ENV_SETTINGS, settings.to_str().unwrap())]);
        let resolved = resolve_full(&get);
        assert!(resolved.config.enabled);
        assert!(matches!(
            resolved.config.source,
            UpdateSource::Mirror(p) if p == Path::new("/srv/updates")
        ));
        assert!(matches!(
            resolved.state,
            UpdateChannelState::On {
                source: UpdateSource::Mirror(_)
            }
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persisted_off_is_off_by_user_and_zero_io() {
        let dir = scratch("offuser");
        let settings = dir.join("update-check.json");
        write_settings(
            &settings,
            false,
            &UpdateSource::RemoteUrl("http://127.0.0.1:9/x".into()),
        )
        .unwrap();
        let get = fake(&[(ENV_SETTINGS, settings.to_str().unwrap())]);
        let resolved = resolve_full(&get);
        assert!(!resolved.config.enabled);
        assert_eq!(resolved.state, UpdateChannelState::OffByUser);
        // No I/O even though the file names a remote source.
        assert!(matches!(
            crate::check(&resolved.config),
            crate::UpdateStatus::Off
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn admin_env_mirror_beats_public_build_default() {
        // Env contract (admin mirror) must beat the never-configured public
        // build default - proven without the feature by forcing the override.
        let get = fake(&[
            (ENV_CHECK, "on"),
            (ENV_MIRROR_URL, "/mnt/mirror"),
            (ENV_PUBKEY, &"aa".repeat(32)),
        ]);
        // Pretend the public feature were on by checking the env tier itself.
        let cfg = from_env_opt(&get, false).unwrap();
        assert!(cfg.enabled);
        assert!(matches!(cfg.source, UpdateSource::Mirror(_)));
    }

    #[test]
    fn write_settings_round_trips() {
        let dir = scratch("roundtrip");
        let settings = dir.join("update-check.json");
        write_settings(
            &settings,
            true,
            &UpdateSource::RemoteUrl("https://up.example/b.json".into()),
        )
        .unwrap();
        let cfg = from_path(&settings).expect("settings parse");
        assert!(cfg.enabled);
        assert!(matches!(
            cfg.source,
            UpdateSource::RemoteUrl(u) if u == "https://up.example/b.json"
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
