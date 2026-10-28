# P10-M5-T01 -- Update checker feature (decision #11)

Status: GREEN (crate shipped; decision #20 surfaces the status in the stack).
Owner: Isolith operations / P10 delivery lane. Version: 1.

## 1. What this is

A designed, advisory update checker that ships with every Isolith stack.
Decision #11 (2026-10-17) makes Isolith' update checking a DESIGNED FEATURE
with hard rules:

- ADVISORY and NON-BLOCKING: the program must run identically whether the
  checker finds a server, finds nothing, or cannot reach anything.
- NO update gate ever prevents launch or breaks an installed stack.
- Air-gap compatible: the checker polls a CONFIGURABLE source - a remote URL,
  an on-site mirror, or an offline signed bundle placed on the network by an
  admin - and never fails closed.
- Payloads consumed are signed (ties to P11 signing; the release engine emits
  the signed update manifest).
- The feature is disable-able and non-phoning by default in any default
  deployment.

## 2. The four acceptance items

1. launch-without-source succeeds: only flags "update unavailable".
2. launch-with-old-bundle succeeds: only flags "update available".
3. Signed manifest is verified BEFORE any download/payload exposure.
4. No analytics reach-back: telemetry stays out (P0-M5-T02 stands; this
   decision only covers update checking, not usage analytics).

All four are pinned as Rust integration tests in
`crates/oep-update/tests/update_checker_acceptance.rs` (TDD RED committed
before the crate exists).

## 3. Design

The checker is the `oep-update` Rust crate plus a thin CLI wrapper. It has
three moving pieces:

- `UpdateConfig`: enabled/disable flag, the CONFIGURABLE source, the public
  key used to verify the signed manifest, the installed version, and a timeout
  for remote fetches. `Default` is OFF: source = `Off`, feature non-phoning.
- `UpdateSource` enum: `Off` (default), `Mirror(PathBuf)` (on-site dir with
  `update.bundle.json`), `RemoteUrl(String)` (http(s) endpoint serving the
  signed bundle), `Bundle(PathBuf)` (offline signed bundle file placed by an
  admin).
- `UpdateStatus` enum: the ADVISORY outcome. `Off`, `Unreachable { detail }`,
  `Untrusted { detail }`, `UpdateAvailable { version, payloads }`. Checking
  NEVER returns a hard error: every failure mode (missing source, empty
  source, refused remote, corrupt body, bad signature, hash mismatch) maps to
  a status - never to a launch-preventing error.
- `Probe::preflight` -> `LaunchDecision`: the launcher seam. EVERY status maps
  to `Proceed`; there is deliberately no `Block` variant. A launcher calls
  preflight, logs the status, and starts the stack regardless.

Signing format (consumed here, produced by the P12 release engine / admin
tooling): an Ed25519 signature over the canonical JSON of the update manifest
(schema, product, version, released, notes, payloads with sha256 + size), with
the signature and an inline base64 payload carried beside it in a single bundle
file. `verify_payload(payload, bytes)` re-checks the manifest sha256 before any
byte is trusted - "signed manifest verified before any download".

## 4. Behavior matrix

| Source                          | check() status      | Launch  |
|---------------------------------|---------------------|---------|
| Off (default)                   | `Off`               | Proceed |
| Mirror missing / empty          | `Unreachable`       | Proceed |
| Remote refused / timed out      | `Unreachable`       | Proceed |
| Bundle present, same/newer      | `UpdateAvailable`   | Proceed |
| Bundle signature invalid        | `Untrusted`         | Proceed |
| Payload hash mismatch           | refuse at verify    | Proceed |

## 5. Non-goals

- No auto-download, no auto-install, no restart orchestration: this is an
  advisory flag only. Applying an update is a separate, human- or
  operator-gated action.
- No usage analytics, no phone-home, no beaconing. The feature is off with no
  source configured; with a source it talks ONLY to that source.

## 6. Surface wiring (decision #20)

The status is surfaced in three places, all reusing the same `oep-update`
crate (no re-implementation, no second config loader):

| Surface | What it does |
|---------|--------------|
| Workbench (`oep-workbench`, the `oep-shell` example binary) | Runs `Probe::preflight` ONCE at startup, logs exactly one advisory line on stderr, and prints a NON-MODAL notice (version, release date, release-notes link) when the status is `UpdateAvailable`. Nothing can block launch or change the exit code (`crates/oep-shell/src/update.rs`). |
| `oep-web` (PDM admin surface) | Computes the status ONCE at startup into a shared `UpdateBanner`, logs one advisory line, and every UI page renders a dismissible banner when an update is available. Page loads never re-probe; only the explicit `GET /ui/update/check` ("check now") touches the source again. Dismissal is remembered PER offered version and survives restarts when a persist dir is set (`<persist-dir>/update-dismissals.json`). |
| `oep` CLI | `oep update status` and `oep update check [--json]` reuse the crate's `check()`, print the advisory line (or the serialized status with `--json`), and never exit non-zero on a status - only command-line misuse exits 2. |

Config resolution is one seam, `oep_update::config::resolve()` (extended for
UPDATE-CHANNEL, decision #20 - the full ladder with the persisted-settings and
build-default tiers; each tier is tested in
`crates/oep-update/tests/update_channel.rs`):

1. `OEP_UPDATE_CONFIG` - path to an explicit JSON config file (the
   `update-check.sample.json` shape). Missing/unparsable falls through with a
   logged reason, never a launch failure. This is the highest authority: an
   explicit config file (admin/orchestrated node) beats every other tier.
2. The env air-gap contract - engaged only when `OEP_UPDATE_CHECK` is present:
   `on` with `OEP_UPDATE_MIRROR_URL` mirror directory /
   `OEP_UPDATE_OFFLINE_BUNDLE` offline bundle and `OEP_UPDATE_PUBKEY` for the
   64-hex verifying key; a present-but-not-on `OEP_UPDATE_CHECK` is an
   explicit admin off that beats a persisted user toggle (air-gap safety:
   decision #11's no-phone-home cannot be overridden by a UI click).
3. The persisted settings file (UPDATE-CHANNEL): written by the on/off toggle
   and the `--source` control to `OEP_UPDATE_SETTINGS`, else
   `<XDG_CONFIG_HOME|HOME/.config>/isolith/update-check.json`. A present
   file means "configured": `enabled: false` = off by the user; `enabled:
   true` + a source = on with that source. User settings beat the build
   default.
4. The build default: a PUBLIC build (`--features
   oep-update/public-default-source`) resolves the never-configured state to
   ON with the default remote source ([`default_public_remote_url()`], a single
   constant - the release store today, the GitHub Releases bundle after the
   migration; that one-line change is called out in the UPDATE-CHANNEL proof).
   Private and air-gapped builds keep `Off`.
5. The non-phoning default: `enabled=false`, source `Off`, zero I/O. This is
   the "off because nothing was ever configured" state, distinguishable from
   "off by the user" (tier 3) at runtime via [`config::resolve_full`].

The UPDATE-CHANNEL tri-state (`On { source }`, `OffByUser`,
`OffNeverConfigured`) is carried by `config::resolve_full()` -> `ResolvedConfig`
and surfaced by `oep update config --status` and the oep-web settings page.

Hard rule (decision #11, re-proven for the surfaces): a config with
`enabled=false` or source `Off` performs NO I/O of any kind. Enforced by
`crates/oep-update/tests/update_surface_gating.rs` and
`crates/oep-update/tests/update_channel.rs` (an enabled config against a dead
port goes `Unreachable`; the same config disabled goes `Off` with zero sockets)
and structurally by oep-web (page loads read the cached status only).
`UpdateAvailable` carries `released` and `release_notes_url` (the manifest's
`notes` field; release cuts set it to the notes page) so surfaces can render
version + release date + release-notes link from one signed source.

Proof: `workstreams/delivery/update-checker/proof/update-surface-2027.1/`
recording all four statuses (Off, Unreachable, Untrusted, UpdateAvailable -
plus UpToDate) against a real signed bundle, through each surface.

## 7. UPDATE-CHANNEL: settings, default source, one-off offer, feed (decision #20)

The channel half of the update work (card UPDATE-CHANNEL) turns the checker
into a real, user-controllable distribution channel.

### 7.1 Persisted settings schema

The settings file uses the SAME shape as `update-check.sample.json` (one
schema, one `config::from_path` reader, no second format), written by the
toggle to a resolved path:

| Field      | Meaning |
|------------|---------|
| `enabled`  | The on/off flag. `false` in a PRESENT file = off by the user. |
| `source`   | `null` (off) or `{"mirror": PATH}`, `{"url": URL}`, `{"bundle": PATH}`. |
| `public_key` / `current_version` / `timeout_ms` | Optional overrides (defaults applied). |

Resolution order of the file path (first hit): `OEP_UPDATE_SETTINGS` env, else
`$XDG_CONFIG_HOME/isolith/update-check.json`, else
`$HOME/.config/isolith/update-check.json`, else no persistence (a
never-configured build).

Three states, distinguishable at runtime (`UpdateChannelState`):

- `On { source }` - enabled with a real source (user-set, admin-set, or a
  public build's default).
- `OffByUser` - the user/operator explicitly turned it off (persisted
  `enabled: false`, an explicit env off, or an explicit config file saying
  off). Zero I/O.
- `OffNeverConfigured` - nothing was ever configured (no file, no env, private
  build). Zero I/O.

### 7.2 Public vs private build default

`cargo build` of the surfaces with `--features
oep-update/public-default-source` produces a PUBLIC build: it resolves a
never-configured environment to `enabled: true` with the baked default remote
source and the committed release verifying key, so it checks for updates on
first launch with no configuration at all. Without the feature (private,
air-gapped, or plain source builds) the never-configured state stays `Off` and
performs no I/O. The default URL is ONE constant
(`config::default_public_remote_url()`); moving from the release store to the
GitHub Releases bundle is a one-line change (proven by the UPDATE-CHANNEL
proof record).

### 7.3 The toggle and the one-off failure offer

- CLI: `oep update config --status | --enable [--source SPEC] | --disable |
  --source SPEC | --accept-offer COND | --decline-offer COND`, where SPEC is
  `off`, `url:URL`, `mirror:PATH`, `bundle:PATH` (or a bare `http(s)://` URL).
  `--status` prints the tri-state and exits 0 with no I/O; writes persist to
  the settings file; `--disable` never opens a socket (gating-proven).
- oep-web: `/ui/update/settings` (tri-state + toggle + source form),
  `/ui/update/toggle?enabled=on|off`, `/ui/update/source?source=SPEC`.
- One-off failure offer: when a live check (`oep update check`, or the oep-web
  settings surface) returns `Unreachable` or `Untrusted`, the program's update
  channel OFFERS ONCE to turn update checking off (`OfferLedger` in
  `crates/oep-update/src/offer.rs`, persisted to
  `<settings-dir>/update-offers.json` or `OEP_UPDATE_OFFERS`). Answering:
  accept (`oep update config --accept-offer COND`, or the web accept link)
  records `Accepted` AND disables checking; decline (`--decline-offer COND`,
  or the web decline link) records `Declined`. The answer survives restarts,
  never repeats for the same condition, and declining does not re-ask on the
  next launch - both answers tested in `crates/oep-update/tests/update_channel.rs`
  and `crates/oep-web/tests/update_channel.rs`.

### 7.4 Feed: one signed file, everywhere

Release cuts publish the signed `update.bundle.json` to the SAME store as the
artifacts (the release `dist/<version>/` directory; `sign-release.sh` emits it
alongside the binaries and the checksum sidecars). It is listed in the
machine-readable download manifest `docs-site/downloads.json`, so the docs site
and the in-program check read the same signed file. Air-gap bundles
(`scripts/airgap/build-offline-bundle.sh`) carry the identical
`update.bundle.json` inside, so an air-gapped admin points
`OEP_UPDATE_OFFLINE_BUNDLE` at the same verified bytes the connected build
used.

Proof: `workstreams/delivery/update-checker/proof/update-channel-2027.1/`.
