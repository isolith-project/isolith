---
title: Keelworks Message Bus Decision (NATS self-hosted default) and Topic Taxonomy
decision: adopt_nats_selfhosted_for_oep_message_bus_with_committed_topic_taxonomy
version: 1.0.0
date: 2026-10-17
status: Ratified
owner: Keelworks program governance, P9-M1 (message bus)
applies_to: P9-M2 scheduler, P9-M3 workers, P9-M4 GPU/heterogeneous lane, P9-M5 ops dashboard, and every publisher/subscriber in the P9 compute lane and across oep-pdm and the P2P mesh
supersedes: nothing (first P9 message bus decision record)
---

# Keelworks Message Bus Decision

This record is the binding product decision for how the Keelworks compute lane
ships messages: it answers the P9-M1-T02 question "NATS self-hosted default +
topic taxonomy" and names NATS, self-hosted, as the message bus, with the topic
taxonomy committed to the repository. It is versioned like the web-stack
decision (P8-M1-T01), the renderer-backend decision (P6-M5-T01), the boolean-ops
decision (P6-M3-T01), and the release train decision (P0-M5-T03); any material
change bumps the version in the front matter and adds a line to the change log
at the bottom.

The decision is ratified against recorded real material: the committed probe in
`P9-M1/spike/` spoke the public NATS 2.x client protocol over a raw socket to a
self-hosted nats-server (2.15.1) container on the reference host U1, published
to the committed taxonomy subjects, subscribed with the committed wildcard
patterns, and verified the wildcard semantics and a request-reply round trip on
the live server. The recorded `run-log.json` shows the full exchange. The
acceptance test in `tests/test_p9m1t02_message_bus_taxonomy.py` asserts the
ADR and the taxonomy and the run-log together, on real material, not on mocks.

## 1. Decision

Keelworks ADOPTS **NATS, self-hosted**, as the message bus for the P9 compute
lane. "Self-hosted" means the bus is one open-source `nats-server` binary
running inside our own infrastructure (the same self-hosted stack as oep-pdm,
on the NAS and the U1/GPU nodes), with no connection to any externally hosted
service. Together with the bus, Keelworks commits the **topic taxonomy** in
`P9-M1/topic-taxonomy.json` (machine-readable contract) and
`P9-M1/topic-taxonomy.md` (human reference) as the complete set of subjects and
subscription patterns publishers and consumers may use.

The decision covers the transport, the durability layer, and the vocabulary:

1. Transport: NATS core pub/sub with the two documented wildcards (`*`, exactly
   one token; `>`, one or more trailing tokens, last position only). Subjects
   live under the `oep` root per the taxonomy.
2. Durability: **JetStream** for persistent job queues, dead-letter, journals,
   and the key-value buckets the scheduler needs (queue state, node registry,
   worker registry, scheduler meta).
3. Request-reply: NATS request-reply over concrete `oep.rpc.*` service subjects
   with `_INBOX.*` reply subjects for submit/cancel/status/stats.
4. Vocabulary: the committed topic taxonomy. Six areas under the `oep` root:
   jobs, workers, nodes, vault, rpc, internal.

The card's acceptance requires "self-hosted, no cloud dependency (sovereignty
principle)". That is a hard product boundary, not a preference: every component
of the compute lane must run inside the fence, work offline or air-gapped
(P10-M2-T02), and never depend on a cloud bus. NATS satisfies it with a single
open-source binary; Section 4 weighs it against alternatives, several of which
are rejected precisely because they break this boundary.

## 2. Why this decision

Six facts drive it:

1. **Self-hosted by construction, sovereign by default.** `nats-server` is a
   single Apache-2.0 binary that runs anywhere our stack runs: the NAS, the
   U1, Docker, GPU nodes. There is no NATS-ingested cloud control plane, no
   external catalogue to phone home to. This is the whole point of the
   sovereignty principle for the compute lane, and the probe proves the full
   exchange against a container on loopback with zero outside traffic.

2. **Core pub/sub plus JetStream covers every P9-M2 requirement in one
   component.** The scheduler card needs queues, priorities, DAG deps, retries,
   and dead-letter; the distributed-state card needs at-least-once with
   idempotent jobs. JetStream consumers give exactly that vocabulary: durable
   streams with workqueue retention for per-queue assignment, consumer acks
   with redelivery for at-least-once, `max_deliver` tipping into a dead-letter
   stream, and stream Dedup as a helper for idempotent replay keyed by job id.
   The fair-share card (P9-M2-T02) maps to per-team queue streams plus the
   `SCHED_META`/`QUEUE_STATE` KV buckets.

3. **Request-reply and wildcard pub/sub are first-class.** The scheduler is a
   mix of event lanes (state, progress, log) and synchronous services (submit,
   cancel, status, stats). NATS ships both natively with one protocol, one
   connection, one auth story. The probe exercised a real request-reply round
   trip and the wildcard patterns against the committed taxonomy.

4. **Multi-node and multi-site routing is ours to keep simple.** NATS clusters
   let the NAS, U1, and GPU nodes form one logical bus, and leafnodes and
   gateways bridge remote sites for P10-M4. Leafnode topology in particular
   lets a remote or air-gapped node attach to the bus while the site keeps its
   own control over what crosses the link.

5. **Rust-native clients match the Rust-first core decision.** async-nats and
   nats.rs are the idiomatic Rust clients, Apache-2.0, and slot into the same
   toolchain and CI as oep-pdm and oep-geo, exactly as axum does for the web
   stack (P8-M1-T01).

6. **Small ops surface fits a self-hosted team.** One server binary, simple
   configuration, no broker-side administrative UI to babysit, no JVM or
   Erlang runtime. Low enough ceremony that the compute lane does not become a
   second product to run.

## 3. Semantics the downstream cards build on

- Queue lanes. The taxonomy names the queue segment (default, priority, render,
  convert, gpu, and per-team lanes). Each lane maps to a `JOBS_QUEUE_<QUEUE>`
  workqueue stream bound to `oep.jobs.<queue>.state.>`; workers consume via a
  competing queue group and ack per message. P9-M2-T01 implements the exact
  stream configuration (AckWait, MaxDeliver, Dedup window) over this binding.
- At-least-once + idempotency. Consumer acks give at-least-once; job
  idempotency keys plus stream dedup make replays safe. State is written by the
  scheduler to `QUEUE_STATE` KV and by the PDM audit store (P8-M3-T03), so a
  node loss does not invent or destroy state (P9-M2-T03).
- Dead-letter. `max_deliver` exhaustion moves the exhausted job to the
  `JOBS_DLQ` stream bound to `oep.jobs.*.dlq.>`; the ops dashboard and an
  alerting subscriber watch `oep.jobs.*.dlq.>`.
- Request-reply RPC. Services listen on concrete `oep.rpc.*` subjects shared by
  a queue group of handler instances; callers reply on `_INBOX.*`. Verified
  live by the probe (two connections: requester on one, service on another).
- Presence. Worker and node liveness is soft (heartbeat subjects with short
  TTL), and the durable source of truth is the `WORKERS`/`NODES` KV buckets
  plus the audit store; losing heartbeats marks a node draining, never erases
  state.
- Worker sandbox. Workers may open connections only to the bus and to PDM; the
  rest of the network is egress-blocked at the sandbox (P9-M3-T03). The bus is
  the only firehose into and out of a job.

## 4. Options considered

1. **NATS (ADOPTED).** Apache-2.0, single self-hosted binary, core pub/sub with
   wildcards, JetStream streams/KV/object store for queues and scheduler state,
   request-reply, clusters and leafnodes for multi-node and multi-site, Rust
   clients. Verified live on U1 against a dockerized 2.15.1 server (Section 6).

2. **Redis Streams + Pub/Sub.** Strong KV and good streams (XADD/XADDGROUP),
   often already present in a homelab stack. Per the published NATS guidance
   and our own requirements, core Redis Pub/Sub is fire-and-forget (a
   subscriber that is slow or absent loses messages; no consumer ack +
   redelivery in pub/sub proper), so the durable job-queue half leans entirely
   on XREADGROUP consumer groups. That works, but the result is two different
   delivery models in one system and no wildcard-topic fan-out equivalent to
   NATS subjects. DEFERRED as the escape hatch (Section 8), not adopted: the
   taxonomy seam keeps the bus library-swappable, and a Redis fallback remains
   the first backup because the scheduler KV wants would align with it.

3. **RabbitMQ (AMQP).** Mature broker, but an Erlang VM plus exchange/binding
   administration that is heavier than a P2P job mesh needs. Topic fan-out is
   config-driven rather than subject-driven, and the ops surface does not match
   a single-binary self-hosted team. REJECTED for wave 1.

4. **Apache Kafka / Redpanda.** Log-centric, JVM-heavy operations (Redpanda is
   Rust but adds a storage/journal plane of its own), consumer-group machinery
   aimed at long retention event history. Our compute lane needs durable lanes
   and journals with modest retention, not an event log of record. REJECTED for
   wave 1; revisit only if unlimited event-history becomes a requirement.

5. **MQTT (EMQX / Mosquitto).** Excellent for IoT and sensor edge (QoS at the
   protocol level), but the core spec has no durable consumer groups or
   scheduler-style job queues with ack and dead-letter as first-class shapes.
   The compute lane would fight the profile. REJECTED as the primary; MQTT
   remains fine for unrelated edge telemetry later.

6. **ZeroMQ.** A brokerless library, not a server: no server-side durability,
   no persistence, no wildcard routing across a heterogeneous fleet without us
   maintaining a full mesh. Wrong shape for persistent job queues. REJECTED.

7. **Zenoh.** Geo-distributed pub/sub with storage, scouting, and gateways;
   conceptually the closest thing to NATS for a multi-site mesh, and Rust-based.
   Ecosystem and operational maturity are not there yet for wave 1. DEFERRED,
   watched as a leafnode-grade alternative for P10 multi-site / air-gap
   phases.

8. **Cloud busses (AWS SQS/SNS, GCP Pub/Sub, Azure Service Bus).** All break
   the sovereignty principle: outbound dependency, data crossing the fence,
   no air-gap path. REJECTED absolutely and permanently for the product; the
   topic taxonomy and the probe design assume loopback-capable self-hosting.

## 5. License

NATS is open source: `nats-server`, nats.rs, and async-nats are Apache-2.0.
That matches the program hybrid posture directly: Apache-2.0 for applications
and libraries; AGPL-3.0 where our own server and P2P orchestration live. An
Apache-2.0 client linked into an AGPL-3.0 server is not a copyleft problem:
Apache-2.0 is permissive, and the AGPL obligation attaches to our own server
code, not to the dependencies it links. There is no copyleft contamination of
either license posture.

## 6. Clean-room

Like the Open CASCADE binding (P6-M3-T01) and the wgpu binding (P6-M5-T01),
binding a public, open-source, permissively licensed broker is a dependency
inclusion, not a copied-source or vendor-binary violation. The program
clean-room invariant (no vendor binaries, no copied source; lawful observation
and public specs only) is not touched by running an Apache-2.0 server we
operate ourselves. Everything above the wire is ours: the payload schemas, the
job spec, the scheduler logic, the topic taxonomy, and the worker sandbox. The
raw NATS client protocol used by the probe is a published public spec, exactly
the "lawful observation" the program prefers.

## 7. Real-material verification

The spike in `P9-M1/spike/` is committed source plus a recorded run-log, and it
is the acceptance-time evidence for this decision:

- Host: the reference host U1, NVIDIA GB10 (arm64). Server: `nats:2-alpine`
  container (nats-server 2.15.1) on loopback 127.0.0.1:4222, all traffic over
  the loopback interface with no cloud endpoint contacted (`run-log.json`
  records `host_connected: "127.0.0.1"` and `no_cloud_endpoint: true`).
- Method: the probe speaks the public NATS 2.x client protocol (INFO, CONNECT,
  PING/PONG, PUB, SUB, MSG) over a raw TCP socket, stdlib only, no client
  library. It publishes the committed taxonomy subjects, subscribes with the
  committed wildcard patterns, and asserts the wildcard semantics on the live
  server.
- Verified live (all `checks` true in run-log.json):
  - `oep.jobs.*.state.>` received state transitions for three different
    queues through one pattern (render, convert, gpu).
  - `oep.jobs.*.dlq.>` received the dead-letter entry.
  - `oep.workers.*.heartbeat` and `oep.workers.*.status` matched one token
    each; the deeper `oep.workers.wrk-u1-01.sub.status` did NOT match the
    `*`-based pattern (exactly-one-token proof).
  - `oep.nodes.>` matched nested subjects (`oep.nodes.nas-ds.heartbeat.nested`)
    but NOT the bare tail `oep.nodes` (> needs at least one token).
  - `oep.vault.object.*.published` matched a published object event.
  - Request-reply round-tripped between two connections: node published to
    `oep.rpc.jobs.submit` with an `_INBOX.oepprobe.01` reply; the service
    received the request with the reply field attached and published an ack to
    the inbox; the node received the ack (rpc_request 1, rpc_reply 1).
  - The server REJECTED a `>` wildcard placed mid-subject
    (`SUB foo.>.mid` -> `-ERR Invalid Subject`), confirming the "last position
    only" rule documented in the taxonomy.
- Reproduce: `docker run --rm -d --name oep-nats-probe -p 4222:4222
  nats:2-alpine`, then `python3 P9-M1/spike/nats_probe.py --host 127.0.0.1
  --port 4222 --out P9-M1/spike/run-log.json`, then `docker rm -f
  oep-nats-probe`. The acceptance test asserts the committed run-log, the
  taxonomy JSON (subject well-formedness, wildcard grammar, publish coverage by
  subscriptions, template fit, stream/KV topology), and the ADR together, on
  real material.

One probe finding is worth recording: two connections (requester and service)
are used for the request-reply leg because a single connection replying to its
own `_INBOX` is the shape the server intentionally suppresses. This mirrors the
production split (a client process and a scheduler handler process), so no
product rule is bent by it.

## 8. Sovereignty and deployment notes

- The bus runs as one `nats-server` in the self-hosted stack (P10-M1-T01),
  bound to the internal mesh interface; no cloud or external dependency.
- Worker sandbox allows bus and PDM egress only (P9-M3-T03); the topology
  diagram in the taxonomy keeps the bus as the single firehose.
- Air-gap: NATS runs fully offline; nothing in the bus requires a registration
  server or catalogue. This keeps the P10-M2-T02 ITAR air-gap install path
  intact.

## 9. Guardrails

- **Version pin.** The repository pins the nats-server major and the client
  crate versions; the probe pinned the 2.x line (2.15.1 at ratification).
  Upgrades are deliberate, tested PRs, not drifts.
- **Escape hatch.** The bus sits behind a thin messaging seam (the taxonomy is
  the contract, not the transport). If NATS stops fitting, Redis Streams
  (Section 4, option 2) is the named first fallback; callers do not change
  because they depend on taxonomy subjects and payloads, not on NATS APIs.
- **Taxonomy is the contract.** New subjects enter through the versioned JSON
  (bump `taxonomy_version`, update both faces) and must pass the acceptance
  test's coverage checks; nobody invents a parallel namespace.
- **No-cloud enforcement.** The acceptance test asserts the run-log records a
  loopback-only connection and no cloud endpoint; the probe itself refuses to
  target any non-loopback host by sitting on a raw local socket. A CI
  reproduction of the probe is documented but the committed run-log is what CI
  asserts, since host runners are not guaranteed docker.
- **Auth and boundary.** NATS authentication binds to the mesh identity layer
  (P8-M3-T01 identity), with TLS between nodes; the sanctum of the bus is the
  internal network, and worker connections are policy-restricted.

## 9. Change log

- 1.0.0 (2026-10-17): ratified. NATS self-hosted as the default message bus for
  the P9 compute lane; topic taxonomy committed (P9-M1/topic-taxonomy.json and
  .md), six areas under the `oep` root; JetStream chosen for durable queues and
  dead-letter; alternatives (Redis, RabbitMQ, Kafka/Redpanda, MQTT, ZeroMQ,
  Zenoh, cloud busses) weighed; sovereignty, license/copyleft, clean-room, and
  guardrails recorded; real-material probe run-log committed (nats-server
  2.15.1, loopback only). Cross-references: P9-M1-T01 job spec, P9-M2-T01/02/03,
  P9-M3-T02/T03, P9-M4, P9-M5, P8-M1-T01, P8-M3-T01 identity, P8-M3-T03 audit,
  P8-M6-T01 REST, P10-M1-T01 self-hosted stack, P10-M2-T02 air-gap,
  P10-M4-T01 multi-site.
