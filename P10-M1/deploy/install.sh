#!/usr/bin/env bash
# KEELWORKS OEP - One-command install (P10-M1-T01).
#
# From a clean state (no images, no containers, no .env) this script:
#   1. preflights Docker / Compose / disk / OS
#   2. loads the offline image tarball (--offline-bundle) when supplied,
#      else builds the stack image from the repo (docker compose build)
#   3. writes .env from .env.example when missing
#   4. starts the stack (pdm + viewer + p2p)
#   5. waits until every health check is green
#   6. prints the operator summary
#
# Exit code 0 iff all three services are healthy at the end. Same offline-image
# pattern discipline as Bastion S2: `release.sh --tarball` produces the bundle;
# `install.sh --offline-bundle <tar.gz>` runs with no network on the target
# (the P10-M2-T02 downstream card signs and ships that bundle).
#
# Usage:
#   ./install.sh                     # build from source (needs network at build)
#   ./install.sh --offline-bundle keelworks-oep-stack-0.1.0.tar.gz   # air-gap
#   ./install.sh --no-build          # reuse an already-built/loaded image
#   ./install.sh --down              # stop the stack

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[1;33m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
log()  { echo -e "${GREEN}[OEP]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
info() { echo -e "${BLUE}[INFO]${NC} $*"; }

OFFLINE_BUNDLE=""
NO_BUILD=false
ACTION="up"

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --offline-bundle) OFFLINE_BUNDLE="${2:?--offline-bundle needs a path}"; shift 2 ;;
            --no-build) NO_BUILD=true; shift ;;
            --down) ACTION="down"; shift ;;
            *) err "unknown argument: $1"; exit 2 ;;
        esac
    done
}
parse_args "$@"

compose() { docker compose -f docker-compose.yml --env-file .env "$@"; }

image_tag() {
    # shellcheck disable=SC2016
    grep -E '^OEP_VERSION=' .env | cut -d= -f2
}

preflight() {
    log "Preflight... (docker + compose + disk)"
    if ! command -v docker >/dev/null; then err "Docker Engine required: https://docs.docker.com/engine/install/"; exit 1; fi
    if ! docker info >/dev/null 2>&1; then err "Docker daemon not running. Start it first."; exit 1; fi
    if ! docker compose version >/dev/null 2>&1; then err "Docker Compose v2 required."; exit 1; fi
    local avail
    avail=$(df -BG . | awk 'NR==2{print $4}' | tr -d 'G' || echo 0)
    if [[ "${avail:-0}" -lt 5 ]]; then warn "Less than 5GB free in $(pwd); stack may fail."; fi
    if [[ ! -f .env ]]; then
        cp .env.example .env
        log "Wrote .env from .env.example (host binds default to 127.0.0.1)."
    fi
    set -a; source .env; set +a
    log "Preflight ok."
}

load_or_build() {
    local TAG
    TAG="$(image_tag)"
    if [[ "$ACTION" != "up" ]]; then return; fi
    if [[ -n "$OFFLINE_BUNDLE" ]]; then
        if [[ ! -f "$OFFLINE_BUNDLE" ]]; then err "offline bundle not found: $OFFLINE_BUNDLE"; exit 1; fi
        if [[ -f "${OFFLINE_BUNDLE}.sha256" ]]; then
            log "Verifying ${OFFLINE_BUNDLE}.sha256..."
            ( cd "$(dirname "$OFFLINE_BUNDLE")" && sha256sum -c "$(basename "${OFFLINE_BUNDLE}.sha256")" ) || {
                err "offline bundle checksum mismatch; refusing to load"; exit 1; }
        fi
        log "Loading offline image tarball ${OFFLINE_BUNDLE} (no build, no network)..."
        docker load -i "$OFFLINE_BUNDLE"
        NO_BUILD=true
    fi
    if [[ "$NO_BUILD" == "true" ]]; then
        if ! docker image inspect "keelworks/oep-stack:${TAG}" >/dev/null 2>&1; then
            err "image keelworks/oep-stack:${TAG} not present and --no-build/offline bundle given."
            exit 1
        fi
        log "Reusing prebuilt image keelworks/oep-stack:${TAG}."
    else
        log "Building stack image keelworks/oep-stack:${TAG} (first build fetches crates, minutes)..."
        docker compose -f docker-compose.yml build
    fi
}

wait_healthy() {
    log "Waiting for health checks to go green (up to 180s)..."
    local deadline=$(( $(date +%s) + 180 ))
    local want=3 got=0
    while (( $(date +%s) < deadline )); do
        got=$(docker ps --filter name=keelworks- --format '{{.Names}} {{.Status}}' \
            | grep -c 'healthy' || true)
        [[ "$got" -ge "$want" ]] && break
        sleep 3
    done
    if [[ "$got" -lt "$want" ]]; then
        err "Timed out: only $got/$want services healthy."
        docker ps --filter name=keelworks-
        exit 1
    fi
    log "All $want services healthy."
}

show_summary() {
    local pdm="${OEP_PDM_BIND:-127.0.0.1}:${OEP_PDM_PORT:-8787}"
    local viewer="${OEP_VIEWER_BIND:-127.0.0.1}:${OEP_VIEWER_PORT:-8790}"
    local p2p="${OEP_P2P_BIND:-127.0.0.1}:${OEP_P2P_PORT:-8080}"
    echo
    echo "  PDM     (REST API v1, health /api/v1/health)  http://${pdm}/api/v1/health"
    echo "  Viewer  (P6 preview + browser UI)              http://${viewer}/"
    echo "  P2P     (compute lane dashboard)               http://${p2p}/      /healthz"
    echo
}

verify_health() {
    local pdm_port="${OEP_PDM_PORT:-8787}"
    local viewer_port="${OEP_VIEWER_PORT:-8790}"
    local p2p_port="${OEP_P2P_PORT:-8080}"
    log "Verifying each health endpoint directly..."
    curl -fsS "http://127.0.0.1:${pdm_port}/api/v1/health"    >/dev/null && log "pdm    /api/v1/health        OK"
    curl -fsS -o /dev/null "http://127.0.0.1:${viewer_port}/" >/dev/null && log "viewer /                     OK"
    curl -fsS "http://127.0.0.1:${p2p_port}/healthz"          >/dev/null && log "p2p    /healthz             OK"
}

main() {
    preflight
    if [[ "$ACTION" == "down" ]]; then
        compose down; log "Stack stopped."; return 0
    fi
    load_or_build
    log "Starting stack (pdm + viewer + p2p)..."
    compose up -d
    wait_healthy
    verify_health
    show_summary
    log "One-command install complete: the full Keelworks stack is up with health checks green."
}
main
