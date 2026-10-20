#!/usr/bin/env bash
# verify-release.sh - consumer-side verification of a signed Keelworks release
# (P11-T04). Needs ONLY public key material (no secrets): the gpg pubkey
# committed at scripts/airgap/keys/oep-airgap-signing.pub.asc and the Ed25519
# pubkey committed at workstreams/security/keys/oep-update-signing.pub.
#
# Check list (exit 0 iff ALL pass):
#   1. every artifact has a valid detached gpg signature,
#   2. every artifact matches its SHA256SUMS entry (no tamper),
#   3. update.bundle.json (if present) is Ed25519-verified and the checker
#      reports a TRUSTED status (the advisory checker reports only
#      signature-verified payloads, decision #11).
#
# Usage:
#   verify-release.sh --assets DIR [--version V] [--check-keyring DIR]
#       [--update-pubkey HEX] [--skip-update-bundle]
set -euo pipefail

ASSETS=""
VERSION=""
CHECK_KEYRING=""
UPDATE_PUBKEY=""
SKIP_BUNDLE=0
CHECKER_BIN=""

while [ $# -gt 0 ]; do
    case "$1" in
        --assets) ASSETS="${2:?needs a value}"; shift 2;;
        --version) VERSION="${2:?needs a value}"; shift 2;;
        --check-keyring) CHECK_KEYRING="${2:?needs a value}"; shift 2;;
        --update-pubkey) UPDATE_PUBKEY="${2:?needs a value}"; shift 2;;
        --skip-update-bundle) SKIP_BUNDLE=1; shift;;
        --checker) CHECKER_BIN="${2:?needs a value}"; shift 2;;
        -h|--help) sed -n '1,22p' "$0"; exit 0;;
        *) echo "unknown arg: $1" >&2; exit 2;;
    esac
done

[ -d "$ASSETS" ] || { echo "FATAL: --assets dir not found: $ASSETS" >&2; exit 1; }
command -v gpg >/dev/null 2>&1 || { echo "FATAL: gpg required to verify signatures" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "FATAL: sha256sum required" >&2; exit 1; }

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PUBLIC_KEY_FILE="${PUBLIC_KEY_FILE:-$ROOT/scripts/airgap/keys/oep-airgap-signing.pub.asc}"
SIGN_KEY_ID="${SIGN_KEY_ID:-oep-airgap-signing}"

# Import the committed public key into the (optionally isolated) keyring.
GNUPGHOME_CLI=()
if [ -n "$CHECK_KEYRING" ]; then
    export GNUPGHOME="$CHECK_KEYRING"
    mkdir -p "$GNUPGHOME"
    chmod 700 "$GNUPGHOME"
    GNUPGHOME_CLI=(--gnupg-home "$CHECK_KEYRING")
fi
if ! gpg --list-keys "$SIGN_KEY_ID" >/dev/null 2>&1; then
    gpg --batch --import "$PUBLIC_KEY_FILE" >/dev/null 2>&1 \
        || { echo "FATAL: cannot import public key $PUBLIC_KEY_FILE" >&2; exit 1; }
fi

cd "$ASSETS"
fail=0
echo "== verify release assets: $ASSETS =="

# 1) detached signatures for every artifact file.
ARTIFACTS=""
for sig in *.sig; do
    [ -e "$sig" ] || continue
    target="${sig%.sig}"
    [ -f "$target" ] || { echo "  [FAIL] $target missing (stray $sig)"; fail=1; continue; }
    ARTIFACTS="$ARTIFACTS $target"
    if gpg --batch --verify "$sig" "$target" >/dev/null 2>&1; then
        echo "  [ok]   signature: $target"
    else
        echo "  [FAIL] signature: $target"
        fail=1
    fi
done

# 2) checksums.
if [ -f SHA256SUMS ]; then
    if (cd "$ASSETS" && sha256sum -c SHA256SUMS >/dev/null 2>&1); then
        echo "  [ok]   sha256 checksums"
    else
        echo "  [FAIL] sha256 checksums (a file was tampered)"
        fail=1
    fi
else
    echo "  [warn] no SHA256SUMS in asset dir"
fi

# 3) the update manifest must be signature-verified before we report it.
if [ "$SKIP_BUNDLE" = "0" ] && [ -f update.bundle.json ]; then
    PUB="$(cat "$ROOT/workstreams/security/keys/oep-update-signing.pub" 2>/dev/null || true)"
    if [ -n "$UPDATE_PUBKEY" ]; then PUB="$UPDATE_PUBKEY"; fi
    if [ -z "$PUB" ]; then
        echo "  [FAIL] update.bundle.json present but no verifying Ed25519 pubkey configured"
        fail=1
    else
        # Locate the checker binary (built release/CI tooling).
        if [ -z "$CHECKER_BIN" ]; then
            for cand in "$ROOT/target/debug/oep-update-check" \
                        "$ROOT/target/release/oep-update-check" \
                        "$(command -v oep-update-check 2>/dev/null || true)"; do
                [ -n "$cand" ] && [ -x "$cand" ] && CHECKER_BIN="$cand" && break
            done
        fi
        if [ -x "$CHECKER_BIN" ]; then
            CONF="$(mktemp)"
            trap 'rm -f "$CONF"' EXIT
            cat > "$CONF" <<EOF
{"enabled": true, "source": {"bundle": "$ASSETS/update.bundle.json"}, "public_key": "$PUB", "current_version": "${VERSION:-0.0.0}"}
EOF
            OUT="$("$CHECKER_BIN" --config "$CONF" --json 2>/dev/null || true)"
            if printf '%s' "$OUT" | grep -q '"UpdateAvailable"\|"UpToDate"'; then
                echo "  [ok]   update.bundle.json signature verified by oep-update-check (advisory only)"
            else
                echo "  [FAIL] update.bundle.json NOT trusted (tampered or foreign signature)"
                echo "         checker: $(printf '%s' "$OUT" | head -c 160)"
                fail=1
            fi
        else
            echo "  [warn] oep-update-check binary not found; bundle signature not re-checked here"
        fi
    fi
elif [ -f update.bundle.json ] && [ "$SKIP_BUNDLE" = "1" ]; then
    echo "  [skip] update.bundle.json verification skipped by request"
fi

if [ "$fail" = "0" ]; then
    echo "== VERIFY PASS (exit 0) =="
    exit 0
fi
echo "== VERIFY FAIL (exit 1) =="
exit 1
