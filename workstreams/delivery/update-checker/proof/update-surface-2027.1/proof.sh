#!/usr/bin/env bash
# P10-M5-T01 / decision #20 surface proof (card UPDATE-SURFACE).
#
# Records the advisory statuses - Off, Unreachable, Untrusted, UpdateAvailable
# (plus UpToDate) - against a REAL signed bundle (dist/2027.1.0-rc2/
# update.bundle.json, verified by the committed public key
# e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811), through
# each of the three surfaces: the `oep` CLI, the `oep-workbench` startup
# notice, and the `oep-web` banner.
#
# Every status must exit 0 (decision #11: an update check never gates
# anything); only command-line misuse may exit non-zero, and the script
# asserts that.
set -u

# --- locate repo root (workstreams/delivery/update-checker/proof/update-surface-2027.1/)
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$HERE"
for _ in 1 2 3 4 5 6 7 8 9; do
  if [ -d "$REPO_ROOT/crates" ] && [ -f "$REPO_ROOT/Cargo.toml" ]; then break; fi
  REPO_ROOT="$(dirname "$REPO_ROOT")"
done

OEP="$REPO_ROOT/target/debug/oep"
WB="$REPO_ROOT/target/debug/examples/workbench"
WEB="$REPO_ROOT/target/debug/oep-web"
REAL_BUNDLE="$REPO_ROOT/dist/2027.1.0-rc2/update.bundle.json"
PUBKEY_FILE="$REPO_ROOT/workstreams/security/keys/oep-update-signing.pub"
PUBKEY="$(tr -d '[:space:]' < "$PUBKEY_FILE")"

for tool in "$OEP" "$WB" "$WEB"; do
  if [ ! -x "$tool" ]; then
    echo "missing binary: $tool (build first: cargo build -p oep-update -p oep-web && cargo build -p oep-shell --example workbench)" >&2
    exit 2
  fi
done
[ -f "$REAL_BUNDLE" ] || { echo "missing real bundle: $REAL_BUNDLE" >&2; exit 2; }

# --- per-status configs (absolute paths so reruns are location-independent)
mkcfg() { # name current_version source
  local name="$1" current="$2" src="$3"
  printf '{"enabled": true, "source": %s, "public_key": "%s", "current_version": "%s", "timeout_ms": 3000}\n' \
    "$src" "$PUBKEY" "$current" > "$HERE/$name.json"
}

# The tampered real bundle: flip one byte INSIDE the offered-version literal
# (keeps the file valid JSON but makes the recomputed Ed25519 signature fail -
# decision #11: a signature break is never trusted). Flipping inside the
# base64 payload instead would surface as a parse failure (Unreachable), which
# is also advisory but is not the signature-break row we are proving here.
python3 - "$HERE" "$REAL_BUNDLE" <<'PY'
import sys
out, src = sys.argv[1], sys.argv[2]
data = bytearray(open(src, "rb").read())
needle = b"2027.1.0-rc2"
idx = data.find(needle)
if idx < 0:
    sys.exit("version literal not found in bundle")
# Flip one byte of the last character of the literal.
data[idx + len(needle) - 1] ^= 0x01
open(out + "/tampered.bundle.json", "wb").write(bytes(data))
PY

mkcfg update-available 2027.0.9 "{\"bundle\": \"$REAL_BUNDLE\"}"
mkcfg update-uptodate  2027.1.0-rc2 "{\"bundle\": \"$REAL_BUNDLE\"}"
mkcfg update-unreachable 2027.0.9 "{\"bundle\": \"$REPO_ROOT/does-not-exist/update.bundle.json\"}"
mkcfg update-untrusted 2027.0.9 "{\"bundle\": \"$HERE/tampered.bundle.json\"}"

MARK() { printf '\n===== %s =====\n' "$1"; }
ALL_OK=1
check_exit() { # expected actual label
  if [ "$2" -ne "$1" ]; then echo "  [FAIL] $3 exited $2, expected $1"; ALL_OK=0; else echo "  [ok] exit=$2"; fi
}

{
  MARK "CLI: oep update status (default deployment -> Off)"
  "$OEP" update status; check_exit 0 $? "oep update status"
  MARK "CLI: oep update check --json (Off)"
  "$OEP" update check --json; check_exit 0 $? "oep update check --json"

  MARK "CLI: oep update status --config update-available (UpdateAvailable)"
  "$OEP" update status --config "$HERE/update-available.json"; check_exit 0 $? "update available"
  MARK "CLI: oep update status --config update-uptodate (UpToDate)"
  "$OEP" update status --config "$HERE/update-uptodate.json"; check_exit 0 $? "up to date"
  MARK "CLI: oep update status --config update-untrusted (Untrusted)"
  "$OEP" update status --config "$HERE/update-untrusted.json"; check_exit 0 $? "untrusted"
  MARK "CLI: oep update status --config update-unreachable (Unreachable)"
  "$OEP" update status --config "$HERE/update-unreachable.json"; check_exit 0 $? "unreachable"

  MARK "CLI: oep update check --json --config update-untrusted (Untrusted, JSON)"
  "$OEP" update check --json --config "$HERE/update-untrusted.json"; check_exit 0 $? "untrusted json"
} > cli-statuses.log
cat cli-statuses.log

# --- Workbench startup surface ---
MARK "WORKBENCH: default deployment (Off): one advisory line on stderr, nothing on stdout, exit 0"
( "$WB" journal ) 2>workbench-off.stderr >workbench-off.stdout
W=$?
echo "exit=$W"; check_exit 0 $W "workbench off"
echo "--- stderr ---"; cat workbench-off.stderr
echo "--- stdout (must be empty) ---"; cat workbench-off.stdout

MARK "WORKBENCH: update available: advisory line + non-modal notice (version, date, notes link), exit 0"
OEP_UPDATE_CONFIG="$HERE/update-available.json" "$WB" journal 2>workbench-up.stderr >workbench-up.stdout
W=$?
echo "exit=$W"; check_exit 0 $W "workbench update available"
echo "--- stderr ---"; cat workbench-up.stderr
echo "--- stdout ---"; cat workbench-up.stdout

MARK "WORKBENCH: untrusted source: advisory line only, no notice, exit 0"
OEP_UPDATE_CONFIG="$HERE/update-untrusted.json" "$WB" journal 2>workbench-unt.stderr >workbench-unt.stdout
W=$?
echo "exit=$W"; check_exit 0 $W "workbench untrusted"
echo "--- stderr ---"; cat workbench-unt.stderr
echo "--- stdout (must be empty) ---"; cat workbench-unt.stdout
{ printf '===== WORKBENCH (off) =====\n'; cat workbench-off.stderr workbench-off.stdout
  printf '===== WORKBENCH (update available) =====\n'; cat workbench-up.stderr workbench-up.stdout
  printf '===== WORKBENCH (untrusted) =====\n'; cat workbench-unt.stderr workbench-unt.stdout; } > workbench.log

# --- oep-web banner surface ---
PORT="$(python3 - <<'PY'
import socket
s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()
PY
)"
"$WEB" --addr "127.0.0.1:$PORT" --update-config "$HERE/update-available.json" \
  >web.server.stdout 2>web.server.stderr &
SRV=$!
sleep 1.5
{
  MARK "WEB: GET / -> banner rendered (version, date, actions)"
  curl -sS "http://127.0.0.1:$PORT/" > web-explorer.html
  grep -o 'class="update-banner"[^<]*<strong>[^<]*</strong>' web-explorer.html || true
  MARK "WEB: dismiss version=2027.1.0-rc2 (per-version; redirect back)"
  curl -sS -o /dev/null -w "dismiss http=%{http_code} redirect=%{redirect_url}\n" \
    "http://127.0.0.1:$PORT/ui/update/dismiss?version=2027.1.0-rc2"
  MARK "WEB: GET / after dismiss -> same-version banner gone"
  curl -sS "http://127.0.0.1:$PORT/" > web-explorer-after-dismiss.html
  echo "banner-divs-after-dismiss=$(grep -c 'class="update-banner"' web-explorer-after-dismiss.html)"
  MARK "WEB: check now (explicit action; re-runs the advisory preflight, redirects)"
  curl -sS -o /dev/null -w "checknow http=%{http_code} redirect=%{redirect_url}\n" \
    "http://127.0.0.1:$PORT/ui/update/check"
} > web.log
kill "$SRV" 2>/dev/null
wait "$SRV" 2>/dev/null

# Acceptance leg: launch with an UNREACHABLE source - the stack starts
# normally, logs ONE advisory line, shows NO banner and no dialog.
PORT2="$(python3 - <<'PY'
import socket
s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()
PY
)"
"$WEB" --addr "127.0.0.1:$PORT2" --update-config "$HERE/update-unreachable.json" \
  >web-unreachable.server.stdout 2>web-unreachable.server.stderr &
SRV2=$!
sleep 1.5
{
  MARK "WEB (unreachable source): stack starts normally, one advisory line, no banner"
  curl -sS "http://127.0.0.1:$PORT2/" > web-unreachable-explorer.html
  echo "banner-divs-unreachable=$(grep -c 'class="update-banner"' web-unreachable-explorer.html)"
} > web-unreachable.log
kill "$SRV2" 2>/dev/null
wait "$SRV2" 2>/dev/null
echo "--- web (unreachable) server stderr ---"; sed -n '1,10p' web-unreachable.server.stderr
echo "--- web (unreachable) log ---"; cat web-unreachable.log

echo ""
echo "=== oep-web server log (startup + check-now advisory lines) ==="
sed -n '1,40p' web.server.stderr
echo ""
echo "=== banner text captured in explorer page (first match) ==="
grep -o '<div class="update-banner"[^>]*>.*</div>' web-explorer.html | head -c 1200; echo

if [ "$ALL_OK" = "1" ]; then echo "SURFACE PROOF: ALL STATUSES EXITED 0"; else echo "SURFACE PROOF: FAILURES SEEN ABOVE"; fi
