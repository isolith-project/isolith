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
# GHCR publish (DOCKER-NAS-DEPLOYMENT, card t_82f606dd): with --push-ghcr the
# script ALSO tags and pushes the image to
#   ghcr.io/<GHCR_ORG>/isolith/oep-stack:<ver>        (stable version tag)
#   ghcr.io/<GHCR_ORG>/isolith/oep-stack:latest       (moving tag)
# and writes the pushed image digest (repo digest, sha256:...) to
# --digest-out FILE so the release body can record it. GHCR, not Docker Hub.
# The NAS compose (docker-compose.nas.yml) pulls exactly this reference.
#
# GATE (2026-10-27, Don): no PUBLIC GHCR publish until Don opens the surface.
# This flag is the one explicit switch used at that moment; until then use the
# mechanism against a private/pre-release registry (this was verified in
# t_82f606dd against a local OCI registry) and ship offline bundles.
#
# Usage (run from workstreams/delivery/deploy/deploy/):
#   ./release.sh 0.1.0            # build + tag image only
#   ./release.sh 0.1.0 --tarball  # build + write offline image tarball
#   ./release.sh 0.1.0 --tarball --out ../releases
#   ./release.sh 0.1.0 --push-ghcr --ghcr-org <org> --digest-out ../proof/digest.txt
#
# Prereqs: Docker Engine 24+, Docker Compose v2, sha256sum. For --push-ghcr:
# `docker login ghcr.io` with a token that has write:packages.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
log()  { echo -e "${GREEN}[RELEASE]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
info() { echo -e "${BLUE}[INFO]${NC} $*"; }

VERSION="${1:-}"
TARBALL=false
OUT_DIR="."
PUSH_GHCR=false
GHCR_ORG=""
DIGEST_OUT=""

if [[ -z "$VERSION" ]]; then
    err "Usage: $0 <version> [--tarball] [--out DIR] [--push-ghcr --ghcr-org ORG --digest-out FILE]"
    exit 1
fi
shift
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tarball) TARBALL=true ;;
        --out) OUT_DIR="${2:?--out needs a dir}"; shift ;;
        --push-ghcr) PUSH_GHCR=true ;;
        --ghcr-org) GHCR_ORG="${2:?--ghcr-org needs an org}"; shift ;;
        --digest-out) DIGEST_OUT="${2:?--digest-out needs a file}"; shift ;;
        *) err "unknown option: $1"; exit 1 ;;
    esac
    shift
done

IMAGE="isolith/oep-stack:${VERSION}"
BUNDLE="${OUT_DIR}/isolith-oep-stack-${VERSION}.tar.gz"
GHCR_IMAGE="ghcr.io/${GHCR_ORG}/isolith/oep-stack:${VERSION}"

# --- Preflight ---
command -v docker >/dev/null || { err "docker required"; exit 1; }
docker info >/dev/null 2>&1 || { err "docker daemon not running"; exit 1; }
[[ -f docker-compose.yml ]] || { err "docker-compose.yml missing"; exit 1; }
mkdir -p "$OUT_DIR"
if [[ "$PUSH_GHCR" == "true" ]]; then
    [[ -n "$GHCR_ORG" ]] || { err "--push-ghcr requires --ghcr-org ORG"; exit 1; }
    [[ -n "$DIGEST_OUT" ]] || { err "--push-ghcr requires --digest-out FILE (release body evidence)"; exit 1; }
    docker info 2>/dev/null | grep -qi "Username:" || { err "--push-ghcr requires docker login (write:packages on the token)"; exit 1; }
fi

# --- Build ---
log "Building ${IMAGE} (this compiles the Rust workspace in the builder image)..."
docker compose -f docker-compose.yml build

# --- Tag latest ---
docker tag "${IMAGE}" "isolith/oep-stack:latest"
log "Tagged isolith/oep-stack:latest."

# --- GHCR publish (gated: only with --push-ghcr) ---
if [[ "$PUSH_GHCR" == "true" ]]; then
    log "Tagging + pushing ${GHCR_IMAGE} ..."
    docker tag "${IMAGE}" "${GHCR_IMAGE}"
    docker push "${GHCR_IMAGE}"
    docker tag "${IMAGE}" "ghcr.io/${GHCR_ORG}/isolith/oep-stack:latest"
    docker push "ghcr.io/${GHCR_ORG}/isolith/oep-stack:latest"
    # The repo digest AFTER push is the immutable, content-addressed handle.
    DIGEST="$(docker image inspect "${GHCR_IMAGE}" --format '{{index .RepoDigests 0}}')"
    echo "$DIGEST" > "$DIGEST_OUT"
    info "GHCR digest -> ${DIGEST} (record in the release body)"
fi

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
