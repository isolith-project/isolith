# Isolith P11-T02 - Dependency SBOM + CVE scanning in CI (every release)

Version: 1.0.0 (2026-10-18)

Status: active release-engineering routine (P11). Before and during every
release the exact release tree must produce an SBOM and a vulnerability scan
report, and the scan must show no unfixed critical/high CVEs (or every one
covered by a committed waiver in `workstreams/security/waivers.toml`) for the release to pass
the gate. This is the Isolith form of supply-chain due diligence wired into
CI and into the release drill.

Owner: P11 supply-chain / release-engineering lane.
Card: P11-T02 on the Isolith board.
Ties to: P11-T06 (reproducible build + supply-chain doctrine; the SBOM is
derived from the committed Cargo.lock, so the BOM matches what got built),
P11-T01 (threat model: supply-chain boundary the SBOM is the inventory for),
P12-T05 (release train: every train push and v-tag triggers this gate in CI),
P12-T07 (release notes: the scan report ships alongside the release notes).
Clean-room invariant: scanning is lawful inventory/analysis, never copying.
The advisory-db is a git clone (cache), never committed; nothing from it is
copied into the tree.

## 1. What this gates

- The Rust crate dependency tree: `cargo metadata`-equivalent inventory read
  from the committed `Cargo.lock` (the exact resolution the release build
  uses - dev/test dependencies are resolved into the lockfile like any other,
  so a vulnerable test dependency is caught by design).
- The container images the release embeds: base images declared by the repo
  Dockerfiles (`workstreams/delivery/deploy/deploy/Dockerfile`, `scripts/airgap/Dockerfile.*`)
  emitted as a CycloneDX BOM with `pkg:docker` purls.
- The CVE scan source of truth is the rustsec advisory-db (the same data
  `cargo audit` consumes), parsed locally with severity honouring the
  advisory's declared severity or its CVSS v3.1 vector. AOSV cross-check leg
  (report-only) and a trivy pass over the built isolith image (real, on the
  release host, recorded under `workstreams/security/proof/`) enrich the report.

Gate policy (defaults, overridable in `workstreams/security/waivers.toml [policy]`):
`block_severities = ["critical", "high"]`. The gate exits 1 (block the
release) iff a scan finding is unfixed AND at a blocking severity AND not
matched by a committed, non-revoked entry in `workstreams/security/waivers.toml`. Waivers are
version-controlled; a re-opened advisory = remove the waiver row and let the
branch fail its scan again.

## 2. Where it runs

- CI (Gitea Actions, `.gitea/workflows/ga-gate-p11.yaml`, job
  `p11t02-sbom-cve`) runs on every push/PR - and therefore on every
  `release/*` branch push and `v*` tag cut by the release train. It clones
  the advisory-db (shallow), runs the hermetic TDD acceptance suite + the
  live green-scan check, emits the BOMs, runs the gate, and uploads
  `rust.cdx.json`, `rust.spdx.json`, `images.cdx.json`, `scan-report.json`
  and `scan-report.md` as a per-build artifact.
- The release drill (Wave-3 GA gate) additionally commits the durable proof:
  `scripts/sbom-cve-gate.sh` records `workstreams/security/proof/sbom-scan-<ver>.{json,md}` at
  every gate invocation, so a release carries a reproducible, auditable
  scan record in-tree.

## 3. Run it

From the repo root at the release-candidate head (CI-safe; no local state):

    ./scripts/sbom-cve-gate.sh --repo "$PWD" --ref <release-ver> --out /tmp/p11out --osv
    echo $?    # 0 => gate PASS, release may proceed; 1 => gate FAIL, do not release

On the release host, fold the REAL built-image scan in (so a critical/high OS
CVE in the shipped image blocks unless waivered) and record the deliberate-
failure repro:

    python3 -m oep_tooling.sbom_cve trivy-image --image isolith/oep-stack:<ver> --out workstreams/security/proof
    ./scripts/sbom-cve-gate.sh --repo "$PWD" --ref <release-ver> \
        --osv --trivy-scan workstreams/security/proof/trivy-image-findings.json
    bash scripts/sbom-cve-fail-repro.sh --advisory-db workstreams/security/.advisory-db   # seeded critical must FAIL

The driver wraps `python3 -m oep_tooling.sbom_cve gate`. Subcommands:
`sbom` (emit BOMs only), `scan` (scan + report only), `gate` (both + enforce),
`trivy-image` (host-only trivy scan of a built image). Options: `--offline`
(never clone - requires an existing advisory-db), `--refresh` (git-pull the
advisory-db), `--waivers PATH`.

Real container-image scan (requires docker + trivy on the release host):

    python3 -m oep_tooling.sbom_cve trivy-image --image isolith/oep-stack:0.1.0 --out workstreams/security/proof
    # -> workstreams/security/proof/trivy-image.cdx.json + trivy-image.json

## 4. Accept a waiver (the only sanctioned way to ship an unfixed critical/high)

Add an entry to the committed `workstreams/security/waivers.toml`. Each waiver must name the
advisory id and package, carry a date, an owner and a one-line justification
(exploit path not reachable, feature never exercised, fixed-version upgrade
scheduled, decision reference). Revoking is set `revoked = true`. The gate is
the source of truth: if the entry doesn't match, the release blocks.

## 5. Proof contract

Every green gate produces:
- `workstreams/security/artifacts/<ver>/` - the release SBOM set and report (committed per
  release): `rust.cdx.json` + `rust.spdx.json` (crate-tree BOMs),
  `images.cdx.json` (container base-image BOM), `scan-report.json` (full
  report: findings index, severity counts, blocking list, waived entries,
  legs) and `scan-report.md`. CI uploads the identical per-build files as a
  workflow artifact on every push.
- `workstreams/security/proof/sbom-scan-<ver>.json` / `.md` - the durable committed report
  copy (same content as the CI artifact).
- `workstreams/security/proof/trivy-image.cdx.json` + `trivy-image-findings.json` - the real
  container-image SBOM and normalized findings from the release-host trivy
  scan (the gate folds these in via `--trivy-scan`).
- `workstreams/security/proof/deliberate-failure-repro.json` / `.md` - the seeded-critical-vuln
  repro record: a release tree whose Cargo.lock carries a seeded critical
  advisory MUST fail this gate; recorded on each drill and asserted in CI.

The deliberate-failure repro - a release tree whose Cargo.lock carries a
seeded critical advisory (e.g. base64 0.4.x / RUSTSEC-2017-0004, or a test
dependency) - MUST fail this gate; that is part of the P11-T02 acceptance and
is re-demonstrated on each release drill.
