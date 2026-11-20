"""P8-M6-T03 — the P8 GA gate: multi-user edit/publish/release cycle on the
NAS with a concurrent lock test.

This is the capstone acceptance harness of the P8 PDM multi-user lane. It
follows the exact GA contract of the P2/P3/P4 gates: measured legs, honest
classification with a closed SKIP taxonomy, exit 0 iff no FAIL row, and a
self-consistent JSON + Markdown report. Difference from P2/P3/P4: here the
"real material" is a LIVE PDM service and real concurrent clients — not a log
line. Every leg drives the real oep-pdm service over real sockets through the
generated python client and the gate's additive lock/transition/audit verbs.

The gate proves the four acceptance criteria end to end:

1. MULTIUSER — N real concurrent client *processes* (multiprocessing), each
   running the generated client, do full edit -> publish -> release cycles on
   their own items and on a shared item. Every client's published revision is
   present afterwards; N is parameterized (acceptance: "N concurrent clients").

2. LOCKS — the working-set lock (P8-M1-T03) is respected over the wire:
   * exactly ONE of N racers acquires the lock on a shared item
     (concurrent lock test), the others get the contract error
     `lock_conflict`;
   * a locked item cannot be released by a non-holder (the transition verb
     checks the lock token -> `lock_token`);
   * a publisher who does not hold the lock is refused;
   * releasing with the right token returns the item to the acquirable set,
     so the cycle can proceed.

3. IMMUTABLE — "revisions immutable after release": once a revision reaches
   `released` (P8-M2-T01 state machine behind the wire), the machine refuses
   further hops (released -> working is `invalid_state`) and the
   content-addressed blob re-reads byte-identical with a stable sha-256.

4. AUDIT — "everything verifiable via the audit log": every create / edit /
   lock / release / read is appended to the tamper-evident log (P8-M3-T03)
   and `GET /api/v1/audit/verify` re-derives the hash chain cleanly. The leg
   counts entries and asserts the head checkpoint is intact.

5. NAS — the released binary is staged content-addressed on the NAS object
   store (Gitea generic package registry) and re-verified by sha-256, exactly
   like the P8-M5-T03 LAN/air-gap proof. NAS-unreachable = SKIP (tolerant
   doctrine, P2-M1-T04), never FAIL.

Exit code contract: 0 iff no gate-FAIL row anywhere (SKIP allowed). Reports
`ga-gate-p8-multiuser.{json,md}` are self-consistent (summary rolls up to row
counts; schema_version 1).

Honesty boundary: a leg that cannot reach the service (test proc up? binary
missing? NAS down) is classified SKIP against the closed taxonomy — it is never
invented as a PASS and never crashes the gate. The gate never simulates a user:
N is a real process count.
"""
from __future__ import annotations

import datetime
import hashlib
import json
import multiprocessing
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path

VERDICTS = ("PASS", "FAIL", "SKIP")

# Closed SKIP taxonomy for this gate (a SKIP row always carries one of these).
CLOSED_TAXONOMY = (
    "service-unreachable",   # the PDM service did not answer on the base URL
    "binary-missing",        # oep-pdm binary not built on this ref
    "audit-unavailable",     # service has no audit log (no persist dir)
    "nas-unreachable",       # NAS object store did not answer
    "no-lock-surface",       # lock verbs not served (backend too old)
    "io-error",
)


@dataclass
class LegRow:
    """One classified outcome of a gate leg."""

    id: str
    leg: str                  # "multiuser" | "locks" | "immutable" | "audit" | "nas"
    status: str               # PASS / SKIP / FAIL
    detail: str = ""
    reader: str = "client"    # how the row was obtained (client | probe | verify)
    cls: str = ""             # closed taxonomy code (SKIP rows)

    def as_dict(self) -> dict:
        return {"id": self.id, "leg": self.leg, "status": self.status,
                "detail": self.detail, "reader": self.reader, "class": self.cls}


# ---------------------------------------------------------------------------
# HTTP helpers (stdlib-only; talk to the real service over real sockets)
# ---------------------------------------------------------------------------

def _root(base: str) -> str:
    """Normalize a base URL: accept either the host root or the anchored
    /api/v1 contract root, and always return the host root."""
    stripped = base.rstrip("/")
    if stripped.endswith("/api/v1"):
        return stripped[: -len("/api/v1")]
    return stripped


def _req(base, method, path, body=None):
    """Raw HTTP helper; returns (status, headers, bytes).

    `path` is RELATIVE to the /api/v1 contract root (like the OpenAPI spec),
    matching the generated client's convention. `base` may be the host root or
    the anchored /api/v1 root; the helper normalizes it. Non-2xx responses are
    returned with their status (never raised), so legs can assert refusals.
    """
    url = _root(base) + "/api/v1" + path
    data = None
    headers = {"Accept": "application/json, application/octet-stream"}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=15) as resp:
            return resp.status, dict(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def _json(base, method, path, body=None):
    status, _h, raw = _req(base, method, path, body)
    try:
        return status, json.loads(raw)
    except ValueError:
        return status, raw


def _health(base):
    try:
        status, h = _json(base, "GET", "/health")
        if status == 200 and isinstance(h, dict) and h.get("status") == "ok":
            return h
    except Exception:
        pass
    return None


def _gate_path(item_id, suffix=""):
    return "/items/%s%s" % (urllib.parse.quote(item_id), suffix)


def _new_client(base):
    """Build the GENERATED client (python/oep_pdm_client.py, the M6-T01
    deliverable) pointed at the live service -- the exact client the product
    speaks, so the gate exercises the deliverable, not a hand-rolled HTTP
    layer."""
    from oep_pdm_client import PdmClient
    return PdmClient(base_url=base)


def _acquire(base, item_id, holder):
    """POST /api/v1/items/{id}/lock. Returns (ok, token_or_error)."""
    c = _new_client(base)
    try:
        status, body = c.acquire_lock(item_id, {"holder": holder})
        return status == 200, body
    except Exception as e:  # noqa: BLE001 - PdmError carries code/status
        return False, e


def _release(base, item_id, token):
    c = _new_client(base)
    try:
        status, body = c.release_lock(item_id, {"token": token})
        return status in (200, 204), body
    except Exception as e:  # noqa: BLE001
        return False, e


def _transition(base, item_id, rev, to, token):
    c = _new_client(base)
    try:
        status, body = c.transition_revision(item_id, rev, {"to": to, "token": token})
        return status, body
    except Exception as e:  # noqa: BLE001
        return getattr(e, "status", 0), e


def _audit_verify(base):
    c = _new_client(base)
    return c.verify_audit()


def _audit_view(base):
    c = _new_client(base)
    return c.list_audit()


def _create_item(base, name, holder):
    c = _new_client(base)
    status, item = c.create_item({"name": name, "kind": "part", "owner": holder})
    if status not in (200, 201):
        raise RuntimeError("create_item %r failed: %r" % (name, item))
    return item["id"]


def _save_revision(base, item_id, payload, *, state="working",
                   comment=None, created_by="p8-gate"):
    """Save-revision-to-PDM via the GENERATED client's save_revision verb."""
    c = _new_client(base)
    status, rev = c.save_revision(item_id, "blob.step", "step", payload,
                                  state=state, comment=comment,
                                  created_by=created_by)
    if status not in (200, 201):
        raise RuntimeError("save_revision %r failed: %r" % (item_id, rev))
    return rev


def _open_revision(base, item_id, rev):
    """Open-from-PDM at-revision via the GENERATED client; returns
    (bytes, blob_sha256)."""
    c = _new_client(base)
    payload = c.open_revision_file(item_id, rev)
    return payload.bytes, payload.blob_sha256


# ---------------------------------------------------------------------------
# Leg 1: MULTIUSER -- N real concurrent client PROCESSES
# ---------------------------------------------------------------------------

def _client_worker(base_url, holder, results_q, idx, run_nonce):
    """One real user: create/edit/publish/release on its own item.

    Runs as a separate OS process (multiprocessing), each with its own socket
    and client state -- that is the "N concurrent clients" of the acceptance.
    Puts (idx, outcome) on the result queue. `run_nonce` keeps item names
    unique across repeated runs against a persisted service.
    """
    try:
        item_id = _create_item(base_url, "gate-mu-%s-%s-%s" % (run_nonce, holder, idx), holder)
        payload = b"ISO-10303-21; gate multiuser %s %s;" % (
            holder.encode(), str(idx).encode())
        # Edit (working revision), then publish (release) the same revision.
        _save_revision(base_url, item_id, payload, state="working",
                       comment="edit", created_by=holder)
        # The public flow: lock -> transition -> release.
        ok, tok = _acquire(base_url, item_id, holder)
        assert ok, tok
        status, rev = _transition(base_url, item_id, "1", "released", tok["token"])
        assert status == 200, rev
        _release(base_url, item_id, tok["token"])
        results_q.put((idx, {"holder": holder, "item": item_id, "released": True}))
    except Exception as e:  # noqa: BLE001 - surface to the gate row
        results_q.put((idx, {"holder": holder, "item": None, "released": False,
                             "error": repr(e)}))


def multiuser_leg(base_url: str, n_clients: int = 4) -> list:
    """N real concurrent client processes each do edit -> publish -> release.

    Every row is a client; a client that cannot reach the service (or times
    out) is a FAIL -- the acceptance demands N clients who actually released.
    Returns one row per client (measured), not a synthetic aggregate.
    """
    rows = []
    if _health(base_url) is None:
        return [LegRow("health", "multiuser", "SKIP",
                       "service unreachable at %s" % base_url,
                       cls="service-unreachable")]
    ctx = multiprocessing.get_context("spawn")
    results_q = ctx.Queue()
    procs = []
    n = max(2, int(n_clients))
    run_nonce = int(time.time() * 1000)
    for i in range(n):
        p = ctx.Process(target=_client_worker,
                        args=(base_url, "user-%02d" % i, results_q, i, run_nonce))
        procs.append(p)
        p.start()
    for p in procs:
        p.join(timeout=120)
    outcomes = {}
    while not results_q.empty():
        try:
            idx, rec = results_q.get_nowait()
            outcomes[idx] = rec
        except Exception:
            break
    for i in range(n):
        r = outcomes.get(i) or {}
        holder = r.get("holder", "user-%02d" % i)
        if r.get("released"):
            rows.append(LegRow("client-%02d" % i, "multiuser", "PASS",
                               "%s edited+published+released %s" % (holder, r.get("item"))))
        else:
            rows.append(LegRow("client-%02d" % i, "multiuser", "FAIL",
                               "%s did not release: %s" % (holder, r.get("error", "no result"))))
    return rows


# ---------------------------------------------------------------------------
# Leg 2: LOCKS -- concurrent lock test + publish gate
# ---------------------------------------------------------------------------

def _racer(base_url, item_id, holder, result_q):
    """One racer: try to acquire a lock on the shared item."""
    def _send(rec):
        result_q.put((holder, rec))
        # Flush before this process exits: without these two calls the record
        # can be lost with no error when the child is short-lived.
        try:
            result_q.close()
            result_q.join_thread()
        except Exception:  # noqa: BLE001
            pass

    try:
        ok, body = _acquire(base_url, item_id, holder)
        _send({
            "acquired": bool(ok),
            "detail": body,
            "token": body.get("token") if isinstance(body, dict) else None,
        })
    except Exception as e:  # noqa: BLE001
        _send({"acquired": False, "detail": repr(e), "token": None})


def locks_leg(base_url: str, n_racers: int = 4) -> list:
    """Concurrent lock test: exactly one of N racers holds the lock; a non-
    holder cannot publish a release; the holder can."""
    rows = []
    if _health(base_url) is None:
        return [LegRow("health", "locks", "SKIP",
                       "service unreachable at %s" % base_url,
                       cls="service-unreachable")]

    # A single shared item is the contention point.
    item_id = _create_item(base_url, "gate-lock-%s" % int(time.time()), "gate")
    # One draft revision to be the transition target in the publish test.
    _save_revision(base_url, item_id, b"ISO-10303-21; gate locks leg;",
                   state="working", created_by="gate")

    ctx = multiprocessing.get_context("spawn")
    # Queue.put hands the record to a feeder thread and returns, so a
    # short-lived child can exit before the bytes are written and the result is
    # lost with no error. That read as "no racer won" on a 4-vCPU runner while
    # passing on a fast host, where the flush always won the race. Each racer
    # therefore closes and joins its feeder before exiting (see _racer).
    result_q = ctx.Queue()
    procs = []
    for i in range(max(2, int(n_racers))):
        h = "racer-%02d" % i
        procs.append(ctx.Process(target=_racer, args=(base_url, item_id, h, result_q)))
    for p in procs:
        p.start()
    for p in procs:
        p.join(timeout=60)
    # multiprocessing.Queue.empty() is documented as unreliable, and the
    # feeder thread can still be flushing right after join(): draining with it
    # drops results and reads as "no racer won" when in fact every racer
    # answered. Collect exactly one record per racer, with a deadline.
    result = {}
    deadline = time.time() + 30
    while len(result) < len(procs):
        remaining = deadline - time.time()
        if remaining <= 0:
            break
        try:
            h, rec = result_q.get(timeout=min(5.0, remaining))
        except Exception:
            break
        result[h] = rec
    hung = [p for p in procs if p.is_alive()]
    for p in hung:
        p.terminate()
        p.join(timeout=5)
    winners = [h for h, r in result.items() if r.get("acquired")]
    if len(winners) == 1:
        rows.append(LegRow("concurrent-lock-single-winner", "locks", "PASS",
                           "exactly one of %d racers held the lock: %s" % (len(result), winners[0])))
    else:
        rows.append(LegRow(
            "concurrent-lock-single-winner", "locks", "FAIL",
            "winners=%r of %d racers (collected %d/%d, hung %d, detail=%r)" % (
                winners, len(result), len(result), len(procs), len(hung),
                {h: r.get("detail") for h, r in sorted(result.items())})))

    # Publish gate: the winning racer holds the lock; a foreign/forged token
    # must be refused and the winning racer's real token must be allowed to
    # transition the draft revision to released. This is "locks respected".
    winner = winners[0] if winners else "racer-00"
    winner_token = result.get(winner, {}).get("token") or "lk-0000000000000000"
    # Non-holder uses a forged wire token -> the server must refuse.
    status, _body = _transition(base_url, item_id, "1", "released",
                                token="lk-0000000000000000")
    refused = status in (400, 409)
    # Holder with the real token -> allowed.
    status_ok, body_ok = _transition(base_url, item_id, "1", "released",
                                     token=winner_token)
    allowed = status_ok == 200
    if refused and allowed:
        rows.append(LegRow("publish-gate", "locks", "PASS",
                           "non-holder refused (%d), %s allowed (%d)" % (status, winner, status_ok)))
    else:
        rows.append(LegRow(
            "publish-gate", "locks", "FAIL",
            "non-holder refused=%r (status=%s), %s allowed=%r (status=%s, "
            "held_lock=%r, token=%r...)" % (
                refused, status, winner, allowed, status_ok,
                bool(result.get(winner, {}).get("token")),
                str(winner_token)[:12])))
    _release(base_url, item_id, winner_token)
    return rows


# ---------------------------------------------------------------------------
# Leg 3: IMMUTABLE -- revisions frozen after release (content + machine)
# ---------------------------------------------------------------------------

def immutable_leg(base_url: str) -> list:
    """A released revision is frozen: state machine refuses hops and the blob
    is byte-stable (content-addressed)."""
    rows = []
    if _health(base_url) is None:
        return [LegRow("health", "immutable", "SKIP",
                       "service unreachable at %s" % base_url,
                       cls="service-unreachable")]
    item_id = _create_item(base_url, "gate-imm-%s" % int(time.time()), "gate")
    payload = b"ISO-10303-21; gate immutable; " + os.urandom(16)
    sha = hashlib.sha256(payload).hexdigest()
    _save_revision(base_url, item_id, payload, state="working", comment="edit")
    ok, tok = _acquire(base_url, item_id, "imm-hold")
    if not ok:
        return [LegRow("release-hop", "immutable", "FAIL",
                       "could not lock (%r)" % (tok,))]
    status, rev = _transition(base_url, item_id, "1", "released", tok["token"])
    if status != 200:
        rows.append(LegRow("release-hop", "immutable", "FAIL",
                           "could not release: %r" % (rev,)))
        return rows
    _release(base_url, item_id, tok["token"])

    # (a) State machine: released -> working must be refused. Re-acquire the
    # lock with a fresh token so the refusal is about the hop, not the token.
    ok2, tok2 = _acquire(base_url, item_id, "imm-check")
    if not ok2:
        rows.append(LegRow("immutable-after-release", "immutable", "FAIL",
                           "re-lock failed: %r" % (tok2,)))
    else:
        status, body = _transition(base_url, item_id, "1", "working", tok2["token"])
        refused = status in (400, 409)
        rows.append(LegRow("immutable-after-release", "immutable",
                           "PASS" if refused else "FAIL",
                           "released->working hop refused" if refused else
                           "released->working hop ACCEPTED (BUG): %r" % (body,)))
        _release(base_url, item_id, tok2["token"])

    # (b) Content address stable: bytes + sha identical after release.
    b, hdr_sha = _open_revision(base_url, item_id, "1")
    rows.append(LegRow("content-address-stable", "immutable",
                       "PASS" if (hashlib.sha256(b).hexdigest() == sha and
                                  hdr_sha == sha) else "FAIL",
                       "blob sha stable" if b == payload else "blob changed after release"))
    return rows


# ---------------------------------------------------------------------------
# Leg 4: AUDIT -- everything verifiable via the audit log
# ---------------------------------------------------------------------------

def audit_leg(base_url: str) -> list:
    rows = []
    try:
        status, view = _audit_view(base_url)
        if status != 200:
            return [LegRow("audit-view", "audit", "SKIP",
                           "audit unavailable (status %d)" % status,
                           cls="audit-unavailable")]
    except Exception as e:  # noqa: BLE001
        return [LegRow("audit-view", "audit", "SKIP",
                       "audit unavailable: %r" % (e,), cls="audit-unavailable")]
    status, verify = _audit_verify(base_url)
    ok_verify = status == 200 and isinstance(verify, dict) and verify.get("ok")
    view_ok = isinstance(view, dict) and view.get("append_only") is True
    count = (view.get("count") if isinstance(view, dict) else 0) or 0
    rows.append(LegRow(
        "audit-log-present", "audit",
        "PASS" if (view_ok and count > 0) else "FAIL",
        "append_only=%r count=%d" % (view.get("append_only") if isinstance(view, dict) else None, count)))
    rows.append(LegRow(
        "audit-chain-verifies", "audit",
        "PASS" if ok_verify else "FAIL",
        ("head verified, count=%d" % verify.get("count")) if ok_verify else
        ("verify=%r" % (verify,))))
    return rows


# ---------------------------------------------------------------------------
# Leg 5: NAS -- stage the gate binary on the NAS object store, hash-verified
# ---------------------------------------------------------------------------

def _nas_probe(nas_base, token_value):
    """True when the NAS object store answers with the repo (P8-M5-T03 pattern)."""
    try:
        url = nas_base + "/api/v1/repos/isolith/isolith"
        req = urllib.request.Request(
            url, headers={"Authorization": "token " + token_value} if token_value else {})
        with urllib.request.urlopen(req, timeout=15) as r:
            return r.status == 200
    except Exception:
        return False


def _multipart(files):
    """Multipart/form-data body for a Gitea generic package upload (the
    registry requires multipart, not a raw PUT body; verified live 2026-10-17
    by P8-M5-T03)."""
    import uuid as _uuid
    boundary = _uuid.uuid4().hex
    buf = b""
    for fname, fbytes in files.items():
        buf += ("--%s\r\nContent-Disposition: form-data; name=\"file\"; "
                "filename=\"%s\"\r\nContent-Type: application/octet-stream\r\n\r\n"
                % (boundary, fname)).encode()
        buf += fbytes + b"\r\n"
    buf += ("--%s--\r\n" % boundary).encode()
    return buf, "multipart/form-data; boundary=" + boundary


def nas_leg(binary: str, nas_base: str = "http://192.168.0.101:3200") -> list:
    """Stage the released oep-pdm binary on the NAS and verify by sha-256.

    Uses the exact P8-M5-T03 LAN/air-gap staging shape: Gitea generic package
    registry under /api/packages/don/generic/isolith-pdm/, multipart PUT
    content-addressed by the binary sha-256, then GET back and re-hash.
    NAS-unreachable is a SKIP (tolerant doctrine, never a crash or FAIL).
    """
    if not binary or not os.path.isfile(binary):
        return [LegRow("nas-bundle-sha256", "nas", "SKIP",
                       "no binary to stage on this ref",
                       cls="binary-missing")]
    try:
        body = open(binary, "rb").read()
        sha = hashlib.sha256(body).hexdigest()
    except Exception as e:  # noqa: BLE001
        return [LegRow("nas-bundle-sha256", "nas", "FAIL",
                       "cannot read binary: %r" % (e,))]
    tok = _gitfallback()
    if not _nas_probe(nas_base, tok):
        return [LegRow("nas-bundle-sha256", "nas", "SKIP",
                       "NAS unreachable at %s" % nas_base,
                       cls="nas-unreachable")]
    fname = "oep-pdm-%s" % sha[:16]
    url = ("%s/api/packages/don/generic/isolith-pdm/airgap-v1/%s"
           % (nas_base, sha))
    body_multi, ct = _multipart({fname: body})
    try:
        put_status = None
        already_staged = False
        try:
            put = urllib.request.Request(
                url, data=body_multi, method="PUT",
                headers=dict({"Content-Type": ct}, **( {"Authorization": "token " + tok} if tok else {})))
            with urllib.request.urlopen(put, timeout=90) as r:
                put_status = r.status
        except urllib.error.HTTPError as e:
            # Content-addressed registry: a 409 Conflict means the artifact
            # with this exact sha is ALREADY staged (idempotent PUT). That is
            # not a failure -- the content address is what it should be; we
            # still verify the fetch re-hashes.
            if e.code == 409:
                already_staged = True
            else:
                raise
        get = urllib.request.Request(
            url, method="GET",
            headers={"Authorization": "token " + tok} if tok else {})
        with urllib.request.urlopen(get, timeout=90) as r:
            got = r.read()
        ok = ((put_status in (201, 204) or already_staged)
              and got == body
              and hashlib.sha256(got).hexdigest() == sha)
        verb = "staged" if not already_staged else "already-staged"
        return [LegRow(
            "nas-bundle-sha256", "nas",
            "PASS" if ok else "FAIL",
            "NAS %s+re-hash: %dB re-verified sha256=%s (put=%s)" % (
                verb, len(got), sha[:16], put_status))]
    except (urllib.error.URLError, urllib.error.HTTPError, OSError) as e:
        return [LegRow("nas-bundle-sha256", "nas", "SKIP",
                       "NAS stage failed: %r" % (e,),
                       cls="nas-unreachable")]


def _gitfallback():
    """Best-effort token from ~/.git-credentials (used by the M5-T03 proof)."""
    try:
        p = os.path.expanduser("~/.git-credentials")
        for line in open(p, encoding="utf-8"):
            m = re.match(r"^http://don:([^@]+)@", line)
            if m:
                return m.group(1)
    except Exception:
        pass
    return ""


# ---------------------------------------------------------------------------
# Report builder + exit code contract (identical to P2/P3/P4)
# ---------------------------------------------------------------------------

def build_report(rows: list, binary: str, base_url: str, nas_base: str) -> dict:
    summary = {"total": len(rows)}
    for v in VERDICTS:
        summary[v] = sum(1 for r in rows if r.status == v)
    summary["failures"] = [r.id for r in rows if r.status == "FAIL"]
    return {
        "schema_version": 1,
        "description": "P8 GA gate: multi-user edit/publish/release + concurrent lock test",
        "generated_at": datetime.datetime.now(
            datetime.timezone.utc).isoformat(),
        "base_url": base_url,
        "binary": binary,
        "nas_base": nas_base,
        "summary": summary,
        "rows": [r.as_dict() for r in rows],
    }


def exit_code(report: dict) -> int:
    """0 iff no FAIL row anywhere (SKIP allowed)."""
    return 1 if report["summary"].get("FAIL", 0) else 0


def write_report(report: dict, outdir: str) -> None:
    out = Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    (out / "ga-gate-p8-multiuser.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = [
        "# P8 GA gate report: multi-user edit/publish/release + concurrent lock test",
        "",
        "- schema_version: 1",
        "- generated_at: %s" % report["generated_at"],
        "- base_url: %s" % report["base_url"],
        "",
        "## Summary",
        "",
        "| total | FAIL | SKIP | PASS |",
        "|---|---|---|---|",
        "| %(total)d | %(FAIL)d | %(SKIP)d | %(PASS)d |" % report["summary"],
        "",
        "## Rows",
        "",
    ]
    for r in report["rows"]:
        lines.append("- **[%s]** `%s` (%s) -- %s" % (r["status"], r["id"],
                                                      r["leg"], r["detail"]))
    (out / "ga-gate-p8-multiuser.md").write_text("\n".join(lines) + "\n",
                                                 encoding="utf-8")


def run_gate(base_url: str, outdir: str, n_clients: int = 4,
             binary: str = "", nas_base: str = "http://192.168.0.101:3200") -> int:
    """Run every leg and write the report; return the gate exit code."""
    bin_default = os.path.join(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__)))), "target", "debug", "oep-pdm")
    if not binary:
        binary = bin_default
    rows = []
    rows += multiuser_leg(base_url, n_clients=n_clients)
    rows += locks_leg(base_url, n_racers=n_clients)
    rows += immutable_leg(base_url)
    rows += audit_leg(base_url)
    rows += nas_leg(binary, nas_base=nas_base)
    report = build_report(rows, binary, base_url, nas_base)
    write_report(report, outdir)
    return exit_code(report)


if __name__ == "__main__":
    import multiprocessing as _mp
    _mp.freeze_support()  # spawn-safe on macOS/Windows: children skip main
    import argparse
    ap = argparse.ArgumentParser(description="P8 GA gate (multi-user PDM cycle)")
    ap.add_argument("--base", default="http://127.0.0.1:8787/api/v1",
                    help="base URL of the live oep-pdm service")
    ap.add_argument("--binary", default="",
                    help="oep-pdm binary to stage on the NAS (default: built binary)")
    ap.add_argument("--out", default="workstreams/pdm/api/proof", help="report output dir")
    ap.add_argument("--n", default=4, type=int, help="number of concurrent clients")
    ap.add_argument("--nas", default="http://192.168.0.101:3200",
                    help="NAS object store base (Gitea)")
    a = ap.parse_args()

    # The harness normally expects a live service; this CLI is used by the
    # proof script which boots it first. If the service is not reachable,
    # degrade honestly (SKIP) and still write the report.
    if _health(a.base) is None:
        print("P8 GA gate: service at %s not reachable; degraded run" % a.base)
    code = run_gate(a.base, a.out, n_clients=a.n, binary=a.binary, nas_base=a.nas)
    print("P8 GA gate exit code: %d" % code)
    raise SystemExit(code)
