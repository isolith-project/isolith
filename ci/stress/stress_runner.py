#!/usr/bin/env python3
"""Isolith P0-M4-T03 — nightly stress/regression runner + status page.

Runs a registered set of long fuzz/regression jobs, records each job's
status/duration/timed-out/exit code into a JSON report, and renders a
self-contained static status page (HTML) reflecting those results.

Contract (acceptance: tests/stress_suite_test.py):
  - `--help` exits 0 with usage;
  - a failing or timing-out suite makes the runner exit non-zero;
  - every job-derived string on the status page is HTML-escaped (job names and
    logs are data, never markup) -- fuzz job attacks this invariant nightly;
  - outputs are decode-safe (binary/odd encodings never crash the run).

Run locally (publishes docs/status/{report.json,status.html}):
    python3 ci/stress/stress_runner.py --jobs ci/stress/jobs.json

Nightly: .gitea/workflows/nightly-stress.yaml (cron) drives the same command.
New lanes are added to ci/stress/jobs.json AND to
tests/stress_suite_test.py's REQUIRED_JOBS together.
"""
import argparse
import html
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone

REPO = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))          # repo root (ci/stress -> repo)
TAIL_CHARS = 4000                          # log tail captured per job
CONTROL_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")

SCHEMA_VERSION = 1


# --------------------------------------------------------------------------- #
# Job loading and execution
# --------------------------------------------------------------------------- #
def load_jobs(path):
    """Load and basic-validate the job registry."""
    with open(path, "rb") as fh:
        data = json.load(fh)
    jobs = data.get("jobs")
    if not isinstance(jobs, list):
        raise ValueError("jobs.json must contain a 'jobs' list")
    seen = set()
    for j in jobs:
        if not j.get("name") or j["name"] in seen:
            raise ValueError("job name missing or duplicate: %r" % (j.get("name"),))
        seen.add(j["name"])
        if j.get("category") not in ("stress", "regression"):
            raise ValueError("job %r category must be stress|regression" % j["name"])
        if not isinstance(j.get("timeout_seconds"), (int, float)) \
                or j["timeout_seconds"] <= 0:
            raise ValueError("job %r timeout_seconds must be > 0" % j["name"])
        if not isinstance(j.get("command"), list) or not j["command"]:
            raise ValueError("job %r command must be a non-empty list" % j["name"])
    return jobs


def _decode_tail(data, limit=TAIL_CHARS):
    text = (data or b"").decode("utf-8", errors="replace")
    return text[-limit:]


def run_job(job, cwd):
    """Execute one job; never raises. Returns a result dict (see schema)."""
    cmd = job["command"]
    timeout = float(job["timeout_seconds"])
    started = time.monotonic()
    base = {"name": job["name"], "category": job.get("category", "regression")}
    try:
        proc = subprocess.Popen(cmd, cwd=cwd, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE)
    except Exception as e:
        return dict(base, status="error", exit_code=None,
                    duration_s=round(time.monotonic() - started, 3),
                    output_tail="spawn error: %r" % (e,))
    try:
        out, err = proc.communicate(timeout=timeout)
        rc = proc.returncode
        status = "pass" if rc == 0 else "fail"
        tail = _decode_tail(out + err)
        return dict(base, status=status, exit_code=rc,
                    duration_s=round(time.monotonic() - started, 3),
                    output_tail=tail)
    except subprocess.TimeoutExpired:
        proc.kill()
        try:
            proc.wait(timeout=10)
        except Exception:
            pass
        out, err = proc.communicate()
        tail = _decode_tail(out + err)
        return dict(base, status="timeout", exit_code=None,
                    duration_s=round(time.monotonic() - started, 3),
                    output_tail=tail + "[TIMEOUT after %ss]\n" % timeout)


def git_identity(cwd):
    repo = subprocess.run(["git", "remote", "get-url", "gitea"], cwd=cwd,
                          capture_output=True, text=True, timeout=20)
    url = repo.stdout.strip() or "don/isolith"
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=cwd,
                            capture_output=True, text=True, timeout=20)
    return url, (commit.stdout.strip() or "unknown")


# --------------------------------------------------------------------------- #
# Report
# --------------------------------------------------------------------------- #
def build_report(results, generated_at, repo, commit):
    total = len(results)
    summary = {
        "total": total,
        "passed": sum(1 for r in results if r["status"] == "pass"),
        "failed": sum(1 for r in results if r["status"] == "fail"),
        "timed_out": sum(1 for r in results if r["status"] == "timeout"),
        "skipped": sum(1 for r in results if r["status"] == "skip"),
        "error": sum(1 for r in results if r["status"] == "error"),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "repo": repo,
        "commit": commit,
        "generated_at": generated_at,
        "jobs": results,
        "summary": summary,
    }


def clean_for_html(text):
    """Make an arbitrary string safe and tidy for the status page."""
    if text is None:
        return ""
    s = CONTROL_RE.sub("", str(text))      # strip C0 controls (keep \t \n)
    return html.escape(s, quote=True)


# --------------------------------------------------------------------------- #
# Status page renderer (fuzzed nightly by stress_fuzz_report.py)
# --------------------------------------------------------------------------- #
def render_status_html(report):
    summary = report.get("summary", {})
    total = summary.get("total", 0)
    passed = summary.get("passed", 0)
    failed = summary.get("failed", 0)
    timed = summary.get("timed_out", 0)
    state = "PASS" if (failed == 0 and timed == 0 and total > 0) else "FAIL"

    rows = []
    for job in report.get("jobs", []):
        status = job.get("status", "?")
        cls = {"pass": "good", "fail": "bad", "timeout": "warn",
               "skip": "warn", "error": "bad"}.get(status, "warn")
        dur = job.get("duration_s")
        dur_txt = ("%.2fs" % dur) if isinstance(dur, (int, float)) else "-"
        exit_txt = job.get("exit_code")
        end = "" if exit_txt is None else "(exit %s)" % exit_txt
        log = clean_for_html(job.get("output_tail", "")).rstrip() or "-"
        rows.append(
            "<tr class=\"%s\">"
            "<td class=\"job\">%s</td>"
            "<td>%s</td>"
            "<td class=\"status\">%s</td>"
            "<td class=\"num\">%s %s</td>"
            "<td><pre>%s</pre></td>"
            "</tr>" % (
                cls,
                clean_for_html(job.get("name", "?")),
                clean_for_html(job.get("category", "")),
                status,
                dur_txt, end,
                log,
            )
        )

    page = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Isolith Nightly Stress Status</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
          margin: 2rem auto; max-width: 1100px; padding: 0 1rem; }}
  h1 {{ font-size: 1.4rem; }}
  .badge {{ display: inline-block; padding: .2rem .7rem; border-radius: 999px;
            font-weight: 700; }}
  .badge.good {{ background: #1b5e20; color: #fff; }}
  .badge.bad  {{ background: #b71c1c; color: #fff; }}
  .meta {{ color: #888; font-size: .8rem; }}
  table {{ border-collapse: collapse; width: 100%; margin-top: 1rem; }}
  th, td {{ border: 1px solid #444; padding: .45rem .6rem; text-align: left;
            vertical-align: top; }}
  th {{ background: #222; }}
  tr.good td.status {{ color: #4caf50; }}
  tr.bad  td.status {{ color: #f44336; }}
  tr.warn td.status {{ color: #ffb300; }}
  td.job {{ font-weight: 700; }}
  td.num {{ white-space: nowrap; }}
  pre {{ margin: 0; white-space: pre-wrap; word-break: break-word;
         max-height: 8rem; overflow: auto; }}
  footer {{ margin-top: 2rem; color: #888; font-size: .75rem; }}
</style>
</head>
<body>
<h1>Isolith — Nightly Stress Status</h1>
<p class="meta">repo: {repo} &middot; commit: {commit}<br>
generated: {ts} UTC</p>
<p>Suite state: <span class="badge {state_cls}">{state}</span>
&nbsp; {passed}/{total} passed, {failed} failed, {timed} timed out</p>
<table>
<thead><tr><th>Job</th><th>Category</th><th>Status</th><th>Duration</th>
<th>Output tail</th></tr></thead>
<tbody>
{rows}
</tbody>
</table>
<footer>Generated by ci/stress/stress_runner.py (P0-M4-T03). Do not edit by
hand; refresh with: python3 ci/stress/stress_runner.py --jobs
ci/stress/jobs.json</footer>
</body>
</html>""".format(
        repo=clean_for_html(report.get("repo", "")),
        commit=clean_for_html(report.get("commit", "")),
        ts=clean_for_html(report.get("generated_at", "")),
        state=state,
        state_cls="good" if state == "PASS" else "bad",
        passed=passed, total=total, failed=failed, timed=timed,
        rows="\n".join(rows),
    )
    return page


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #
def main(argv=None):
    ap = argparse.ArgumentParser(
        prog="stress_runner.py",
        description="Isolith nightly stress/regression runner and status page.")
    ap.add_argument("--jobs", default="ci/stress/jobs.json",
                    help="job registry JSON (default: ci/stress/jobs.json)")
    ap.add_argument("--out-dir", default="docs/status",
                    help="directory for report.json + status.html "
                         "(default: docs/status)")
    ap.add_argument("--report", default="report.json",
                    help="report file name (default: report.json)")
    ap.add_argument("--page", default="status.html",
                    help="status page file name (default: status.html)")
    args = ap.parse_args(argv)

    jobs_path = os.path.join(REPO, args.jobs) \
        if not os.path.isabs(args.jobs) else args.jobs
    if not os.path.isfile(jobs_path):
        print("error: no job registry at %s" % jobs_path, file=sys.stderr)
        return 2

    jobs = load_jobs(jobs_path)
    repo, commit = git_identity(REPO)
    generated_at = datetime.now(timezone.utc).isoformat()
    started = time.monotonic()
    results = [run_job(job, REPO) for job in jobs]
    suite_dur = time.monotonic() - started
    report = build_report(results, generated_at, repo, commit)
    report["suite_duration_s"] = round(suite_dur, 3)

    out_dir = args.out_dir if os.path.isabs(args.out_dir) \
        else os.path.join(REPO, args.out_dir)
    os.makedirs(out_dir, exist_ok=True)
    report_path = os.path.join(out_dir, args.report)
    page_path = os.path.join(out_dir, args.page)
    with open(report_path, "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2, sort_keys=True)
    with open(page_path, "w", encoding="utf-8") as fh:
        fh.write(render_status_html(report))

    s = report["summary"]
    print("Isolith nightly stress run")
    print("repo=%s commit=%s generated=%s" % (repo, commit, generated_at))
    print("-" * 64)
    for r in results:
        print("[%6s] %-24s %8.2fs  %s" %
              (r["status"].upper(), r["name"], r["duration_s"],
               "" if r["exit_code"] is None else "exit %s" % r["exit_code"]))
    print("-" * 64)
    print("summary: %s/%d passed, %d failed, %d timed out (suite %.1fs)"
          % (s["passed"], s["total"], s["failed"], s["timed_out"], suite_dur))
    print("wrote %s" % report_path)
    print("wrote %s" % page_path)
    print("SUITE %s" % ("PASS" if (s["failed"] == 0 and s["timed_out"] == 0
                                   and s["error"] == 0) else "FAIL"))
    return 0 if (s["failed"] == 0 and s["timed_out"] == 0
                 and s["error"] == 0) else 1


if __name__ == "__main__":
    sys.exit(main())
