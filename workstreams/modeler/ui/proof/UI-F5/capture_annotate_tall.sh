#!/usr/bin/env bash
# UI-F5 wave-F close: deterministic real-binary screenshot of the ANNOTATE
# surface at a TALLER 1280x1200 native window (same precedent as the wave-E
# mate capture at 1280x1500): the annotation RECORD ROWS render below the
# drafting scroll area's clip at the shipped 1280x800 window (the export
# section starts immediately below them, so no scroll position at 800 tall
# reveals them). At 1200 tall the drafting panel is taller and the full
# annotate section - title block, Edit part name, placed dimension with its
# value=80 measured=80 geometry_w=80 readout, Add leader / Add note / Clear
# annotations, and BOTH restored records - fits in one frame.
#
# Gesture order (tall window scroll 10, coordinates from a same-window OCR
# probe of the merged head):
#   1) Add leader   (1015, 868)
#   2) Add note     (1080, 868)
#   3) Place dimension (front width) (1040, 813)  -> value readout appears
# Then park neutral and capture. Two independent launches must produce
# byte-identical PNGs; the .sha256 is the determinism oracle.
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-F5"
INJ="$(gcc -O2 -o /tmp/ui_f5_inject "$P/ui_f5_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_f5_inject)"
RZ="$(gcc -O2 -o /tmp/ui_f5_resize "$P/ui_f5_resize.c" -lX11 && echo /tmp/ui_f5_resize)"
[ -n "$INJ" ] && [ -n "$RZ" ] || { echo "capture: tool build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1300x1220x24"; W=1280; H=1200
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"
OUT="workstreams/modeler/ui/proof/UI-F5/annotate.png"

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
    SW, SH = u32(16), u32(20)
    bpp = u32(44); bpl = u32(48); nc = u32(76)
    assert (pf, pd) == (2, 24), f"unexpected pixmap {pf} {pd}"
    assert SW >= w and SH >= h, f"screen {SW}x{SH} too small for {w}x{h}"
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
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]))
PY

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
/tmp/ui_f5_resize "\$DISP" 1280 1200 || exit 1
sleep 1
"\$INJ" "\$DISP" move 1200 950 || exit 1
"\$INJ" "\$DISP" wheel 1200 950 down 10 || exit 1
sleep 1
"\$INJ" "\$DISP" click 1015 868 || exit 1   # Add leader
sleep 1
"\$INJ" "\$DISP" click 1080 868 || exit 1   # Add note
sleep 1
"\$INJ" "\$DISP" click 1040 813 || exit 1   # Place dimension
sleep 1
"\$INJ" "\$DISP" move 640 1160 || exit 1
sleep 1
xwd -root -out "\$RAW" || exit 1
kill \$APP 2>/dev/null
wait \$APP 2>/dev/null
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
    echo "capture(annotate-tall): nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/annotate.png.sha256"
echo "capture(annotate-tall): ok"
echo "  $OUT"
echo "  sha256 $HA"
