#!/usr/bin/env bash
# airgap-install.sh — install the full Keelworks stack on an air-gapped machine
# with NO network access, from a signed offline bundle (P10-M2-T02).
#
# Runs ENTIRELY offline: no pulls, no registry, no phone-home, no reach-back.
# The machine need never have had a default route. Steps:
#   1. verify   — the bundle tarball signature + SHA-256 + manifest signature
#   2. extract  — expand the bundle into $AIRGAP_INSTALL_DIR
#   3. load     — `docker load` every embedded image tarball
#   4. configure — generate .env from the embedded env.example (update-check OFF)
#   5. start    — bring the stack up with `docker compose up -d` (no --pull)
#   6. verify   — run the same health checks the online install uses
#   7. record   — write a proof/install-record JSON for the operator + audit
# Rollback is provided separately by scripts/airgap/airgap-rollback.sh.
#
# Usage (on the target host, from the USB/ISO copy):
#   airgap-install.sh --bundle /media/usb/keelworks-offline-0.1.0.tar.gz \
#       [--compose /opt/keelworks/compose.yaml] [--install-dir /opt/keelworks] \
#       [--project oep] [--no-record]
#
# Options:
#   --bundle PATH   the signed offline bundle tarball (required)
#   --compose PATH  compose file to use (default: the one embedded in the bundle,
#                   or $INSTALL_DIR/compose.yaml)
#   --install-dir   where to extract (default: /opt/keelworks)
#   --project NAME  docker compose project name (default: oep)
#   --no-record     skip writing the install record JSON
set -euo pipefail
cd "$(dirname "$0")"
source ./airgap-lib.sh
# Ensure the shipped verification key is present before any signature check so
# an operator with only the bundle + scripts can verify without prior setup.
import_public_key

BUNDLE=""
INSTALL_DIR="/opt/keelworks"
COMPOSE=""
PROJECT="oep"
RECORD=1
HEALTHCHECK_URL="${HEALTHCHECK_URL:-http://127.0.0.1:8787/api/v1/health}"

while [ $# -gt 0 ]; do
    case "$1" in
        --bundle) BUNDLE="${2:?--bundle needs a path}"; shift 2;;
        --compose) COMPOSE="${2:?--compose needs a path}"; shift 2;;
        --install-dir) INSTALL_DIR="${2:?--install-dir needs a path}"; shift 2;;
        --project) PROJECT="${2:?--project needs a name}"; shift 2;;
        --no-record) RECORD=0; shift;;
        *) echo "unknown arg: $1" >&2; exit 2;;
    esac
done

[ -n "$BUNDLE" ] || die "--bundle is required"
[ -f "$BUNDLE" ] || die "bundle not found: $BUNDLE"
command -v docker >/dev/null 2>&1 || die "docker required on the target"
command -v gpg    >/dev/null 2>&1 || die "gpg required to verify the bundle"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum required"

log "=== Keelworks air-gap install (no-phone-home, signed bundle) ==="
mkdir -p "$INSTALL_DIR"

# --- 1&2. Verify the bundle tarball signature + checksum, then extract. ---
BUNDLE_BASENAME="$(basename "$BUNDLE")"
BUNDLE_SHA_FILE="$BUNDLE.sha256"
log "verifying bundle signature"
gpg_detach_verify "$BUNDLE" || die "bundle signature verification FAILED (tampered?)"
log "bundle signature OK"
if [ -f "$BUNDLE_SHA_FILE" ]; then
    expected="$(cat "$BUNDLE_SHA_FILE")"
    actual="$(sha256sum "$BUNDLE" | awk '{print $1}')"
    [ "$expected" = "$actual" ] || die "bundle sha256 mismatch (corrupt media?)"
    log "bundle sha256 OK: $actual"
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
log "extracting bundle"
tar xzf "$BUNDLE" -C "$STAGE"
BUNDLE_DIR="$(find "$STAGE" -maxdepth 1 -type d -name 'keelworks-offline-*' | head -1)"
[ -n "$BUNDLE_DIR" ] || die "bundle did not contain a keelworks-offline-* directory"
log "bundle root: $BUNDLE_DIR"

# --- Verify the signed MANIFEST inside the bundle. ---
log "verifying signed MANIFEST.json"
gpg_detach_verify "$BUNDLE_DIR/MANIFEST.json" \
    || die "MANIFEST.json signature verification FAILED"
log "manifest signature OK"
log "verifying all on-media file checksums against the manifest"
python3 - "$BUNDLE_DIR" <<'PY'
import hashlib, json, os, sys
root = sys.argv[1]
with open(os.path.join(root, "MANIFEST.json")) as f:
    man = json.load(f)
bad = 0
for rel, want in man["files"].items():
    p = os.path.join(root, rel)
    if not os.path.isfile(p):
        print("MISSING", rel); bad += 1; continue
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    if h.hexdigest() != want:
        print("CORRUPT", rel); bad += 1
print("manifest files verified:", len(man["files"]), "corrupt/missing:", bad)
sys.exit(1 if bad else 0)
PY
log "on-media integrity OK"

# --- Copy the runtime tree into place. ---
log "installing files to $INSTALL_DIR"
cp -r "$BUNDLE_DIR/." "$INSTALL_DIR/"
mkdir -p "$INSTALL_DIR/images"
for img in "$BUNDLE_DIR"/images/*.tar.gz; do [ -f "$img" ] && cp "$img" "$INSTALL_DIR/images/"; done
chmod +x "$INSTALL_DIR"/scripts/*.sh 2>/dev/null || true

# --- 3. Load images — no pull is ever attempted. ---
# Snapshot the current image set first so rollback can restore the previous
# known-good tags.
mkdir -p "$INSTALL_DIR/proof"
docker images --format '{{.Repository}}:{{.Tag}}' | sort > "$INSTALL_DIR/proof/image-snapshot.txt" 2>/dev/null || true
log "snapshotted current local images for rollback"
log "loading images (offline — docker load only, no pull)"
for img in "$INSTALL_DIR"/images/*.tar.gz; do
    [ -f "$img" ] || continue
    log "  docker load < $(basename "$img")"
    docker load -q < "$img"
done

# --- 4. Generate environment from the template (update-check OFF by default). ---
if [ ! -f "$INSTALL_DIR/.env" ]; then
    # Seed with the embedded template; the operator may edit before start.
    cp "$INSTALL_DIR/env.example" "$INSTALL_DIR/.env"
    # Rotate a fresh local secret so two air-gapped installs never share one.
    python3 - "$INSTALL_DIR/.env" <<'PY'
import secrets, sys
p = sys.argv[1]
s = secrets.token_hex(32)
data = open(p).read()
if "OEP_LOCAL_SECRET=" not in data:
    data += f"\nOEP_LOCAL_SECRET={s}\n"
open(p, "w").write(data)
PY
    log "generated $INSTALL_DIR/.env from embedded template (fresh OEP_LOCAL_SECRET)"
fi
log "update-checker posture in .env:"
grep -E "^(OEP_UPDATE_CHECK|OEP_UPDATE_MIRROR_URL|OEP_UPDATE_OFFLINE_BUNDLE)=" "$INSTALL_DIR/.env" || true

# Decision #11 hard guard: in an OFFLINE deployment the update checker must be
# OFF unless it is pointed only at a local signed bundle that is actually
# present. No update check may ever reach outside the machine.
uc="$(grep -E '^OEP_UPDATE_CHECK=' "$INSTALL_DIR/.env" | head -1 | cut -d= -f2)"
if [ "$uc" != "off" ]; then
    local_bundle="$(grep -E '^OEP_UPDATE_OFFLINE_BUNDLE=' "$INSTALL_DIR/.env" | head -1 | cut -d= -f2)"
    if [ -z "$local_bundle" ] || [ ! -f "$local_bundle" ]; then
        die "air-gap install refuses OEP_UPDATE_CHECK=$uc without a local offline bundle — decision #11: no phone-home, no reach-back"
    fi
    log "update checker enabled but constrained to a LOCAL offline bundle ($local_bundle) — still no internet"
fi

# --- 5. Start the stack (compose attached to embedded/local compose file). ---
COMPOSE_FILE="${COMPOSE:-$INSTALL_DIR/compose.yaml}"
[ -f "$COMPOSE_FILE" ] || die "compose file not found: $COMPOSE_FILE"
log "compose file: $COMPOSE_FILE"
log "starting stack (docker compose up -d, never pulls)"
COMPOSE_PROJECT_NAME="$PROJECT" docker compose -f "$COMPOSE_FILE" up -d

# --- 6. Same health checks as the online install. ---
# The compose file's own healthchecks ARE the canonical health check (identical
# definitions the online install uses). On a real air-gapped server the
# operator additionally curls the published endpoint; inside a simulated
# `--network none` target that curl cannot reach the host-published port, so we
# wait on the compose health status (authoritative) and report the curl probe
# as best-effort.
log "waiting for compose service health (same healthchecks as online install)"
ok=0
for _ in $(seq 1 40); do
    status="$(COMPOSE_PROJECT_NAME="$PROJECT" docker compose -f "$COMPOSE_FILE" ps --format '{{.Service}} {{.Health}}' 2>/dev/null || true)"
    if [ -n "$status" ]; then
        # Each line is "<service> <health>"; health is the LAST token. All lines
        # must end in 'healthy'.
        bad="$(printf '%s\n' "$status" | awk '{n=split($0,a," "); if(a[n]!="healthy") non=1} END{print (non?1:0)}')"
        if [ "$bad" = "0" ]; then ok=1; break; fi
    fi
    sleep 2
done
if [ "$ok" != "1" ]; then
    log "services did not all become healthy; starting rollback"
    if [ -f "$(dirname "$0")/airgap-rollback.sh" ]; then
        bash "$(dirname "$0")/airgap-rollback.sh" --install-dir "$INSTALL_DIR" --project "$PROJECT" || true
    fi
    die "health check failed — stack rolled back"
fi
log "compose service health ALL HEALTHY (same checks as online install)"

# Best-effort HTTP probe of the published health endpoint (authoritative where
# reachable — on a real air-gapped server this is the operator's check).
probe_result="unreachable (no-network target)"
if curl -fsS --max-time 5 "$HEALTHCHECK_URL" >/dev/null 2>&1; then
    probe_result="pass"
    log "health endpoint $HEALTHCHECK_URL: PASS"
else
    log "health endpoint $HEALTHCHECK_URL not reachable from here (expected inside a --network none target); compose health already confirmed"
fi

# --- 7. Record the install. ---
if [ "$RECORD" = "1" ]; then
    mkdir -p "$INSTALL_DIR/proof"
    python3 - "$INSTALL_DIR" "$BUNDLE_BASENAME" "$HEALTHCHECK_URL" "$probe_result" <<'PY'
import datetime, json, os, subprocess, sys
install_dir, bundle, hc, probe = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
def sh(c):
    return subprocess.run(c, shell=True, capture_output=True, text=True).stdout.strip()
rec = {
    "card": "P10-M2-T02",
    "mode": "airgap",
    "bundle": bundle,
    "installed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "host": sh("hostname") or os.uname().nodename,
    "health_check": hc,
    "health_probe": probe,
    "result": "PASS" if probe == "pass" else "PASS (compose health; endpoint probe unreachable in this no-network context)",
    "update_check": "off (default)",
    "compose_services": sh(f"docker compose -f {install_dir}/compose.yaml ps --services"),
}
with open(os.path.join(install_dir, "proof", "airgap-install-record.json"), "w") as f:
    json.dump(rec, f, indent=2)
    f.write("\n")
print("install record:", os.path.join(install_dir, "proof", "airgap-install-record.json"))
PY
fi
log "=== AIR-GAP INSTALL COMPLETE (no network used) ==="
