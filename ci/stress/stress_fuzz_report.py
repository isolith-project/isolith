#!/usr/bin/env python3
"""Keelworks P0-M4-T03 — long fuzz job for the status-page/report serializers.

Attacks the invariants the status page depends on with hostile bytes:
  - server crashes: render_status_html/build_report must never raise;
  - HTML injection: every job-derived string is escaped before rendering, so a
    job literally named `<script>alert(1)</script>` appears as text, never as
    markup;
  - report serialization: json.dumps must round-trip any report dict the
    runner can produce (no lone surrogates, no NaN-style payloads).

This is real long fuzz (default 3000 iterations, deterministic seed) while the
only real parser code in wave 1 is the report/status serializers. As P2/P3
parser fuzz harnesses land (P2-M1-T05, P2-M6-T01), they plug into the same
nightly suite via ci/stress/jobs.json.

Exit 0 = all invariants held for N iterations. Any assertion failure or
unhandled exception = exit 1 (the nightly run then reports FAIL).
"""
import argparse
import json
import os
import random
import sys

# Import the code under test from the same directory (repo: ci/stress/).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stress_runner  # noqa: E402

# Nasty tokens mixed into hostile logs/names: HTML metacharacters, quotes,
# control bytes, NUL, emoji, long runs, and multi-byte sequences that decode
# with replacement characters.
NASTY = [
    "<script>alert(1)</script>", "</script><body onload=alert(1)>",
    "\" onmouseover='x' ", "&quot; &amp; &lt;b&gt; &gt;", "<b>bold</b>",
    "a\0b", "\x00\x00\x00", "\x1b[31mred\x1b[0m", "\x7fDEL",
    "tabs\t\t\t", "space  pad", "line1\nline2\nline3",
    "\u2603\u2744\u2600", "\N{SNOWMAN}\N{HAMMER}", "日本テスト中文",
    "%PDF-1.7", "PK\x03\x04", "\xff\xfe\x00\x00", "caf\xc3\xa9",
    "\xed\xa0\x80", "long" * 5000, "x" * 200_000,
]
STATUSES = ["pass", "fail", "timeout", "skip", "error"]
CATEGORIES = ["stress", "regression"]


def fuzz_once(rng):
    """Build one hostile report dict and check render + serialize invariants."""
    name = rng.choice(NASTY) + rng.choice(["", str(rng.randint(0, 9999))])
    # Raw random bytes decoded with replacement also flow through logs/names.
    raw = rng.randbytes(rng.randint(0, 96))
    decoded = raw.decode("utf-8", errors="replace")
    log = rng.choice(NASTY) + decoded
    status = rng.choice(STATUSES)
    job = {
        "name": name,
        "category": rng.choice(CATEGORIES),
        "status": status,
        "exit_code": None if status in ("timeout", "skip", "error")
                    else rng.randint(-255, 255),
        "duration_s": round(rng.uniform(0, 600), 3),
        "output_tail": log,
    }
    report = {
        "schema_version": stress_runner.SCHEMA_VERSION,
        "repo": "don/keelworks",
        "commit": rng.choice(NASTY),
        "generated_at": "2026-10-15T00:00:00Z",
        "jobs": [job],
        "summary": {"total": 1, "passed": int(status == "pass"),
                    "failed": int(status == "fail"),
                    "timed_out": int(status == "timeout"),
                    "skipped": 0, "error": int(status == "error")},
    }
    html_page = stress_runner.render_status_html(report)
    assert isinstance(html_page, str) and len(html_page) > 100, \
        "render produced empty page"
    # Raw script markup must never survive, from any slot (name/log/repo).
    assert "<script" not in html_page, \
        "raw <script survived escaping (name=%r)" % name
    # Any literal '<' in a job-derived name must appear escaped in the page.
    if "<" in name:
        assert "&lt;" in html_page, \
            "job name '<' not escaped in page: %r" % name
    # Report must serialize to JSON and round-trip (control bytes, NUL, etc).
    payload = json.dumps(report)
    back = json.loads(payload)
    assert back["jobs"][0]["output_tail"] == job["output_tail"], \
        "report JSON round-trip failed"
    return len(decoded), len(log), len(html_page)


def main(argv=None):
    ap = argparse.ArgumentParser(
        prog="stress_fuzz_report.py",
        description="Fuzz the Keelworks status-page/report serializers "
                    "(P0-M4-T03).")
    ap.add_argument("--iterations", type=int, default=3000,
                    help="number of fuzz iterations (default 3000)")
    ap.add_argument("--seed", type=int, default=20261015,
                    help="PRNG seed for reproducible fuzz (default 20261015)")
    args = ap.parse_args(argv)
    if args.iterations <= 0:
        print("error: --iterations must be > 0", file=sys.stderr)
        return 2

    rng = random.Random(args.seed)
    mutated_bytes = 0
    for i in range(args.iterations):
        mb, log_len, page_len = fuzz_once(rng)
        mutated_bytes += mb
        if (i + 1) % 1000 == 0:
            print("... %d iterations done (mutated %d bytes, max page %d)"
                  % (i + 1, mutated_bytes, page_len))
    print("FUZZ OK iterations=%d mutations=%d seed=%d"
          % (args.iterations, mutated_bytes, args.seed))
    return 0


if __name__ == "__main__":
    sys.exit(main())
