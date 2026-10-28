# Update surface proof (decision #20) - 2027.1

Host: U1 (GB10, decision #9 approved run target). Binaries in this run are
`target/debug/` from the update-surface worktree: `oep` and `oep-update-check`
(`crates/oep-update`), `oep-workbench` (`crates/oep-shell` example, the shipped
desktop workbench), `oep-web` (`crates/oep-web` PDM admin surface).

Real signed bundle: `dist/2027.1.0-rc2/update.bundle.json` (release-cut signed
bundle), verified by the committed public key
`workstreams/security/keys/oep-update-signing.pub` =
`e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811`. The
tampered variant (`tampered.bundle.json`) is the same file with one byte
flipped inside the offered-version literal, so the recomputed Ed25519 signature
fails while the JSON stays valid.

Reproduce: `bash workstreams/delivery/update-checker/proof/update-surface-2027.1/proof.sh`
from anywhere (it regenerates the per-status configs with absolute paths and
rewrites the log files in place).

## 1. The `oep` CLI: all statuses, all exit 0

(from `cli-statuses.log`; every row exits 0 - a status never gates anything)

| Status | Invocation | Observed line / output |
|--------|-----------|------------------------|
| Off | `oep update status` (default) | `update check: OFF (no source configured; non-phoning default)` |
| Off (JSON) | `oep update check --json` | `"Off"` |
| UpdateAvailable | `oep update status --config update-available.json` (installed 2027.0.9, offers 2027.1.0-rc2) | `update check: update available -> 2027.1.0-rc2 (advisory; install is a separate operator action)` |
| UpToDate | `--config update-uptodate.json` (installed 2027.1.0-rc2) | `update check: up to date` |
| Untrusted | `--config update-untrusted.json` (tampered real bundle) | `update check: source NOT TRUSTED - no payloads consumed (update bundle signature/verify failed: Ed25519 signature verification failed)` |
| Untrusted (JSON) | `oep update check --json --config update-untrusted.json` | `{"Untrusted": {"detail": "update bundle signature/verify failed: Ed25519 signature verification failed"}}` |
| Unreachable | `--config update-unreachable.json` (missing bundle file) | `update check: unavailable (cannot read .../does-not-exist/update.bundle.json: No such file or directory (os error 2))` |

## 2. Workbench startup (from `workbench.log`)

`oep-workbench <command>` runs `Probe::preflight` ONCE at startup, logs exactly
one advisory line on stderr, and prints a non-modal notice when an update is
available. Exit code untouched (all rows 0).

| Config | stderr (the single advisory line) | stdout | exit |
|--------|-----------------------------------|--------|------|
| default (Off) | `oep-workbench: update check: OFF (no source configured; non-phoning default)` | (empty) | 0 |
| update-available | `oep-workbench: update check: update available -> 2027.1.0-rc2 (advisory; ...)` | `oep-workbench: update available: 2027.1.0-rc2 (released 2026-10-19)` | 0 |
| update-untrusted | `oep-workbench: update check: source NOT TRUSTED - ...` | (empty) | 0 |

The release-notes link renders when the signed manifest carries a `notes` URL;
the 2027.1.0-rc2 real bundle ships `notes: null`, so no link is shown for it
(the link path is covered by `crates/oep-shell/tests/update_surface.rs` and
`crates/oep-web/tests/update_banner.rs` with a notes-bearing bundle).

## 3. oep-web banner (from `web.log`, `web.server.stderr`, `web-explorer*.html`)

Server started with `--update-config update-available.json`. Startup logs one
advisory line; every page renders the banner from the CACHED status (no page
load re-probes). Dismissal is per offered version.

- Startup log: `oep-web: update check: update available -> 2027.1.0-rc2 (advisory; install is a separate operator action)`
- `GET /` banner (full div, captured from the served page):

  `<div class="update-banner" role="status"><strong>Isolith update available: 2027.1.0-rc2</strong><span>released 2026-10-19</span><a class="update-link" href="/ui/update/check">check now</a><a class="update-link" href="/ui/update/dismiss?version=2027.1.0-rc2">dismiss</a></div>`

- `GET /ui/update/dismiss?version=2027.1.0-rc2` -> `303` redirect to `/`; next
  `GET /` rendered **0** banner divs (dismissed for that version only).
- `GET /ui/update/check` -> `303` redirect; the server re-ran the preflight
  (second advisory line in `web.server.stderr`).
- Unreachable-source leg (`update-unreachable.json`): the stack starts
  normally - one advisory line `oep-web: update check: unavailable (...)`, the
  page serves with **0** banner divs, no dialog, no exit-code change.

## 4. No-I/O-when-Off (proven by test, not just run)

`crates/oep-update/tests/update_surface_gating.rs`: the same dead-port config
returns `Unreachable` when enabled and `Off` when `enabled=false` or source
`Off` - i.e. the gate fires before any socket, and page loads in oep-web read
the cached status only (`crates/oep-web/tests/update_banner.rs`).

## 5. Acceptance mapping (card UPDATE-SURFACE)

- Stack starts normally with unreachable/empty/untrusted source, one advisory
  line, no dialog, no exit-code change: sections 1-3.
- `UpdateAvailable` produces the workbench notice and the oep-web banner;
  dismissing stops the banner for that version only: sections 2-3 (+
  `crates/oep-web/tests/update_banner.rs`).
- CLI `oep update status` / `oep update check [--json]` never exit non-zero on
  a status: section 1 (all exit 0) + shell-misuse exit 2 on bad subcommands.
- No new network I/O when the source is `Off` or the feature flag is off,
  proven by test: section 4.
