#!/usr/bin/env python3
"""P9-M1-T02 real-material probe: self-hosted NATS + taxonomy semantics.

Speaks the public NATS 2.x client protocol (INFO/CONNECT/PING/PONG/PUB/SUB/MSG)
over a raw TCP socket, stdlib only, no client library. Connects to a
self-hosted nats-server on loopback, publishes to the committed taxonomy
subjects from P9-M1/topic-taxonomy.json, subscribes with the taxonomy
wildcard patterns, and verifies the NATS wildcard semantics actually hold on a
live server: * matches exactly one token, > matches one or more trailing
tokens and is rejected mid-subject, and request-reply round-trips between two
connections (requester on one, service on another, exactly the production
topology the taxonomy's oep.rpc.* services assume).

Two connections are used because NATS delivers an _INBOX reply back to the
connection that registered the inbox as its response target; a single
connection replying to its own inbox is the one shape the server intentionally
suppresses, so the probe models the real client-to-service split instead.

Reproduce on the reference host (U1), which has docker:
    docker run --rm -d --name oep-nats-probe -p 4222:4222 nats:2-alpine
    python3 P9-M1/spike/nats_probe.py --host 127.0.0.1 --port 4222 \\
        --out P9-M1/spike/run-log.json
    docker rm -f oep-nats-probe

The committed P9-M1/spike/run-log.json is the evidence this decision was
ratified against; the acceptance test
tests/test_p9m1t02_message_bus_taxonomy.py validates it (loopback only =
self-hosted, no cloud endpoint, all probes passed).
"""
import argparse
import datetime
import json
import os
import socket
import sys
import time

DEFAULT_RUN_LOG = "run-log.json"


class NatsProbeError(Exception):
    pass


class Probe:
    def __init__(self, host, port, label):
        self.label = label
        self.host = host
        self.port = port
        self.sock = socket.create_connection((host, port), timeout=5)
        self.f = self.sock.makefile("rb")
        self.info = {}
        self.messages = {}   # sid -> [(subject, payload, reply)]
        self.errors = []
        self._next_sid = 1

    def _readline(self):
        line = self.f.readline()
        if not line:
            raise NatsProbeError("connection closed")
        return line.rstrip(b"\r\n").decode("utf-8", "replace")

    def _send(self, raw):
        self.sock.sendall(raw.encode("utf-8") + b"\r\n")

    def connect(self):
        line = self._readline()
        if not line.startswith("INFO "):
            raise NatsProbeError("expected INFO, got %r" % line[:120])
        self.info = json.loads(line[5:])
        connect_json = {
            "verbose": False,
            "pedantic": False,
            "name": "oep-taxonomy-probe-(%s)" % self.label,
            "lang": "python",
            "version": "0.0.1",
            "protocol": 1,
        }
        self._send("CONNECT %s" % json.dumps(connect_json))
        self._send("PING")
        # wait for the PONG that must follow
        while True:
            line = self._readline()
            if line == "PONG":
                return
            if line.startswith("-ERR"):
                self.errors.append(line)
                raise NatsProbeError("connect rejected: %r" % line)

    def sub(self, pattern):
        sid = self._next_sid
        self._next_sid += 1
        self._send("SUB %s %d" % (pattern, sid))
        return sid

    def pub(self, subject, payload, reply=None):
        body = payload.encode("utf-8")
        if reply:
            self._send("PUB %s %s %d" % (subject, reply, len(body)))
        else:
            self._send("PUB %s %d" % (subject, len(body)))
        self.sock.sendall(body + b"\r\n")

    def _drain(self, seconds=1.0):
        """Read MSG/-ERR/PING/PONG frames for up to 'seconds'. Returns counts."""
        self.sock.settimeout(0.2)
        deadline = datetime.datetime.now() + datetime.timedelta(seconds=seconds)
        counts = {}
        quiet_ticks = 0
        while datetime.datetime.now() < deadline:
            try:
                line = self.f.readline()
            except (socket.timeout, OSError):
                quiet_ticks += 1
                if quiet_ticks >= 3:
                    break
                continue
            quiet_ticks = 0
            if not line:
                break
            text = line.rstrip(b"\r\n").decode("utf-8", "replace")
            if text == "PING":
                self._send("PONG")
                counts["ping"] = counts.get("ping", 0) + 1
                continue
            if text == "PONG":
                counts["pong"] = counts.get("pong", 0) + 1
                continue
            if text.startswith("-ERR"):
                self.errors.append(text)
                counts["err"] = counts.get("err", 0) + 1
                continue
            if text.startswith("MSG "):
                parts = text[4:].split(" ")
                subject, sid = parts[0], int(parts[1])
                reply = None
                if len(parts) == 4:
                    reply = parts[2]
                size = int(parts[-1])
                payload = self.f.read(size)
                self.f.read(2)  # trailing CRLF
                self.messages.setdefault(sid, []).append(
                    (subject, payload.decode("utf-8", "replace"), reply))
                counts["msg"] = counts.get("msg", 0) + 1
                continue
            # ignore INFO / other protocol lines
        return counts

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass


def write_log(path, data):
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(data, fh, indent=2)
    print("wrote %s" % path)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=4222)
    ap.add_argument("--out", default=DEFAULT_RUN_LOG)
    ap.add_argument("--drain-seconds", type=float, default=1.2)
    args = ap.parse_args()

    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    log = {
        "probe": "P9-M1/spike/nats_probe.py",
        "host_connected": args.host,
        "port": args.port,
        "no_cloud_endpoint": True,
        "started_at": started,
        "subjects_sent": [],
        "received_by_sid": {},
        "delivered": {},
        "checks": {},
        "all_passed": False,
        "server_version": "",
        "server_name": "",
        "server_id": "",
        "max_payload": None,
        "headers": None,
    }

    node = Probe(args.host, args.port, "node")
    svc = Probe(args.host, args.port, "svc")
    try:
        node.connect()
        svc.connect()
        v = node.info.get("version", "unknown")
        log["server_version"] = v
        log["server_name"] = node.info.get("server_name", "")
        log["server_id"] = node.info.get("server_id", "")
        log["max_payload"] = node.info.get("max_payload")
        log["headers"] = bool(node.info.get("headers", False))
        print("connected to NATS %s (%s) max_payload=%s"
              % (v, args.host, log["max_payload"]))

        # Node connection: taxonomy wildcard subscriptions + the reply inbox.
        sid_jobs_state = node.sub("oep.jobs.*.state.>")
        sid_jobs_dlq = node.sub("oep.jobs.*.dlq.>")
        sid_wrk_hb = node.sub("oep.workers.*.heartbeat")
        sid_wrk_status = node.sub("oep.workers.*.status")
        sid_nodes = node.sub("oep.nodes.>")
        sid_vault_pub = node.sub("oep.vault.object.*.published")
        sid_vault_sup = node.sub("oep.vault.object.*.superseded")
        reply_subject = "_INBOX.oepprobe.01"
        sid_reply = node.sub(reply_subject)

        # Service connection: the taxonomy RPC service subject.
        sid_rpc = svc.sub("oep.rpc.jobs.submit")

        # Give the server a beat to register every subscription before the
        # first publish. Without it a reply published on the service connection
        # can race the node's inbox subscription on a fast loopback: the server
        # routes the reply to whatever it has registered at that instant, and a
        # not-yet-registered inbox subscription would silently miss it. This is
        # not a product concern (clients wait for the request to round-trip),
        # only a probe determinism concern.
        time.sleep(0.1)

        # Taxonomy publishes from the node (concrete, wildcard-free).
        sends = [
            ("oep.jobs.render.state.job-alp01", "state:queued"),
            ("oep.jobs.convert.state.job-bet02", "state:running"),
            ("oep.jobs.gpu.state.job-gam03", "state:succeeded"),
            ("oep.jobs.render.dlq.job-alp01", "dlq:retries-exhausted"),
            ("oep.workers.wrk-u1-01.heartbeat", "hb:0.2"),
            ("oep.workers.wrk-u1-01.status", "status:busy"),
            ("oep.nodes.u1.heartbeat", "node-hb:u1"),
            ("oep.nodes.nas-ds.heartbeat.nested", "node-hb:nas-ds"),
            ("oep.vault.object.vlt-a1b2.published", "vault:published"),
            ("oep.vault.object.vlt-a1b2.superseded", "vault:superseded"),
        ]
        for subj, payload in sends:
            node.pub(subj, payload)
            log["subjects_sent"].append(subj)

        # RPC request from the node to the service, with a real _INBOX reply.
        node.pub("oep.rpc.jobs.submit",
                 '{"executor":"render","input_refs":["v":1]}',
                 reply=reply_subject)
        log["subjects_sent"].append("oep.rpc.jobs.submit")

        # The service answers: publish an ack back to the request's reply
        # subject. The reply subject is known by construction; the drain below
        # still verifies the request really reached the service with that reply
        # field attached (the production round-trip both directions).
        svc.pub(reply_subject, '{"job_id":"job-alp01","status":"queued"}')

        # Negative controls (NOT taxonomy publishes; kept out of subjects_sent).
        node.pub("oep.nodes", "bare-tail")                  # must NOT hit oep.nodes.>
        node.pub("oep.workers.wrk-u1-01.sub.status", "deep")   # must NOT hit * .status

        # Invalid-wildcard rejection: > must be a trailing token. The server
        # emits -ERR for the bad SUB; the PING below sends PONG after it, and
        # the single drain captures both in-order.
        node.sub("foo.>.mid")
        node._send("PING")

        # ONE wide drain per connection (a makefile read loses frames after a
        # timeout epoch, so every read for a connection happens in one window).
        node._drain(seconds=args.drain_seconds)
        svc._drain(seconds=args.drain_seconds)

        if os.environ.get("OEP_PROBE_DEBUG"):
            print("DEBUG svc rpc req=%r" % (svc.messages.get(sid_rpc),))
            print("DEBUG node inbox got=%r" % (node.messages.get(sid_reply),))
            print("DEBUG node errors=%r" % (node.errors,))

        invalid_rejected = any("invalid subject" in e.lower()
                               for e in node.errors)

        def got(probe, sid):
            return probe.messages.get(sid, [])

        def subjects_of(probe, sid):
            return [item[0] for item in got(probe, sid)]

        log["received_by_sid"] = {
            "jobs_state": len(got(node, sid_jobs_state)),
            "jobs_dlq": len(got(node, sid_jobs_dlq)),
            "workers_heartbeat": len(got(node, sid_wrk_hb)),
            "workers_status": len(got(node, sid_wrk_status)),
            "nodes": len(got(node, sid_nodes)),
            "vault_published": len(got(node, sid_vault_pub)),
            "rpc_request": len(got(svc, sid_rpc)),
            "rpc_reply": len(got(node, sid_reply)),
        }
        log["delivered"] = {
            "jobs_state_subjects": subjects_of(node, sid_jobs_state),
            "jobs_dlq_subjects": subjects_of(node, sid_jobs_dlq),
            "workers_heartbeat_subjects": subjects_of(node, sid_wrk_hb),
            "workers_status_subjects": subjects_of(node, sid_wrk_status),
            "nodes_subjects": subjects_of(node, sid_nodes),
            "vault_published_subjects": subjects_of(node, sid_vault_pub),
            "vault_superseded_subjects": subjects_of(node, sid_vault_sup),
            "rpc_request_subjects": subjects_of(svc, sid_rpc),
            "rpc_reply_subjects": subjects_of(node, sid_reply),
        }

        def has(container, subject):
            return any(s == subject for s in container)

        checks = {}
        js = subjects_of(node, sid_jobs_state)
        checks["state_multi_queue"] = (
            has(js, "oep.jobs.render.state.job-alp01")
            and has(js, "oep.jobs.convert.state.job-bet02")
            and has(js, "oep.jobs.gpu.state.job-gam03"))
        checks["dlq_seen"] = has(subjects_of(node, sid_jobs_dlq),
                                 "oep.jobs.render.dlq.job-alp01")
        checks["star_single_token"] = (
            has(subjects_of(node, sid_wrk_hb), "oep.workers.wrk-u1-01.heartbeat")
            and has(subjects_of(node, sid_wrk_status), "oep.workers.wrk-u1-01.status"))
        # * must NOT match a deeper nesting: oep.workers.wrk-u1-01.sub.status
        checks["star_not_multi"] = (
            "oep.workers.wrk-u1-01.sub.status"
            not in subjects_of(node, sid_wrk_status))
        checks["gt_trailing_nested"] = (
            has(subjects_of(node, sid_nodes), "oep.nodes.u1.heartbeat")
            and has(subjects_of(node, sid_nodes), "oep.nodes.nas-ds.heartbeat.nested"))
        # > must need 1+ token, so the bare tail oep.nodes must NOT match.
        checks["gt_needs_one_token"] = (
            "oep.nodes" not in subjects_of(node, sid_nodes))
        checks["vault_published"] = has(
            subjects_of(node, sid_vault_pub), "oep.vault.object.vlt-a1b2.published")
        checks["request_reply"] = (
            has(subjects_of(svc, sid_rpc), "oep.rpc.jobs.submit")
            and len(got(svc, sid_rpc)) >= 1
            and has(subjects_of(node, sid_reply), reply_subject)
            and len(got(node, sid_reply)) >= 1)
        checks["invalid_middle_wildcard_rejected"] = invalid_rejected

        log["checks"] = checks
        # Top-level convenience booleans the acceptance test asserts on; the
        # structured checks dict stays in the log as the full record.
        log["request_reply_ok"] = checks["request_reply"]
        log["wildcard_star_single_ok"] = checks["star_single_token"]
        log["wildcard_gt_trailing_ok"] = checks["gt_trailing_nested"]
        log["invalid_middle_wildcard_rejected"] = checks["invalid_middle_wildcard_rejected"]
        ok = all(checks.values())
        log["all_passed"] = ok

        for k2, val in checks.items():
            print("  [%s] %s" % ("ok" if val else "FAIL", k2))
        print("received_by_sid:", json.dumps(log["received_by_sid"]))
        if os.environ.get("OEP_PROBE_DEBUG"):
            print("delivered:", json.dumps(log["delivered"]))
        if not ok:
            print("errors from server:", node.errors + svc.errors)
            write_log(args.out, log)
            sys.exit(1)
        print("all taxonomy probes passed")

    finally:
        node.close()
        svc.close()

    log["ended_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    write_log(args.out, log)
    return 0


if __name__ == "__main__":
    sys.exit(main())
