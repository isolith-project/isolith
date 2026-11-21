#!/usr/bin/env bash
# UI-A8: deterministic screenshots of the real merged-head Isolith binary for
# the three wave-close screenshots (shell, palette, viewport).
#
# Same discipline as shot.sh: two independent app launches must produce
# byte-identical PNGs (the sha256 is the determinism oracle), and each mode is
# driven by the XTEST injector (proof/ui-a8/xtest_inject) so the palette
# screenshot actually has the Ctrl+K palette open and the viewport screenshot
# has a navigated camera - real app renders, not stubs.
#
# Usage: bash capture_ui.sh <shell|palette|viewport>  (writes proof/ui-a8/<mode>.png + .sha256)
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/ui-a8"
MODE="${1:?usage: capture_ui.sh <shell|palette|viewport>}"
INJ="$ROOT/$P/xtest_inject"
[ -x "$INJ" ] || { echo "capture: injector missing, build it: gcc -O2 -o $INJ $P/xtest_inject.c -lX11 -Wl,-l:libXtst.so.6" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"
case "$MODE" in
  shell)    INJMODE="AND" ;;
  palette)  INJMODE="PAL" ;;
  viewport) INJMODE="ORB" ;;
  *) echo "usage: capture_ui.sh <shell|palette|viewport>" >&2; exit 2;;
esac

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
    def u32(o): return struct.unpack_from(he + "I", data, o)[0]
    hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
    w, h, xo, bo = u32(16), u32(20), u32(24), u32(28)
    bpp = u32(44); bpl = u32(48); nc = u32(76)
    assert (pf, pd, w, h) == (2, 24, 1280, 800), f"unexpected pixmap: format={pf} depth={pd} {w}x{h}"
    d0 = hs + nc * 12
    order = "<I" if bo == 0 else ">I"
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

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; INJMODE="$INJMODE"; RAW="$RAW"
"\$BIN" &
APP=\$!
found=0
for i in \$(seq 1 100); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
"\$INJ" "\$DISPLAY" "\$INJMODE" || { echo 'capture: injector failed' >&2; kill \$APP 2>/dev/null; exit 1; }
sleep 1
xwd -root -out "\$RAW" || { echo 'capture: xwd failed' >&2; kill \$APP 2>/dev/null; exit 1; }
kill \$APP 2>/dev/null
wait \$APP 2>/dev/null
exit 0
EOF
chmod +x "$WORK/capture_once.sh"

capture_once() {
    local out="$1" attempt
    for attempt in 1 2 3 4 5; do
        rm -f "$RAW"
        if xvfb-run -a -s "-screen 0 $SCREEN" "bash" "$WORK/capture_once.sh" \
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
