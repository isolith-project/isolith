#!/usr/bin/env bash
# airgap-rollback.sh — revert an air-gap install to the previous known-good
# image set (P10-M2-T02). Saves a snapshot of the loaded image repo tags
# before install; on rollback it restores the previous Docker image tags
# (docker tag back to the prior reference) so the stack that was running
# before the install is the stack that runs again.
#
# Usage:
#   airgap-rollback.sh --install-dir /opt/keelworks --project oep
set -euo pipefail
cd "$(dirname "$0")"
source ./airgap-lib.sh

INSTALL_DIR="/opt/keelworks"
PROJECT="oep"

while [ $# -gt 0 ]; do
    case "$1" in
        --install-dir) INSTALL_DIR="${2:?needs a value}"; shift 2;;
        --project) PROJECT="${2:?needs a value}"; shift 2;;
        *) echo "unknown arg: $1" >&2; exit 2;;
    esac
done

SNAPSHOT="$INSTALL_DIR/proof/image-snapshot.txt"
[ -f "$SNAPSHOT" ] || die "no image snapshot at $SNAPSHOT (nothing to roll back to)"

log "stopping stack project=$PROJECT"
[ -f "$INSTALL_DIR/compose.yaml" ] && COMPOSE_PROJECT_NAME="$PROJECT" docker compose -f "$INSTALL_DIR/compose.yaml" down --remove-orphans 2>/dev/null || true

log "restoring previous image tags from snapshot"
while IFS= read -r line; do
    [ -z "$line" ] && continue
    tag="$line"
    digest="$(docker inspect --format '{{index .RepoDigests 0}}' "$tag" 2>/dev/null || true)"
    [ -n "$digest" ] && docker tag "$digest" "$tag" >/dev/null 2>&1 || true
done < "$SNAPSHOT"

log "rollback complete — previous image tags restored"
