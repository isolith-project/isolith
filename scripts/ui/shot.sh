#!/usr/bin/env bash
# UI-A2 UI-HARNESS: deterministic screenshot of the real Isolith app.
#
# Runs the real `isolith` binary (crates/oep-app) under xvfb-run on a fixed
# 1280x800 virtual screen, captures the rendered window with xwd, decodes it
# deterministically to proof/ui-a2/frame.png, and writes frame.png.sha256.
#
# Determinism guards (both fail LOUDLY, non-zero exit; neither warns):
#   1. Two independent app launches + captures in this invocation must produce
#      byte-identical PNGs (nondeterminism guard).
#   2. When a golden reference already exists (proof/ui-a2/frame.png.sha256
#      from an earlier run), today's frame must hash identically. Any drift
#      (e.g. a one-pixel rendering change) makes the script exit non-zero with
#      both hashes printed, instead of silently rewriting the golden.
#
# Usage:
#   bash scripts/ui/shot.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PROOF_REL="workstreams/modeler/ui/proof/ui-a2"
PROOF_DIR="$ROOT/$PROOF_REL"
mkdir -p "$PROOF_DIR"

FRAME_PNG="$PROOF_DIR/frame.png"
FRAME_SHA="$PROOF_DIR/frame.png.sha256"
BIN="$ROOT/target/debug/isolith"
SCREEN="1280x800x24"

fail() {
    echo "shot.sh: FATAL: $*" >&2
    exit 1
}

# Build the real binary if it is missing. Existing artifacts are reused.
if [ ! -x "$BIN" ]; then
    echo "shot.sh: building oep-app binary (first run)..."
    cargo build -p oep-app || fail "cargo build -p oep-app failed"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"

# Deterministic XWD -> PNG decoder (no ImageMagick dependency; the host ships
# PIL). Strict: any deviation from the 1280x800 TrueColor depth-24 contract
# aborts, because shot.sh treats the PNG sha256 as the determinism oracle.
cat > "$WORK/xwd2png.py" <<'PY'
import struct, sys
import PIL.Image

def main(src, dst):
    with open(src, "rb") as fh:
        data = fh.read()
    le = int.from_bytes(data[0:4], "little")
    be = int.from_bytes(data[0:4], "big")
    he = "<" if 100 <= le <= 1000 else (">" if 100 <= be <= 1000 else None)
    assert he, f"cannot determine header byte order (le={le} be={be})"
    def u32(o):
        return struct.unpack_from(he + "I", data, o)[0]
    hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
    w, h, xo, bo = u32(16), u32(20), u32(24), u32(28)
    bpp = u32(44)
    bpl = u32(48)
    nc = u32(76)
    assert (pf, pd, w, h) == (2, 24, 1280, 800), (
        f"unexpected pixmap: format={pf} depth={pd} {w}x{h}")
    d0 = hs + nc * 12
    order = "<I" if bo == 0 else ">I"  # X11: LSBFirst=0, MSBFirst=1
    rm, gm, bm = u32(56), u32(60), u32(64)
    rj = (rm & -rm).bit_length() - 1
    gj = (gm & -gm).bit_length() - 1
    bj = (bm & -bm).bit_length() - 1
    pxs = bpp // 8
    img = PIL.Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        base = d0 + y * bpl
        for x in range(w):
            p = struct.unpack_from(order, data, base + x * pxs)[0]
            px[x, y] = ((p & rm) >> rj, (p & gm) >> gj, (p & bm) >> bj)
    img.save(dst, "PNG")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
PY

# capture_once: launch the real app on a fresh 1280x800 Xvfb, wait for its
# window to be mapped, let the first frame settle, xwd the root (the app
# window is 1280x800 at +0+0, i.e. the whole screen), decode. Retried because
# the first xwd after map can race GL presentation.
capture_once() {
    local out="$1" attempt
    for attempt in 1 2 3 4 5; do
        rm -f "$RAW"
        xvfb-run -a -s "-screen 0 $SCREEN" \
            bash -c "
                '$BIN' &
                APP=\$!
                found=0
                for i in \$(seq 1 60); do
                    if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then
                        found=1
                        break
                    fi
                    sleep 0.25
                done
                if [ \"\$found\" -ne 1 ]; then
                    echo 'shot.sh: window not found' >&2
                    kill \$APP 2>/dev/null
                    exit 1
                fi
                sleep 1
                xwd -root -out '$RAW' || { echo 'shot.sh: xwd failed' >&2; kill \$APP 2>/dev/null; exit 1; }
                kill \$APP 2>/dev/null
                wait \$APP 2>/dev/null
                exit 0
            " && [ -s "$RAW" ] && python3 "$WORK/xwd2png.py" "$RAW" "$out" && return 0
    done
    fail "xvfb-run launch/capture failed after 5 attempts (raw=$RAW)"
}

capture_once "$WORK/frame_a.png"
HASH_A="$(sha256sum "$WORK/frame_a.png" | awk '{print $1}')"

capture_once "$WORK/frame_b.png"
HASH_B="$(sha256sum "$WORK/frame_b.png" | awk '{print $1}')"

if [ "$HASH_A" != "$HASH_B" ]; then
    fail "nondeterminism: two captures of the same binary differ
  capture A sha256: $HASH_A
  capture B sha256: $HASH_B"
fi

# Drift guard against the committed golden reference.
if [ -f "$FRAME_SHA" ]; then
    GOLDEN="$(cat "$FRAME_SHA")"
    if [ "$HASH_A" != "$GOLDEN" ]; then
        fail "frame drift detected vs committed golden
  current : $HASH_A
  golden  : $GOLDEN
  reference image: $FRAME_PNG"
    fi
fi

cp "$WORK/frame_a.png" "$FRAME_PNG"
printf '%s\n' "$HASH_A" > "$FRAME_SHA"

echo "shot.sh: ok"
echo "  frame    : $PROOF_REL/frame.png"
echo "  sha256   : $HASH_A"
