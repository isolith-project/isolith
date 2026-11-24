#!/usr/bin/env bash
# UI-C5: deterministic screenshot of the real Isolith binary for the
# UI-MEASURE proof - press M (open the measure surface), pick point p0 then
# point p1, and capture the annotated measurement on the viewport: the value
# label "distance point p0 -> point p1 = 40.000 mm" plus the leader line in
# the preview box.
#
# Same discipline as every wave capture: two independent app launches, each
# driven through XTEST (proof/UI-C5/measure_inject.c), must produce
# byte-identical PNGs; the .sha256 file is the determinism oracle. Click
# coordinates come from the same shell-layout math the panel draws (window
# 1280x800, pixels-per-point 1.0 == X pixels):
#   viewport = (tree 240, middle_top 32) .. (1280-properties 300, 800-status 26)
#           = x 240..980, y 32..774
#   measure card: x = 240+8 = 248, w = 218, card_h = 236
#   y = 774 - 236 - 8 = 530
#   row i: x + 108 = 356, w-120 = 98, ry = 530 + 32 + i*20  (row 0 = 562)
#   row centre = (356 + 49, ry + 9)
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-C5"
INJ="$(gcc -O2 -o /tmp/ui_c5_inject "$P/measure_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_c5_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"
# kittest/shell-layout-probed coordinates (1280x800 default layout, ppp=1.0).
# The central panel inner margin shifts the region up by 8px, so the card sits
# 8px higher than the raw layout math: row0 centre = 563, row1 = 583.
P0_X=405; P0_Y=563   # "point p0" row centre
P1_X=405; P1_Y=583   # "point p1" row centre
MOVE_X=640; MOVE_Y=760

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
"$INJ" "\$DISP" key m || { echo 'capture: M failed' >&2; kill \$APP 2>/dev/null; exit 1; }
sleep 0.5
"$INJ" "\$DISP" click $P0_X $P0_Y || { echo 'capture: p0 click failed' >&2; kill \$APP 2>/dev/null; exit 1; }
sleep 0.3
"$INJ" "\$DISP" click $P1_X $P1_Y || { echo 'capture: p1 click failed' >&2; kill \$APP 2>/dev/null; exit 1; }
sleep 0.5
"$INJ" "\$DISP" move $MOVE_X $MOVE_Y || { echo 'capture: move failed' >&2; kill \$APP 2>/dev/null; exit 1; }
sleep 0.5
xwd -root -out "$RAW" || { echo 'capture: xwd failed' >&2; kill \$APP 2>/dev/null; exit 1; }
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

mkdir -p "$P"
capture_once "$WORK/a.png"
HA="$(sha256sum "$WORK/a.png" | awk '{print $1}')"
capture_once "$WORK/b.png"
HB="$(sha256sum "$WORK/b.png" | awk '{print $1}')"
if [ "$HA" != "$HB" ]; then
    echo "capture: nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
OUT="$P/measure.png"
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/measure.png.sha256"
echo "capture: ok"
echo "  $P/measure.png"
echo "  sha256 $HA"
