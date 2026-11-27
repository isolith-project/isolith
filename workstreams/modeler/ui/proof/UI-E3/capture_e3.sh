#!/usr/bin/env bash
# UI-E3 wave-E: deterministic real-binary screenshots of the diagnostics panel.
#
#   1) dof : open the two-part fixture, select base and Ground it (tree). The
#      diagnostics section (rendered in flow under the tree toolbar, before the
#      mate tool) reports the live degrees of freedom from the solver seam and
#      the interference readout for the committed base+cap placement (none).
#      This proves the readout renders and is reachable in the shipped
#      1280x800 window.
#   2) list: (optional) the same grounded state, captured to the committed
#      PNG set that the E5 integrate card will refresh on the merged head.
#
# All coordinates are logical points == pixels at the 1600x1000 Xvfb screen
# with the 1280x800 app window mapped at the origin (no WM, NorthWest), and
# were derived from a kittest geometry probe of the SAME code at the same
# window size (base row center (85,459), Ground center (163,577) - both
# mid-window, below the tree rows and inside the diagnostics section's
# column).
#
# Usage: bash capture_e3.sh <dof>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-E3"
MODE="${1:?usage: capture_e3.sh <dof>}"
INJ="$(gcc -O2 -o /tmp/ui_e3_inject "$P/ui_e3_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_e3_inject)"
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

# Probe-derived click targets (logical == pixel at this screen, window at origin).
# base row (85,459) selects base; Ground (163,577) grounds it; the diagnostics
# section below then reports the tree-level dof + interference readout.
case "$MODE" in
  dof)
    GESTURES=(
      "click 85 459"
      "click 163 577"
    ) ;;
  *) echo "usage: capture_e3.sh <dof>" >&2; exit 2 ;;
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
