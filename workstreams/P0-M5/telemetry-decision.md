---
title: Keelworks Telemetry Decision
decision: telemetry_off_by_default_optin_crash_reports_only
version: 1.0.0
date: 2026-10-15
status: Ratified
owner: Keelworks program governance, P0-M5 (MBD/corpus standards and product policy)
applies_to: all components (crates oep-cgr, oep-3dxml, oep-geo, oep-step, oep-pdm; P2P workers, delivery packaging, CLI and GUI entry points)
supersedes: nothing (first telemetry record)
---

# Keelworks Telemetry Decision

This record is the binding product decision for telemetry in the Keelworks
open engineering platform. It implements charter goal 5 ("Sovereignty
defaults: no telemetry by default, opt-in crash reports only, air-gap capable,
deployable on a single NAS") and the clean-room, self-hosted invariants in
board.program.json. Every downstream workstream that can observe a user
(readers, PDM server, P2P compute, delivery) implements against this record.

This document is versioned like the charter. Any material change bumps the
version in the front matter and adds a line to the change log at the bottom.

## 1. Decision

Keelworks ships with NO telemetry of any kind by default. No usage analytics,
no error logs off-box, no session replay, no feature-flag telemetry, no
marketplace or update-check pings, no crash reporting. The default build of
every component performs no outbound network traffic for observation, and
every component works identically when the host has no network connectivity.

The SINGLE exception is crash reports, and crash reporting is strictly
opt-in: it is OFF unless the user explicitly enables it, it is never bundled
into an install as a pre-checked box, and it collects only diagnostic material
bounded by section 4.

## 2. Why this decision

Three facts drive it:

1. Sovereignty first. The program exists so small shops, defense
   subcontractors, and sovereign users keep control of their data and their
   network. A component that phones home undermines that promise even when the
   data is small. The air-gap promise (charter goal 5, P10-M2-T02) is only
   real if no component has a hidden network dependency.
2. Trust in clean-room work. The corpus (P1) and reader work (P2, P3, P4)
   operate on third-party engineering material under strict license and ITAR
   hygiene (P1-M1-T06). Sending any part of that material, or metadata about
   it, to an uncontrolled third party is incompatible with the legal and
   export-control posture in P0-M2.
3. Corruption of the model. Telemetry is a per-seat surveillance tax on the
   model Keelworks replaces. Adopting the incumbent's habit of harvesting user
   behavior is the one thing the program cannot copy.

## 3. Options considered

1. Telemetry on by default, easy to turn off. Rejected. "Easy to turn off" is
   the documented trick of the commercial model; default-off means default
   sovereign. Defaults are the product.
2. No telemetry, no crash reports at all. Rejected. Crash reports are a real
   reliability tool for an engineering suite, and the ability to fix field
   crashes matters on the critical path. The solution is to make crash
   reporting opt-in and user-controlled, not to forbid it.
3. No telemetry by default, opt-in crash reports only. Adopted. This
   record.

## 4. Crash report scope

A crash report may contain ONLY what is needed to investigate the fault:

- crash signature and panic message
- stack trace / backtrace
- component name and version/build (the oep-* crate and its version)
- host OS and CPU architecture
- a locally generated timestamp and a random per-crash correlation ID
  (random, generated per crash, retained only as long as the report is being
  handled; not a persistent device ID, not a fingerprint, no cookie)

A crash report must NEVER contain:

- model geometry, part/assembly data, or preview data
- file paths, file names, or project names (redacted before send)
- environment variables, tokens, keys, licenses, or vault/PDM object IDs
- user identity, account name, email, or IP-tied identifiers
  (the receiving end does not log raw source IPs beyond the transport epoch
  and deletes them on receipt)

Where the code cannot guarantee a field is diagnostic-only, the field is not
collected. This is the "when in doubt, drop it" rule from the program's
data-minimization discipline.

## 5. Where reports go

The report endpoint is user-controlled by default: a self-hosted collector
configured by the operator (the PDM host or a dedicated crash collector on the
same network), exactly like the rest of the stack. No report is ever sent to a
Keelworks-run endpoint automatically. If a hosted collection option is ever
offered, it must be (a) explicitly selected by the user at runtime, and (b)
preceded by a plain-language notice of what is sent and where. The default
remains self-hosted.

## 6. Implementation guardrails

These are hard, testable rules for every component, including third-party
dependencies pulled into the tree:

- No third-party telemetry or analytics SDK as a dependency, default or
  otherwise. Crash collection is first-party code behind a feature flag.
  A build that adds any analytics/crash SDK with a network capability fails
  review on sight.
- Crash reporting is gated by an explicit opt-in flag (build-time feature
  flag and/or runtime config key). The flag defaults to OFF in every release
  build, for every platform. Enabling it is a conscious operator act.
- Disabling or never enabling the flag changes nothing functionally. There is
  no feature that requires telemetry to be on, and no component requires
  network egress to run.
- The opt-in is silent about nothing: enabling a crash report shows the
  operator the collected diagnostic surface before it leaves the machine.
- No component may ping a vendor or program endpoint for updates, licenses,
  or "reachability". Update checks, where they exist, are user-initiated or
  pointed at a self-hosted mirror.
- Delivery packaging (P10, P12) ships the flag OFF. Telemetry is a per-build
  choice a packager has to make deliberately, never a default.

## 7. Enforcement

The decision is enforced by automation so a future change cannot silently
re-introduce default telemetry:

- CI deny-list scan: manifests (Cargo.toml, package specs, python deps) are
  grepped for known telemetry/analytics SDK names; any hit fails the pipeline.
  This scan is part of the P0-M4 CI skeleton and the P11 SBOM/CVE pass.
- Flag-default test: a CI job builds every release profile with the crash
  flag unset and asserts it compiles with crash reporting OFF and performs no
  outbound network calls at startup (checked with a localhost-off / egress
  harness on the test runner).
- The acceptance test in this repo, tests/test_telemetry_decision.py, stays
  green; it exists to catch edits that weaken the record itself.

## 8. Coverage across the program

- P8 PDM server (AGPL-3.0): telemetry off by default; crash reports only via
  operator-configured self-hosted collector; the audit log is operator-side
  and local, not telemetry.
- P9 P2P compute workers: workers run with no network egress by default
  (P9-M3-T03) and therefore cannot be a telemetry channel. This decision is
  the policy layer under that sandbox.
- P2/P3/P4 readers and P6 geometry core (libraries): no telemetry surface at
  all. Crash diagnostics are the caller's choice via the shared opt-in flag.
- P10/P12 packaging: installers and containers ship telemetry off, with the
  documented single opt-in for crash reports.

## 9. References

- charter.md, goal 5 (sovereignty defaults)
- board.program.json, invariants (clean-room, self-hosted, Rust-first)
- P0-M2-T01 legal review (export-control and data posture)
- P0-M4-T01 (CI skeleton where the deny-list scan lands)
- P0-M5-T03 (release train and versioning; telemetry posture ships per release)
- P1-M1-T06 (privacy/ITAR sweep discipline that the "never send material"
  rule extends)

## Change log

- 1.0.0 (2026-10-15): initial record. Decides no telemetry by default,
  opt-in crash reports only, per P0-M5-T02. Ratified.
