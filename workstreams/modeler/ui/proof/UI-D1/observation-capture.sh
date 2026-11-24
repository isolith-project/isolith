#!/usr/bin/env bash
# UI-D1 observe-launch-frame (NOT the shot.sh golden path): runs the real
# binary once under Xvfb, waits for the Isolith window to map, xwd's the root
# (window is 1280x800 at +0+0), decodes with shot.sh's proven decoder and
# prints the PNG sha256. Observation only: the shot.sh golden at proof/ui-a2/
# is refreshed by the wave-D integrate card (per the card's DO NOT refresh).
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../../.." && pwd)"
out="$SCRIPT_DIR/launch-frame-after-gpu.png"
cd "$ROOT"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
RAW="$work/raw.xwd"
BIN="$ROOT/target/debug/isolith"
SCREEN="1280x800x24"

cat > "$work/xwd2png.py" <<'PY'
import struct, sys
import PIL.Image
def main(src, dst):
    with open(src, "rb") as fh:
        data = fh.read()
    le = int.from_bytes(data[0:4], "little"); be = int.from_bytes(data[0:4], "big")
    he = "<" if 100 <= le <= 1000 else (">" if 100 <= be <= 1000 else None)
    assert he, f"cannot determine header byte order (le={le} be={be})"
    def u32(o): return struct.unpack_from(he + "I", data, o)[0]
    hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
    w, h, xo, bo = u32(16), u32(20), u32(24), u32(28)
    bpp = u32(44); bpl = u32(48); nc = u32(76)
    assert (pf, pd, w, h) == (2, 24, 1280, 800), f"unexpected pixmap: format={pf} depth={pd} {w}x{h}"
    d0 = hs + nc * 12
    order = "<I" if bo == 0 else ">I"
    rm, gm, bm = u32(56), u32(60), u32(64)
    rj = (rm & -rm).bit_length() - 1; gj = (gm & -gm).bit_length() - 1; bj = (bm & -bm).bit_length() - 1
    pxs = bpp // 8
    img = PIL.Image.new("RGB", (w, h)); px = img.load()
    for y in range(h):
        base = d0 + y * bpl
        for x in range(w):
            p = struct.unpack_from(order, data, base + x * pxs)[0]
            px[x, y] = ((p & rm) >> rj, (p & gm) >> gj, (p & bm) >> bj)
    img.save(dst, "PNG")
if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
PY

cat > "$work/inner.sh" <<EOF
"$BIN" &
APP=\$!
found=0
for i in \$(seq 1 60); do
    if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
    sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
xwd -root -out "$RAW" || { echo 'xwd failed' >&2; kill \$APP 2>/dev/null; exit 1; }
kill \$APP 2>/dev/null; wait \$APP 2>/dev/null; exit 0
EOF

for attempt in 1 2 3 4 5; do
    rm -f "$RAW"
    if xvfb-run -a -s "-screen 0 $SCREEN" bash "$work/inner.sh" \
        && [ -s "$RAW" ] && python3 "$work/xwd2png.py" "$RAW" "$out"; then
        break
    fi
    echo "attempt $attempt failed, retrying" >&2
done

if [ -f "$out" ]; then
    echo "wrote $out"
    echo "sha256:"
    sha256sum "$out"
else
    echo "capture failed" >&2
    exit 1
fi
