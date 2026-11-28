#!/usr/bin/env bash
# UI-E5 wave-E: deterministic real-binary screenshots of the assembly lane.
#
#   1) assembly-tree : default merge-head launch at the shipped 1280x800
#      window. The model-tree column shows the model tree and below it the
#      assembly tree (base/cap rows, the Insert/Ground/Isolate toolbar) with
#      the diagnostics section. Two independent launches must produce
#      byte-identical PNGs.
#   2) mate          : click base row -> Ground -> "+ Add mate". The default
#      coincident mate (A cap:origin == B base:top) solves the cap onto the
#      base top; the Mates section shows the pinned report line
#      `solve dof=3 moved=1 max_dist=0.000e0` and the added mate row
#      `coincident cap:origin == base:top`. The Mates section sits below the
#      fold at the shipped 1280x800 window (the fixed-height assembly region
#      clips it), so this mode drives the SAME real binary at a 1280x1500
#      native window - the assembly region is height-proportional (UI-A4), and
#      1500 reveals the full Mates section. Coordinates are the probe values
#      scaled to the taller region.
#   3) interference  : the SAME mated state at the shipped 1280x800 window,
#      diagnostics section live clash readout: the real
#      `oep_analysis::clash::detect_in_assembly` reports the genuine
#      `1 pair, base x cap (overlap 2902.0 mm3 ...)` - a real overlap created
#      by the coincident mate, never faked.
#
# All coordinates are logical points == pixels at the Xvfb screen with the
# app window mapped at the origin (no WM, NorthWest), from the wave-E kittest
# geometry probe of the SAME merged head.
#
# Honest explode note (recorded in PROOF.md, not captured): the explode panel
# is shipped (UI-E4) and test-proven (--test assy_explode 142/0) but renders
# BELOW the fixed-height assembly region's clip in the real 1280x800 binary -
# the mates ScrollArea consumes the region's available height, so no scroll or
# window resize (verified at 1800 and 2400 tall) reveals it. No fake frame.
#
# Usage: bash capture_e5.sh <assembly-tree|mate|interference>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-E5"
MODE="${1:?usage: capture_e5.sh <assembly-tree|mate|interference>}"
INJ="$(gcc -O2 -o /tmp/ui_e5_inject "$P/ui_e5_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_e5_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"

cat > "$WORK/xwd2png.py" <<'PY'
import struct, sys
import PIL.Image
def main(src, dst, w, h):
    data = open(src, "rb").read()
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

case "$MODE" in
  assembly-tree)
    SCREEN="1600x1000x24"; W=1600; H=1000
    RESIZE=""      # shipped size, window at origin
    GESTURES=() ;;
  mate)
    SCREEN="1600x1600x24"; W=1600; H=1600
    RESIZE="1280 1500"
    # region-top at H=800 is 418; at H=1500 is 782; content offsets scale.
    GESTURES=(
      "click 85 824"
      "click 163 942"
      "click 41 1145"
    ) ;;
  interference)
    SCREEN="1600x1000x24"; W=1600; H=1000
    RESIZE=""
    GESTURES=(
      "click 85 460"
      "click 163 578"
      "click 41 781"
    ) ;;
  *) echo "usage: capture_e5.sh <assembly-tree|mate|interference>" >&2; exit 2 ;;
esac

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"; RESIZE="$RESIZE"
"\$BIN" &
APP=\$!
found=0
for i in \$(seq 1 140); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
if [ -n "\$RESIZE" ]; then
  /tmp/ui_e5_resize "\$DISPLAY" \$RESIZE
  sleep 1
fi
EOF
for g in "${GESTURES[@]}"; do
  printf '  "%s" "$DISPLAY" %s\n' "$INJ" "$g" >> "$WORK/capture_once.sh"
done
cat >> "$WORK/capture_once.sh" <<'EOF'
sleep 1
xwd -root -out "$RAW" || { echo 'capture: xwd failed' >&2; kill "$APP" 2>/dev/null; exit 1; }
kill "$APP" 2>/dev/null
wait "$APP" 2>/dev/null
exit 0
EOF
chmod +x "$WORK/capture_once.sh"

capture_once() {
    local out="$1" attempt
    for attempt in 1 2 3 4 5; do
        rm -f "$RAW"
        if xvfb-run -a -s "-screen 0 $SCREEN" bash "$WORK/capture_once.sh" \
            && [ -s "$RAW" ] \
            && python3 "$WORK/xwd2png.py" "$RAW" "$out" $W $H; then
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
