#!/usr/bin/env bash
# UI-D9 UI-PERF-PROFILE: per-stage frame cost split, recorded numbers.
#
# Attributes the interactive seam's committed p95 (UI-D5: 434.94 ms at
# 1,002,528 triangles / 1600x1000 on the real NVIDIA GB10 adapter, 20 warm-up
# + 120 measured frames) across the frame's stages, so the render-seam fix is
# chosen from measured evidence once (the UI-D9 ADR's input), never guessed.
#
# Stage attribution method: `perf record`/`perf report` is BLOCKED on this
# host (`kernel.perf_event_paranoid = 4` denies unprivileged sampling, no
# passwordless sudo, no setcap on /usr/bin/perf (verified at authoring time),
# so the split uses the env-gated wall-clock stage timers added to
# crates/oep-viewer/src/gpu/mod.rs behind ISOLITH_PERF_STAGES=1, the card's
# sanctioned fallback ("only if samples cannot separate the stages"). The
# timers are inert when the variable is unset: this script proves the fast
# smoke is green and the GPU frame hash is identical with the variable unset
# and set (legs 1 and 2), then takes the numbers with it set (leg 4) against
# the clean uninstrumented p95 (leg 3, the UI-D5 protocol number).
#
# Legs:
#   1. fast smoke, unset                 -> green; prints CPU demo frame hash +
#                                           GPU structural echo hash
#   2. fast smoke, ISOLITH_PERF_STAGES=1 -> green; same hashes + one stage line
#                                           (proves the gate is inert-safe)
#   3. heavy 1M, unset (release)         -> UI_D5_PERF_JSON ->
#                                           proof/UI-D9/perf.json (clean p95)
#   4. heavy 1M, ISOLITH_PERF_STAGES=1   -> stage lines per frame -> transcript;
#                                           second report -> perf-set.json
#   5. aggregate                         -> proof/UI-D9/profile.json: per-stage
#                                           mean ms + share, stage sum vs p95
#                                           with the gap stated, both hashes
#
# Exit code: 0 when proof/UI-D9/profile.json is written with real numbers from
# this host (this is a profile + ADR card, not the budget gate). The heavy
# legs each exit non-zero by design (their committed gate p95 <= 16.6 ms is a
# measured miss at 1M triangles on this device and the bar is never lowered);
# those non-zero exits are captured and do not fail the profile.
#
# Outputs (owned by the UI-D9 card):
#   workstreams/modeler/ui/proof/UI-D9/profile.json
#   workstreams/modeler/ui/proof/UI-D9/perf.json        (clean p95, leg 3)
#   workstreams/modeler/ui/proof/UI-D9/perf-set.json    (instrumented, leg 4)
#   workstreams/modeler/ui/proof/UI-D9/transcripts/*.txt

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PROOF_DIR="$ROOT/workstreams/modeler/ui/proof/UI-D9"
TRANS="$PROOF_DIR/transcripts"
mkdir -p "$TRANS"

echo "== UI-D9 per-stage frame profile on $(hostname) =="
date -u +"started: %Y-%m-%dT%H:%M:%SZ"

# --- Leg 1: fast smoke (unset) ---------------------------------------------
echo; echo "== leg 1: perf_budget fast smoke (ISOLITH_PERF_STAGES unset) =="
set +e
env -u ISOLITH_PERF_STAGES cargo test -p oep-viewer --test perf_budget \
  -- --nocapture 2>&1 | tee "$TRANS/01-fast-smoke-unset.txt"
leg1=${PIPESTATUS[0]}
set -e
if [ "$leg1" -ne 0 ]; then
  echo "FATAL: perf_budget fast smoke is red (leg 1); fix before profiling." >&2
  exit "$leg1"
fi

# --- Leg 2: fast smoke (ISOLITH_PERF_STAGES=1) ------------------------------
echo; echo "== leg 2: perf_budget fast smoke (ISOLITH_PERF_STAGES=1) =="
set +e
ISOLITH_PERF_STAGES=1 cargo test -p oep-viewer --test perf_budget \
  -- --nocapture 2>&1 | tee "$TRANS/02-fast-smoke-set.txt"
leg2=${PIPESTATUS[0]}
set -e
if [ "$leg2" -ne 0 ]; then
  echo "FATAL: perf_budget fast smoke is red with ISOLITH_PERF_STAGES=1 (leg 2)." >&2
  exit "$leg2"
fi

# --- Leg 3: heavy 1M, unset (clean p95, the UI-D5 protocol number) ---------
echo; echo "== leg 3: heavy 1M 1600x1000, unset (release; clean p95) =="
set +e
UI_D5_PERF_JSON="$PROOF_DIR/perf.json" \
  env -u ISOLITH_PERF_STAGES \
  cargo test -p oep-viewer --release --test perf_budget \
    -- --ignored perf_budget_1m_gpu_heavy --nocapture 2>&1 \
    | tee "$TRANS/03-heavy-unset.txt"
leg3=${PIPESTATUS[0]}
set -e
if [ ! -s "$PROOF_DIR/perf.json" ]; then
  echo "FATAL: perf.json was not produced by the heavy unset leg (leg 3)." >&2
  exit 1
fi
echo "heavy-unset exit=$leg3 (non-zero = the committed 16.6 ms bar is still missed at this scene size; bar never lowered)"

# --- Leg 4: heavy 1M, ISOLITH_PERF_STAGES=1 (per-frame stage lines) --------
echo; echo "== leg 4: heavy 1M 1600x1000, ISOLITH_PERF_STAGES=1 (release) =="
set +e
UI_D5_PERF_JSON="$PROOF_DIR/perf-set.json" \
  ISOLITH_PERF_STAGES=1 \
  cargo test -p oep-viewer --release --test perf_budget \
    -- --ignored perf_budget_1m_gpu_heavy --nocapture 2>&1 \
    | tee "$TRANS/04-heavy-set.txt"
leg4=${PIPESTATUS[0]}
set -e
if [ ! -s "$PROOF_DIR/perf-set.json" ]; then
  echo "FATAL: perf-set.json was not produced by the heavy set leg (leg 4)." >&2
  exit 1
fi
echo "heavy-set exit=$leg4 (non-zero expected: bar missed; stage numbers below)"
if ! grep -q "ISOLITH_PERF_STAGES frame=" "$TRANS/04-heavy-set.txt"; then
  echo "FATAL: no per-frame ISOLITH_PERF_STAGES stage lines captured in leg 4." >&2
  exit 1
fi

# --- Leg 5: aggregate into profile.json ------------------------------------
echo; echo "== leg 5: aggregate profile.json =="
python3 - "$PROOF_DIR" "$TRANS" <<'PY'
import json, os, re, sys, datetime

proof_dir, trans = sys.argv[1], sys.argv[2]

# stage lines: ISOLITH_PERF_STAGES frame=N total=123.456ms name=1.23ms ...
line_re = re.compile(
    r"ISOLITH_PERF_STAGES frame=(\d+) total=([0-9.]+)ms (.*)$"
)
stage_re = re.compile(r"([A-Za-z0-9.+_-]+)=([0-9.]+)ms")

frames = []  # list of dicts: frame, total, {stage: ms}
with open(os.path.join(trans, "04-heavy-set.txt"), encoding="utf-8", errors="replace") as fh:
    for ln in fh:
        m = line_re.search(ln)
        if not m:
            continue
        frame = int(m.group(1))
        total = float(m.group(2))
        stages = {}
        for name, val in stage_re.findall(m.group(3)):
            stages[name] = float(val)
        frames.append({"frame": frame, "total": total, "stages": stages})

# The heavy test measures 120 frames after 20 warm-up => last 120 stage lines.
measured = frames[-120:] if len(frames) >= 120 else frames
if len(measured) < 120:
    print(f"WARNING: expected 120 measured stage lines, captured {len(measured)}", file=sys.stderr)

# Card-level partition for the ADR's per-stage table. `frame_time` (the p95
# protocol) is captured in render_frame BEFORE the final frame_hash over the
# readback, so `hash` is post-frame oracle work, not part of the seam's
# interactive frame; it is reported separately below.
card_stages = ["assemble", "vertex-prep", "submit", "readback+w"]
post_frame = ["hash"]
# assemble detail sub-stages (they partition `assemble` exactly; the residual
# the five measured segments do not own is reported as assemble.other):
assemble_subs = [
    "assemble.positions", "assemble.normals", "assemble.project+cull",
    "assemble.sort", "assemble.expand", "assemble.other",
]

means = {}
for f in measured:
    for name, ms in f["stages"].items():
        means.setdefault(name, []).append(ms)
stage_mean = {name: sum(v) / len(v) for name, v in means.items()}

mean_total = sum(f["total"] for f in measured) / len(measured)
card_sum = sum(stage_mean.get(s, 0.0) for s in card_stages)
post_frame_sum = sum(stage_mean.get(s, 0.0) for s in post_frame)
# Small untimed slivers inside frame_time that the four seam stages do not
# own: begin-frame bookkeeping, shading::state(), encoder creation, the gap
# between assemble's return and vertex-prep start, print_frame itself.
slivers = mean_total - card_sum

# Median frame (by total) raw line, for a concrete one-frame stage table.
med = sorted(measured, key=lambda f: f["total"])[len(measured) // 2]
med_line = (
    f"ISOLITH_PERF_STAGES frame={med['frame']} total={med['total']:.3f}ms "
    + " ".join(f"{n}={ms:.3f}ms" for n, ms in med["stages"].items())
)

# Clean p95 from the uninstrumented leg 3 report (same protocol as UI-D5).
with open(os.path.join(proof_dir, "perf.json"), encoding="utf-8") as fh:
    clean = json.load(fh)
# Instrumented run's own report (leg 4) for comparison.
with open(os.path.join(proof_dir, "perf-set.json"), encoding="utf-8") as fh:
    setrep = json.load(fh)

cpu_hash = clean.get("cpu_demo_frame_hash")
cpu_golden = clean.get("cpu_demo_golden_hash")


def gpu_echo(text):
    m = re.search(r"gpu structural echo: frame_hash (\d+) -> (\d+), identical = (\w+)", text)
    if m:
        return {"h1": int(m.group(1)), "h2": int(m.group(2)), "identical": m.group(3) == "true"}
    # older-style print
    m = re.search(r"gpu structural echo: frame_hash (\d+) identical = (\w+)", text)
    if m:
        return {"h1": int(m.group(1)), "h2": None, "identical": m.group(2) == "true"}
    return {"h1": None, "h2": None, "identical": None}

echo_unset = gpu_echo(open(os.path.join(trans, "01-fast-smoke-unset.txt"), encoding="utf-8", errors="replace").read())
echo_set = gpu_echo(open(os.path.join(trans, "02-fast-smoke-set.txt"), encoding="utf-8", errors="replace").read())
gpu_hash_identical = (
    echo_unset.get("identical") is True
    and echo_set.get("identical") is True
    and (echo_unset.get("h1") == echo_set.get("h1") or echo_unset.get("h1") is None)
)

profile = {
    "benchmark": "ui-d9-per-stage-frame-split",
    "card": "UI-D9",
    "host": clean.get("host"),
    "date_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "adapter": clean.get("adapter"),
    "scene_triangles": clean.get("scene_triangles"),
    "width": clean.get("width"),
    "height": clean.get("height"),
    "warmup_frames": clean.get("warmup_frames"),
    "measured_frames": len(measured),
    "stage_method": (
        "env-gated wall-clock stage timers (ISOLITH_PERF_STAGES=1) in "
        "crates/oep-viewer/src/gpu/mod.rs; perf record blocked on this host"
    ),
    "perf_unavailable_reason": (
        "kernel.perf_event_paranoid=4 denies unprivileged sampling "
        "(perf record: 'Access to performance monitoring and observability "
        "operations is limited'); no passwordless sudo, no setcap on perf"
    ),
    "clean_p95_frame_ms": clean.get("p95_frame_ms"),
    "clean_mean_frame_ms": clean.get("mean_frame_ms"),
    "instrumented_mean_frame_ms": setrep.get("mean_frame_ms"),
    "instrumented_p95_frame_ms": setrep.get("p95_frame_ms"),
    "frame_total_mean_ms": round(mean_total, 4),
    "card_stage_mean_ms": {s: round(stage_mean.get(s, 0.0), 4) for s in card_stages},
    "card_stage_share_pct": {
        s: round(100.0 * stage_mean.get(s, 0.0) / mean_total, 2) if mean_total else 0.0
        for s in card_stages
    },
    "assemble_subs_mean_ms": {s: round(stage_mean.get(s, 0.0), 4) for s in assemble_subs},
    "post_frame_hash_mean_ms": round(stage_mean.get("hash", 0.0), 4),
    "card_stage_sum_ms": round(card_sum, 4),
    "seam_end_to_end_mean_ms": round(card_sum + post_frame_sum, 4),
    "untimed_slivers_ms": round(slivers, 4),
    "stage_sum_vs_mean_total_gap_ms": round(mean_total - card_sum, 4),
    "stage_sum_vs_clean_p95_gap_ms": round((clean.get("p95_frame_ms") or 0.0) - card_sum, 4),
    "median_frame": {"frame": med["frame"], "total_ms": round(med["total"], 4)},
    "median_frame_line": med_line,
    "cpu_demo_frame_hash": cpu_hash,
    "cpu_demo_golden_hash": cpu_golden,
    "cpu_determinism_unchanged": clean.get("cpu_determinism_unchanged"),
    "gpu_echo_hash_unset": echo_unset,
    "gpu_echo_hash_set": echo_set,
    "gpu_frame_hash_identical_unset_vs_set": gpu_hash_identical,
    "note_stage_coverage": (
        "the four seam stages (assemble + vertex-prep + submit + readback+w) "
        "partition the interactive frame; assemble.* are the internal sub-split "
        "of the CPU re-derivation and partition `assemble` exactly (incl. the "
        "unmetered residual assemble.other). frame_time is captured in "
        "render_frame BEFORE the final frame_hash, so `hash` is post-frame "
        "oracle work reported separately (seam_end_to_end = card_sum + hash). "
        "'readback+w' includes the synchronous device.poll that waits for GPU "
        "raster + copy completion (the seam is synchronous), so it owns the GPU "
        "exec time. The gap from the stage sum to the clean p95 = the small "
        "untimed slivers plus frame-to-frame variance between the batched "
        "(unset/set) heavy legs."
    ),
}

out = os.path.join(proof_dir, "profile.json")
with open(out, "w", encoding="utf-8") as fh:
    json.dump(profile, fh, indent=2, sort_keys=True)
print("profile written:", out)

# Human-readable stage table (real numbers for the proof).
print()
print("per-stage frame cost, 1,002,528 triangles @ 1600x1000, NVIDIA GB10")
print(f"  measured frames: {len(measured)}  (mean total {mean_total:.1f} ms)")
print(f"  {'stage':<26}{'mean ms':>10}{'share':>8}")
for s in card_stages:
    print(f"  {s:<26}{stage_mean.get(s,0.0):>10.2f}{100.0*stage_mean.get(s,0.0)/mean_total:>7.1f}%")
for s in assemble_subs:
    print(f"    {s:<24}{stage_mean.get(s,0.0):>10.2f}")
print()
print(f"  card-stage sum       : {card_sum:.2f} ms")
print(f"    + post-frame hash  : {stage_mean.get('hash',0.0):.2f} ms")
print(f"    = end-to-end       : {card_sum + post_frame_sum:.2f} ms")
print(f"  untimed slivers      : {slivers:.2f} ms")
print(f"  clean p95 (leg 3)    : {clean.get('p95_frame_ms'):.2f} ms")
print(f"  instrumented p95     : {setrep.get('p95_frame_ms'):.2f} ms")
print(f"  gap sum->clean p95   : {(clean.get('p95_frame_ms') or 0.0) - card_sum:.2f} ms")
print()
print("  median measured frame line:")
print("   ", med_line)
print()
print(f"  cpu determinism      : demo frame_hash {cpu_hash} == golden {cpu_golden} (match={clean.get('cpu_determinism_unchanged')})")
print(f"  gpu echo hash        : unset {echo_unset} | set {echo_set}  identical-unset-vs-set={gpu_hash_identical}")
PY

if [ ! -s "$PROOF_DIR/profile.json" ]; then
  echo "FATAL: profile.json was not produced (leg 5)." >&2
  exit 1
fi

echo
echo "======================================================================"
echo "UI-D9 per-stage frame profile:"
echo "  report : $PROOF_DIR/profile.json"
echo "  clean p95 : $(python3 -c "import json;print(json.load(open('$PROOF_DIR/profile.json'))['clean_p95_frame_ms'])") ms"
echo "======================================================================"
exit 0
