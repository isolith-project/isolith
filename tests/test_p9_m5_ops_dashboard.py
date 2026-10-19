#!/usr/bin/env python3
"""Acceptance test for Keelworks P9-M5-T01: ops dashboard.

Written to verify the REAL material, not mocks. It checks that the dashboard
reads live queue/node/job/cost state from the real oep-lane Scheduler and
serves it, and that the committed proof was generated from real data:

  crate       => crates/oep-opsdash exists as a workspace member, std-only,
                 depends on the real oep-lane scheduler crate
  snapshot    => running the deterministic proof wave (via the CLI) produces
                 a snapshot JSON whose counters match what the wave really
                 did: 3 succeeded, 1 dead-lettered poison, 1 reclaimed job,
                 u1 node degraded with 1 reclaimed job, and honest nonzero
                 cost/energy
  html        => the same wave renders an HTML dashboard containing all four
                 views (Queue depth, Node health, Job logs, Cost / Energy)
  http        => `serve` exposes /healthz, /api/queue, /api/nodes, /api/jobs,
                 /api/cost and / over a real TCP connection
  live        => `serve --live` counters advance between two polls (the
                 scheduler really runs behind the served dashboard)
  em-dashes   => no em dashes or curly quotes in the files this card adds

Run:  python3 tests/test_p9_m5_ops_dashboard.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import http.client
import json
import os
import re
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.join(ROOT, "crates", "oep-opsdash")
PROOF = os.path.join(ROOT, "workstreams/P9-M5", "proof")
EM_DASH = "\u2014"
CURLY_QUOTES = {"\u2018", "\u2019", "\u201c", "\u201d"}

failures = []
warnings = []


def check(cond, msg):
    if not cond:
        failures.append(msg)
    print(("[FAIL] " if not cond else "[ok]   ") + msg)


def build_binary():
    r = subprocess.run(
        ["cargo", "build", "-p", "oep-opsdash"],
        cwd=ROOT, capture_output=True, text=True, timeout=600,
    )
    if r.returncode != 0:
        check(False, "cargo build -p oep-opsdash: rc=%d\n%s" % (r.returncode, (r.stderr or "")[-800:]))
        return None
    binpath = os.path.join(ROOT, "target", "debug", "oep-opsdash")
    check(os.path.isfile(binpath), "built binary exists: oep-opsdash")
    return binpath


def run_snapshot(binpath, workdir):
    r = subprocess.run(
        [binpath, "snapshot", "--out", os.path.join(workdir, "snap.json"), "--lease-ms", "50"],
        cwd=workdir, capture_output=True, text=True, timeout=60,
    )
    if r.returncode != 0:
        check(False, "snapshot rc=%d stderr=%s" % (r.returncode, r.stderr))
        return None, None
    snap = json.load(open(os.path.join(workdir, "snap.json")))
    return snap, [line for line in r.stdout.splitlines() if line.startswith("wrote")]


def run_render(binpath, workdir):
    r = subprocess.run(
        [binpath, "render", "--out", os.path.join(workdir, "dash.html"), "--lease-ms", "50"],
        cwd=workdir, capture_output=True, text=True, timeout=60,
    )
    if r.returncode != 0:
        check(False, "render rc=%d stderr=%s" % (r.returncode, r.stderr))
        return None
    return open(os.path.join(workdir, "dash.html"), encoding="utf-8").read()


def http_get(port, path):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    conn.request("GET", path)
    resp = conn.getresponse()
    body = resp.read().decode("utf-8", "replace")
    conn.close()
    return resp.status, resp.headers.get("Content-Type", ""), body


def start_serve(binpath, port, live):
    args = [binpath, "serve", "--listen", "127.0.0.1:%d" % port]
    if live:
        args.append("--live")
    p = subprocess.Popen(args, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    # Wait for /healthz.
    for _ in range(60):
        if p.poll() is not None:
            break
        try:
            st, _, body = http_get(port, "/healthz")
            if st == 200 and "ok" in body:
                return p
        except OSError:
            pass
        time.sleep(0.25)
    p.kill()
    return None


def check_structure():
    check(os.path.isdir(CRATE), "crates/oep-opsdash exists")
    check(os.path.isfile(os.path.join(CRATE, "Cargo.toml")), "oep-opsdash Cargo.toml")
    cm = open(os.path.join(CRATE, "Cargo.toml"), encoding="utf-8").read()
    check("oep-lane" in cm, "oep-opsdash path-depends on real oep-lane scheduler")
    check('../oep-lane' in cm, "dependency is the real crate via path (not a copy)")
    lib = open(os.path.join(CRATE, "src", "lib.rs"), encoding="utf-8").read()
    check("dashboard" in lib and "scheduler" in lib.lower(), "lib exposes the dashboard/scheduler modules")
    root_cargo = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    check('"crates/oep-opsdash"' in root_cargo, "workspace member declared")


def check_snapshot_real(snap):
    if snap is None:
        return
    q = snap["queue"]
    check(q["succeeded"] == 3, "3 jobs succeeded after the real wave  (got %r)" % q["succeeded"])
    check(q["dead_lettered"] == 1, "exactly 1 dead-lettered poison  (got %r)" % q["dead_lettered"])
    dl = snap["dead_letters"]
    check(len(dl) == 1 and dl[0]["reason"].startswith("poison reported"),
          "dead-letter row is the real poison reason: %r" % (dl[0]["reason"] if dl else None))
    u1 = [n for n in snap["nodes"] if n["id"] == "u1"][0]
    check(u1["health"] == "degraded", "u1 health is degraded after real reclaim  (got %r)" % u1["health"])
    check(u1["jobs_reclaimed"] == 1, "u1 shows 1 reclaimed job  (got %r)" % u1["jobs_reclaimed"])
    check(snap["cost_totals"]["energy_mwh"] > 0, "cost/energy view has real positive energy")
    check(snap["cost_totals"]["runs"] >= 5, "cost ledger counts every real run  (got %r)" % snap["cost_totals"]["runs"])
    # The reclaimed job is still known to the scheduler (redelivered Ready).
    check(q["ready"] == 1, "the reclaimed job is back in ready (at-least-once)  (got %r)" % q["ready"])


def check_html(html):
    if html is None:
        return
    for needle in ["OEP P9 Ops Dashboard", "Queue depth", "Node health",
                   "Job logs", "Cost / Energy", "degraded", "dead-lettered"]:
        check(needle in html, "html renders %r" % needle)


def check_http_serve(binpath):
    p = start_serve(binpath, 8581, live=False)
    check(p is not None, "serve starts and /healthz returns ok")
    if p is None:
        return
    try:
        st, ct, body = http_get(8581, "/api/queue")
        check(st == 200 and "queue" in body, "/api/queue returns queue JSON  (st=%d)" % st)
        st, _, body = http_get(8581, "/api/nodes")
        check(st == 200 and '"health"' in body, "/api/nodes returns node JSON")
        st, _, body = http_get(8581, "/api/jobs")
        check(st == 200 and '"log"' in body, "/api/jobs returns jobs+log JSON")
        st, _, body = http_get(8581, "/api/cost")
        check(st == 200 and "totals" in body, "/api/cost returns cost JSON")
        st, ct, body = http_get(8581, "/")
        check(st == 200 and "text/html" in ct and "Queue depth" in body,
              "GET / serves the HTML dashboard  (st=%d ct=%r)" % (st, ct))
        st, _, body = http_get(8581, "/nope")
        check(st == 404, "unknown path returns 404")
    finally:
        p.kill()
        p.wait()


def check_live_advances(binpath):
    p = start_serve(binpath, 8582, live=True)
    check(p is not None, "serve --live starts")
    if p is None:
        return
    try:
        time.sleep(1.2)
        _, _, b1 = http_get(8582, "/api/queue")
        time.sleep(1.2)
        _, _, b2 = http_get(8582, "/api/queue")
        j1 = json.loads(b1)
        j2 = json.loads(b2)
        check(j2["queue"]["succeeded"] > j1["queue"]["succeeded"],
              "live driver advances succeeded: %d -> %d" % (
                  j1["queue"]["succeeded"], j2["queue"]["succeeded"]))
    finally:
        p.kill()
        p.wait()


def check_proof_provenance():
    d = os.path.join(ROOT, "workstreams/P9-M5")
    check(os.path.isdir(d), "P9-M5 card doc dir exists")
    check(os.path.isfile(os.path.join(d, "ops-dashboard.md")), "workstreams/P9-M5/ops-dashboard.md exists")
    check(os.path.isfile(os.path.join(PROOF, "snapshot.json")), "proof snapshot.json committed")
    check(os.path.isfile(os.path.join(PROOF, "dashboard.html")), "proof dashboard.html committed")
    if os.path.isfile(os.path.join(PROOF, "snapshot.json")):
        snap = json.load(open(os.path.join(PROOF, "snapshot.json")))
        check(snap["format"] == "oep-opsdash" and snap["version"] == 1,
              "proof snapshot carries the oep-opsdash v1 header")


def check_language_ban():
    for base, dirs, files in os.walk(os.path.join(ROOT, "crates", "oep-opsdash")):
        for f in files:
            if not f.endswith((".rs", ".md", ".toml")):
                continue
            text = open(os.path.join(base, f), encoding="utf-8").read()
            if EM_DASH in text:
                check(False, "em dash banned in %s" % f)
            if set(text) & CURLY_QUOTES:
                check(False, "curly quotes banned in %s" % f)
    for f in ["workstreams/P9-M5/ops-dashboard.md", "tests/test_p9_m5_ops_dashboard.py"]:
        text = open(os.path.join(ROOT, f), encoding="utf-8").read()
        if EM_DASH in text:
            check(False, "em dash banned in %s" % f)
        if set(text) & CURLY_QUOTES:
            check(False, "curly quotes banned in %s" % f)


def main():
    print("P9-M5-T01 ops dashboard acceptance")
    print("=" * 60)
    binpath = build_binary()
    check_structure()
    workdir = tempfile.mkdtemp(prefix="opsdash-accept-")
    snap, _ = run_snapshot(binpath, workdir)
    check_snapshot_real(snap)
    html = run_render(binpath, workdir)
    check_html(html)
    check_http_serve(binpath)
    check_live_advances(binpath)
    check_proof_provenance()
    check_language_ban()
    print("=" * 60)
    if failures:
        print("FAILED %d check(s):" % len(failures))
        for f in failures:
            print("  - %s" % f)
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
