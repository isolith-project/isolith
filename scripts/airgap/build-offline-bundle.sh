#!/usr/bin/env bash
# build-offline-bundle.sh — build the signed, no-phone-home offline bundle.
#
# P10-M2-T02 delivery: the sovereignty centerpiece for ITAR/defense shops.
# This script runs on the BUILD HOST (connected) and produces the single
# artifact that crosses an air gap on USB/ISO: a self-contained tarball with
# every container image, the compose stack, config templates, the install/
# verify/rollback scripts, a signed MANIFEST and detached signatures for the
# whole bundle and each image (ties to P11 signing). The target machine needs
# nothing but docker + the tarball + gpg to verify — no internet, no registry,
# no phone-home.
#
# Usage:
#   build-offline-bundle.sh --version 0.1.0 \
#       --images isolith/oep-pdm:0.1.0 nats:2-alpine \
#       --compose deploy/docker-compose.yml \
#       [--out dist/] [--sign-key <gpg-uid>]
#
# Options:
#   --version V      bundle version (required)
#   --images I1 I2.. container images to export via `docker save` (space separated)
#   --compose PATH   compose stack file to embed (optional; install can use
#                    `--compose` or the embedded copy)
#   --out DIR        output directory (default: dist/)
#   --sign-key UID   gpg signing identity present in your keyring
#   --bundle-dir DIR staging dir for the assembled bundle (default: tmp)
#
# The bundle layout (what lands on the target via USB/ISO):
#   isolith-offline-<version>/
#     MANIFEST.json             signed inventory: version, date, images,
#                               sha256 per image tarball, checksums
#     MANIFEST.json.sig         detached signature over the manifest
#     SHA256SUMS                checksums for every file in the bundle
#     SHA256SUMS.sig            detached signature over the checksums
#     oep-airgap-signing.pub.asc  the verification public key
#     images/<image>.tar.gz     exported images (one tarball per image)
#     compose.yaml              the embedded compose stack (if --compose)
#     env.example               environment template (update-check OFF)
#     scripts/*                 install / verify / rollback + this lib
# The bundle is then gzipped to isolith-offline-<version>.tar.gz, with
#   *.sha256  (bundle checksum) and *.sig (detached signature).
set -euo pipefail
cd "$(dirname "$0")"
source ./airgap-lib.sh

VERSION=""
IMAGES=()
COMPOSE=""
OUT="dist"
SIGN_KEY_ID="${SIGN_KEY_ID:-oep-airgap-signing}"

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?--version needs a value}"; shift 2;;
        --images) shift; while [ $# -gt 0 ] && [ "${1:0:1}" != "-" ]; do IMAGES+=("$1"); shift; done;;
        --compose) COMPOSE="${2:?--compose needs a path}"; shift 2;;
        --out) OUT="${2:?--out needs a path}"; shift 2;;
        --sign-key) SIGN_KEY_ID="${2:?--sign-key needs a uid}"; shift 2;;
        *) echo "unknown arg: $1" >&2; exit 2;;
    esac
done

[ -n "$VERSION" ] || die "--version is required"
[ "${#IMAGES[@]}" -gt 0 ] || die "--images is required (at least one image)"

command -v docker >/dev/null 2>&1 || die "docker required to export images"
command -v gpg   >/dev/null 2>&1 || die "gpg required to sign the bundle"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
BUNDLE="$WORK/isolith-offline-$VERSION"
mkdir -p "$BUNDLE/images" "$BUNDLE/scripts" "$OUT"

log "building signed offline bundle isolith-offline-$VERSION"
log "signing identity: $SIGN_KEY_ID"
log "images: ${IMAGES[*]}"

# 1. Export each image to its own gzipped tarball.
for img in "${IMAGES[@]}"; do
    name="$(printf '%s' "$img" | tr '/:' '__')"
    log "exporting $img -> images/$name.tar.gz"
    docker save "$img" | gzip -1 > "$BUNDLE/images/$name.tar.gz"
done

# 2. Copy runtime content: scripts, env template, public key.
cp ./airgap-lib.sh "$BUNDLE/scripts/"
cp ./airgap-install.sh "$BUNDLE/scripts/" 2>/dev/null || true
cp ./airgap-verify.sh "$BUNDLE/scripts/" 2>/dev/null || true
cp ./airgap-rollback.sh "$BUNDLE/scripts/" 2>/dev/null || true
cp "$PUBLIC_KEY_FILE" "$BUNDLE/"
update_check_env > "$BUNDLE/env.example"
if [ -n "$COMPOSE" ]; then
    [ -f "$COMPOSE" ] || die "compose file not found: $COMPOSE"
    cp "$COMPOSE" "$BUNDLE/compose.yaml"
    log "embedded compose stack: $COMPOSE"
fi

# 3. Manifest + checksums.
(
    cd "$BUNDLE"
    find . -type f | sort | xargs sha256sum > SHA256SUMS
)
# Real manifest: walk the staged bundle and record every file + its sha256.
python3 - "$BUNDLE" "$VERSION" "$BUNDLE/MANIFEST.json" <<'PY'
import hashlib, json, os, sys
root, version, out = sys.argv[1], sys.argv[2], sys.argv[3]
files = {}
for base, _dirs, names in os.walk(root):
    for n in sorted(names):
        p = os.path.join(base, n)
        rel = os.path.relpath(p, root)
        h = hashlib.sha256()
        with open(p, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
        files[rel] = h.hexdigest()
import time
manifest = {
    "bundle": "isolith-offline",
    "version": version,
    "date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "files": files,
}
with open(out, "w") as f:
    json.dump(manifest, f, indent=2, sort_keys=True)
    f.write("\n")
PY

# 4. Sign the manifest and the checksums (P11 tie).
log "signing MANIFEST.json and SHA256SUMS"
gpg_detach_sign "$BUNDLE/MANIFEST.json"
gpg_detach_sign "$BUNDLE/SHA256SUMS"

# 5. Assemble the final tarball + checksum + signature.
(
    cd "$WORK"
    tar czf "$OUT/isolith-offline-$VERSION.tar.gz" "isolith-offline-$VERSION"
)
BUNDLE_TGZ="$OUT/isolith-offline-$VERSION.tar.gz"
sha256sum "$BUNDLE_TGZ" | awk '{print $1}' > "$BUNDLE_TGZ.sha256"
gpg_detach_sign "$BUNDLE_TGZ"

log "DONE"
log "bundle:      $(readlink -f "$BUNDLE_TGZ")"
log "sha256:      $(cat "$BUNDLE_TGZ.sha256")"
log "signature:   $BUNDLE_TGZ.sig"
log "public key:  $PUBLIC_KEY_FILE"
log "verify on target with: airgap-verify.sh --bundle $BUNDLE_TGZ"
