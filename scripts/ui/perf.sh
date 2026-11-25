#!/usr/bin/env bash
# UI-D5 UI-PERF: interactive frame budget on U1, recorded numbers.
#
# The single repeatable perf harness: builds and runs the oep-viewer perf
# harness (crates/oep-viewer/tests/perf_budget.rs), which renders a
# 1,000,000+-triangle scene at 1600x1000 through the real GPU viewport
# backend, and writes the measured report. Proof of the recorded numbers (the
# real command output) lands under workstreams/modeler/ui/proof/UI-D5/.
#
# Legs:
#   1. fast smoke  -> cargo test -p oep-viewer --test perf_budget   (always green)
#   2. heavy 1M    -> the #[ignore]d heavy measurement, run in --release so the
#                     numbers are real, writing proof/UI-D5/perf.json
#   3. app smoke   -> xvfb-run -a cargo run -p oep-app -- --smoke  (exit 0)
#
# Exit code: 0 when every leg is green INCLUDING the committed interactive
# threshold (p95 <= 16.6 ms read from the generated report). Non-zero when the
# threshold is missed on this device — the report is STILL written with the
# real measured p95, and the card must then be blocked `capability` with that
# number (the bar is never lowered here).

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PROOF_DIR="$ROOT/workstreams/modeler/ui/proof/UI-D5"
TRANS="$PROOF_DIR/transcripts"
REPORT="$PROOF_DIR/perf.json"
mkdir -p "$TRANS"

echo "== UI-D5 perf harness on $(hostname) =="
date -u +"started: %Y-%m-%dT%H:%M:%SZ"

# --- Leg 1: fast smoke (always green; the acceptance leg) ---------------------
echo; echo "== leg 1: perf_budget fast smoke =="
set +e
cargo test -p oep-viewer --test perf_budget 2>&1 | tee "$TRANS/01-fast-smoke.txt"
leg1=${PIPESTATUS[0]}
set -e
if [ "$leg1" -ne 0 ]; then
  echo "FATAL: perf_budget fast smoke is red (leg 1) — fix before measuring." >&2
  exit "$leg1"
fi

# --- Leg 2: heavy 1M measurement (release; writes perf.json) ------------------
echo; echo "== leg 2: heavy 1M-triangle 1600x1000 measurement (release) =="
set +e
UI_D5_PERF_JSON="$REPORT" \
  cargo test -p oep-viewer --release --test perf_budget \
    -- --ignored perf_budget_1m_gpu_heavy --nocapture 2>&1 \
    | tee "$TRANS/02-heavy-perf.txt"
leg2=${PIPESTATUS[0]}
set -e

# The report must exist regardless of the verdict (a miss still records facts).
if [ ! -s "$REPORT" ]; then
  echo "FATAL: $REPORT was not produced by the heavy leg (leg 2)." >&2
  exit 1
fi
echo "report written: $REPORT"

# --- Leg 3: app smoke ----------------------------------------------------------
echo; echo "== leg 3: app smoke =="
set +e
xvfb-run -a cargo run -p oep-app -- --smoke 2>&1 | tee "$TRANS/03-app-smoke.txt"
leg3=${PIPESTATUS[0]}
set -e
if [ "$leg3" -ne 0 ]; then
  echo "FATAL: app smoke is red (leg 3)." >&2
  exit "$leg3"
fi

# --- Verdict ---------------------------------------------------------------
# Decode the committed threshold verdict from the report (real numbers).
p95="$(python3 -c "import json,sys; print(json.load(open('$REPORT'))['p95_frame_ms'])" 2>/dev/null || echo '?')"
met="$(python3 -c "import json,sys; print(json.load(open('$REPORT'))['budget_met'])" 2>/dev/null || echo '?')"
tris="$(python3 -c "import json,sys; print(json.load(open('$REPORT'))['scene_triangles'])" 2>/dev/null || echo '?')"
adapter="$(python3 -c "import json,sys; print(json.load(open('$REPORT'))['adapter']['name'])" 2>/dev/null || echo '?')"

echo
echo "================================================================"
echo "UI-D5 interactive budget verdict:"
echo "  adapter : $adapter"
echo "  scene   : $tris triangles @ 1600x1000"
echo "  p95     : $p95 ms (committed threshold 16.6 ms)"
echo "  budget_met: $met"
echo "  report  : $REPORT"
echo "================================================================"

if [ "$met" = "True" ]; then
  echo "BUDGET MET — interactive at 1M triangles on this device."
  exit 0
else
  echo "THRESHOLD MISSED on this device: p95 = $p95 ms > 16.6 ms. Bar NOT lowered." >&2
  echo "Recorded in $REPORT; this card is a legitimate 'capability' block." >&2
  exit 1
fi
