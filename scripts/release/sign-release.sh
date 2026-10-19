#!/usr/bin/env bash
# sign-release.sh - build-host signer for Keelworks release artifacts (P11-T04).
#
# Runs ONLY on the build host, where the two signing secrets live:
#   * gpg key     oep-airgap-signing   (secret in the build host keyring;
#                                       pubkey committed at
#                                       scripts/airgap/keys/oep-airgap-signing.pub.asc)
#   * Ed25519 key oep-update-signing   (secret hex held under
#                                       $OEP_UPDATE_SIGN_KEYFILE or $OEP_UPDATE_SIGN_KEY;
#                                       pubkey committed at P11/keys/oep-update-signing.pub)
#
# It produces, per release version, in the output dir:
#   <artifact>          (unchanged release binary/asset)
#   <artifact>.sig      gpg detached (armored) signature
#   SHA256SUMS          content checksums for every asset
#   SHA256SUMS.sig      gpg detached signature over the checksums
#   RELEASE-MANIFEST.json  inventory (release, version, date, artifacts)
#   update.bundle.json  Ed25519-signed update manifest the P10 update checker
#                       consumes (decision #11); inline payload = the
#                       RELEASE-MANIFEST.json, payloads[0] = that manifest,
#                       payloads[1..] = each artifact with sha256/size/url
#   release-signatures.json  machine-readable inventory of the above
#
# Usage:
#   sign-release.sh --assets DIR --version V [--out OUT_DIR] \
#       [--sign-key UID] [--mirror-url BASE] \
#       [--secret HEX64 | --secret-file PATH | OEP_UPDATE_SIGN_KEY[FILE] env]
set -euo pipefail

ASSETS=""
VERSION=""
OUT="dist"
SIGN_KEY_ID="${SIGN_KEY_ID:-oep-airgap-signing}"
MIRROR_URL=""
SECRET=""

# Ed25519 secret for the update manifest: --secret wins, else --secret-file,
# else env OEP_UPDATE_SIGN_KEY / OEP_UPDATE_SIGN_KEYFILE.
secret_hex() {
    if [ -n "$SECRET" ]; then printf '%s' "$SECRET"; return; fi
    if [ -n "${OEP_UPDATE_SIGN_KEYFILE:-}" ] && [ -f "$OEP_UPDATE_SIGN_KEYFILE" ]; then
        tr -d '[:space:]' < "$OEP_UPDATE_SIGN_KEYFILE"; return
    fi
    if [ -f "${OEP_UPDATE_SIGN_KEYFILE:-}" ] && [ -n "${OEP_UPDATE_SIGN_KEYFILE:-}" ]; then
        tr -d '[:space:]' < "$OEP_UPDATE_SIGN_KEYFILE"; return
    fi
    if [ -n "${OEP_UPDATE_SIGN_KEY:-}" ]; then printf '%s' "$OEP_UPDATE_SIGN_KEY"; return; fi
    printf ''
}

while [ $# -gt 0 ]; do
    case "$1" in
        --assets) ASSETS="${2:?--assets needs a value}"; shift 2;;
        --version) VERSION="${2:?--version needs a value}"; shift 2;;
        --out) OUT="${2:?--out needs a value}"; shift 2;;
        --sign-key) SIGN_KEY_ID="${2:?--sign-key needs a value}"; shift 2;;
        --mirror-url) MIRROR_URL="${2:?--mirror-url needs a value}"; shift 2;;
        --secret) SECRET="${2:?--secret needs a value}"; shift 2;;
        --secret-file) OEP_UPDATE_SIGN_KEYFILE="${2:?--secret-file needs a value}"; shift 2;;
        -h|--help) sed -n '1,32p' "$0"; exit 0;;
        *) echo "unknown arg: $1" >&2; exit 2;;
    esac
done

[ -d "$ASSETS" ] || { echo "FATAL: --assets dir not found: $ASSETS" >&2; exit 1; }
[ -n "$VERSION" ] || { echo "FATAL: --version is required" >&2; exit 1; }

command -v gpg >/dev/null 2>&1 || { echo "FATAL: gpg required to sign artifacts" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "FATAL: sha256sum required" >&2; exit 1; }

# Locate the oep-update-sign binary (built on the build host).
SIGNER=""
for cand in \
    "$(dirname "$0")/../..//target/debug/oep-update-sign" \
    "$(pwd)/target/debug/oep-update-sign" \
    "$(pwd)/target/release/oep-update-sign"; do
    [ -x "$cand" ] && SIGNER="$cand" && break
done
if [ -z "$SIGNER" ] && command -v cargo >/dev/null 2>&1; then
    # Build host fallback: build the bin and use it.
    CROOT="$(cd "$(dirname "$0")/../.." && pwd)"
    cargo build --manifest-path "$CROOT/Cargo.toml" -q -p oep-update --bin oep-update-sign
    SIGNER="$CROOT/target/debug/oep-update-sign"
fi

mkdir -p "$OUT"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -a "$ASSETS"/. "$STAGE/" 2>/dev/null || true

cd "$STAGE"

# 1) Checksums: recompute for every asset (skip signatures and manifests).
excluded_re='\.(sig|sha256)$|SHA256SUMS$|RELEASE-MANIFEST\.json$|update\.bundle\.json$|release-signatures\.json$'
find . -maxdepth 1 -type f | sed 's#^\./##' | grep -vE "$excluded_re" | sort \
    | xargs -r sha256sum > SHA256SUMS

# 2) gpg detached signatures for every asset + the checksums.
for f in $(sed 's/  / /' SHA256SUMS | awk '{print $2}'); do
    gpg --batch --yes --armor --detach-sign --local-user "$SIGN_KEY_ID" \
        --output "$f.sig" "$f"
done
gpg --batch --yes --armor --detach-sign --local-user "$SIGN_KEY_ID" \
    --output SHA256SUMS.sig SHA256SUMS

# 3) Release manifest (the update bundle's inline payload).
DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
python3 - "$VERSION" "$DATE" "$MIRROR_URL" <<'PY'
import hashlib, json, os, sys
version, date, mirror = sys.argv[1], sys.argv[2], sys.argv[3] or ""
artifacts = []
for f in sorted(os.listdir(".")):
    if f.endswith((".sig", ".sha256")) or f in ("SHA256SUMS", "RELEASE-MANIFEST.json",
                                                  "update.bundle.json", "release-signatures.json",
                                                  "SHA256SUMS.sig"):
        continue
    if not os.path.isfile(f):
        continue
    h = hashlib.sha256()
    with open(f, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    url = f"{mirror}/{f}" if mirror else None
    artifacts.append({"name": f, "sha256": h.hexdigest(),
                      "size": os.path.getsize(f), "url": url})
manifest = {"manifest": "keelworks-release-v1", "release": version,
            "product": "oep", "released": date, "artifacts": artifacts}
with open("RELEASE-MANIFEST.json", "w") as fh:
    json.dump(manifest, fh, indent=2, sort_keys=True)
    fh.write("\n")
PY

# 4) Ed25519-signed update bundle (decision #11 signed manifest).
sig_secret="$(secret_hex)"
bundle="SKIPPED"
if [ -n "$sig_secret" ] && [ -x "$SIGNER" ]; then
    ARGS=(--out update.bundle.json --version "$VERSION" --secret "$sig_secret"
          --released "$(date -u +%Y-%m-%d)" --inline-payload RELEASE-MANIFEST.json)
    while IFS= read -r art; do
        [ -n "$art" ] || continue
        sha="$(sha256sum "$art" | awk '{print $1}')"
        size="$(stat -c %s "$art")"
        if [ -n "$MIRROR_URL" ]; then
            ARGS+=(--item "$art:$sha:$size:$MIRROR_URL/$art")
        else
            ARGS+=(--item "$art:$sha:$size")
        fi
    done < <(python3 -c "import json;print('\n'.join(a['name'] for a in json.load(open('RELEASE-MANIFEST.json'))['artifacts']))")
    "$SIGNER" "${ARGS[@]}" >/dev/null
    bundle="signed"
else
    echo "WARN: no Ed25519 secret or signer available; update.bundle.json NOT emitted (signature required before drill ships)"
fi

# 5) Inventory.
python3 - "$VERSION" "$DATE" "$bundle" > release-signatures.json <<'PY'
import hashlib, json, os, sys
version, date, bundle = sys.argv[1], sys.argv[2], sys.argv[3]
signatures = {}
for f in sorted(os.listdir(".")):
    if f.endswith(".sig"):
        signatures[f] = True
checksums = {}
with open("SHA256SUMS") as fh:
    for line in fh:
        h, name = line.split(None, 1)
        checksums[name.strip()] = h
out = {"schema": "keelworks.release-signatures.v1", "version": version,
       "signed": date, "update_bundle": bundle,
       "artifacts": len([k for k in signatures if k != "SHA256SUMS.sig"]),
       "detached_signatures": sorted(signatures),
       "checksums": checksums,
       "public_keys": {
           "gpg_artifact": "scripts/airgap/keys/oep-airgap-signing.pub.asc",
           "ed25519_update": "P11/keys/oep-update-signing.pub"}}
json.dump(out, open("release-signatures.json", "w"), indent=2, sort_keys=True)
PY

cp -a . "$OUT/"
echo "DONE: signed release $VERSION -> $(cd "$OUT" && pwd)"
echo "  update bundle: $bundle"
echo "  signature count: $(find "$OUT" -name '*.sig' | wc -l)"
echo "  verify with: verify-release.sh --assets $(cd "$OUT" && pwd)"
