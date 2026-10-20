#!/usr/bin/env bash
# P12-T06 -- Keelworks release install rehearsal (r1a-release-workflow pattern).
#
# Before a release ships, rehearse the one-command install on a fresh clean
# machine. For every release this runs the P10-M1-T01 one-command install
# (`workstreams/delivery/deploy/deploy/install.sh`) from a clean state on the two approved hosts
# (decision #9): machine A = U1 (the local host), machine B = a U1/VM-class
# host reached over SSH (default: the keelwb2 QEMU/KVM VM on U1; set
# MACHINE_B_HOST/PORT to point at any approved second host, e.g. the garage PC
# over WoL). The NAS stays repo/storage only -- it is never a run target.
#
# All three health checks must come up green on both hosts:
#   PDM     GET /api/v1/health  -> HTTP 200 {"status":"ok",...}
#   Viewer  GET /               -> HTTP 200 (explorer HTML)
#   P2P     GET /healthz        -> HTTP 200 {"status":"ok"}
# plus the worker binaries (oep-batch-convert, oep-render-worker) present in
# the installed image. The run is recorded to workstreams/release-engineering/proof/release-rehearsal-
# <ver>.{json,md} with a strict exit-code contract: exit 0 iff no FAIL row.
#
# A fresh clean machine per release (the r1a-release-workflow rule) means the
# rehearsal never reuses a previous install: this script tears down all
# keelworks containers, volumes and the stack image and removes the generated
# .env before each install leg, so every run starts from the same clean state
# the checklist in workstreams/release-engineering/install-rehearsal.md tells an operator to start from.
#
# Usage:
#   ./install-rehearsal.sh                             # rehearse the .env.example version
#   ./install-rehearsal.sh --version 0.1.0             # rehearse a specific release version
#   ./install-rehearsal.sh --machine-b-host op@127.0.0.1 --machine-b-port 2229
#   ./install-rehearsal.sh --selfcheck                 # CI-safe doc/version checks, no docker
#   ./install-rehearsal.sh --help
#
# Exit code 0 iff the recorded report has zero FAIL rows (same contract as the
# P8/P9/P10 gate proof scripts).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
ROOT="$PWD"

VERSION_ARGS=""
SELFCHECK=false
MACHINE_B_HOST="${MACHINE_B_HOST:-op@127.0.0.1}"
MACHINE_B_PORT="${MACHINE_B_PORT:-2229}"
OUT_BASE="workstreams/release-engineering/proof/release-rehearsal"

usage() {
    sed -n '1,40p' "${BASH_SOURCE[0]}" | sed -n 's/^# \{0,1\}//p'
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version) VERSION_ARGS="${2:?--version needs a value}"; shift 2 ;;
            --machine-b-host) MACHINE_B_HOST="${2:?--machine-b-host needs a value}"; shift 2 ;;
            --machine-b-port) MACHINE_B_PORT="${2:?--machine-b-port needs a value}"; shift 2 ;;
            --out) OUT_BASE="${2:?--out needs a value}"; shift 2 ;;
            --selfcheck) SELFCHECK=true; shift ;;
            --help|-h) usage; exit 0 ;;
            *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
        esac
    done
}

parse_args "$@"

# --- measured report (same shape as the P8/P9/P10 gate proof reports) --------
REPORT_DIR="$(dirname "$OUT_BASE")"
ROWS=()   # "leg|id|status|detail"

row() { ROWS+=("$1|$2|$3|$4"); }

summary() {
    local total=0 pass=0 fail=0 skip=0 st
    for r in "${ROWS[@]}"; do
        st="${r#*|*|}"; st="${st%%|*}"
        total=$(( total + 1 ))
        case "$st" in PASS) pass=$(( pass + 1 ));; FAIL) fail=$(( fail + 1 ));; SKIP) skip=$(( skip + 1 ));; esac
    done
    echo "total=$total PASS=$pass FAIL=$fail SKIP=$skip"
}

exit_code() {
    local st
    for r in "${ROWS[@]}"; do
        st="${r#*|*|}"; st="${st%%|*}"
        [[ "$st" == "FAIL" ]] && return 1
    done
    return 0
}

write_report() {
    local base="$1" ts sum ec leg id st det
    mkdir -p "$REPORT_DIR"
    ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    sum="$(summary)"
    ec=0; exit_code || ec=1
    {
        echo "{"
        echo "  \"task\": \"P12-T06\","
        echo "  \"title\": \"Install rehearsal (fresh clean machine per release)\","
        echo "  \"version\": \"$VERSION\","
        echo "  \"timestamp\": \"$ts\","
        echo "  \"schema_version\": 1,"
        echo "  \"summary\": {"
        echo "$sum" | sed -E 's/^total=([0-9]+) PASS=([0-9]+) FAIL=([0-9]+) SKIP=([0-9]+)$/    "total": \1, "PASS": \2, "FAIL": \3, "SKIP": \4/'
        echo "  },"
        echo "  \"exit_code\": $ec,"
        echo "  \"rows\": ["
        local first=true
        for r in "${ROWS[@]}"; do
            [[ "$first" == false ]] && echo ","
            first=false
            leg="${r%%|*}"; det="${r#*|}"
            id="${det%%|*}"; det="${det#*|}"
            st="${det%%|*}"; det="${det#*|}"
            printf '    {"leg":"%s","id":"%s","status":"%s","detail":"%s"}' "$leg" "$id" "$st" "$det"
        done
        echo ""
        echo "  ]"
        echo "}"
    } > "$base.json"
    {
        echo "# P12-T06 -- Install rehearsal for release $VERSION"
        echo ""
        echo "Run: $ts"
        echo ""
        echo "**Summary:** $sum"
        echo ""
        echo "## Rows"
        echo ""
        echo "| leg | id | status | detail |"
        echo "|-----|----|--------|--------|"
        for r in "${ROWS[@]}"; do
            leg="${r%%|*}"; det="${r#*|}"
            id="${det%%|*}"; det="${det#*|}"
            st="${det%%|*}"; det="${det#*|}"
            echo "| $leg | $id | $st | $det |"
        done
        echo ""
    } > "$base.md"
}

# --- helpers ----------------------------------------------------------------
on_target() { # host port cmd  (host == LOCAL runs locally, else over ssh)
    if [[ "$1" == "LOCAL" ]]; then
        bash -c "$3"
    else
        ssh -o BatchMode=yes -o ConnectTimeout=15 -o StrictHostKeyChecking=accept-new \
            -p "$2" "$1" "$3"
    fi
}

# --- version derivation -------------------------------------------------------
ENV_EXAMPLE_VERSION="$(sed -n 's/^OEP_VERSION=//p' "$ROOT/workstreams/delivery/deploy/deploy/.env.example" | head -1)"
ENV_EXAMPLE_VERSION="${ENV_EXAMPLE_VERSION:-0.1.0}"
VERSION="${VERSION_ARGS:-$ENV_EXAMPLE_VERSION}"
if [[ "$OUT_BASE" == /* ]]; then
    OUT_ABS="${OUT_BASE}-${VERSION}"
else
    OUT_ABS="$ROOT/$OUT_BASE-$VERSION"
fi

# --- release leg: version consistency -----------------------------------------
COMPOSE="$ROOT/workstreams/delivery/deploy/deploy/docker-compose.yml"
if [[ ! -f "$ROOT/workstreams/delivery/deploy/deploy/install.sh" || ! -f "$ROOT/workstreams/delivery/deploy/deploy/.env.example" || ! -f "$COMPOSE" ]]; then
    row "RELEASE" "version" "FAIL" \
        "workstreams/delivery/deploy/deploy install surface missing (need install.sh + .env.example + docker-compose.yml)"
elif [[ "$VERSION" != "$ENV_EXAMPLE_VERSION" ]]; then
    row "RELEASE" "version" "FAIL" \
        "requested version $VERSION != deploy .env.example OEP_VERSION=$ENV_EXAMPLE_VERSION"
elif ! grep -q "OEP_VERSION:-$VERSION}" "$COMPOSE"; then
    row "RELEASE" "version" "FAIL" \
        "compose stack image tag does not default to OEP_VERSION=$VERSION"
else
    row "RELEASE" "version" "PASS" \
        "rehearsing release $VERSION: install.sh + compose + .env.example all pin OEP_VERSION=$VERSION"
fi

# --- runbook leg (self-consistency, docker-free) ------------------------------
RUNBOOK="$ROOT/workstreams/release-engineering/install-rehearsal.md"
if [[ ! -f "$RUNBOOK" ]]; then
    row "DOCS" "runbook" "FAIL" "workstreams/release-engineering/install-rehearsal.md missing"
elif ! grep -q "curl -fsS http://127.0.0.1:8787/api/v1/health" "$RUNBOOK" \
  || ! grep -q "http://127.0.0.1:8790/" "$RUNBOOK" \
  || ! grep -q "curl -fsS http://127.0.0.1:8080/healthz" "$RUNBOOK"; then
    row "DOCS" "runbook" "FAIL" "runbook missing one of the three health-check surfaces"
elif ! grep -qiE "U1|keelwb|VM-class|machine B|second host" "$RUNBOOK"; then
    row "DOCS" "runbook" "FAIL" "runbook does not name the two approved host classes (decision #9)"
else
    row "DOCS" "runbook" "PASS" "runbook present, names 3 health surfaces + 2 approved host classes"
fi

DOCKER_OK=false
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    DOCKER_OK=true
fi

# --- live machine legs (docker + the second host required) --------------------
if [[ "$SELFCHECK" == true ]]; then
    # CI-safe degraded mode: the live-machine legs classify SKIP under the
    # closed taxonomy (decision #9: no second host / no docker in CI), exactly
    # how the P8/P9/P10 gates treat their live-LAN legs.
    row "INSTALL-A" "install-A" "SKIP" "no-host (CI-safe degraded mode)"
    row "INSTALL-B" "install-B" "SKIP" "no-host (CI-safe degraded mode)"
else
    # RC head guard: no unmerged conflict markers anywhere on this head.
    if git grep -n -E '^(<<<<<<<|=======|>>>>>>>)(' -- "$ROOT" ':!Cargo.lock' >/dev/null 2>&1 \
       || grep -qE '^(<<<<<<<|=======|>>>>>>>)' "$ROOT/Cargo.lock" 2>/dev/null; then
        echo "[rehearsal] unmerged conflict markers present on this head; aborting" >&2
        row "INSTALL-B" "install-B" "FAIL" "RC head has unmerged conflict markers"
        write_report "$OUT_ABS"
        exit 1
    fi

    # one clean-state one-command install + health assertions on one machine.
    # $1 = machine label (A|B), $2 = "LOCAL" or ssh host, $3 = ssh port
    run_machine_leg() {
        local label="$1" host="$2" port="$3"
        local id="install-$label" leg="INSTALL-$label"
        local pre
        if [[ "$host" == "LOCAL" ]]; then pre=""; else pre="cd ~/keelworks && "; fi
        local clean_cmd="docker ps -aq --filter name=keelworks- | xargs -r docker rm -f; \
docker volume ls -q --filter name=pdm_data | xargs -r docker volume rm; \
docker volume ls -q --filter name=viewer_data | xargs -r docker volume rm; \
docker image rm -f keelworks/oep-stack:$VERSION 2>/dev/null; \
rm -f workstreams/delivery/deploy/deploy/.env; true"
        local prep_rc=0 prep_out
        prep_out="$(on_target "$host" "$port" "$pre$clean_cmd" 2>&1)" || prep_rc=$? || true
        if [[ "$prep_rc" != 0 ]]; then
            row "$leg" "$id" "FAIL" "clean-state prep failed: ${prep_out: -200}"
            return
        fi
        # reproduce the docs' one-command install
        local inst_rc=0 inst
        # note: install.sh builds/loads + waits for all 3 compose healthchecks
        inst="$(on_target "$host" "$port" "$pre cd workstreams/delivery/deploy/deploy && ./install.sh" 2>&1)" || inst_rc=$? || true
        if [[ "$inst_rc" != 0 ]]; then
            row "$leg" "$id" "FAIL" "install.sh exited $inst_rc: ${inst: -200}"
            return
        fi
        # three health surfaces, run on the machine that hosts the stack
        local pdm viewer p2p pdm_body p2p_body
        pdm="$(on_target "$host" "$port" "$pre curl -fsS -o /dev/null -w '%{http_code}' http://127.0.0.1:8787/api/v1/health || true")" || true
        pdm_body="$(on_target "$host" "$port" "$pre curl -fsS http://127.0.0.1:8787/api/v1/health || true")" || true
        viewer="$(on_target "$host" "$port" "$pre curl -fsS -o /dev/null -w '%{http_code}' http://127.0.0.1:8790/ || true")" || true
        p2p="$(on_target "$host" "$port" "$pre curl -fsS -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/healthz || true")" || true
        p2p_body="$(on_target "$host" "$port" "$pre curl -fsS http://127.0.0.1:8080/healthz || true")" || true
        local pdm_ok=false view_ok=false p2p_ok=false
        [[ "$pdm" == "200" ]] && [[ "$pdm_body" == *'"status":"ok"'* || "$pdm_body" == *'"status": "ok"'* ]] && pdm_ok=true
        [[ "$viewer" == "200" ]] && view_ok=true
        [[ "$p2p" == "200" ]] && [[ "$p2p_body" == *'"status":"ok"'* || "$p2p_body" == *'"status": "ok"'* ]] && p2p_ok=true
        # worker binaries shipped in the installed image (checklist steps 6-7)
        local w1 w2 w_ok=false
        w1="$(on_target "$host" "$port" "$pre docker run --rm --entrypoint oep-batch-convert keelworks/oep-stack:$VERSION --help >/dev/null 2>&1; echo $?")" || true
        w2="$(on_target "$host" "$port" "$pre docker run --rm --entrypoint oep-render-worker keelworks/oep-stack:$VERSION --describe >/dev/null 2>&1; echo $?")" || true
        [[ "$w1" == "0" && "$w2" == "0" ]] && w_ok=true
        if [[ "$pdm_ok" == true && "$view_ok" == true && "$p2p_ok" == true && "$w_ok" == true ]]; then
            row "$leg" "$id" "PASS" \
                "clean-state one-command install on $label ($host) -> pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present in image"
        else
            row "$leg" "$id" "FAIL" \
                "post-install: pdm=${pdm} json=$pdm_ok viewer=${viewer} p2p=${p2p} json=$p2p_ok workers=$w_ok"
        fi
    }

    # machine B reachable?
    B_REACHABLE=false
    if on_target "$MACHINE_B_HOST" "$MACHINE_B_PORT" "true" >/dev/null 2>&1; then
        echo "[rehearsal] syncing the release tree to machine B ($MACHINE_B_HOST:$MACHINE_B_PORT)"
        rsync -az --delete \
            -e "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -p $MACHINE_B_PORT" \
            --exclude '.git/' --exclude 'target/' --exclude '.worktrees/' \
            --exclude '*.qcow2' --exclude '*.typescript' --exclude '*.timing' \
            --exclude '*.mp4' --exclude 'workstreams/delivery/update-checker/proof/video/' \
            "$ROOT/" "$MACHINE_B_HOST:~/keelworks/"
        B_REACHABLE=true
    else
        echo "[rehearsal] machine B unreachable ($MACHINE_B_HOST:$MACHINE_B_PORT)" >&2
    fi

    if $DOCKER_OK; then
        echo "[rehearsal] machine A = U1 (local): clean-state install for release $VERSION"
        run_machine_leg "A" "LOCAL" ""
    else
        row "INSTALL-A" "install-A" "FAIL" "no docker daemon on the host"
    fi

    if $DOCKER_OK && $B_REACHABLE; then
        echo "[rehearsal] machine B = $MACHINE_B_HOST (U1/VM-class): clean-state install for release $VERSION"
        run_machine_leg "B" "$MACHINE_B_HOST" "$MACHINE_B_PORT"
    elif $DOCKER_OK && ! $B_REACHABLE; then
        row "INSTALL-B" "install-B" "FAIL" "machine B ($MACHINE_B_HOST:$MACHINE_B_PORT) unreachable over SSH"
    fi
fi

# --- write + report ----------------------------------------------------------
write_report "$OUT_ABS"
echo "[rehearsal] wrote $OUT_ABS.json + $OUT_ABS.md"
echo "[rehearsal] release $VERSION rehearsal (machine A = U1, machine B = $MACHINE_B_HOST): $(summary)"
rc=0; exit_code || rc=$?
exit "$rc"
