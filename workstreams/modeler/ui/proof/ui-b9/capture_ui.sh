#!/usr/bin/env bash
# UI-B9: three deterministic screenshots of the real merged-head Isolith binary
# for the wave-B close card - draw, snap, dimension.
#
# Same discipline as every wave capture: two independent app launches per mode,
# each driven through XTEST (proof/ui-b9/ui_b9_inject), must produce
# byte-identical PNGs; the sha256 file is the determinism oracle. The pointer
# sequences below are the exact, kittest-probe-verified coordinates each card
# used (UI-B4 draw, UI-B5 snap-on, UI-B7 dimension), re-run against the real
# merged-head code so the shots show the wave-B union, not per-card branches.
#
# Usage: bash capture_ui.sh <draw|snap|dimension>   (writes proof/ui-b9/<mode>.png + .sha256)
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/ui-b9"
MODE="${1:?usage: capture_ui.sh <draw|snap|dimension>}"
INJ="$(gcc -O2 -o /tmp/ui_b9_inject "$P/ui_b9_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_b9_inject)"
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

# Per-mode gesture; each line is `"INJ" "$DISPLAY" <verb> ...`.
case "$MODE" in
  draw)
    GESTURES=(
      "click 662 408"   # enter XY sketch (UI-B4-proven enter click)
      "key r"           # Rectangle tool
      "click 430 330"   # corner A placed
      "click 650 330"   # corner B
      "click 650 460"   # corner C (closes rectangle preview)
      "move 560 520"    # rubber band from the last corner
    ) ;;
  snap)
    GESTURES=(
      "click 662 408"   # enter XY sketch
      "key l"           # Line tool
      "click 430 330"   # p0
      "click 650 330"   # p1 (commits the line)
      "move 643 321"    # hover near endpoint p1 -> Endpoint inference glyph + label
    ) ;;
  dimension)
    GESTURES=(
      "click 662 408"   # enter XY sketch
      "key l"           # Line tool
      "click 354 345"   # two vertices commit line l0 (length 80)
      "click 850 345"
      "click 1047 100"  # properties "Place dimension" (width = 80, volume 32000)
      "click 1072 134"  # focus the expression field
      "type width*1.5"  # expression text
      "click 1218 134"  # Apply (width = 120, volume 72000)
      "move 556 760"    # pointer off the panel
    ) ;;
  *) echo "usage: capture_ui.sh <draw|snap|dimension>" >&2; exit 2;;
esac

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"
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
  printf '"%s" "$DISPLAY" %s || { echo "capture: %s failed" >&2; kill $APP 2>/dev/null; exit 1; }\n' "$INJ" "$*" "$*" >> "$WORK/capture_once.sh"
done
cat >> "$WORK/capture_once.sh" <<'EOF'
sleep 1
xwd -root -out "$RAW" || { echo 'capture: xwd failed' >&2; kill $APP 2>/dev/null; exit 1; }
kill $APP 2>/dev/null
wait $APP 2>/dev/null
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
