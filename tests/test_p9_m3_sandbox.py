#!/usr/bin/env python3
"""P9-M3-T03 acceptance test: worker sandbox enforces CPU/RAM/disk caps and
blocks network egress by default; escaping the sandbox fails a test.

Runs the crate's own acceptance suite (`cargo test -p oep-sandbox`), which
spawns real worker processes (`sbox-probe`) through the sandbox, and asserts
the acceptance tests actually ran (by name) and the whole suite is green.

The egress tests are environment-adaptive by design: on hosts that permit
installing the seccomp filter (the intended worker hosts), they assert the
live `EPERM` block; on hosts that forbid nested seccomp (Docker's default
profile, locked-down sandboxes) they assert the fail-closed refusal — the
sandbox never produces an uncaged worker. The filter's decision table is
additionally unit-verified by simulating the exact BPF instruction stream, so
the security property is covered in both postures.

"Done" for P9-M3-T03 means this test passes against the real tree AND the
sandbox suite it drives is green (see workstreams/compute/workers/sandbox.md for the
verification record).
"""
import os
import shutil
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Acceptance tests that must have RUN (grep'd from cargo's test output). If a
# name is missing the suite did not exercise the boundary.
REQUIRED_TESTS = [
    "cpu_cap_enforced_kills_runaway_worker",
    "ram_cap_enforced_allocation_fails_enomem",
    "disk_cap_enforced_writes_fail",
    "egress_blocked_by_default_connect_fails",
    "egress_blocked_by_default_raw_packet_fails",
    "escape_via_connect_fails",
    "escape_via_raw_inet_socket_fails",
    "escape_via_unbounded_cpu_fails",
    "escape_via_unbounded_ram_fails",
    "escape_via_disk_flood_fails",
]

_SUITE = None


def run_sandbox_suite():
    """Run the crate's suite once and cache the (rc, combined-output) pair so
    the pytest functions do not rebuild three times."""
    global _SUITE
    if _SUITE is not None:
        return _SUITE
    if shutil.which("cargo") is None:
        raise RuntimeError("cargo not found; cannot run the sandbox acceptance suite")
    r = subprocess.run(
        ["cargo", "test", "-p", "oep-sandbox"],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=600,
    )
    _SUITE = (r.returncode, r.stdout + "\n" + r.stderr)
    return _SUITE


def check(name, ok, detail=""):
    print("[%s] %s%s" % ("PASS" if ok else "FAIL", name, (" -- " + detail if detail else "")))
    return ok


def test_sandbox_suite_green():
    rc, out = run_sandbox_suite()
    assert rc == 0, "sandbox suite failed (rc=%d):\n%s" % (rc, out.strip()[-2000:])
    check("cargo test -p oep-sandbox exits 0", rc == 0)


def test_each_acceptance_test_ran():
    _, out = run_sandbox_suite()
    missing = [t for t in REQUIRED_TESTS if t not in out]
    assert not missing, "acceptance tests did not run: %s" % missing
    check("all acceptance tests present in output", not missing, ", ".join(REQUIRED_TESTS))


def test_resource_caps_not_vacuous():
    """The CPU/RAM/disk caps are exercised live (not gated by the host's
    seccomp posture): the two hard-cap probes must have run and passed."""
    _, out = run_sandbox_suite()
    assert "cpu_cap_enforced_kills_runaway_worker ... ok" in out
    assert "ram_cap_enforced_allocation_fails_enomem ... ok" in out
    check("live CPU/RAM cap tests green", True)


def main():
    print("Isolith P9-M3-T03 worker sandbox acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    fails = 0
    try:
        rc, out = run_sandbox_suite()
    except Exception as e:  # noqa: BLE001 - report and exit on any setup failure
        print("[FAIL] could not run suite: %s" % e)
        return 1
    if not check("sandbox suite green", rc == 0, "see output above"):
        fails += 1
    missing = [t for t in REQUIRED_TESTS if t not in out]
    if not check("all acceptance tests ran", not missing, "missing: %s" % missing):
        fails += 1
    print("=" * 60)
    if fails:
        print("FAILED %d check(s)" % fails)
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
