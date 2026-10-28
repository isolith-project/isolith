# Update channel proof (decision #20) - UPDATE-CHANNEL - 2027.1

Host: U1 (GB10, decision #9 approved run target). Binary: `target/debug/oep`
from the update-channel worktree (a private build - no `public-default-source`
feature - then rebuilt with the feature for the public-build leg, then restored).

Real signed bundle: `dist/2027.1.0-rc2/update.bundle.json`, verified by the
committed public key `workstreams/security/keys/oep-update-signing.pub` =
`e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811`.

Reproduce: `bash workstreams/delivery/update-checker/proof/update-channel-2027.1/proof.sh`
from anywhere (it rebuilds the oep binary as needed and rewrites `proof-run.log`
and `cli-statuses.log` in place).

## Acceptance mapping (card UPDATE-CHANNEL)

### 1. Fresh public build checks on first launch; fresh private build is Off

- Private (no feature) with an EMPTY environment resolves `OFF (never
  configured)` (`proof-run.log` section 1). Zero I/O (source `Off`; the crate
  gating tests prove no socket for any off/source-Off config).
- Public (`--features oep-update/public-default-source`, default URL pointed at
  a local HTTP server serving the real signed bundle) with an EMPTY environment
  resolves `ON (source: http://127.0.0.1:PORT/update.bundle.json)` and the
  first-launch check returns `UpdateAvailable 2027.1.0-rc2` - it VERIFIED the
  signed bundle through the default source with no configuration at all
  (section 5). The same path without a store is unit-tested in
  `crates/oep-update/tests/update_channel.rs` (fresh public -> On with the
  default remote source).

### 2. Disabling via the toggle persists across restarts, no sockets

`oep update config --disable` writes `{ "enabled": false, "source": ... }` to
the settings file, and a FRESH process re-reads it as `OFF (disabled by user)`
(section 2). `crates/oep-update/tests/update_channel.rs` binds the same flow to
a dead port and asserts `check()` still returns `Off` (zero I/O) on both
processes.

### 3. Failing check offers the toggle exactly once; both answers tested

- A settings file pointing at a missing bundle makes the advisory status
  `Unreachable` and, with an empty offer ledger, the program offers once
  (section 3a).
- **Decline**: `oep update config --decline-offer unreachable` records
  `Declined` in `update-offers.json`; a fresh process with the SAME files does
  not re-ask, and the settings file is untouched (checking stays on) (3b).
- **Accept**: `oep update config --accept-offer unreachable` records
  `Accepted` AND disables checking (settings `enabled:false`); a fresh process
  does not re-ask and stays Off (3c).

Both answers are also pinned as Rust tests (crate + oep-web over real HTTP) in
`crates/oep-update/tests/update_channel.rs` and
`crates/oep-web/tests/update_channel.rs`, including restart survival.

### 4. Source precedence

Documented in `workstreams/delivery/update-checker/update-checker.md` section 7;
tested in `crates/oep-update/tests/update_channel.rs` and shown live here:

- Persisted user source beats the build default (section 4; the injected
  build-default-vs-user-ladder is unit-tested since the feature is off in this
  worktree's default build).
- Admin env mirror/bundle beats the public URL (section 4: `OEP_UPDATE_CHECK=on
  OEP_UPDATE_MIRROR_URL=/admin/mirror` yields the admin mirror).
- An explicit config file beats everything (unit-tested).

### 5. Feed: same signed file everywhere

`docs-site/downloads.json` now lists the signed `update.bundle.json` feed row
(kind `update-feed`, artifact `update.bundle.json`, `sha256`
`4df5f764...`), and the proof verifies that sha256 equals the committed
release-store file's fingerprint - the site and the in-program check read the
same signed bytes (section 6). The air-gap bundle producer
(`scripts/airgap/build-offline-bundle.sh`) carries the identical
`update.bundle.json` into the offline bundle via `--update-bundle` (or the
versioned dist copy), so an air-gapped admin points `OEP_UPDATE_OFFLINE_BUNDLE`
at the same file.
