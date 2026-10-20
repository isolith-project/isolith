#!/usr/bin/env bash
# ISOLITH OEP - Release / offline-image bundler (P10-M1-T01).
#
# Builds the stack image and, with --tarball, saves it into an offline
# bundle for air-gapped installs. Same offline-image pattern discipline as
# Bastion S2:
#
#   build --> docker save | gzip --> isolith-oep-stack-<ver>.tar.gz
#           + <tar.gz>.sha256 checksum
#
# The downstream air-gap card (P10-M2-T02) signs and ships this bundle; the
# one-command install loads it with `install.sh --offline-bundle <tar.gz>`.
#
# Usage (run from workstreams/delivery/deploy/deploy/):
#   ./release.sh 0.1.0            # build + tag image only
#   ./release.sh 0.1.0 --tarball  # build + write offline image tarball
#   ./release.sh 0.1.0 --tarball --out ../releases
#
# Prereqs: Docker Engine 24+, Docker Compose v2, sha256sum.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
log()  { echo -e "${GREEN}[RELEASE]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
info() { echo -e "${BLUE}[INFO]${NC} $*"; }

VERSION="${1:-}"
TARBALL=false
OUT_DIR="."

if [[ -z "$VERSION" ]]; then
    err "Usage: $0 <version> [--tarball] [--out DIR]"
    exit 1
fi
shift
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tarball) TARBALL=true ;;
        --out) OUT_DIR="${2:?--out needs a dir}"; shift ;;
        *) err "unknown option: $1"; exit 1 ;;
    esac
    shift
done

IMAGE="isolith/oep-stack:${VERSION}"
BUNDLE="${OUT_DIR}/isolith-oep-stack-${VERSION}.tar.gz"

# --- Preflight ---
command -v docker >/dev/null || { err "docker required"; exit 1; }
docker info >/dev/null 2>&1 || { err "docker daemon not running"; exit 1; }
[[ -f docker-compose.yml ]] || { err "docker-compose.yml missing"; exit 1; }
mkdir -p "$OUT_DIR"

# --- Build ---
log "Building ${IMAGE} (this compiles the Rust workspace in the builder image)..."
docker compose -f docker-compose.yml build

# --- Tag latest ---
docker tag "${IMAGE}" "isolith/oep-stack:latest"
log "Tagged isolith/oep-stack:latest."

# --- Offline tarball ---
if [[ "$TARBALL" == "true" ]]; then
    log "Saving offline image tarball -> ${BUNDLE}"
    docker save "${IMAGE}" | gzip > "${BUNDLE}"
    ( cd "$OUT_DIR" && sha256sum "$(basename "${BUNDLE}")" > "$(basename "${BUNDLE}.sha256")" )
    checksum=$(awk '{print $1}' "${BUNDLE}.sha256")
    info "Bundle: ${BUNDLE}"
    info "SHA256: ${checksum}"
    log "Done. On the target host (air-gap): install.sh --offline-bundle ${BUNDLE}"
else
    log "Done (no tarball; use --tarball for the offline bundle)."
fi
