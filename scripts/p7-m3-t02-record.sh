#!/usr/bin/env bash
# P7-M3-T02 record: solve + render the crank-slider mates/kinematics demo and
# package the PPM frames into a playable GIF/MP4.
#
# The crate itself is 100% dependency-free (own solver, own rasterizer); this
# script only packages the recorded PPM frames with ffmpeg on the recording
# host -- ffmpeg is NOT a crate or CI dependency.
#
# Usage: scripts/p7-m3-t02-record.sh [outdir] [frames]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/demos/p7-m3-t02}"
FRAMES="${2:-90}"

echo "== p7-m3-t02 record: frames=${FRAMES} outdir=${OUT} =="

# Solve + render (own solver + own rasterizer, std-only).
cargo run --quiet --manifest-path "$ROOT/Cargo.toml" \
  -p oep-kinematics --bin oep-kinematics-demo -- "$OUT" "$FRAMES"

cd "$OUT"

# Pack frames: MP4 (H.264) + GIF (palette).
ffmpeg -hide_banner -loglevel error -y -framerate 24 \
  -i "frames/frame_%04d.ppm" -c:v libx264 -pix_fmt yuv420p \
  -movflags +faststart crank-slider-demo.mp4

ffmpeg -hide_banner -loglevel error -y -framerate 24 -i "frames/frame_%04d.ppm" \
  -vf "split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  crank-slider-demo.gif

ls -la crank-slider-demo.mp4 crank-slider-demo.gif
echo "== recorded: $(pwd) =="
