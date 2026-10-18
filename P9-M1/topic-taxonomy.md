---
title: Keelworks OEP Message Topic Taxonomy
taxonomy_version: 1.0.0
date: 2026-10-17
status: Ratified
owner: Keelworks program governance, P9-M1 (message bus decision)
applies_to: P9-M2 scheduler, P9-M3 workers, P9-M4 GPU/heterogeneous lane, P9-M5 ops dashboard, and every publisher/subscriber on the Keelworks message bus
applies_to_json: P9-M1/topic-taxonomy.json (machine-readable contract, source of truth for this document)
---

# Keelworks OEP Message Topic Taxonomy

This document and its machine-readable twin `P9-M1/topic-taxonomy.json`
together form the committed topic taxonomy for the Keelworks message bus. The
JSON file is the contract: every subject defined here is validated by the
acceptance test `tests/test_p9m1t02_message_bus_taxonomy.py` against the NATS
client-protocol subject rules, and subscribers must only subscribe with the
patterns recorded there. Change either face of the taxonomy only through a
versioned update (bump `taxonomy_version` in both the JSON front matter and
this file) and a passing acceptance run.

The taxonomy is decided together with the message bus decision record
`P9-M1/message-bus-adr.md`, which names NATS self-hosted as the default bus
and records why. Read the two together: the ADR decides the transport, this
document decides the vocabulary spoken over it.

## 1. Naming rules

Every subject is a dot-separated token path under the single domain root `oep`.
The rules enforce the NATS client-protocol grammar and our own hygiene:

- Lowercase only; tokens are `a-z 0-9 - _` (hyphen preferred over underscore
  in new subjects; underscores reserved for stream and bucket names).
- Tokens are separated by single dots. No whitespace, no dots inside a token,
  no empty tokens, no leading or trailing dots.
- At most 16 tokens and under 256 characters per subject (the program ceiling,
  matching the documented NATS guidance for comfortable default limits).
- Wildcards appear in SUBSCRIPTIONS only. Publishers always emit fully
  specified subjects. `*` matches exactly one token; `>` matches one or more
  trailing tokens and is only legal as the last token of a pattern.
- Reserved NATS system namespaces (`$SYS`, `$JS`, `$KV`, `_INBOX.*`) are never
  used as product subjects; the taxonomy lives entirely under the `oep` root.
- Identifiers (job-id, worker-id, node-id, vault-id, queue) are lowercase
  `a-z 0-9 - _` and never contain wildcard characters.

Subscribe patterns recorded in the JSON are cast against the publish list by
the acceptance test: every publish subject must be matched by at least one
pattern, and every pattern must match at least one publish subject, so a typo
cannot silently orphan a lane.

## 2. Subject map by area

The taxonomy has six areas. The JSON `publish` list holds concrete examples,
the JSON `subscribe` list holds the patterns consumers use. The canonical
shapes (templates) are:

    oep.jobs.{queue}.state.{job-id}       oep.jobs.{queue}.progress.{job-id}
    oep.jobs.{queue}.log.{job-id}         oep.jobs.{queue}.dlq.{job-id}
    oep.workers.{worker-id}.heartbeat     oep.workers.{worker-id}.status
    oep.nodes.{node-id}.heartbeat         oep.nodes.{node-id}.leave
    oep.vault.object.{vault-id}.published   oep.vault.object.{vault-id}.superseded
    oep.rpc.jobs.submit                   oep.rpc.jobs.status.{job-id}
    oep.rpc.scheduler.stats
    oep.metrics.{node-id}.{metric}        oep.audit.jobs.{job-id}

Job lanes. Queue values are the named lanes: `default`, `priority`, `render`,
`convert`, `gpu`, and per-team lanes. State transitions carry the full job
record plus attempt number; on success the record carries the output vault
object refs (P8 object refs are the input and output currency). Progress and
log subjects carry lighter payloads. Dead-letter entries carry the exhausted
job record and the fatal error, and are also mirrored into the PDM audit store.

Workers and nodes. Worker registration data is pushed to the WORKERS KV bucket
through the PDM idempotency path, and the bus carries only the streaming facets:
heartbeat and status per worker. Nodes are the machine envelope (a node runs
many workers) and announce presence through heartbeat and leave subjects under
`oep.nodes.<node-id>.`, with optional deeper detail segments
(`oep.nodes.<node-id>.<event>.<detail>`) so a trailing `oep.nodes.>` consumer
sees presence at any depth.

Vault. PDM publishes lifecycle events so the scheduler can trigger jobs on
publish and warn consumers on supersede. The `published` event explicitly may
carry a suggested queue, but the scheduler always owns final routing; it is a
trigger, not a command.

RPC. Request-reply services use concrete subjects so a queue group of handler
instances can share one service. Callers publish to the service subject with an
`_INBOX.<token>` reply subject; the first available handler answers. Queue
semantics for read-heavy lookups (status, stats) are supported by NATS request
reply and need no extra machinery.

Internal. The metrics feed backs the P9-M5 ops dashboard (queue depth, node
load, completed-job counters). The audit pointer event references the
tamper-evident record in the PDM audit store (P8-M3-T03); the bus is a pointer,
the PDM store is the source of truth.

## 3. Streams and KV (planned topology)

Durability lives in JetStream, decided in the ADR. The taxonomy commits the
binding of topics to streams and buckets here so P9-M2-T01 (scheduler) and
P9-M2-T02 (fair share) build on one map:

- `JOBS_QUEUE_<QUEUE>`: workqueue retention bound to
  `oep.jobs.<queue>.state.>`. This is the durable assignment queue per lane;
  workers consume via a competing queue group, ack per message, and
  `max_deliver` moves exhausted deliveries to the dead-letter stream.
- `JOBS_STATE`: limits retention over `oep.jobs.*.state.>` for replay and the
  dashboard.
- `JOBS_DLQ`: limits retention over `oep.jobs.*.dlq.>` (the dead-letter sink).
- `WORKER_EVENTS` (interest), `NODE_EVENTS` (interest), `VAULT_EVENTS`
  (limits), `METRICS_<SITE>` (limits window) bind their area roots.
- KV buckets: `QUEUE_STATE` (per-job priority, DAG refs, attempt, owner),
  `NODES` (node registry), `WORKERS` (worker registry with capabilities, slots,
  team), `SCHED_META` (scheduler epoch and placements).

Exact per-stream configuration (dedup window, AckWait, delivery policy) is the
P9-M2-T01 scheduler card's to finalize; the taxonomy fixes the binding map now
so nothing downstream invents a parallel one.

## 4. How to add a subject

1. Add the template and a concrete example to the JSON under the right area,
   and a subscription pattern that covers it if none does.
2. Keep it under the `oep` root, lowercase, within 16 tokens and 256 chars.
3. Run the acceptance test: it will verify the new subject is well formed,
   covered by a pattern, and fits its area template.
4. Bump `taxonomy_version` in the JSON front matter and this file, and log the
   change at the bottom of the ADR.

## 5. Cross-references

- P9-M1-T01 job spec: the state/progress/log payload shapes follow its schema.
- P9-M2-T01 scheduler: queues, priorities, DAG, retries, dead-letter all bind
  to the streams mapped above.
- P9-M2-T02 fair share: per-team queue and consumer-group topology above is
  where quota is enforced.
- P9-M2-T03 distributed state: at-least-once with idempotent jobs uses the
  `QUEUE_STATE` bucket and stream dedup.
- P9-M3-T03 worker sandbox: workers may connect out to the bus and PDM only;
  no other network egress.
- P9-M5-T01 ops dashboard: consumes `oep.metrics.>` and the state/log facets.

## Change log

- 1.0.0 (2026-10-17): ratified with the message bus ADR (P9-M1-T02). Six areas
  under the `oep` root, Jetsstream stream and KV binding map, NATS wildcard
  semantics, machine-readable contract in topic-taxonomy.json.
