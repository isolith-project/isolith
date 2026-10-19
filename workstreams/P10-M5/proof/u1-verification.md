# P10-M5-T01 -- Update checker: U1 live verification (2026-10-17)

Host: GB10 (U1-class, decision #9 approved run target). Binaries built from
this commit's worktree (`cargo build --workspace`), then driven end-to-end.

## Scenarios (all must exit 0 = never blocks launch)

| # | Scenario | CLI / driver | Observed | exit |
|---|----------|--------------|----------|------|
| 1 | Default deployment (no config) | `oep-update-check` | `update check: OFF (no source configured; non-phoning default)` | 0 |
| 2 | Signed bundle, newer version (0.2.0 vs 0.1.0) | `oep-update-check --config workstreams/P10-M5/proof/live/check-config.json` | `update check: update available -> 0.2.0 (advisory; install is a separate operator action)` | 0 |
| 3 | Signed bundle, same version (0.2.0 == 0.2.0) | `--config /tmp/check-same.json` | `update check: up to date` | 0 |
| 4 | Tampered bundle (version flipped -> signature fails) | `--config /tmp/check-tampered.json` | `update check: source NOT TRUSTED - no payloads consumed (Ed25519 signature verification failed)` | 0 |
| 5 | Missing source (mirror dir absent) | `--config /tmp/check-missing.json` | `update check: unavailable (cannot read .../update.bundle.json ...)` | 0 |
| 6 | No public key configured | `--config /tmp/check-nokey.json` | `update check: source NOT TRUSTED - no payloads consumed (no public key configured...)` | 0 |
| 7 | Refused remote URL (acceptance suite, port-scoped) | `cargo test -p oep-update` | `test result: ok. 9 passed` (includes `refused_remote_source_flags_unavailable_and_proceeds`) | 0 |

## Acceptance mapping (decision #11)

- launch-without-source succeeds, only flags "update unavailable": #1, #5.
- launch-with-old-bundle succeeds, only flags "update available": #2, #3.
- signed manifest verified before any download: #4 (tamper -> Untrusted,
  payloads never exposed), plus `payload_hash_mismatch_is_refused` in the
  acceptance suite (verify_payload gate before bytes are consumed).
- no analytics reach-back: default config has an empty source (Off) and exit 0
  with zero I/O; when enabled the checker talks only to the configured source;
  no telemetry surface exists in the crate (P0-M5-T02 stands).
- disable-able and non-phoning by default: `UpdateConfig::default()` is
  enabled=false + source=Off (asserted in
  `default_config_is_off_and_launch_proceeds`).

## Evidence files (committed)

- workstreams/P10-M5/proof/live/payload.bin (21-byte test payload)
- workstreams/P10-M5/proof/live/update.bundle.json (signed bundle, version 0.2.0)
- workstreams/P10-M5/proof/live/check-config.json (bundle source + pubkey config)
- Acceptance/unit results: `cargo test --workspace` (all pass),
  `python3 tests/monorepo_layout_test.py` (ALL CHECKS PASSED),
  `python3 -m pytest -q` (67 passed), cargo fmt clean, clippy -D warnings clean.
