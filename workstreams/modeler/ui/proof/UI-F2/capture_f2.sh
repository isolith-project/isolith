#!/usr/bin/env bash
# UI-F2 wave-F: deterministic real-binary screenshot of the views placement
# panel at the default 1280x800 window, byte-identical across two independent
# app launches.
#
# The drafting panel opens the committed bracket drawing on its first frame
# (A4 landscape, third-angle) and renders the sheet tree + properties; the new
# Views placement tools sit BELOW the drafting fold inside the drafting scroll
# area, so the capture (1) wheel-scrolls the drafting scroll area to its bottom
# clamp and (2) clicks the four placement tools (place base front, projected
# top, section, detail) at their post-scroll screen coordinates (derived from a
# same-code kittest probe). The result shows the placed base chip (accent), the
# projected top chip, the hatched section outline and the detail chip inside the
# sheet-preview canvas. The pointer is then parked neutral and the frame is
# captured. Two independent launches must produce byte-identical PNGs; the
# .sha256 file is the determinism oracle.
#
# Usage: bash capture_f2.sh
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-F2"
INJ="$(gcc -O2 -o /tmp/ui_f2_inject "$P/ui_f2_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_f2_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"

cat > "$WORK/xwd2png.py" <<'PY'
import struct, sys
import PIL.Image
def main(src, dst, w, h):
    with open(src, "rb") as fh:
        data = fh.read()
    le = int.from_bytes(data[0:4], "little")
    be = int.from_bytes(data[0:4], "big")
    he = "<" if 100 <= le <= 1000 else (">" if 100 <= be <= 1000 else None)
    assert he, "cannot determine header byte order"
    def u32(o): return struct.unpack_from(he + "I", data, o)[0]
    hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
    W, H = u32(16), u32(20)
    bpp = u32(44); bpl = u32(48); nc = u32(76)
    assert (pf, pd, W, H) == (2, 24, w, h), f"unexpected pixmap {pf} {pd} {W}x{H}"
    d0 = hs + nc * 12
    order = "<I" if u32(28) == 0 else ">I"
    rm, gm, bm = u32(56), u32(60), u32(64)
    rj = (rm & -rm).bit_length() - 1
    gj = (gm & -gm).bit_length() - 1
    bj = (bm & -bm).bit_length() - 1
    pxs = bpp // 8
    img = PIL.Image.new("RGB", (W, H)); px = img.load()
    for y in range(H):
        base = d0 + y * bpl
        for x in range(W):
            p = struct.unpack_from(order, data, base + x * pxs)[0]
            px[x, y] = ((p & rm) >> rj, (p & gm) >> gj, (p & bm) >> bj)
    img.save(dst, "PNG")
if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]))
PY

NEUTRAL_X=640; NEUTRAL_Y=760   # pointer parked out of every panel
# Post-scroll tool centres at the drafting scroll-area bottom clamp, derived
# from a same-code kittest probe at the real 1280x800 window size.
BASE_X=1018;  BASE_Y=567
TOP_X=1145;   TOP_Y=567
SEC_X=1154;   SEC_Y=590
DET_X=1005;   DET_Y=613
SCROLL_X=1200; SCROLL_Y=700; SCROLL_NOTCHES=30

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"; DISP="\$DISPLAY"
"\$BIN" &
APP=\$!
found=0
for i in \$(seq 1 140); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
# 1) wheel-scroll the drafting scroll area down to its clamp so the views
#    tools (below the fold) become visible.
"$INJ" "\$DISPLAY" move $SCROLL_X $SCROLL_Y || exit 1
"$INJ" "\$DISPLAY" wheel $SCROLL_X $SCROLL_Y down $SCROLL_NOTCHES || exit 1
sleep 1
# 2) click the four placement tools at the post-scroll coordinates.
"$INJ" "\$DISPLAY" click $BASE_X $BASE_Y || exit 1
"$INJ" "\$DISPLAY" click $TOP_X $TOP_Y || exit 1
"$INJ" "\$DISPLAY" click $SEC_X $SEC_Y || exit 1
"$INJ" "\$DISPLAY" click $DET_X $DET_Y || exit 1
sleep 1
# 3) park the pointer neutral and capture.
"$INJ" "\$DISPLAY" move $NEUTRAL_X $NEUTRAL_Y || exit 1
sleep 1
xwd -root -out "$RAW" || exit 1
kill "\$APP" 2>/dev/null
wait "\$APP" 2>/dev/null
exit 0
EOF
chmod +x "$WORK/capture_once.sh"

capture_once() {
    local out="$1" attempt
    for attempt in 1 2 3 4 5; do
        rm -f "$RAW"
        if xvfb-run -a -s "-screen 0 $SCREEN" bash "$WORK/capture_once.sh" \
            && [ -s "$RAW" ] \
            && python3 "$WORK/xwd2png.py" "$RAW" "$out" 1280 800; then
            return 0
        fi
    done
    echo "capture: FATAL: launch/capture failed after 5 attempts" >&2
    exit 1
}

capture_once "$WORK/a.png"
HA="$(sha256sum "$WORK/a.png" | awk '{print $1}')"
capture_once "$WORK/b.png"
HB="$(sha256sum "$WORK/b.png" | awk '{print $1}')"
if [ "$HA" != "$HB" ]; then
    echo "capture(ui-f2): nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
OUT="$P/views.png"
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/views.png.sha256"
echo "capture(ui-f2): ok"
echo "  $P/views.png"
echo "  sha256 $HA"
