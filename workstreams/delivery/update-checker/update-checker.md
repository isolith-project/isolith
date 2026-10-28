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

Config resolution is one seam, `oep_update::config::resolve()`:

1. `OEP_UPDATE_CONFIG` - path to a JSON config file (the
   `update-check.sample.json` shape). Missing/unparsable falls through with a
   logged reason, never a launch failure.
2. `OEP_UPDATE_CHECK=on` plus the air-gap `.env` source variables
   (`OEP_UPDATE_MIRROR_URL` mirror directory / `OEP_UPDATE_OFFLINE_BUNDLE`
   offline bundle, `OEP_UPDATE_PUBKEY` for the 64-hex verifying key).
3. The non-phoning default: `enabled=false`, source `Off`, zero I/O.

Hard rule (decision #11, re-proven for the surfaces): a config with
`enabled=false` or source `Off` performs NO I/O of any kind. Enforced by
`crates/oep-update/tests/update_surface_gating.rs` (an enabled config against a
dead port goes `Unreachable`; the same config disabled goes `Off` with zero
sockets) and structurally by oep-web (page loads read the cached status only).
`UpdateAvailable` carries `released` and `release_notes_url` (the manifest's
`notes` field; release cuts set it to the notes page) so surfaces can render
version + release date + release-notes link from one signed source.

Proof: `workstreams/delivery/update-checker/proof/update-surface-2027.1/`
recording all four statuses (Off, Unreachable, Untrusted, UpdateAvailable -
plus UpToDate) against a real signed bundle, through each surface.
