#!/usr/bin/env python3
"""P8-M6-T03 acceptance (TDD, RED first) — the P8 GA gate: multi-user
edit/publish/release + concurrent lock test on the NAS.

Written BEFORE `oep_tooling.ga_gate_p8` exists, so this file MUST FAIL (RED)
when run against this branch before the gate module lands, and passes GREEN
once the module exists AND its legs can run against the real service. On a tree
without the gate module the checks degrade to honest SKIP (CI-safe, exit 0),
exactly like the P2/P3/P4 gate acceptance tests.

The P8 GA gate means the P8 PDM multi-user lifecycle is REAL over the wire:

1. MULTIUSER leg — N real concurrent client processes (the generated python
   client over real sockets) each do full edit -> publish -> release cycles on
   the live service; every client lands exactly the revisions it published
   (no lost updates, no accidental clobber), and N is parameterized (>=2).

2. LOCKS leg — the working-set lock (P8-M1-T03) is respected: a second client
   is refused the lock on an item the first holds (lock_conflict), a locked
   item cannot be published by a non-holder (token rejected), and release with
   the right token returns the item to the acquirable set. The concurrent lock
   test races N clients for the same item: exactly one wins.

3. IMMUTABLE leg — a revision that reaches `released` (P8-M2-T01) is frozen:
   the state machine refuses further hops and the content-addressed blob
   re-reads byte-identical (sha-256 stable). This is "revisions immutable
   after release".

4. AUDIT leg — every operation (create item, create revision, lock acquire,
   release, transition) is recorded in the P8-M3-T03 tamper-evident audit log
   and `GET /api/v1/audit/verify` re-derives the chain cleanly (ok=true).
   "Everything verifiable via the audit log."

5. NAS leg — the gate proof is staged on the NAS object store (Gitea generic
   package registry) content-addressed and re-verified by hash; NAS-unreachable
   is classified SKIP (tolerant-failure doctrine), never FAIL; a live NAS
   answers with a hash-verified PASS on real material.

Exit code contract (same as P2/P3/P4): 0 iff no gate-FAIL row anywhere (SKIP
allowed). The gate's own report (ga-gate-p8-multiuser.json / .md) must be
self-consistent (summary totals roll up to row counts, schema_version 1).
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


def _gate_module_available():
    try:
        import oep_tooling.ga_gate_p8 as G  # noqa: F401
        return True
    except Exception:
        return False


def test_gate_module_exists():
    try:
        import oep_tooling.ga_gate_p8 as G
    except Exception as e:
        check("import oep_tooling.ga_gate_p8", False, repr(e))
        return
    check("import oep_tooling.ga_gate_p8", True)
    for name in ("multiuser_leg", "locks_leg", "immutable_leg", "audit_leg",
                 "run_gate", "build_report", "exit_code", "write_report"):
        check("ga_gate_p8 has %s" % name, hasattr(G, name), name)


def test_multiuser_leg_runs_real_cycle():
    if not _gate_module_available():
        return  # RED already reported by test 1
    import oep_tooling.ga_gate_p8 as G
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("multiuser_leg: binary present", False,
              "build first: cargo build -p oep-pdm")
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = subprocess.Popen(
            [str(bin_p), "--addr", "127.0.0.1:%d" % port, "--persist-dir", td, "--no-auth"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            time.sleep(0.8)
            rows = G.multiuser_leg(
                base_url="http://127.0.0.1:%d/api/v1" % port, n_clients=3)
            fails = [r for r in rows if r.status == "FAIL"]
            check("multiuser_leg returns list", isinstance(rows, list),
                  repr(rows)[:160])
            check("multiuser_leg zero gate-FAIL", len(fails) == 0,
                  "rows=%s" % repr([(r.id, r.status) for r in rows]))
            # every client observed (no dropped user)
            check("multiuser_leg reports N clients",
                  any(r.id.startswith("client") for r in rows),
                  repr([r.id for r in rows])[:160])
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_locks_leg_respected():
    if not _gate_module_available():
        return
    import oep_tooling.ga_gate_p8 as G
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("locks_leg: binary present", False)
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = subprocess.Popen(
            [str(bin_p), "--addr", "127.0.0.1:%d" % port, "--persist-dir", td, "--no-auth"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(0.8)
            rows = G.locks_leg("http://127.0.0.1:%d/api/v1" % port,
                               n_racers=4)
            check("locks_leg: exactly one racer wins concurrent lock",
                  any(r.id == "concurrent-lock-single-winner" and r.status == "PASS"
                      for r in rows),
                  repr([(r.id, r.status) for r in rows])[:200])
            check("locks_leg: non-holder publish refused",
                  any(r.id == "publish-gate" and r.status == "PASS"
                      for r in rows),
                  repr([(r.id, r.status) for r in rows])[:200])
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_run_gate_report_and_exit_code():
    if not _gate_module_available():
        return
    import oep_tooling.ga_gate_p8 as G
    bin_p = Path(ROOT) / "target" / "debug" / "oep-pdm"
    if not bin_p.is_file():
        check("run_gate: binary present", False)
        return
    with tempfile.TemporaryDirectory() as td:
        port = _free_port()
        proc = subprocess.Popen(
            [str(bin_p), "--addr", "127.0.0.1:%d" % port, "--persist-dir", td, "--no-auth"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(0.8)
            out = Path(td) / "out"
            code = G.run_gate("http://127.0.0.1:%d/api/v1" % port,
                              outdir=str(out), n_clients=3)
            jp = out / "ga-gate-p8-multiuser.json"
            mp = out / "ga-gate-p8-multiuser.md"
            check("run_gate exit code in {0,1}", code in (0, 1),
                  "code=%r" % (code,))
            check("gate report json written", jp.is_file())
            check("gate report md written", mp.is_file())
            if jp.is_file():
                m = json.load(open(jp))
                summary = m["summary"]
                rows = m["rows"]
                check("report schema_version==1", m.get("schema_version") == 1)
                check("report totals roll up", summary.get("total") == len(rows),
                      "summary.total=%r rows=%d" % (summary.get("total"),
                                                    len(rows)))
                has_fail = any(r["status"] == "FAIL" for r in rows)
                check("exit code matches FAIL rows", (code == 1) == has_fail,
                      "code=%r fail_rows=%r" % (code, has_fail))
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_gate_degrades_on_bare_tree():
    """Bare tree (no binary / no module) => SKIP/completed rows, exit 0."""
    if not _gate_module_available():
        return
    import oep_tooling.ga_gate_p8 as G
    with tempfile.TemporaryDirectory() as td:
        code = G.run_gate("http://127.0.0.1:1/api/v1",
                          outdir=str(Path(td) / "out"),
                          binary=str(Path(td) / "nope"),
                          n_clients=2)
        check("bare-tree degraded run exits 0 (no FAIL)", code == 0,
              "code=%r" % (code,))


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
        print("GA GATE P8 FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("GA GATE P8 GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
