---
title: Keelworks P8 PDM Web-Stack Decision (Rust axum)
decision: adopt_rust_axum_for_oep_pdm_service
version: 1.0.0
date: 2026-10-17
status: Ratified
owner: Keelworks program governance, P8-M1 (oep-pdm service skeleton)
applies_to: crates/oep-pdm (server/PDM), P8-M6-T01 (REST API v1), the P9 P2P orchestrator server (same stack decision, tracking this record)
supersedes: the P7-M2-T02 stub deferral of the framework choice ("stub uses no web framework so P8 owns the framework decision")
---

# Keelworks P8 PDM Web-Stack Decision

This record is the binding product decision for the oep-pdm service web stack:
it answers the P8-M1-T01 question "Rust axum or Python FastAPI for the
self-hosted PDM / ENOVIA-lite service" and names Rust axum as the stack. It is
versioned like the renderer-backend decision (P6-M5-T01), the boolean-ops
decision (P6-M3-T01), the telemetry decision (P0-M5-T02), and the release
train decision (P0-M5-T03); any material change bumps the version in the front
matter and adds a line to the change log at the bottom.

The decision is ratified against the committed service skeleton in
`crates/oep-pdm` (an axum Router served by a single `oep-pdm` binary) plus the
recorded real-material proof in `workstreams/pdm/service/proof/health.json`. The acceptance test
in `tests/test_p8_m1_web_stack_adr.py` asserts this document and the skeleton
and the proof together, on real material, not on mocks. The service serves
`GET /api/v1/health` from the proof file (byte-identical to what a live run
returned on the reference host, U1) and passes the full P8 REST API v1
contract suite inherited from P7-M2-T02, which the skeleton satisfies without
rewriting the stub tests.

## 1. Decision

Keelworks ADOPTS **Rust axum** as the web stack for the oep-pdm PDM service
skeleton, and thereby for the P8 server lane. The service is implemented as an
axum Router over the existing `VaultStore` and `api` contract types, compiled
into a single `oep-pdm` binary, served on the contract base path `/api/v1`.

The P7-M2-T02 stub (std-only HTTP/1.1, deliberately framework-free) is not
replaced as a separate code path: its stable symbols (`StubServer`,
`oep-pdm` client) are retained and now back onto the axum skeleton, so the
browser/modeler client and the P7 integration surface work unchanged. The P7
contract tests, whose server side the stub used to provide, become the
contract tests of the real service: the same test files now drive the axum
server and gate every future P8 change.

Python FastAPI is REJECTED for the server core with reasons (section 4). The
Python ecosystem remains allowed and used for tooling and for independent
contract-test clients, exactly as the Rust-first decision (board invariant,
decision 6) and the hybrid license posture (decision 3) already prescribe.

## 2. Why this decision

Five facts drive it:

1. **Rust-first is a binding board decision and the skeleton has no reason to
   leave it.** The program invariants and the Rust-first decision (decision 6)
   make Rust the default for core; Python is explicitly "allowed for
   tooling". The PDM service is core, not tooling, so Rust needs no special
   pleading here. Choosing axum keeps the whole P8 lane in the same
   toolchain, dependency tree, and CI pipeline as `oep-geo`, `oep-step`, and
   the other core crates.

2. **The contract types are already Rust serde structs.** P7-M2-T02 shipped
   the OpenAPI v1 contract projection as `crates/oep-pdm/src/api.rs` and the
   vault data layer as `store.rs`, both in Rust. An axum server consumes them
   directly with zero serialization-layer rewrite and no FFI or language
   boundary. FastAPI would force either a second description of the same wire
   shapes (drift risk, exactly what the contract-first design is meant to
   kill) or a Rust service behind it, which is the axum path anyway.

3. **axum is the idiomatic Rust HTTP framework and fits the self-hosted
   shape.** Single binary, tokio-native async, middleware story via tower,
   static/low-ceremony routing for a fixed 8-endpoint contract. It is the
   natural successor to the std-only stub transport, which existed only to
   defer this decision; the deferral is now resolved.

4. **AGPL-3.0 is the required license for the server/PDM (decision 3), and
   axum is license-clean under it.** axum (MIT OR Apache-2.0) links freely
   into an AGPL-3.0 application; AGPL-3.0 applies to OUR server code, which
   we license under AGPL-3.0 ourselves. There is no copyleft contamination
   for the Apache-2.0 applications/libraries posture. FastAPI would be no
   better on this axis (BSD-3-Clause), so license does not favor it.

5. **The GA gate (P8-M6-T03) is a multi-user concurrent edit/release cycle on
   the NAS, and the async runtime is a good base for it.** axum/tokio give the
   skeleton a concurrency model that the later state machine, lock protocol
   (P8-M1-T03), and multi-user RBAC (P8-M3) can sit on without a transport
   rewrite. The skeleton owns the transport; the milestones above own the
   domain.

## 3. AGPL-3.0 license posture

Per decision 3 (coarse license posture, ratified 2026-10-15), the PDM server
is AGPL-3.0. This record fixes that at the crate file: `crates/oep-pdm` is the
PDM server, so the service sources carry an AGPL-3.0-or-later SPDX header and
the ADR is the license decision record for that crate. Nothing in the axum
stack changes the license calculus: axum and its transitive dependencies are
permissive, and the separation (AGPL server, Apache-2.0 libraries) is the one
the legal pass (P0-M2) already outlined. The full SPDX pass for every crate is
still owned by P0-M2; this record covers the PDM crate now because the P8 lane
starts now.

## 4. Options considered

1. **Rust axum (ADOPTED).** Idiomatic Rust HTTP framework, tokio-native,
   permissive license, single binary, direct reuse of the existing serde
   contract types and store. Matches the Rust-first invariant and the
   self-hosted PDM shape. Version-pinned in the crate manifest and by the
   workspace lockfile.

2. **Python FastAPI (REJECTED for the server core).** FastAPI is an excellent
   API framework and the Python tooling lane may keep using it for contract
   test clients and tooling. It is rejected as the server core because the
   server is core (Rust-first decision), because serving the Rust contract
   types from Python would create a second wire-shape description (drift
   risk), and because it splits the P8 lane into two toolchains when the
   existing code is already Rust. No license or feature advantage. NOT
   superseded as a tooling language; superseded only as the server language.

3. **Rust actix-web (DEFERRED).** Production-grade and fully capable, but
   axum's tokio-native design, tower middleware story, and the ecosystem's
   momentum make axum the lower-ceremony choice for a fixed small contract.
   Revisit only if axum proves inadequate for P8-M6-T03 concurrency, in which
   case the seam (a Router over the store) makes the swap cheap.

4. **raw hyper/tower HTTP (REJECTED as primary).** Fine for a library or a
   hand-rolled transport, which the stub already was, but it is the wrong
   level for a service that will grow routing, middleware, tracing, and
   multi-user state. The stub used a no-framework transport on purpose; that
   deferral is what this record resolves.

5. **Continue with the std-only transport (REJECTED).** The stub's value was
   deferring the framework choice, not being the framework. Keeping it would
   leave P8-M6-T03 rebuilding routing/middleware/concurrency handling by hand
   for no benefit.

## 5. Contract and queue (the stub becomes a contract test)

The P8-M6-T01 contract lives at `workstreams/pdm/api/openapi-v1.yaml` and is the single
source of truth for the wire. The axum skeleton implements every path of that
contract (health, items catalogue and create, item get, latest-file download,
revision list and create, revision get, revision-file download) with the
contract's status codes, headers, and `{"error":{code,message}}` envelope.
The P7-M2-T02 tests continue to run unchanged against this implementation:

- `crates/oep-pdm/tests/pdm_contract_acceptance.rs` (Rust PdmClient over real
  sockets), which boots `StubServer`; `StubServer` now means the axum
  skeleton, so this file is byte-for-byte the retained stub test, now gating
  the real service.
- `tests/test_p8_pdm_stub_contract.py` (independent Python urllib client),
  which boots the `oep-pdm` binary and also exercises `--persist-dir`
  snapshot persistence.

The client surface (`PdmClient`, `StubServer`) keeps its public API so the P7
browser swap contract in `workstreams/modeler/interchange/pdm-integration.md` holds: same contract,
same base URL, same client, no rewrite. This is the split this card
explicitly asks for: the stub is not a throwaway, it is the contract test.

## 6. Real-material proof

Committed with this record:

- `crates/oep-pdm/src/service.rs` -- the axum Router + `PdmService`, the real
  service skeleton.
- `crates/oep-pdm/src/bin/oep-pdm.rs` -- the single-binary entrypoint.
- `workstreams/pdm/service/proof/health.json` -- the exact bytes a live `GET /api/v1/health`
  returned from a running `oep-pdm` binary on the reference host (U1,
  2026-10-17). `{"status":"ok","service":"oep-pdm","version":"1.0.0"}`.
- The retained contract suites in section 5, which boot the axum server over
  real sockets and verify the whole contract, including a byte-exact STEP
  round-trip on the committed fixture.

The acceptance test `tests/test_p8_m1_web_stack_adr.py` reads both the ADR and
these artifacts on real material, mirroring the renderer-backend ADR test.

## 7. Guardrails

- **Pin the stack.** axum 0.8.x and tokio are pinned in the crate manifest and
  the workspace lockfile; toolchain stays on the P0-M3-T02 pin
  (`rust-toolchain.toml`). No unpinned framework upgrades.
- **Contract over framework.** `workstreams/pdm/api/openapi-v1.yaml` and `api.rs` remain the
  source of truth; the transport layer must not change wire shapes. CI runs
  the contract suite on every push (same gate as P7-M2-T02).
- **Escape hatch.** The Router is a thin layer over `VaultStore`; if axum ever
  fails the P8-M6-T03 concurrency bar, actix-web or raw hyper can replace it
  without touching the store or the client (the P7 client never changes).
- **Language rule.** Server code and docs use the program surface-language
  rule (no em dashes, no curly quotes) enforced by the ADR test.

## 8. Change log

- 1.0.0 (2026-10-17): Ratified. Rust axum adopted for the oep-pdm service
  skeleton; AGPL-3.0 posture recorded for the PDM server per decision 3; the
  P7-M2-T02 stub tests retained as the real service's contract tests.
