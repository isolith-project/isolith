"""P4-M5-T02: MBD validator CLI + report output (the wedge product surface).

Command-line surface over the P4-M5-T01 MBD validator service. It gathers the
inputs (a fixture manifest, or bundle/directory paths), runs the rule engine
through the service API (mbd.build_report / render_markdown / write_report are
reused, never re-implemented), emits the report artifact (JSON + Markdown),
and maps the outcome onto a documented, scriptable exit code:

    0  OK        every input was checked (or tolerated as an error row) and no
                 finding reached the --fail-on threshold (default: any FAIL);
                 the report artifact was written.
    1  FINDINGS  the run and report succeeded, but at least one input breached
                 the threshold (default: a FAIL finding) or an input could not
                 be validated. The report is still written and shows exactly
                 what happened. An error row is never a PASS.
    2  USAGE     invalid invocation: unknown option or rule set, missing or
                 conflicting input mode, --only without --manifest.
    3  INPUT     an input source could not be read: missing or malformed
                 manifest, missing/malformed explicit input file, a directory
                 with no loadable input, or zero inputs to check.
    4  INTERNAL  unexpected failure inside the validator (guard; not expected
                 in normal operation).

Full usage and exit-code contract:
docs/re-notes/p4-m5-t02-mbd-validator-cli.md
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import NoReturn

from oep_tooling import mbd_validate as mbd

EXIT_OK = 0
EXIT_FINDINGS = 1
EXIT_USAGE = 2
EXIT_INPUT = 3
EXIT_INTERNAL = 4

# Alias so the module surface mirrors the service it wraps.
mbd_validate = mbd

# Directories are scanned for "input" fixtures: bundled/bare oep-pmi JSON.
# `*.model.json` files carry oep-model-facts and are consumed through a bundle
# or manifest (model_file), never standalone, so a directory scan skips them.
_DIR_SKIP_SUFFIX = ".model.json"


class _UsageError(Exception):
    """Invalid invocation -> exit 2 (matches argparse's native usage code)."""


class _InputError(Exception):
    """An input source cannot be read -> exit 3."""


# ---------------------------------------------------------------------------
# Rule-set selection
# ---------------------------------------------------------------------------
def _parse_rule_sets(raw: str | None) -> list:
    if raw is None:
        return sorted(mbd.RULE_SETS)
    names = [n.strip() for n in raw.split(",") if n.strip()]
    if not names:
        raise _UsageError("--rule-sets must name at least one rule set")
    unknown = sorted(n for n in names if n not in mbd.RULE_SETS)
    if unknown:
        raise _UsageError(
            f"unknown rule set(s): {', '.join(unknown)}; "
            f"known: {', '.join(sorted(mbd.RULE_SETS))}")
    return names


# ---------------------------------------------------------------------------
# Input gathering
# ---------------------------------------------------------------------------
def _stem(path: str) -> str:
    base = os.path.basename(path)
    for suffix in (".oeppmi.json", ".model.json", ".json"):
        if base.endswith(suffix):
            return base[: -len(suffix)]
    return os.path.splitext(base)[0]


def _error_row(fid: str, path: str, rule_sets: list, err: Exception) -> list:
    """A tolerant error row per rule set (same shape as run_manifest's rows)."""
    return [{
        "input": {"file": path},
        "id": fid,
        "rule_set": sid,
        "findings": [],
        "status": "error",
        "error": str(err),
        "summary": {k: 0 for k in mbd.STATUSES} | {"total": 0},
    } for sid in rule_sets]


def _collect_file(path: str, rule_sets: list, tolerant: bool) -> list:
    """Check one bundle/oep-pmi file under every rule set.

    `tolerant=False` (explicit CLI input) surfaces load errors as a hard input
    failure; `tolerant=True` (directory scan) records an error row instead
    (tolerant-failure doctrine from P4-M5-T01).
    """
    try:
        bundle = mbd.load_input(path)
    except mbd.MbdInputError as exc:
        if tolerant:
            return _error_row(_stem(path), path, rule_sets, exc)
        raise _InputError(str(exc)) from exc
    bundle.setdefault("id", _stem(path))
    bundle.setdefault("source", {"file": os.path.basename(path)})
    return [mbd.check_input(bundle, sid) for sid in rule_sets]


def _collect_dir(path: str, rule_sets: list) -> list:
    """Check every loadable JSON input in a directory (tolerant per file)."""
    files = sorted(
        f for f in os.listdir(path)
        if f.endswith(".json") and not f.endswith(_DIR_SKIP_SUFFIX)
        and not f.startswith("."))
    if not files:
        raise _InputError(f"directory contains no *.json inputs to check: {path}")
    reports = []
    for f in files:
        reports += _collect_file(os.path.join(path, f), rule_sets, tolerant=True)
    return reports


def _collect_manifest(path: str, only: list | None, rule_sets: list) -> list:
    """Run the fixture manifest (T01 service path) under every rule set.

    A missing/malformed manifest is a hard input failure; per-fixture load
    problems inside a manifest stay tolerant error rows (service behaviour).
    """
    if not os.path.exists(path):
        raise _InputError(f"manifest file not found: {path}")
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        raise _InputError(f"manifest is not readable JSON: {path}: {exc}") from exc
    if not isinstance(data, dict) or "fixtures" not in data:
        raise _InputError(f"manifest has no 'fixtures' list: {path}")
    reports = mbd.run_manifest(path, only=only, rule_sets=rule_sets)
    if not reports:
        raise _InputError(f"no fixtures to check in manifest: {path}")
    return reports


def _collect_reports(inputs: list, manifest: str | None,
                     only: list | None, rule_sets: list) -> list:
    if manifest:
        return _collect_manifest(manifest, only, rule_sets)
    reports = []
    for path in inputs:
        if os.path.isdir(path):
            reports += _collect_dir(path, rule_sets)
        else:
            reports += _collect_file(path, rule_sets, tolerant=False)
    return reports


# ---------------------------------------------------------------------------
# Report writing (reuses the service pipeline)
# ---------------------------------------------------------------------------
def _write(report: dict, outdir: str, fmt: str) -> list:
    """Emit the report artifact per --format; returns the written paths.

    Default `both` delegates to the service's write_report (single source of
    truth for stamping + naming); single-format writes replicate the same
    stamping and reuse render_markdown.
    """
    if fmt == "both":
        return list(mbd.write_report(report, outdir))
    os.makedirs(outdir, exist_ok=True)
    from datetime import datetime, timezone
    stamped = dict(report)
    stamped["generated"] = datetime.now(timezone.utc).isoformat()
    paths = []
    if fmt == "json":
        path = os.path.join(outdir, "mbd-validate-report.json")
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(stamped, fh, indent=2, sort_keys=False)
            fh.write("\n")
        paths.append(path)
    else:  # fmt == "md"
        path = os.path.join(outdir, "mbd-validate-report.md")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(mbd.render_markdown(stamped))
        paths.append(path)
    return paths


# ---------------------------------------------------------------------------
# Exit-code decision
# ---------------------------------------------------------------------------
_THRESHOLD = {
    "fail": ("fail",),
    "warn": ("fail", "warn"),
    "unverified": ("fail", "warn", "unverified"),
}

def _breaches_threshold(reports: list, fail_on: str) -> bool:
    """True when the run must return EXIT_FINDINGS (1)."""
    if fail_on == "none":
        return False
    statuses = _THRESHOLD[fail_on]
    for r in reports:
        if r.get("status") == "error":
            return True  # an unvalidated input is never a PASS
        for f in r.get("findings", []):
            if f.get("status") in statuses:
                return True
    return False


# ---------------------------------------------------------------------------
# Product CLI
# ---------------------------------------------------------------------------
def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(
        prog="oep-mbd-validate",
        description="MBD validator CLI + report output (P4-M5-T02 wedge "
                    "surface) over the oep_tooling.mbd_validate service "
                    "(P4-M5-T01): checks supplied PMI/model bundles against "
                    "the y4141 and prime rule sets and emits an honest "
                    "JSON + Markdown report.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "exit codes: 0=clean 1=findings at/above --fail-on 2=usage "
            "3=input error 4=internal\n"
            "documented in docs/re-notes/p4-m5-t02-mbd-validator-cli.md"))
    ap.add_argument("inputs", nargs="*", metavar="INPUT",
                    help="bundle/oep-pmi JSON file(s) or a directory of them "
                         "(mutually exclusive with --manifest)")
    ap.add_argument("--manifest", metavar="PATH",
                    help="fixture manifest JSON to check (alternative to INPUT)")
    ap.add_argument("--only", metavar="ID", action="append", default=None,
                    help="restrict manifest fixtures to these ids (repeatable); "
                         "requires --manifest")
    ap.add_argument("--rule-sets", metavar="SETS", default=None,
                    help="comma-separated rule-set names (default: all known: "
                         + ", ".join(sorted(mbd.RULE_SETS)) + ")")
    ap.add_argument("--out", metavar="DIR", default=".",
                    help="report output directory (default: current directory)")
    ap.add_argument("--format", dest="fmt", choices=("json", "md", "both"),
                    default="both",
                    help="report artifacts to write (default: both)")
    ap.add_argument("--fail-on", dest="fail_on",
                    choices=("fail", "warn", "unverified", "none"),
                    default="fail",
                    help="status threshold that flips the exit code to 1 "
                         "(default: fail; 'none' never fails a successful run)")
    ap.add_argument("--verbose", action="store_true",
                    help="print a one-line rollup per input row")
    ap.add_argument("--version", action="store_true",
                    help="print tool name and version and exit")
    return ap


def main(argv=None) -> int:
    ap = build_parser()
    args = ap.parse_args(argv)

    if args.version:
        print(f"{mbd.TOOL['name']} {mbd.TOOL['version']}")
        return EXIT_OK

    if args.manifest and args.inputs:
        _usage(ap, "--manifest and INPUT are mutually exclusive; pick one input mode")
    if not args.manifest and not args.inputs:
        _usage(ap, "nothing to check: pass INPUT path(s) or use --manifest")
    if args.only and not args.manifest:
        _usage(ap, "--only requires --manifest")

    try:
        rule_sets = _parse_rule_sets(args.rule_sets)
        reports = _collect_reports(args.inputs, args.manifest, args.only, rule_sets)
    except _UsageError as exc:
        _usage(ap, str(exc))
    except _InputError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return EXIT_INPUT
    except Exception as exc:  # internal guard - not expected in normal use
        print(f"internal error: {exc}", file=sys.stderr)
        return EXIT_INTERNAL

    report = mbd.build_report(reports, {
        "task": "P4-M5-T02",
        "rule_sets": sorted({r["rule_set"] for r in reports}),
    })
    try:
        paths = _write(report, args.out, args.fmt)
    except OSError as exc:
        print(f"error: could not write report to {args.out}: {exc}", file=sys.stderr)
        return EXIT_INPUT

    s = report["summary"]
    print(f"mbd-validate: {s['inputs']} input rows | "
          f"PASS {s['pass']} FAIL {s['fail']} WARN {s['warn']} "
          f"UNVERIFIED {s['unverified']}")
    for p in paths:
        print(f"report: {os.path.abspath(p)}")
    if args.verbose:
        for row in report["inputs"]:
            rn = row["id"] or "?"
            if row.get("status") == "error":
                print(f"  [{rn}] ({row['rule_set']}): ERROR {row.get('error', '')}")
            else:
                sm = row["summary"]
                print(f"  [{rn}] ({row['rule_set']}): pass {sm.get('pass', 0)} "
                      f"fail {sm.get('fail', 0)} warn {sm.get('warn', 0)} "
                      f"unverified {sm.get('unverified', 0)}")

    if _breaches_threshold(reports, args.fail_on):
        return EXIT_FINDINGS
    return EXIT_OK


def _usage(ap: argparse.ArgumentParser, message: str) -> NoReturn:
    print(f"error: {message}", file=sys.stderr)
    ap.print_usage(sys.stderr)
    raise SystemExit(EXIT_USAGE)


if __name__ == "__main__":
    sys.exit(main())
