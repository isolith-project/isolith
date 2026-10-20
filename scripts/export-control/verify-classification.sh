#!/usr/bin/env bash
# Keelworks P11-T05: export-control classification gate (the release
# pipeline checkbox). Wraps python/oep_tooling/export_control_gate.py so CI
# and the release-cut step share one entry point.
#
# Exit code contract:
#   verify --release <V>   -> 0 iff workstreams/security/export-control/<V>.classification.toml
#                             exists AND validates (UNCLASSIFIED, data
#                             locality, no ITAR/ECCN-restricted content).
#                             Exit 1 blocks the release (absence included).
#   --selfcheck            -> CI-safe: validate template + every committed
#                             classification file; no specific release needed.
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
ROOT="$PWD"

GATE="python/oep_tooling/export_control_gate.py"
RELEASE=""
SELFCHECK=false

usage() {
    sed -n '1,14p' "${BASH_SOURCE[0]}" | sed -n 's/^# \{0,1\}//p'
    echo
    echo "Usage:"
    echo "  $0 verify --release <V> [--repo DIR]"
    echo "  $0 --selfcheck [--repo DIR]"
    echo "  $0 --help"
}

# --- default repo -----------------------------------------------------------------
REPO="$ROOT"

# --- parse args -------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        verify|selfcheck) shift 1 ;;
        --release) RELEASE="${2:?--release needs a value}"; shift 2 ;;
        --selfcheck) SELFCHECK=true; shift ;;
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

PY=""
if [ -n "${VIRTUAL_ENV:-}" ]; then PY="$VIRTUAL_ENV/bin/python";
elif [ -x ".venv/bin/python" ]; then PY=".venv/bin/python";
elif command -v python3 >/dev/null 2>&1; then PY="python3"; fi
if [ -z "$PY" ]; then echo "FAIL  no python3 found" >&2; exit 1; fi

echo "== P11-T05 export-control gate =="
if $SELFCHECK; then
    "$PY" "$GATE" selfcheck --repo "$REPO"
    rc=$?
elif [ -n "$RELEASE" ]; then
    echo "release=$RELEASE compute=$REPO"
    "$PY" "$GATE" verify --release "$RELEASE" --repo "$REPO"
    rc=$?
else
    echo "FAIL  pick: verify --release <V> or --selfcheck" >&2
    usage; exit 2
fi

if [ "$rc" -eq 0 ]; then
    echo "== P11-T05 export-control gate: PASS =="
else
    echo "== P11-T05 export-control gate: FAIL (release blocked) =="
fi
exit "$rc"
