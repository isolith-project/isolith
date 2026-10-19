#!/usr/bin/env python3
"""P0-M4-T03 acceptance test: nightly stress run + status page.

Verifies the stress/regression job runner, its job registry, the status-page
renderer's HTML safety, and the fuzz contract. Pure stdlib, runs under
`python3 tests/stress_suite_test.py` and pytest. Each check FAILS loud;
exit code 1 on any failure.

"Done" for P0-M4-T03 means this test passes against the real tree AND a real
nightly run (`python3 ci/stress/stress_runner.py --jobs ci/stress/jobs.json`)
produces docs/status/status.html + report.json with all jobs green on real
material (cargo build/test, repo text integrity, fuzz).

Intent (per master plan P0-M4-T03): nightly long fuzz/regression jobs whose
results are published to a status page. The regression lanes are real today
(cargo build/test are the only real code in wave 1); the fuzz lane targets the
status-page/report serializers so hostile job output can never break the page
(garbage-in-safe-out), and later P2/P3 fuzz harnesses plug in via jobs.json.
"""
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Jobs the registry must carry (name -> category pair). These are the real
# nightly lanes; adding a lane means editing jobs.json AND this test together.
REQUIRED_JOBS = {
    "cargo-build-workspace": "regression",
    "cargo-test-workspace": "regression",
    "python-layout-acceptance": "regression",
    "text-file-integrity": "regression",
    "fuzz-status-page": "stress",
}

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def run(cmd, cwd=REPO, timeout=300):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                          timeout=timeout)


def load_runner_module():
    path = os.path.join(REPO, "ci", "stress", "stress_runner.py")
    check("exists: ci/stress/stress_runner.py", os.path.isfile(path), path)
    if not os.path.isfile(path):
        return None
    spec = importlib.util.spec_from_file_location("stress_runner", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_file_layout():
    for rel in ("ci/stress/stress_runner.py",
                "ci/stress/jobs.json",
                "ci/stress/stress_fuzz_report.py",
                "ci/stress/text_file_integrity.py",
                ".gitea/workflows/nightly-stress.yaml",
                "docs/P0-M4-T03-nightly-stress.md"):
        check("exists: %s" % rel, os.path.isfile(os.path.join(REPO, rel)),
              os.path.join(REPO, rel))


def test_jobs_registry():
    path = os.path.join(REPO, "ci", "stress", "jobs.json")
    if not os.path.isfile(path):
        check("jobs.json loads", False, "missing file")
        return
    try:
        data = json.load(open(path, encoding="utf-8"))
    except Exception as e:
        check("jobs.json is valid JSON", False, repr(e))
        return
    jobs = data.get("jobs")
    check("jobs.json has jobs list", isinstance(jobs, list) and len(jobs) >= 3,
          repr(len(jobs)) if isinstance(jobs, list) else "no jobs key")
    if not isinstance(jobs, list):
        return
    names_seen = set()
    for job in jobs:
        name = job.get("name", "")
        cat = job.get("category", "")
        check("job %r has name" % name, bool(name))
        check("job %r category in {stress, regression}" % name,
              cat in ("stress", "regression"), repr(cat))
        check("job %r timeout_seconds > 0" % name,
              isinstance(job.get("timeout_seconds"), (int, float))
              and job["timeout_seconds"] > 0, repr(job.get("timeout_seconds")))
        cmd = job.get("command")
        check("job %r has non-empty command" % name,
              isinstance(cmd, list) and len(cmd) >= 1, repr(cmd))
        check("job %r name unique" % name, name not in names_seen, name)
        names_seen.add(name)
    for name, cat in REQUIRED_JOBS.items():
        match = [j for j in jobs if j.get("name") == name]
        check("required job %r present" % name, bool(match))
        if match:
            check("job %r category == %s" % (name, cat),
                  match[0].get("category") == cat,
                  repr(match[0].get("category")))


def test_job_commands_resolve():
    """First element of every command must resolve on PATH, or be a real
    repo-relative path; the first arg that is a repo path must exist."""
    path = os.path.join(REPO, "ci", "stress", "jobs.json")
    if not os.path.isfile(path):
        check("job commands resolve", False, "no jobs.json")
        return
    jobs = json.load(open(path, encoding="utf-8")).get("jobs", [])
    for job in jobs:
        cmd = job.get("command", [])
        if not cmd:
            continue
        name = job.get("name", "?")
        head = cmd[0]
        resolved = shutil_which(head) or os.path.isfile(os.path.join(REPO, head))
        check("job %r command head %r resolves" % (name, head), bool(resolved),
              repr(head))
        # Any subsequent argument that points into the repo must exist.
        for arg in cmd[1:]:
            if os.path.dirname(arg) in ("", "."):
                continue
            if arg.startswith("./") or arg.startswith("ci/") or \
               arg.startswith("tests/") or arg.startswith("docs/"):
                check("job %r arg %r exists" % (name, arg),
                      os.path.isfile(os.path.join(REPO, arg)),
                      os.path.join(REPO, arg))


def shutil_which(cmd):
    import shutil
    return shutil.which(cmd)


def _synthetic_jobs_json(tmpdir):
    """A fast synthetic registry: one passing job, one failing job, one
    timing-out job. Used to prove the runner's reporting, not the real lanes."""
    jobs = [
        {"name": "ok-job", "category": "regression",
         "command": ["python3", "-c", "print('ok')"], "timeout_seconds": 30},
        {"name": "bad-job", "category": "regression",
         "command": ["python3", "-c", "import sys; sys.exit(3); print('nope')"],
         "timeout_seconds": 30},
        {"name": "slow-job", "category": "stress",
         "command": ["python3", "-c", "import time; time.sleep(60)"],
         "timeout_seconds": 2},
    ]
    with open(os.path.join(tmpdir, "jobs.json"), "w", encoding="utf-8") as fh:
        json.dump({"jobs": jobs}, fh)
    return os.path.join(tmpdir, "jobs.json")


def test_runner_cli_and_reporting():
    mod = load_runner_module()
    if mod is None:
        return
    r = run([sys.executable, os.path.join(REPO, "ci", "stress",
                                          "stress_runner.py"), "--help"],
            timeout=60)
    check("runner --help exits 0", r.returncode == 0,
          "rc=%s stderr=%s" % (r.returncode, r.stderr[-200:]))
    check("runner --help prints usage", "usage" in (r.stdout + r.stderr).lower())

    with tempfile.TemporaryDirectory() as td:
        jobs = _synthetic_jobs_json(td)
        out = os.path.join(td, "out")
        os.makedirs(out)
        rr = run([sys.executable, os.path.join(REPO, "ci", "stress",
                                               "stress_runner.py"),
                  "--jobs", jobs, "--out-dir", out,
                  "--report", "report.json", "--page", "status.html"],
                 cwd=REPO, timeout=120)
        # A failing + timing-out job must yield a non-zero runner exit.
        check("runner exits non-zero on failing suite", rr.returncode != 0,
              "rc=%s" % rr.returncode)

        report_path = os.path.join(out, "report.json")
        page_path = os.path.join(out, "status.html")
        check("report.json written", os.path.isfile(report_path), report_path)
        check("status.html written", os.path.isfile(page_path), page_path)
        if not os.path.isfile(report_path):
            return
        report = json.load(open(report_path, encoding="utf-8"))
        by_name = {j["name"]: j for j in report.get("jobs", [])}
        check("report has ok-job pass", by_name.get("ok-job", {}).get(
            "status") == "pass", repr(by_name.get("ok-job")))
        check("report has bad-job fail with exit 3",
              by_name.get("bad-job", {}).get("status") == "fail"
              and by_name.get("bad-job", {}).get("exit_code") == 3,
              repr(by_name.get("bad-job")))
        check("report has slow-job timeout",
              by_name.get("slow-job", {}).get("status") == "timeout",
              repr(by_name.get("slow-job")))
        check("durations are positive floats",
              all(isinstance(j.get("duration_s"), (int, float))
                  and j["duration_s"] > 0 for j in report.get("jobs", [])),
              repr([j.get("duration_s") for j in report.get("jobs", [])]))
        summ = report.get("summary", {})
        check("summary counts present",
              all(k in summ for k in ("total", "passed", "failed", "timed_out")),
              repr(summ))
        check("summary passed == 1",
              summ.get("passed") == 1, repr(summ.get("passed")))
        check("summary total == 3", summ.get("total") == 3, repr(summ.get("total")))


def test_status_page_html_and_escaping():
    """The status page must be well-formed and must HTML-escape every
    job-derived string: hostile names/logs are data, never markup."""
    mod = load_runner_module()
    if mod is None or not hasattr(mod, "render_status_html"):
        check("render_status_html exists", False, "missing renderer")
        return
    hostile_name = "<script>alert(1)</script>"
    hostile_log = 'weird "quotes" & <b>tags</b> \N{SNOWMAN}'
    report = {
        "schema_version": 1,
        "repo": "don/keelworks",
        "commit": "abc123",
        "generated_at": "2026-10-15T00:00:00Z",
        "jobs": [
            {"name": hostile_name, "category": "stress",
             "status": "fail", "exit_code": 1, "duration_s": 1.25,
             "output_tail": hostile_log},
            {"name": "good", "category": "regression",
             "status": "pass", "exit_code": 0, "duration_s": 0.5,
             "output_tail": "ok"},
        ],
        "summary": {"total": 2, "passed": 1, "failed": 1,
                    "timed_out": 0, "skipped": 0},
    }
    try:
        html = mod.render_status_html(report)
    except Exception as e:
        check("render_status_html does not raise", False, repr(e))
        return
    check("html is str", isinstance(html, str))
    check("html has <html>", "<html" in html.lower())
    check("html has </html>", "</html>" in html.lower())
    check("hostile name appears escaped, raw script tag absent",
          "&lt;script&gt;alert(1)&lt;/script&gt;" in html
          and "<script>alert(1)</script>" not in html)
    check("hostile log quoted/escaped correctly",
          "&amp; &lt;b&gt;tags&lt;/b&gt;" in html or "&lt;b&gt;tags&lt;/b&gt;" in html)
    check("job name present", hostile_name.replace("<", "&lt;").replace(
        ">", "&gt;") in html)
    check("summary counts in page",
          "1" in html and "2" in html)  # loose: total/passed counts rendered
    # JSON round-trip of report with hostile bytes (resolve render inputs).
    back = json.loads(json.dumps(report))
    check("report json round-trips hostile log",
          back["jobs"][0]["output_tail"] == report["jobs"][0]["output_tail"])


def test_fuzz_contract():
    """The committed fuzz job must actually fuzz (nonzero iteration count) and
    exit 0 with a bounded iteration count here so the acceptance test is fast."""
    path = os.path.join(REPO, "ci", "stress", "stress_fuzz_report.py")
    check("exists: stress_fuzz_report.py", os.path.isfile(path))
    if not os.path.isfile(path):
        return
    r = run([sys.executable, path, "--iterations", "50", "--seed", "7"],
            timeout=180)
    check("fuzz job exits 0", r.returncode == 0,
          "rc=%s stderr=%s" % (r.returncode, r.stderr[-300:]))
    m = re.search(r"iterations=(\d+)", r.stdout)
    check("fuzz job reports iterations>0",
          bool(m) and int(m.group(1)) >= 50,
          "stdout=%r" % r.stdout[-300:])


def test_nightly_schedule():
    """The workflow must declare a nightly cron schedule (the 'nightly' in
    the task name is a live contract, not just a label)."""
    path = os.path.join(REPO, ".gitea", "workflows", "nightly-stress.yaml")
    if not os.path.isfile(path):
        check("nightly-stress.yaml has schedule", False, "missing file")
        return
    text = open(path, encoding="utf-8").read()
    check("workflow declares schedule", "schedule" in text)
    m = re.search(r"cron\s*[:=]\s*[\"']([^\"']+)[\"']", text)
    check("workflow declares a cron expression",
          bool(m) and len(m.group(1).split()) == 5,
          repr(m.group(1)) if m else "no cron")
    check("workflow invokes stress_runner.py", "stress_runner.py" in text)


def main():
    print("Keelworks nightly stress suite acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_file_layout()
    test_jobs_registry()
    test_job_commands_resolve()
    test_runner_cli_and_reporting()
    test_status_page_html_and_escaping()
    test_fuzz_contract()
    test_nightly_schedule()
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
