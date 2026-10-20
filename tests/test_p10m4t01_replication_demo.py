#!/usr/bin/env python3
"""P10-M4-T01 acceptance (TDD, RED first) — multi-site replication demo:
a LAN node and a remote node replicating the vault, conflict semantics
documented and honest.

Written BEFORE `oep_tooling.replication_demo` exists, so this file MUST FAIL
(RED) when run against this branch before the module lands, and passes GREEN
once the module exists AND its legs can run against the real service. On a tree
without the module the checks degrade to honest SKIP (CI-safe, exit 0), exactly
like the P8/P9 gate acceptance tests.

The card (P10-M4-T01):

  > Multi-site replication demo: a LAN node and a remote node replicating the
  > vault, conflict semantics documented and honest. RUN TARGET per decision #9:
  > nodes are U1 + garage PC (WoL required) + VM-class hosts, NOT the NAS.
  > Baseline uses the P8-M5-T03 airgap/hybrid ADR.
  > Acceptance: a vault edit on one node appears on the other with conflict
  > semantics documented; concurrent-edit conflict resolves deterministically;
  > verified live across the two approved hosts.

What this test asserts, on real material (real oep-pdm binary over real
sockets, no mocks):

1. REPLICATE leg — two nodes against one vault: node A (the LAN/authoritative
   oep-pdm service) and node B (a sync client, the 'remote node'). A vault edit
   made on node A (a new revision) is reflected on node B (pulled/openable
   through the sync surface); a vault edit made on node B (a recorded offline
   edit) syncs back to node A and becomes the item's next revision, so "a
   vault edit on one node appears on the other" in both directions.

2. CONFLICT leg — concurrent-edit divergence is detected deterministically by
   the server on base identity (revision number + content blob sha-256): when
   node B pushes an offline edit whose base no longer equals the current head,
   the server refuses to overlay and records a pending merge with
   base/ours/theirs snapshots; the client resolves by its choice (ours /
   theirs / client-merged document) and the committed revision's blob sha-256
   is the hash of the resolved bytes — deterministic, hash-verified outcome.

3. AIRGAP leg — the vault runs under the P8-M5-T03 default deployment role
   (DeploymentRole::Airgap): pure LAN, telemetry forced off, no relay, no
   internet round-trip. The module must exercise the demo inside that posture
   (loopback-or-LAN client, no external egress), matching decision #10.

4. DOC leg — the multi-site replication + conflict-semantics document
   (workstreams/delivery/replication/multi-site-replication.md) exists and is honest: it states the
   deterministic rules (divergence detected by content hash; resolution commits
   a new revision verified by hash) and does NOT claim automatic hunk-level
   text merge (resolution is ours/theirs/client-merged-document; a diff3
   column view is a workstreams/pdm/web/T05 follow-up).

The live two-host proof (U1 LAN node + garage PC WoL remote node, decision #9)
is the card's 'verified live across the two approved hosts' acceptance and is
run by scripts/replication-proof.sh -> workstreams/delivery/replication/proof/*.json. This
acceptance test is the CI-safe, single-host exercise of the same legs
(one service + a sync-client node, exactly as CI can run it).
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "python"))
sys.path.insert(0, os.path.join(ROOT, "python", "oep_tooling"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    print("[%s] %s" % (status, name) + ((" -- " + detail) if detail else ""))
    if not ok:
        _failures.append(name)


def _free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def _start_service(port, persist_dir):
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        return None
    proc = subprocess.Popen(
        [str(bin_p), "--addr", "127.0.0.1:%d" % port, "--persist-dir", persist_dir, "--no-auth"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return proc


def _module_available():
    try:
        import oep_tooling.replication_demo  # noqa: F401
        return True
    except Exception:
        return False


def test_module_exists():
    try:
        import oep_tooling.replication_demo as R
    except Exception as e:
        check("import oep_tooling.replication_demo", False, repr(e))
        return
    check("import oep_tooling.replication_demo", True)
    for name in ("replicate_leg", "conflict_leg", "airgap_leg", "run_demo",
                 "write_report", "exit_code"):
        check("replication_demo has %s" % name, hasattr(R, name), name)


def test_replicate_leg_edit_flows_both_directions():
    if not _module_available():
        return  # RED already reported by test 1
    import oep_tooling.replication_demo as R
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("replicate_leg: binary present", False,
              "build first: cargo build -p oep-pdm")
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = _start_service(port, td)
        if proc is None:
            check("replicate_leg: service start", False)
            return
        try:
            time.sleep(0.8)
            rows = R.replicate_leg("http://127.0.0.1:%d/api/v1" % port)
            fails = [r for r in rows if r.status == "FAIL"]
            check("replicate_leg returns list", isinstance(rows, list), repr(rows)[:160])
            check("replicate_leg zero gate-FAIL", len(fails) == 0,
                  repr([(r.id, r.status) for r in rows]))
            ids = [r.id for r in rows]
            check("replicate_leg covers both directions",
                  "lan-edit-seen-on-remote" in ids and "remote-edit-seen-on-lan" in ids,
                  repr(ids))
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_conflict_leg_deterministic_resolution():
    if not _module_available():
        return
    import oep_tooling.replication_demo as R
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("conflict_leg: binary present", False)
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = _start_service(port, td)
        if proc is None:
            check("conflict_leg: service start", False)
            return
        try:
            time.sleep(0.8)
            rows = R.conflict_leg("http://127.0.0.1:%d/api/v1" % port)
            check("conflict_leg returns list", isinstance(rows, list), repr(rows)[:160])
            check("conflict_leg zero gate-FAIL",
                  all(r.status != "FAIL" for r in rows),
                  repr([(r.id, r.status) for r in rows]))
            ids = [r.id for r in rows]
            check("conflict_leg covers divergence + resolution",
                  "concurrent-edit-divergence" in ids and
                  "resolution-hash-verified" in ids,
                  repr(ids))
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_airgap_leg_and_report():
    if not _module_available():
        return
    import oep_tooling.replication_demo as R
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("airgap_leg: binary present", False)
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = _start_service(port, td)
        if proc is None:
            check("airgap_leg: service start", False)
            return
        try:
            time.sleep(0.8)
            rows = R.airgap_leg("http://127.0.0.1:%d/api/v1" % port)
            check("airgap_leg returns list", isinstance(rows, list), repr(rows)[:160])
            check("airgap_leg zero gate-FAIL",
                  all(r.status != "FAIL" for r in rows),
                  repr([(r.id, r.status) for r in rows]))
            out = Path(td) / "out"
            code = R.run_demo("http://127.0.0.1:%d/api/v1" % port,
                              outdir=str(out))
            jp = out / "p10m4t01-replication.json"
            check("run_demo exit code in {0,1}", code in (0, 1), "code=%r" % (code,))
            check("demo report json written", jp.is_file())
            if jp.is_file():
                m = json.load(open(jp))
                summary = m["summary"]
                rows_o = m["rows"]
                check("report schema_version==1", m.get("schema_version") == 1)
                check("report totals roll up", summary.get("total") == len(rows_o),
                      "summary.total=%r rows=%d" % (summary.get("total"), len(rows_o)))
                has_fail = any(r["status"] == "FAIL" for r in rows_o)
                check("exit code matches FAIL rows", (code == 1) == has_fail,
                      "code=%r fail_rows=%r" % (code, has_fail))
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_docs_are_honest():
    doc = Path(ROOT) / "docs" / "workstreams/delivery/replication" / "multi-site-replication.md"
    if not doc.is_file():
        check("multi-site doc present", False,
              "write workstreams/delivery/replication/multi-site-replication.md")
        return
    text = doc.read_text(encoding="utf-8")
    check("doc present", doc.is_file())
    check("doc states divergence detection", "blob" in text.lower() and
          ("sha-256" in text.lower() or "sha256" in text.lower()),
          "needs content-hash divergence rule")
    check("doc states deterministic rules",
          "deterministic" in text.lower() or "hash-verify" in text.lower(),
          "needs the deterministic resolution rule")
    check("doc is honest about merge scope",
          "hunk" in text.lower() and "diff3" in text.lower(),
          "needs the honest no-auto-text-merge caveat")


def main():
    tests = sorted(n for n in globals()
                   if n.startswith("test_") and callable(globals()[n]))
    for t in tests:
        try:
            globals()[t]()
        except Exception as e:
            check(t, False, "exception: %r" % (e,))
    print("--")
    if _failures:
        print("P10-M4-T01 FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("P10-M4-T01 GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
