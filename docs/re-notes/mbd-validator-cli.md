# P4-M5-T02: MBD validator CLI + report output (wedge product surface)

Task t_697255d5, Isolith wave-2 P4 MBD lane. The product surface over the
P4-M5-T01 validator service: a real, scriptable CLI that checks a supplied
model/PMI bundle (or a whole fixture manifest) against the `y4141` and `prime`
rule sets, emits the honest report artifact (JSON + Markdown), and returns a
documented exit code a demo or a QA gate can branch on.

It reuses the service - `mbd.build_report` / `render_markdown` / `write_report`
- and never re-implements the rule engine (P4-M5-T01 contract).

- Version: 1.0 (2026-10-16)
- Status: verified on real corpus material; see section 6.

## 1. Invocation

The module is the CLI (same `python3 -m` convention as the P4-M2-T03 PMI
fidelity runner):

    cd isolith
    PYTHONPATH=python python3 -m oep_tooling.mbd_validate_cli [OPTIONS] [INPUT...]
    PYTHONPATH=python python3 -m oep_tooling.mbd_validate_cli --manifest MANIFEST [OPTIONS]

```
usage: oep-mbd-validate [-h] [--manifest PATH] [--only ID]
                        [--rule-sets SETS] [--out DIR]
                        [--format {json,md,both}] [--fail-on {fail,warn,unverified,none}]
                        [--verbose] [--version]
                        [INPUT ...]
```

| option | meaning |
|---|---|
| `INPUT ...` | one or more bundle/oep-pmi JSON files, or a directory of them |
| `--manifest PATH` | fixture manifest JSON to check (alternative to `INPUT`; mutually exclusive) |
| `--only ID` | restrict manifest fixtures to these ids (repeatable; requires `--manifest`) |
| `--rule-sets SETS` | comma-separated rule-set names from `y4141`, `prime` (default: both) |
| `--out DIR` | report output directory (default: current directory) |
| `--format {json,md,both}` | artifacts to write (default `both`) |
| `--fail-on {fail,warn,unverified,none}` | status threshold that flips the exit code to 1 (default `fail`) |
| `--verbose` | print a one-line rollup per input row |
| `--version` | print tool name/version and exit |

Input modes:

- **Manifest mode** (`--manifest`) - checks every registered corpus fixture
  under every selected rule set. Missing/malformed manifest = INPUT (exit 3);
  a fixture inside the manifest that cannot load is a tolerant error row
  (reported, run continues) per the P4-M5-T01 tolerant-failure doctrine.
- **File mode** (one or more `INPUT` paths) - each path is a full bundle JSON
  or a bare oep-pmi document (service `load_input`). An explicit file that
  cannot be read is a hard INPUT error (exit 3).
- **Directory mode** (`INPUT` is a directory) - every `*.json` in the
  directory is checked; per-file load failures become tolerant error rows.
  `*.model.json` files carry oep-model-facts and are consumed through a bundle/
  manifest (`model_file`), never standalone, so directory scans skip them.

Any number of `--only` ids is honored; out dir and format control the artifact.

## 2. Exit codes (contract)

| code | name | meaning |
|---|---|---|
| 0 | OK | every input was checked (or tolerated as an error row) and **no finding reached the `--fail-on` threshold**; the report artifact was written |
| 1 | FINDINGS | the run and report succeeded, but **at least one input breached the threshold** (default: any FAIL finding) or **an input could not be validated** (error row). The report is still written and shows exactly what happened |
| 2 | USAGE | invalid invocation: unknown option or rule set, missing or conflicting input mode, `--only` without `--manifest` |
| 3 | INPUT | an input source could not be read: missing or malformed manifest, missing/malformed explicit input file, a directory with no loadable input, or zero inputs to check |
| 4 | INTERNAL | unexpected failure inside the validator (guard, not expected in normal operation) |

Threshold semantics (`--fail-on`):

| value | exit 1 when... |
|---|---|
| `fail` (default) | any FAIL finding, or any error row |
| `warn` | any FAIL or WARN finding, or any error row |
| `unverified` | any non-PASS finding (FAIL/WARN/UNVERIFIED), or any error row - the strictest, release-gate posture |
| `none` | a successful run never exits 1; the report is written regardless |

Honesty coupling: UNVERIFIED is never a PASS and does not default to exit 1 -
consistent with the service doctrine (section 2 of the P4-M5-T01 note). Opt in
to treating it as release-blocking with `--fail-on unverified`.

## 3. Report artifact

`--out <dir>` receives (per `--format`):

- `mbd-validate-report.json` - the `oep-mbd-validate` report format v1:
  `summary` (computed from the granular findings, never hand-typed),
  per-input `findings` (rule, status, severity, evidence), honesty
  scope/limits block, tool/task identity.
- `mbd-validate-report.md` - the same data rendered for humans; the status
  legend and honesty scope always render the full PASS / FAIL / WARN /
  UNVERIFIED vocabulary.

The Markdown H1 title tracks the task that produced the run (honest labeling
of the artifact), so a demo report generated by this CLI reads "P4-M5-T02 MBD
Validator Report".

## 4. Demo walkthrough (defense-prime)

Run the whole registered fixture manifest and emit both artifacts:

    cd isolith
    PYTHONPATH=python python3 -m oep_tooling.mbd_validate_cli \
        --manifest corpus/mbd-fixture-manifest.json --out out/
    echo $?            # 1 - the corpus deliberately contains FAIL fixtures

A clean scope (only the complete golden bracket) exits 0:

    PYTHONPATH=python python3 -m oep_tooling.mbd_validate_cli \
        --manifest corpus/mbd-fixture-manifest.json --only bracket-complete \
        --out out-clean/
    echo $?            # 0

Check a single supplied part's PMI bundle (whatever P4-M2-T01 or a future
AP242/QIF importer emits as oep-pmi), then review the honest verdict:

    PYTHONPATH=python python3 -m oep_tooling.mbd_validate_cli \
        part.oeppmi.json --out out-part/ --verbose

See `--verbose` for a per-input rollup; `corpus/mbd-validate-report.{json,md}`
on the P4-M5-T01 branch are the reference outputs this CLI reproduces.

## 5. Acceptance mapping

- CLI runs on a real corpus fixture and emits the report artifact: the
  manifest/fixture demo (section 4) writes JSON + Markdown.
- Exit codes documented: this table (section 2) is pinned by
  `tests/test_mbd_validate_cli_acceptance.py` (0/1/2/3 exit-code tests over
  the real fixture manifest and real defect files).
- Demo-able end to end: one command (section 4) from the repo root.
- Results verified against real data, not a log line: the acceptance test
  re-reads the emitted JSON, recomputes the rollup from the granular findings,
  compares every row's verdicts against the committed reference report, and
  checks per-fixture outcomes against the manifest `expect` blocks - never
  against what the CLI printed.

Re-verify anytime:

    PYTHONPATH=python python3 -m pytest tests/test_mbd_validate_cli_acceptance.py -q

## 6. Verification record (2026-10-16)

Ran against `corpus/mbd-fixture-manifest.json` (8 fixtures x 2 rule sets).
Emitted report: 16 rows, 272 findings; independent recompute from the granular
findings = PASS 246 / FAIL 18 / WARN 2 / UNVERIFIED 6, matching the report
summary exactly; verdict parity with the committed P4-M5-T01 reference report:
0 mismatches. Full suite 74 tests green (55 P4-M5-T01 + 19 P4-M5-T02).

Anti-collision: new files `python/oep_tooling/mbd_validate_cli.py` +
`tests/test_mbd_validate_cli_acceptance.py` + this note; one intentional,
tested line of report-output scope in the service (`render_markdown` H1 title
tracks `task`). No Rust crates touched.
