#!/usr/bin/env bash
# INTERACTIVE-SHELL headless smoke test + two-machine install rehearsal
# (the P10/P12 rehearsal pattern applied to the interactive editor).
#
# Drives the FULL acceptance path through the real CLI binary with default
# features (no OCCT needed): a user sketches a rectangle, constrains it
# (dims driven by the variable set), solves it, extrudes it, changes a
# dimension in the variable set, sees the solid update (two renders differ),
# and exports STEP that re-imports. This is exactly what the mouse-driven
# editor surfaces call under the pointer, text-callable so a clean machine
# can rehearse the install:
#
#   machine A = this host (U1): clean build + drive (default).
#   machine B = a second U1/VM-class host reached over SSH (default keelwb2
#               on U1; set MACHINE_B_HOST/PORT, mirrors install-rehearsal).
#
# Usage:
#   ./scripts/editor-headless-smoke.sh                  # machine A local drive
#   ./scripts/editor-headless-smoke.sh --selfcheck      # CI-safe doc checks only
#   ./scripts/editor-headless-smoke.sh --drive-only     # drive machine A only
#   MACHINE_B_HOST=op@x MACHINE_B_PORT=2229 ./scripts/editor-headless-smoke.sh
#
# Exit 0 iff no FAIL row (same report contract as install-rehearsal.sh).

set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.."
ROOT="$PWD"

SELFCHECK=false
DRIVE_ONLY=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --selfcheck) SELFCHECK=true ;;
    --drive-only) DRIVE_ONLY=true ;;
    --help|-h) sed -n '1,28p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

MACHINE_B_HOST="${MACHINE_B_HOST:-op@127.0.0.1}"
MACHINE_B_PORT="${MACHINE_B_PORT:-2229}"
OUT_BASE="${OUT_BASE:-workstreams/modeler/editor/proof/editor-headless-smoke}"

ROWS=()
row() { ROWS+=("$1|$2|$3|$4"); }
summary() {
  local total=0 pass=0 fail=0 skip=0 st
  local r
  for r in "${ROWS[@]}"; do
    st="${r#*|*|}"; st="${st%%|*}"
    total=$((total + 1))
    case "$st" in PASS) pass=$((pass + 1));; FAIL) fail=$((fail + 1));; SKIP) skip=$((skip + 1));; esac
  done
  echo "total=$total PASS=$pass FAIL=$fail SKIP=$skip"
}
have_fail() {
  local r st
  for r in "${ROWS[@]}"; do st="${r#*|*|}"; st="${st%%|*}"; [[ "$st" == "FAIL" ]] && return 1; done
  return 0
}

# Build the workbench CLI and drive the full acceptance path into $1 (work
# dir). Prints the trace to $1/trace.txt and the STEP to $1/edpart.step.
drive_acceptance() {
  local work="$1"
  mkdir -p "$work"
  if ! cargo build --quiet -p oep-shell --example workbench 2>/dev/null; then
    echo "FAIL: workbench example did not build" >&2
    return 1
  fi
  local bin="target/debug/examples/workbench"
  if [ ! -x "$bin" ]; then
    bin="$(find . -path ./.worktrees -prune -o -type f -name workbench -print 2>/dev/null | head -1)"
    [ -n "$bin" ] && [ -x "$bin" ] || { echo "FAIL: workbench binary not found" >&2; return 1; }
  fi
  local out rc
  out="$("$bin" \
    editor:open \
    sk:point:0:0 sk:point:3:1 sk:point:4:4 sk:point:1:5 \
    sk:line:p0:p1 sk:line:p1:p2 sk:line:p2:p3 sk:line:p3:p0 \
    sk:constrain:fixed:p0:0:0 sk:constrain:horizontal:l0 sk:constrain:horizontal:l2 \
    sk:constrain:vertical:l1 sk:constrain:vertical:l3 \
    sk:constrain:length:l0:width sk:constrain:length:l1:height \
    vars:set:width:80 vars:set:height:40 \
    sk:solve \
    feat:extrude:10 \
    view:frame \
    sk:render:"$work/before.png" \
    vars:set:width:120 \
    sk:render:"$work/after.png" \
    export:step:"$work/edpart.step" \
    load:step:"$work/edpart.step" \
    undo redo \
    journal 2>&1)"; rc=$?
  if [ $rc -ne 0 ]; then
    echo "FAIL: binary exited $rc" >&2
    echo "$out" >&2
    return 1
  fi
  echo "$out" > "$work/trace.txt"
  local ok=1 b a
  echo "$out" | grep -q "solved dof=0"               || { echo "FAIL: solve dof" >&2; ok=0; }
  echo "$out" | grep -q "width = 80.000000"          || { echo "FAIL: width dim" >&2; ok=0; }
  echo "$out" | grep -q "volume 32000.000000"        || { echo "FAIL: extrude volume" >&2; ok=0; }
  echo "$out" | grep -q "48000.000000 live recompute" || { echo "FAIL: drive lane" >&2; ok=0; }
  b="$(echo "$out" | grep -o 'before.png sha256=[0-9a-f]*' | grep -o '[0-9a-f]\{64\}')"
  a="$(echo "$out" | grep -o 'after.png sha256=[0-9a-f]*' | grep -o '[0-9a-f]\{64\}')"
  { [ -n "$b" ] && [ -n "$a" ] && [ "$b" != "$a" ]; } || { echo "FAIL: renders differ ($b vs $a)" >&2; ok=0; }
  echo "$out" | grep -q "shells=1 closed=1"          || { echo "FAIL: step shells" >&2; ok=0; }
  grep -q "SHELL_BASED_SURFACE_MODEL" "$work/edpart.step" || { echo "FAIL: step marker" >&2; ok=0; }
  echo "$out" | grep -q "^loaded step: parts="       || { echo "FAIL: step reimport" >&2; ok=0; }
  return $(( 1 - ok ))
}

# --- docs leg (always runs, docker-free) -------------------------------------
DOCS_OK=true
for f in "demos/editor-sketch-extrude/manifest.json" \
         "demos/editor-sketch-extrude/edpart.step" \
         "workstreams/modeler/editor/interactive-shell.md"; do
  [ -f "$f" ] || { echo "missing $f"; DOCS_OK=false; }
done
grep -q "help-index-version: 1.2.0" docs/re-notes/oep-workbench-help-index.md || DOCS_OK=false
if $DOCS_OK; then
  row "DOCS" "docs" "PASS" "demo manifest + STEP + workstream doc present; help index at 1.2.0"
else
  row "DOCS" "docs" "FAIL" "committed demo/doc artifacts missing"
fi

CARGO_OK=false
command -v cargo >/dev/null 2>&1 && CARGO_OK=true

if $SELFCHECK; then
  row "DRIVE-A" "drive-A" "SKIP" "no-host (CI-safe degraded mode)"
  row "DRIVE-B" "drive-B" "SKIP" "no-host (CI-safe degraded mode)"
elif $CARGO_OK; then
  TMP="$(mktemp -d)"
  if drive_acceptance "$TMP"; then
    row "DRIVE-A" "drive-A" "PASS" "full acceptance path on machine A (U1): solve -> extrude -> live edit -> STEP + reimport"
  else
    row "DRIVE-A" "drive-A" "FAIL" "acceptance drive failed (trace at $TMP/trace.txt)"
  fi

  if $DRIVE_ONLY; then
    row "DRIVE-B" "drive-B" "SKIP" "--drive-only (no second host)"
  else
    B_REACH=false
    if ssh -o BatchMode=yes -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new \
         -p "$MACHINE_B_PORT" "$MACHINE_B_HOST" true >/dev/null 2>&1; then
      B_REACH=true
    fi
    if $B_REACH; then
      echo "[smoke] syncing repo to machine B ($MACHINE_B_HOST:$MACHINE_B_PORT)"
      rsync -az -e "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -p $MACHINE_B_PORT" \
        --delete \
        --exclude '.git/' --exclude '.worktrees/' --exclude 'target/' \
        --exclude '*.qcow2' --exclude '*.typescript' --exclude '*.mp4' \
        "$ROOT/" "$MACHINE_B_HOST:~/isolith-editor-smoke/"
      BOUT="$(ssh -o BatchMode=yes -o ConnectTimeout=15 -p "$MACHINE_B_PORT" "$MACHINE_B_HOST" \
        "cd ~/isolith-editor-smoke && ./scripts/editor-headless-smoke.sh --drive-only" 2>&1)"
      BRC=$?
      if [ $BRC -eq 0 ]; then
        row "DRIVE-B" "drive-B" "PASS" "full acceptance path on machine B ($MACHINE_B_HOST)"
      else
        row "DRIVE-B" "drive-B" "FAIL" "machine B drive failed (rc=$BRC): $(echo "$BOUT" | tail -3)"
      fi
    else
      row "DRIVE-B" "drive-B" "SKIP" "machine B ($MACHINE_B_HOST:$MACHINE_B_PORT) unreachable"
    fi
  fi
else
  row "DRIVE-A" "drive-A" "FAIL" "no cargo on machine A"
fi

# --- report -----------------------------------------------------------------
REPORT_DIR="$(dirname "$OUT_BASE")"
mkdir -p "$REPORT_DIR"
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SUM="$(summary)"
EC=0; have_fail || EC=1
{
  echo "{"
  echo "  \"task\": \"INTERACTIVE-SHELL smoke + rehearsal\","
  echo "  \"timestamp\": \"$TS\","
  echo "  \"summary\": \"$SUM\","
  echo "  \"exit_code\": $EC,"
  echo "  \"rows\": ["
  first=true
  for r in "${ROWS[@]}"; do
    [[ $first == false ]] && echo ","
    first=false
    leg="${r%%|*}"; det="${r#*|}"; idr="${det%%|*}"; det="${det#*|}"
    st="${det%%|*}"; det="${det#*|}"
    printf '    {"leg":"%s","id":"%s","status":"%s","detail":"%s"}' "$leg" "$idr" "$st" "$det"
  done
  echo ""
  echo "  ]"
  echo "}"
} > "$OUT_BASE.json"
echo "[smoke] wrote $OUT_BASE.json"
echo "[smoke] INTERACTIVE-SHELL headless smoke: $SUM"
exit "$EC"
