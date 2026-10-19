#!/usr/bin/env bash
# P12-T09 -- Keelworks E2E scenario suite driver (reference vertical: import
# CGR/STEP -> model -> release via PDM -> render/convert via P2P -> share).
#
# Thin bash wrapper around python/oep_tooling/e2e_vertical.py in the same
# shape as scripts/install-rehearsal.sh: measured legs with a closed SKIP
# taxonomy, exit 0 iff no FAIL row, JSON + Markdown report. Every release runs
# the vertical headlessly against the stack; its pass is a release-blocking CI
# check (ga-gate-p12e2e.yaml) alongside the GA gates.
#
# Usage:
#   ./e2e-vertical.sh --version 0.1.0                          # local real binaries
#   ./e2e-vertical.sh --version 0.1.0 --mode docker            # packaged stack image
#   ./e2e-vertical.sh --version 0.1.0 --break-stage release    # deliberate-failure repro
#   ./e2e-vertical.sh --selfcheck                              # CI-safe docs/contract only
#   ./e2e-vertical.sh --help
#
# Exit code: 0 iff the recorded report has ZERO FAIL rows (SKIP allowed under
# the closed taxonomy). Non-zero = do not ship.

set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.."
ROOT="$PWD"

VERSION=""
MODE="local"
BIN_DIR="${BIN_DIR:-}"
CGR=""
BREAK=""
OUT_BASE="P12/proof/e2e-vertical"
SELFCHECK=false

usage() { sed -n '1,30p' "${BASH_SOURCE[0]}" | sed -n 's/^# \{0,1\}//p'; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
        --mode) MODE="${2:?--mode needs a value}"; shift 2 ;;
        --bin-dir) BIN_DIR="${2:?--bin-dir needs a value}"; shift 2 ;;
        --cgr) CGR="${2:?--cgr needs a value}"; shift 2 ;;
        --break-stage) BREAK="${2:?--break-stage needs a value}"; shift 2 ;;
        --out) OUT_BASE="${2:?--out needs a value}"; shift 2 ;;
        --selfcheck) SELFCHECK=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

if [ -z "$VERSION" ]; then
    V0="$(sed -n 's/^OEP_VERSION=//p' P10-M1/deploy/.env.example 2>/dev/null | head -1)"
    VERSION="${V0:-0.1.0}"
fi

# Match the P12-T06 proof convention: <base>-<version>.{json,md}.
# Always append the version in run mode (deterministic artifact names);
# selfcheck writes <base>.json exactly as the driver does.
if [ "$SELFCHECK" = false ]; then
    OUT_BASE="$OUT_BASE-$VERSION"
fi

ARGS=(--repo "$ROOT" --mode "$MODE" --version "$VERSION" --out "$OUT_BASE")
[ -n "$BIN_DIR" ] && ARGS+=(--bin-dir "$BIN_DIR")
[ -n "$CGR" ] && ARGS+=(--cgr "$CGR")
[ -n "$BREAK" ] && ARGS+=(--break-stage "$BREAK")
[ "$SELFCHECK" = true ] && ARGS+=(--selfcheck)

PY="${PYTHON:-python3}"
if [ -d "$ROOT/python" ]; then
    export PYTHONPATH="$ROOT/python:${PYTHONPATH:-}"
fi
exec "$PY" -m oep_tooling.e2e_vertical "${ARGS[@]}"
