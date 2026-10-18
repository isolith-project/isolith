#!/usr/bin/env bash
# P10-M1-T02: capture a machine-readable proof of the card's acceptance on the
# current host (U1/VM-class). Runs the real binary, real cert tooling, real
# backup + restore round-trip, and records the results as JSON under
# P10-M1/proof/. This is the durable, committed evidence the lane ships.
#
# Usage:
#   scripts/p10m1t02-proof.sh [VAULT_DIR] [ARCHIVE_DIR] [OUT_JSON]
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
WT=$(pwd)

VAULT_DIR="${1:-$(mktemp -d /tmp/p10m1-vault-XXXX)}"
ARCHIVE_DIR="${2:-$(mktemp -d /tmp/p10m1-archive-XXXX)}"
OUT_JSON="${3:-$WT/P10-M1/proof/p10-m1-t02-proof.json}"
PROOF_DIR="$(dirname "$OUT_JSON")"
mkdir -p "$PROOF_DIR" "$VAULT_DIR" "$ARCHIVE_DIR"
BIN="$WT/target/debug/oep-pdm"
PORT=18778
VALS_OK=0

echo "== P10-M1-T02 proof on $(hostname) =="

# ---- 1. TLS terminate ------------------------------------------------------
CERT_DIR="$(mktemp -d /tmp/p10m1-certs-XXXX)"
bash "$WT/scripts/gen-stack-certs.sh" "$CERT_DIR" >/dev/null 2>&1
"$BIN" --addr "127.0.0.1:$PORT" --persist-dir "$VAULT_DIR" \
  --tls-cert "$CERT_DIR/cert.pem" --tls-key "$CERT_DIR/key.pem" \
  >/tmp/p10m1-tls-proof.log 2>&1 &
SRV=$!
sleep 1.5
TLS_HANDSHAKE="fail"
TLS_HEALTH=""
if openssl s_client -connect "127.0.0.1:$PORT" -servername localhost \
    -CAfile "$CERT_DIR/ca.pem" -verify_return_error -quiet \
    < <(printf 'GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n') \
    > /tmp/p10m1-tls-body.txt 2>/dev/null; then
  TLS_HANDSHAKE="ok"
  TLS_HEALTH=$(tr -d '\r\n' < /tmp/p10m1-tls-body.txt)
fi
kill "$SRV" 2>/dev/null || true
sleep 0.5

# ---- 2. seed vault, backup, restore ----------------------------------------
python3 - "$BIN" "$VAULT_DIR" "$PORT" > /tmp/p10m1-seed.json <<'PY'
import json, sys, time, subprocess, urllib.request, base64
bin_p, vault, port = sys.argv[1:4]
base = f"http://127.0.0.1:{port}/api/v1"
# TLS was the contract surface; boot plain for seeding (same router).
proc = subprocess.Popen([bin_p, "--addr", f"127.0.0.1:{port}", "--persist-dir", vault],
                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.2)
def call(method, path, body=None):
    req = urllib.request.Request(base+path, data=json.dumps(body).encode() if body else None,
                                 method=method, headers={"Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req).read())
sample = open("/home/r1a/keelworks-main/.worktrees/p10m1t02-tls-backup-1017a/crates/oep-pdm/tests/fixtures/box-mini.step","rb").read()
count = 5
for i in range(count):
    it = call("POST","/items",{"name":f"Proof-{i}","kind":"part","description":"P10-M1-T02 proof seed"})
    call("POST",f"/items/{it['id']}/revisions",{
        "state":"released" if i==0 else "working","comment":"proof","created_by":"proof",
        "file":{"filename":f"p{i}.step","format":"step","content_type":"application/octet-stream",
                "content": base64.b64encode(sample+b"<%d>"%i).decode()}})
items = call("GET","/items")["total"]
audit = call("GET","/audit/verify")
proc.terminate()
try: proc.wait(timeout=3)
except Exception: proc.kill()
print(json.dumps({"seeded_items": count, "catalogue_total": items, "audit_verify": audit}))
PY

SEEDED=$(python3 -c "import json;print(json.load(open('/tmp/p10m1-seed.json'))['seeded_items'])")
CATALOGUE=$(python3 -c "import json;print(json.load(open('/tmp/p10m1-seed.json'))['catalogue_total'])")
AUDIT_OK=$(python3 -c "import json;print(json.load(open('/tmp/p10m1-seed.json'))['audit_verify']['ok'])")

# Producer (checksummed) -> archive target
bash "$WT/scripts/backup/vault-backup.sh" "$VAULT_DIR" "$ARCHIVE_DIR" proof-bundle \
  > /tmp/p10m1-backup.log 2>&1 || {
  echo "[proof] backup producer FAILED" >&2; cat /tmp/p10m1-backup.log >&2; exit 1; }
BLOSS=$(grep -oE "blobs    : [0-9]+" /tmp/p10m1-backup.log | grep -oE "[0-9]+" | head -1)

# Restore round-trip on this host
bash "$WT/scripts/backup/vault-restore-check.sh" "$ARCHIVE_DIR" "$BIN" \
  > /tmp/p10m1-restore.log 2>&1 || {
  echo "[proof] restore round-trip FAILED" >&2; cat /tmp/p10m1-restore.log >&2; exit 1; }
RESTORE_OK=1

# ---- 3. schedule installed? (cron/user timer entry references the producer) --
SCHEDULE="absent"
crontab -l 2>/dev/null | grep -q "vault-backup.sh" && SCHEDULE="cron-ok"
systemctl --user list-timers 2>/dev/null | grep -q "keelworks-vault-backup" && SCHEDULE="systemd-ok"

# ---- 4. write proof json -----------------------------------------------------
python3 - "$OUT_JSON" "$TLS_HANDSHAKE" "$TLS_HEALTH" "$SEEDED" "$CATALOGUE" "$AUDIT_OK" \
    "$BLOSS" "$RESTORE_OK" "$SCHEDULE" "$VAULT_DIR" "$ARCHIVE_DIR" <<'PY'
import json, socket, sys, datetime
out = {
  "card": "P10-M1-T02: TLS + certs, vault DB/blob backup to second disk/archive (U1/VM verified)",
  "schema_version": 1,
  "host": socket.gethostname(),
  "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
  "tls": {
    "terminates_with_trusted_chain": sys.argv[2] == "ok",
    "health_over_tls": sys.argv[3]
  },
  "backup": {
    "seeded_items": int(sys.argv[4]),
    "catalogue_total": int(sys.argv[5]),
    "audit_verify_ok": (sys.argv[6] == "True" or sys.argv[6] == "true"),
    "blob_count": int(sys.argv[7]),
    "bundle_contract": "keelworks-vault-backup/{latest,manifest.sha256,meta.json}"
  },
  "restore_round_trips_on_this_host": sys.argv[8] == "1",
  "scheduled": sys.argv[9],
  "vault_dir": sys.argv[10],
  "archive_target": sys.argv[11],
}
json.dump(out, open(sys.argv[1], "w"), indent=2)
print("proof written:", sys.argv[1])
print("  tls : %s | backup blobs=%s | restore-roundtrip=%s | schedule=%s" % (
  out["tls"]["terminates_with_trusted_chain"], sys.argv[7], out["restore_round_trips_on_this_host"], out["scheduled"]))
PY
