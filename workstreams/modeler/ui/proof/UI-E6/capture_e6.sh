#!/usr/bin/env bash
# UI-E6 wave-E: deterministic real-binary screenshots of the orientation (nav)
# cube at 1600x1000, byte-identical across two independent app launches.
#
#   standard : click the D4 nav card's "Front" quick button (center 749,665
#              from the UI-D4 05-layout-probe) so the camera lands the exact
#              Front standard view (yaw 0, pitch 0) and the cube's current face
#              is Front; pointer parked neutral.
#   iso      : click "Iso" (center 809,688 from the same probe) so the camera
#              lands the exact isometric view (yaw 0.7854, pitch 0.6155) and
#              the cube shows its isometric corner readout.
#
# Same discipline as every wave capture (UI-D3/D4/D6): two independent app
# launches per mode, driven through XTEST (proof/UI-E6/ui_e6_inject.c), must
# produce byte-identical PNGs; the .sha256 file is the determinism oracle.
#
# The app window is 1280x800 (config::WINDOW_SIZE) mapped at the origin of a
# 1600x1000x24 Xvfb screen (no WM, NorthWest gravity), so the UI-D4 probe
# coordinates are valid unchanged as root coordinates and the captured frame
# is 1600x1000 as the card requires.
#
# Usage: bash capture_e6.sh <standard|iso>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-E6"
MODE="${1:?usage: capture_e6.sh <standard|iso>}"
INJ="$(gcc -O2 -o /tmp/ui_e6_inject "$P/ui_e6_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_e6_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1600x1000x24"
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

# Wave-D merged-head gesture coordinates, valid unchanged (window at origin).
NEUTRAL_X=640; NEUTRAL_Y=760   # pointer parked out of every panel
FRONT_X=749;   FRONT_Y=665      # D4 "Front" quick button center
ISO_X=809;     ISO_Y=688        # D4 "Iso" quick button center

case "$MODE" in
  standard)
    GESTURES=(
      "click $FRONT_X $FRONT_Y"
      "move $NEUTRAL_X $NEUTRAL_Y"
    ) ;;
  iso)
    GESTURES=(
      "click $ISO_X $ISO_Y"
      "move $NEUTRAL_X $NEUTRAL_Y"
    ) ;;
  *) echo "usage: capture_e6.sh <standard|iso>" >&2; exit 2 ;;
esac

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
EOF
for g in "${GESTURES[@]}"; do
  cat >> "$WORK/capture_once.sh" <<EOF
"$INJ" "\$DISPLAY" $g || { echo "capture: $g failed" >&2; kill "\$APP" 2>/dev/null; exit 1; }
EOF
done
cat >> "$WORK/capture_once.sh" <<EOF
sleep 1
xwd -root -out "$RAW" || { echo 'capture: xwd failed' >&2; kill "\$APP" 2>/dev/null; exit 1; }
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
            && python3 "$WORK/xwd2png.py" "$RAW" "$out" 1600 1000; then
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
    echo "capture($MODE): nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
OUT="$P/$MODE.png"
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/$MODE.png.sha256"
echo "capture($MODE): ok"
echo "  $P/$MODE.png"
echo "  sha256 $HA"
