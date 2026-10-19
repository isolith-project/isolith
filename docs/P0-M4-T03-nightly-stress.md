# P0-M4-T03: Nightly Stress Run + Status Page

Status: implemented (2026-10-15). Acceptance: `tests/stress_suite_test.py`.

## What this is

A nightly, unattended stress/regression run whose results are published to a
self-contained static status page. Long fuzz and regression jobs are
registered in one place, executed with per-job timeouts, and every run
produces two artifacts:

- `docs/status/report.json` — machine-readable results (schema below)
- `docs/status/status.html` — human-readable status page (committed snapshot;
  refreshed every nightly run)

This is the P0 slice of the P12-T08 the full status page + on-call story; the
runner and publishing mechanism are what P12 reuses and extends.

## How it runs

Nightly trigger: `.gitea/workflows/nightly-stress.yaml` schedules at 02:00 UTC
and supports manual `workflow_dispatch`. It runs the same local command:

    python3 ci/stress/stress_runner.py --jobs ci/stress/jobs.json

The runner executes every job in `ci/stress/jobs.json` sequentially, each
under its own timeout (a timed-out job is a timed-out job, never a hang of the
whole night), captures a log tail, and writes `report.json` + `status.html`
into `--out-dir` (default `docs/status`). Exit code is non-zero if any lane
fails or times out, so the CI job goes red honestly.

Local run (identical semantics):

    python3 ci/stress/stress_runner.py --jobs ci/stress/jobs.json --out-dir docs/status

Acceptance test first (TDD), per plan doctrine:

    python3 tests/stress_suite_test.py

## Registered lanes (2026-10-15)

| lane | category | what it does |
|---------------------|------------|--------------------------------------------|
| cargo-build-workspace | regression | real Rust workspace build |
| cargo-test-workspace  | regression | real Rust workspace unit tests |
| python-layout-acceptance | regression | existing `tests/monorepo_layout_test.py` against the real tree |
| text-file-integrity | regression | every repo text file: strict UTF-8, no NUL/C0 controls, LF-only |
| fuzz-status-page | stress | long fuzz of the status/report serializers with hostile bytes; garbage-in-safe-out |

The fuzz lane is real long fuzz today because wave 1's only real "parser"
surface is the report/status serializer code this task itself ships: feed it
control bytes, HTML metacharacters, emoji, truncation and raw random bytes and
prove the status page still renders clean HTML and the report still
round-trips through JSON. When P2-M1-T05 (CGR header fuzz) and P2-M6-T01
(fuzz round 2) land, they are added here as new stress lanes in `jobs.json` —
no runner changes needed.

## report.json schema (schema_version 1)

    {
      "schema_version": 1,
      "repo": "don/keelworks",
      "commit": "<short sha>",
      "generated_at": "<ISO8601 UTC>",
      "suite_duration_s": 12.34,
      "jobs": [
        {"name": "...", "category": "stress|regression", "status": "pass|fail|timeout|skip|error",
         "exit_code": 0,                                    # null on timeout/error
         "duration_s": 1.234,
         "output_tail": "<last 4000 chars of combined output>",
        }
      ],
      "summary": {"total": 5, "passed": 5, "failed": 0, "timed_out": 0, "skipped": 0, "error": 0}
    }

## Safety contract

Every job-derived string (name, log tail, repo, commit) is HTML-escaped and
C0-control-stripped before rendering. The status page is fuzzed nightly by
`stress_fuzz_report.py` to prove `<script>` in a job name is text, never
markup, and that any report dict renders + serializes without raising.

## Adding a lane

1. Edit `ci/stress/jobs.json` with the job name, category
   (`stress` or `regression`), command list, and a generous
   `timeout_seconds`.
2. Edit `tests/stress_suite_test.py` `REQUIRED_JOBS` in the same PR — the
   acceptance test enforces that the registry and the test agree.
3. Re-run `python3 tests/stress_suite_test.py`, then a local stress run.

## In flight, related

- P0-M4-T01 builds the on-push CI (lint + unit + integration); this card only
  owns the nightly lane, so the two workflows do not conflict.
- P12-T08 later layers on on-call routing for failures of this nightly run.
