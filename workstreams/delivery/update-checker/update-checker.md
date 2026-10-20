# P10-M5-T01 -- Update checker feature (decision #11)

Status: RED (acceptance + design committed; crate lands in GREEN).
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
