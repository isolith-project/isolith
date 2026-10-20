#!/usr/bin/env python3
"""Acceptance test for Keelworks P9-M1-T02: message bus decision + topic taxonomy.

Written FIRST (TDD). The decision record at workstreams/compute/spec/message-bus-adr.md and the
topic taxonomy at workstreams/compute/spec/topic-taxonomy.json/.md must make this test pass. It
verifies the real material, not mocks:

  decision      => NATS self-hosted as the default/adopted message bus
  sovereignty   => self-hosted, no cloud dependency stated as binding
  alternatives  => Redis, RabbitMQ, Kafka/Redpanda, MQTT, ZeroMQ, Zenoh and
                   the cloud busses (SQS/SNS, Pub/Sub, Service Bus) are weighed,
                   rejected or deferred with reasons
  license       => NATS (Apache-2.0) assessed against the program hybrid posture
  clean-room    => binding an open-source, publicly licensed broker is not a
                   clean-room violation
  guardrails    => version pin, escape hatch (Redis), test gate, auth via the
                   P8 identity layer, no cloud egress from workers
  real material => the committed NATS probe (workstreams/compute/spec/spike/nats_probe.py) and its
                   run-log (workstreams/compute/spec/spike/run-log.json) recorded against a
                   self-hosted nats-server on the reference host, loopback only
  taxonomy       => machine-readable subject taxonomy committed and validated:
                   well-formed subjects and patterns, NATS wildcard semantics
                   (* one token, > trailing 1+ tokens), publish coverage by
                   subscriptions, consistent stream/KV topology

Plus the program surface-language rule (no em dashes, no curly quotes).

Run:  python3 tests/test_p9m1t02_message_bus_taxonomy.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only (python3 + pytest in CI).
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/compute/spec", "message-bus-adr.md")
TAX_JSON = os.path.join(ROOT, "workstreams/compute/spec", "topic-taxonomy.json")
TAX_MD = os.path.join(ROOT, "workstreams/compute/spec", "topic-taxonomy.md")
SPIKE_PY = os.path.join(ROOT, "workstreams/compute/spec", "spike", "nats_probe.py")
RUN_LOG = os.path.join(ROOT, "workstreams/compute/spec", "spike", "run-log.json")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
VERSION_LINE = re.compile(r"^\d+\.\d+\.\d+")
EM_DASH = "\u2014"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
    if not text.startswith("---"):
        return {}, text
    _, fm, body = text.split("---", 2)
    meta = {}
    for line in fm.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or ":" not in stripped:
            continue
        key, _, val = stripped.partition(":")
        meta[key.strip()] = val.strip()
    return meta, body


def section_text(body, heading):
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


# --- NATS subject model (public client-protocol spec) ---------------------

TOKEN_RE = re.compile(r"^(?:[a-z0-9_*-]*[*]?[a-z0-9_-]*|[a-z0-9_-]+)$")


def _token_ok(tok):
    """A token is a literal (a-z 0-9 _ -) or a whole-token wildcard (* or >)."""
    if tok == "*" or tok == ">":
        return True
    return bool(re.fullmatch(r"[a-z0-9_-]{1,64}", tok))


def validate_subject(subj, allow_wildcards):
    """Return list of problems ([]) if subj is a well-formed NATS subject."""
    errs = []
    if not subj or subj != subj.strip():
        errs.append("empty or whitespace-padded")
        return errs
    if len(subj) > 256:
        errs.append("longer than 256 chars")
    toks = subj.split(".")
    if len(toks) > 16:
        errs.append("more than 16 tokens")
    for i, t in enumerate(toks):
        if not _token_ok(t):
            errs.append("bad token %r" % t)
        if ">" in t and t != ">":
            errs.append("'>' must be a whole token, got %r" % t)
        if "*" in t and t != "*":
            errs.append("'*' must be a whole token, got %r" % t)
    if ">" in toks and toks.index(">") != len(toks) - 1:
        errs.append("'>' wildcard must be the last token (%s)" % subj)
    if not allow_wildcards and ("*" in toks or ">" in toks):
        errs.append("publish subject must not contain wildcards (%s)" % subj)
    return errs


def subject_matches(pattern, subject):
    """NATS wildcard matching: * one token, > one or more trailing tokens."""
    p = pattern.split(".")
    s = subject.split(".")
    i = 0
    for j, tok in enumerate(p):
        if tok == ">":
            return i < len(s)
        if i >= len(s):
            return False
        if tok == "*":
            i += 1
            continue
        if tok != s[i]:
            return False
        i += 1
    return i == len(s)


# --- checks ---------------------------------------------------------------

def check_taxonomy(tax):
    t = tax
    check("taxonomy_version" in t, "taxonomy JSON must carry taxonomy_version")
    if "taxonomy_version" in t:
        check(SEMVER.match(t["taxonomy_version"]),
              "taxonomy_version must be semver, got %r" % t.get("taxonomy_version"))
    check(t.get("domain_root") == "oep",
          "taxonomy domain_root must be 'oep'")
    rules = t.get("naming_rules", {})
    check(rules, "taxonomy JSON must carry naming_rules")
    if rules:
        check(rules.get("max_tokens") == 16, "naming_rules.max_tokens must be 16")
        check(rules.get("max_chars") == 256, "naming_rules.max_chars must be 256")

    areas = t.get("areas", {})
    check(isinstance(areas, dict) and len(areas) >= 4,
          "taxonomy must define at least 4 areas (jobs/workers/nodes/vault/rpc/internal)")

    pubs = t.get("publish", [])
    subs = t.get("subscribe", [])
    check(isinstance(pubs, list) and pubs, "taxonomy publish list must be non-empty")
    check(isinstance(subs, list) and subs, "taxonomy subscribe list must be non-empty")

    pub_set = []
    for i, p in enumerate(pubs):
        s = p.get("subject", "")
        for e in validate_subject(s, allow_wildcards=False):
            check(False, "publish[%d] %s invalid: %s" % (i, s, e))
        pub_set.append(s)
        check("area" in p, "publish[%d] %s must carry an area" % (i, s))

    for i, s in enumerate(subs):
        pat = s.get("pattern", "")
        for e in validate_subject(pat, allow_wildcards=True):
            check(False, "subscribe[%d] %s invalid: %s" % (i, pat, e))

    # publish coverage by subscriptions (the reason the taxonomy exists)
    for s in pub_set:
        if not any(subject_matches(x.get("pattern", ""), s) for x in subs):
            check(False, "publish subject %s matches no subscription pattern" % s)

    # no orphan subscription patterns
    for x in subs:
        pat = x.get("pattern", "")
        if not any(subject_matches(pat, s) for s in pub_set):
            check(False, "subscribe pattern %s matches no publish subject" % pat)

    # each area must appear on both sides
    for area in areas:
        if not any(p.get("area") == area for p in pubs):
            check(False, "area %r has no publish subject" % area)
        if not any(s.get("area") == area for s in subs):
            check(False, "area %r has no subscription pattern" % area)

    # subject templates: every concrete publish subject must match a declared
    # template pattern of its own area (template {x} placeholders -> *)
    templates = t.get("subject_templates", [])
    if templates:
        by_area = {}
        for tmpl in templates:
            by_area.setdefault(tmpl.get("area"), []).append(tmpl.get("pattern", ""))
        for p in pubs:
            pats = by_area.get(p.get("area"), [])
            if not pats:
                check(False, "area %r has no subject template" % p.get("area"))
                continue
            conv = [re.sub(r"\{[^}]+\}", "*", x) for x in pats]
            if not any(subject_matches(x, p.get("subject", "")) for x in conv):
                check(False, "publish %s does not fit any template of area %r"
                      % (p.get("subject", ""), p.get("area")))

    # streams and kv topology present
    check(isinstance(t.get("streams"), list) and t["streams"],
          "taxonomy must carry a streams topology list")
    check(isinstance(t.get("kv"), list) and t["kv"],
          "taxonomy must carry a KV bucket list")


def check_run_log(tax):
    if not os.path.isfile(RUN_LOG):
        check(False, "missing spike run-log: workstreams/compute/spec/spike/run-log.json")
        return
    log = json.loads(read(RUN_LOG))
    check(log.get("all_passed") is True, "run-log all_passed must be true")
    check(VERSION_LINE.match(log.get("server_version", "")),
          "run-log must record a real nats-server version, got %r"
          % log.get("server_version"))
    check(log.get("host_connected") == "127.0.0.1",
          "probe must have connected to loopback only (self-hosted), got %r"
          % log.get("host_connected"))
    check(log.get("no_cloud_endpoint") is True,
          "run-log must state no cloud endpoint was contacted")
    check(log.get("request_reply_ok") is True, "run-log request-reply must pass")
    check(log.get("wildcard_star_single_ok") is True,
          "run-log * single-token wildcard probe must pass")
    check(log.get("wildcard_gt_trailing_ok") is True,
          "run-log > trailing wildcard probe must pass")
    check(log.get("invalid_middle_wildcard_rejected") is True,
          "run-log must record the server rejecting a > wildcard in mid-subject")
    rcv = log.get("received_by_sid", {})
    check(len(rcv) >= 4, "run-log received_by_sid must cover multiple sids")

    pub_subjects = {p.get("subject") for p in tax.get("publish", [])}
    sent = log.get("subjects_sent", [])
    for s in sent:
        check(s in pub_subjects,
              "probe sent %r which is not in the taxonomy publish list" % s)


def run_checks():
    print("P9-M1-T02 message bus decision + topic taxonomy acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 0. ADR exists and is versioned.
    check(os.path.isfile(ADR), "missing: workstreams/compute/spec/message-bus-adr.md")
    if not os.path.isfile(ADR):
        return failures
    text = read(ADR)
    meta, body = split_front_matter(text)
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    for key in ("status", "date", "owner", "decision", "applies_to"):
        check(key in meta and meta[key].strip(),
              "front matter must carry %r" % key)
    low = text.lower()

    # 1. NATS is the adopted/default message bus.
    check("nats" in low, "must name NATS explicitly")
    check("message bus" in low or "bus" in low, "must be scoped to the bus")
    check("adopt" in low or "default" in low, "must state NATS as adopted/default")

    # 2. Sovereignty: self-hosted, no cloud dependency.
    check("self-hosted" in low or "self hosted" in low,
          "must state the bus is self-hosted")
    check("no cloud" in low or "cloud-free" in low or "without cloud" in low
          or "no cloud dependency" in low or "no cloud service" in low,
          "must state the no-cloud / sovereignty requirement")
    check("sovereignty" in low, "must name the sovereignty principle")

    # 3. Alternatives are weighed and rejected/deferred with reasons.
    opts = section_text(body, "Options considered")
    oml = opts.lower()
    check(bool(oml), "must have an 'Options considered' section")
    named = 0
    for alt in ("redis", "rabbitmq", "kafka", "mqtt", "zeromq", "zenoh"):
        if alt in oml:
            named += 1
    check(named >= 4,
          "options section must weigh at least 4 concrete alternatives "
          "(got %d: Redis/RabbitMQ/Kafka/MQTT/ZeroMQ/Zenoh)" % named)
    check("cloud" in oml, "options must address the cloud busses (SQS/Pub-Sub)")
    check("reject" in oml or "rejected" in oml or "defer" in oml,
          "alternatives must be rejected or deferred with a reason")

    # 4. JetStream role for durability / retries / dead-letter (P9-M2).
    check("jetstream" in low, "must adopt JetStream for durable queues")
    check("dead-letter" in low or "dlq" in low,
          "must cover the dead-letter path (P9-M2-T01)")
    check("at-least-once" in low, "must cover at-least-once delivery (P9-M2-T03)")
    check("idempotent" in low, "must cover idempotent jobs")

    # 5. Worker sandbox ties: bus is the inbound only, no egress (P9-M3-T03).
    check("egress" in low, "must address no network egress from workers")

    # 6. License assessment.
    lic = section_text(body, "License")
    check(bool(lic), "must have a 'License' section")
    if lic:
        ll = lic.lower()
        check("apache" in ll, "license section must name the NATS Apache-2.0 license")
        check("agpl" in ll or "license posture" in ll or "hybrid" in ll,
              "license section must tie to the program license posture")

    # 7. Clean-room stance.
    cr = section_text(body, "Clean-room")
    check(bool(cr), "must have a 'Clean-room' section")
    if cr:
        lowcr = cr.lower()
        check("open" in lowcr and ("source" in lowcr or "public" in lowcr),
              "clean-room section must note NATS is open source")

    # 8. Guardrails: version pin, escape hatch, test gate.
    grd = section_text(body, "Guardrails")
    check(bool(grd), "must have a 'Guardrails' section")
    if grd:
        gl = grd.lower()
        check("pin" in gl or "version" in gl,
              "guardrails must pin a NATS version")
        check("redis" in gl and ("escape" in gl or "fallback" in gl
                                 or "backup" in gl),
              "guardrails must keep Redis as the escape hatch")
        check("test" in gl or "probe" in gl or "acceptance" in gl
              or "ci" in gl,
              "guardrails must require a test/probe gate")

    # 9. The committed taxonomy is the contract.
    check(os.path.isfile(TAX_JSON), "missing: workstreams/compute/spec/topic-taxonomy.json")
    check(os.path.isfile(TAX_MD), "missing: workstreams/compute/spec/topic-taxonomy.md")
    if os.path.isfile(TAX_JSON):
        tax = json.loads(read(TAX_JSON))
        check_taxonomy(tax)
        # ADR must reference the taxonomy artefact.
        check("topic-taxonomy.json" in text or "topic-taxonomy" in text,
              "ADR must reference the committed taxonomy")
        # Cross-references to downstream cards.
        check("P9-M2" in text, "must tie to the scheduler cards P9-M2")
        check("P9-M3" in text, "must tie to the worker cards P9-M3")
        check("P9-M1-T02" in text, "must cite its own card P9-M1-T02")

    # 10. Real material: spike source + run-log.
    check(os.path.isfile(SPIKE_PY), "missing spike: workstreams/compute/spec/spike/nats_probe.py")
    if os.path.isfile(SPIKE_PY):
        sp = read(SPIKE_PY)
        check("socket" in sp, "spike must use the raw NATS protocol over a socket")
        check("CONNECT" in sp, "spike must speak the NATS client protocol")
        check(">)" in sp or "oep." in sp, "spike must exercise taxonomy subjects")
    if os.path.isfile(TAX_JSON):
        tax = json.loads(read(TAX_JSON))
        check_run_log(tax)

    # 11. Surface language rule: no em dashes, no curly quotes (program rule).
    for path, label in ((ADR, "ADR"), (TAX_MD, "taxonomy md")):
        if os.path.isfile(path):
            doc = read(path)
            check(doc.count(EM_DASH) == 0, "%s contains em dash(es)" % label)
            check(doc.count("\u201c") + doc.count("\u201d")
                  + doc.count("\u2018") + doc.count("\u2019") == 0,
                  "%s contains curly quote(s)" % label)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P9-M1-T02 MESSAGE BUS ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P9-M1-T02 MESSAGE BUS ACCEPTANCE PASSED: "
          "NATS self-hosted default with no cloud dependency, alternatives + "
          "license + clean-room + guardrails recorded, topic taxonomy committed "
          "and validated, live NATS probe log verified on real material.")
    return 0


def test_p9m1t02_message_bus_taxonomy_acceptance():
    failures = run_checks()
    assert not failures, "P9-M1-T02 acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
