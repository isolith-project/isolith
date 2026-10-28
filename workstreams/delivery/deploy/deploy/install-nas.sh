#!/usr/bin/env bash
# ISOLITH OEP - NAS / homelab one-command install (pull-based, DOCKER-NAS-DEPLOYMENT).
#
# The customer-NAS counterpart to install.sh: install.sh BUILDS the stack image
# from source (U1/dev path); this script PULLS the published GHCR image onto a
# NAS or Linux server and starts the stack with all health checks green. No
# build, no Rust toolchain on the target. Works with Synology Container Manager
# expectations too (same compose file can be imported as a Project).
#
#   cp .env.nas.example .env        # then edit for your host
#   ./install-nas.sh                # pull + up + wait healthy
#   ./install-nas.sh --down         # stop the stack
#   ./install-nas.sh --dry-run      # print the image refs compose would use
#
# Exit 0 iff all four services are healthy.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[1;33m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
log()  { echo -e "${GREEN}[NAS]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
info() { echo -e "${BLUE}[INFO]${NC} $*"; }

COMPOSE="docker-compose.nas.yml"
ACTION="up"
DRY_RUN=false

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --down) ACTION="down"; shift ;;
            --dry-run) DRY_RUN=true; shift ;;
            *) err "unknown argument: $1"; exit 2 ;;
        esac
    done
}
parse_args "$@"

compose() { docker compose -f "$COMPOSE" --env-file .env "$@"; }

preflight() {
    log "Preflight... (docker + compose + .env)"
    command -v docker >/dev/null || { err "Docker Engine required: https://docs.docker.com/engine/install/"; exit 1; }
    docker info >/dev/null 2>&1 || { err "Docker daemon not running. Start it first."; exit 1; }
    docker compose version >/dev/null 2>&1 || { err "Docker Compose v2 required."; exit 1; }
    [[ -f .env ]] || { cp .env.nas.example .env; log "Wrote .env from .env.nas.example (host binds default to 127.0.0.1)."; }
    set -a; source .env; set +a
    local avail
    avail=$(df -BG . | awk 'NR==2{print $4}' | tr -d 'G' || echo 0)
    if [[ "${avail:-0}" -lt 1 ]]; then warn "Less than 1GB free in $(pwd); add vault/disk capacity."; fi
    log "Preflight ok."
}

main() {
    preflight
    if [[ "$DRY_RUN" == "true" ]]; then
        info "Image refs this compose would pull:"
        docker compose -f "$COMPOSE" --env-file .env config --images
        exit 0
    fi
    if [[ "$ACTION" == "down" ]]; then
        compose down; log "Stack stopped."; return 0
    fi
    log "Pulling stack image (${OEP_IMAGE:-ghcr.io/isolith/oep-stack:0.1.0}) from GHCR..."
    docker compose -f "$COMPOSE" --env-file .env pull
    compose up -d
    log "Waiting for health checks to go green (up to 180s)..."
    local deadline=$(( $(date +%s) + 180 ))
    local got=0
    while (( $(date +%s) < deadline )); do
        got=$(docker ps --filter name=isolith- --format '{{.Names}} {{.Status}}' | grep -c 'healthy' || true)
        [[ "$got" -ge 4 ]] && break
        sleep 3
    done
    if [[ "$got" -lt 4 ]]; then
        err "Timed out: only $got/4 services healthy."
        docker ps --filter name=isolith-
        exit 1
    fi
    log "All 4 services healthy."
    compose ps
}
main
