# airgap-lib.sh — shared helpers for the Keelworks offline/air-gap install path
# (P10-M2-T02). Sourced by build-offline-bundle.sh, airgap-install.sh,
# airgap-verify.sh and airgap-rollback.sh.
#
# Sovereign-by-default doctrine (decision #10 + #11): the bundle, the install
# and the runtime never phone home, never reach back, and never require a
# network. The update checker is OFF by default in any offline deployment
# (OEP_UPDATE_CHECK=off below); it can be enabled by an admin against a local
# mirror or an offline signed bundle only, and it never blocks the program.
#
# Keep this file POSIX sh — it runs on the build host, the transfer
# workstation *and* the air-gapped target.

set -u

# --- Defaults ---------------------------------------------------------------
AIRGAP_DIR="${AIRGAP_DIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
SIGN_KEY_ID="${SIGN_KEY_ID:-oep-airgap-signing}"
KEYRING_DIR="${KEYRING_DIR:-$AIRGAP_DIR/keys}"
BUNDLE_NAME="${BUNDLE_NAME:-keelworks-offline}"
COMPOSE_BIN="${COMPOSE_BIN:-docker compose}"

# The single public key ships with the repo so ANY operator can verify a
# bundle; the signing secret key lives only on the build host (the release
# lane), never in the repo.
PUBLIC_KEY_FILE="$KEYRING_DIR/oep-airgap-signing.pub.asc"

log() { printf '[airgap] %s\n' "$*"; }
die() { printf '[airgap] FATAL: %s\n' "$*" >&2; exit 1; }

# --- Update-checker posture (decision #11) ----------------------------------
# Off by default for offline deployments. If an admin explicitly enables it,
# the only permitted sources are an on-site mirror URL or a path to an offline
# signed bundle on the machine itself — and even then the checker is advisory
# and never blocks launch. TELEMETRY stays out unconditionally (P0-M5-T02).
update_check_env() {
    cat <<'EOF'
# OEP update-checker posture for offline/air-gapped deployments.
# Decision #11: advisory, non-blocking, disable-able, OFF by default, and in
# an air-gap deployment the source can only be a local path on this machine or
# an on-site mirror. TELEMETRY is always off (P0-M5-T02) — this knob controls
# update *checking* only, never analytics reach-back.
OEP_UPDATE_CHECK=off
# When ON, one of (must be reachable without the public internet):
#   OEP_UPDATE_MIRROR_URL=https://updates.example.local/keelworks   (on-site)
#   OEP_UPDATE_OFFLINE_BUNDLE=/path/to/keelworks-offline-<ver>.tar.gz (local)
OEP_UPDATE_MIRROR_URL=
OEP_UPDATE_OFFLINE_BUNDLE=
EOF
}

# --- Trust setup ------------------------------------------------------------
# Import the committed public key into the caller's keyring (idempotent).
import_public_key() {
    command -v gpg >/dev/null 2>&1 || die "gpg required to verify the bundle signature"
    if ! gpg --list-keys "$SIGN_KEY_ID" >/dev/null 2>&1; then
        gpg --batch --import "$PUBLIC_KEY_FILE" >/dev/null 2>&1 \
            || die "failed to import public key $PUBLIC_KEY_FILE"
    fi
}

# --- Signing / verification -------------------------------------------------
# Sign a file with the release (secret) key. Build-host only.
gpg_detach_sign() {
    local file="$1"
    gpg --batch --yes --armor --detach-sign \
        --local-user "$SIGN_KEY_ID" --output "$file.sig" "$file" \
        || die "gpg signing failed for $file"
}

# Verify a file's detached signature. Returns non-zero on failure.
gpg_detach_verify() {
    local file="$1"
    gpg --batch --verify "$file.sig" "$file" 2>/dev/null
}

# --- Checksums --------------------------------------------------------------
sha256_of() {
    sha256sum "$1" | awk '{print $1}'
}
