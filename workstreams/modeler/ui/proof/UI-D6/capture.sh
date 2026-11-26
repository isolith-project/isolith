#!/usr/bin/env bash
# UI-D6 wave-D close: deterministic real-binary screenshots of the merged-head
# Isolith binary for the wave-D screenshot set.
#
#   shaded    : default studio launch shows the D2 GPU shading surface - the
#               shaded solid with feature edges, opaque, and the D2 shading
#               card (Shaded/Edges/Flat/Smooth/50% transparent + alpha) at the
#               viewport top-right. Pointer parked neutral.
#   view-cube : the D4 nav card ("Orientation": cube faces, Front/Top/Right/Iso
#               quick buttons, section slider, "Triangles" readout) at the
#               viewport lower-centre; we click the "Iso" quick button
#               (center 809,688 from the D4 layout probe) so the camera lands
#               the exact Iso standard view, proving the cube card interacts.
#   picking   : Alt+left-drag box-select across the demo solid (D3 gesture)
#               -> the GPU id-buffer box query outlines the selection while the
#               rubber band is still held (captured mid-drag).
#
# Same discipline as every wave capture: two independent app launches per mode,
# each driven through XTEST (proof/UI-D6/ui_d6_inject.c), must produce
# byte-identical PNGs; the .sha256 file is the determinism oracle. Coordinates
# are the merged-head layout at 1280x800, pixels-per-point 1.0 == X pixels
# (D3 coords from proof/UI-D3, D4 nav card rects from UI-D4 05-layout-probe).
#
# Usage: bash capture.sh <shaded|view-cube|picking>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-D6"
MODE="${1:?usage: capture.sh <shaded|view-cube|picking>}"
INJ="$(gcc -O2 -o /tmp/ui_d6_inject "$P/ui_d6_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_d6_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"

cat > "$WORK/xwd2png.py" <<'PY'
import struct, sys
import PIL.Image
def main(src, dst):
    with open(src, "rb") as fh:
        data = fh.read()
    le = int.from_bytes(data[0:4], "little")
    be = int.from_bytes(data[0:4], "big")
    he = "<" if 100 <= le <= 1000 else (">" if 100 <= be <= 1000 else None)
    assert he, "cannot determine header byte order"
    def u32(o): return struct.unpack_from(he + "I", data, o)[0]
    hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
    w, h = u32(16), u32(20)
    bpp = u32(44); bpl = u32(48); nc = u32(76)
    assert (pf, pd, w, h) == (2, 24, 1280, 800), f"unexpected pixmap {pf} {pd} {w}x{h}"
    d0 = hs + nc * 12
    order = "<I" if u32(28) == 0 else ">I"
    rm, gm, bm = u32(56), u32(60), u32(64)
    rj = (rm & -rm).bit_length() - 1
    gj = (gm & -gm).bit_length() - 1
    bj = (bm & -bm).bit_length() - 1
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

# Merged-head gesture coordinates (1280x800, ppp 1.0 == X pixels).
NEUTRAL_X=640; NEUTRAL_Y=760   # pointer parked out of every panel
ISO_X=809;     ISO_Y=688       # "Iso" quick button center (D4 layout probe)
BX0=360; BY0=300; BX1=880; BY1=520   # box-select across the demo solid (D3)

case "$MODE" in
  shaded)
    GESTURES=( "move $NEUTRAL_X $NEUTRAL_Y" ) ;;
  view-cube)
    GESTURES=(
      "click $ISO_X $ISO_Y"
      "move $NEUTRAL_X $NEUTRAL_Y"
    ) ;;
  picking)
    GESTURES=(
      "dstart $BX0 $BY0 alt"
      "dto $(( (BX0+BX1)/2 )) $(( (BY0+BY1)/2 ))"
      "dto $BX1 $BY1"
      "CAPTURE"
      "dend"
    ) ;;
  *) echo "usage: capture.sh <shaded|view-cube|picking>" >&2; exit 2 ;;
esac

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"; DISP="\$DISPLAY"
"\$BIN" &
APP=\$!
found=0
for i in \$(seq 1 120); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
EOF
for g in "${GESTURES[@]}"; do
  set -- $g
  if [ "$1" = "CAPTURE" ]; then
    printf 'sleep 0.7\nif xwd -root -out "$RAW"; then RAW_CAPTURED=1; else echo "capture: xwd failed" >&2; kill $APP 2>/dev/null; exit 1; fi\n' >> "$WORK/capture_once.sh"
  else
    printf '\"%s\" "$DISPLAY" %s || { echo "capture: %s failed" >&2; kill $APP 2>/dev/null; exit 1; }\n' "$INJ" "$*" "$*" >> "$WORK/capture_once.sh"
  fi
done
cat >> "$WORK/capture_once.sh" <<'EOF'
sleep 1
if [ "${RAW_CAPTURED:-0}" -ne 1 ]; then
  xwd -root -out "$RAW" || { echo 'capture: xwd failed' >&2; kill $APP 2>/dev/null; exit 1; }
fi
kill $APP 2>/dev/null
wait $APP 2>/dev/null
exit 0
EOF
chmod +x "$WORK/capture_once.sh"

capture_once() {
    local out="$1" attempt
    for attempt in 1 2 3 4 5; do
        rm -f "$RAW"
        if xvfb-run -a -s "-screen 0 $SCREEN" bash "$WORK/capture_once.sh" \
            && [ -s "$RAW" ] \
            && python3 "$WORK/xwd2png.py" "$RAW" "$out"; then
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
