#!/usr/bin/env bash
# UI-F4 wave-F: deterministic real-binary screenshot of the export tools at the
# default 1280x800 window, byte-identical across two independent app launches.
#
# The drafting panel opens the committed bracket drawing on its first frame
# (A4 landscape, third-angle). The Export section is the LAST section of the
# drafting scroll area (below the F1 sheet controls, F2 views tools and F3
# annotate tools), so the capture (1) wheel-scrolls the drafting scroll area to
# its bottom clamp and (2) clicks Export SVG at its post-scroll screen
# coordinates (derived from a same-code kittest probe at the real 1280x800
# window). The result shows the print preview rendered from the live sheet
# (45 graphical elements - the same content the writers serialize), the export
# target path, the Export SVG / Export PDF buttons and the real status line
# after the click (exported ... drawing.svg ... 3158 bytes ... hash ...). The
# pointer is then parked neutral and the frame is captured. Two independent
# launches must produce byte-identical PNGs; the .sha256 file is the
# determinism oracle.
#
# The export writes the file through the real UI path: /tmp/ui-f4-capture/
# (created and cleaned here), so the click is a genuine file-producing export,
# matching the kittest acceptance exactly.
#
# Usage: bash capture_f4.sh
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-F4"
INJ="$(gcc -O2 -o /tmp/ui_f4_inject "$P/ui_f4_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_f4_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"
OUTDIR=/tmp/ui-f4-capture
mkdir -p "$OUTDIR"

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
# Post-scroll centres for the export buttons at the drafting scroll-area bottom
# clamp, derived from a same-code kittest probe at the real 1280x800 window.
SVG_X=1004; SVG_Y=748          # Export SVG
SCROLL_X=1200; SCROLL_Y=700; SCROLL_NOTCHES=40

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"; OUTDIR="$OUTDIR"
SVG_X=$SVG_X; SVG_Y=$SVG_Y; SCROLL_X=$SCROLL_X; SCROLL_Y=$SCROLL_Y
SCROLL_NOTCHES=$SCROLL_NOTCHES; NEUTRAL_X=$NEUTRAL_X; NEUTRAL_Y=$NEUTRAL_Y
DISP="\$DISPLAY"
rm -f "\$OUTDIR"/*.svg "\$OUTDIR"/*.pdf
# Launch the real binary FROM the target dir: the panel's default export path
# ("drawing") then resolves to \$OUTDIR, so clicking Export SVG really writes
# \$OUTDIR/drawing.svg - the file proves the UI click produced the export.
(cd "\$OUTDIR" && "\$BIN") &
APP=\$!
found=0
for i in \$(seq 1 140); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
# 1) wheel-scroll the drafting scroll area down to its bottom clamp so the
#    export section (below sheet/views/annotate) becomes visible.
"\$INJ" "\$DISP" move \$SCROLL_X \$SCROLL_Y || exit 1
"\$INJ" "\$DISP" wheel \$SCROLL_X \$SCROLL_Y down \$SCROLL_NOTCHES || exit 1
sleep 1
# 2) click Export SVG: the panel renders its own status line ("exported ...")
#    and writes \$OUTDIR/drawing.svg through the real UI path.
"\$INJ" "\$DISP" click \$SVG_X \$SVG_Y || exit 1
sleep 1
# 3) park the pointer neutral and capture.
"\$INJ" "\$DISP" move \$NEUTRAL_X \$NEUTRAL_Y || exit 1
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
    echo "capture(ui-f4): nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
OUT="$P/export.png"
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/export.png.sha256"
echo "capture(ui-f4): ok"
echo "  $P/export.png"
echo "  sha256 $HA"
echo "  export target written by the UI:"
ls -la "$OUTDIR" 2>/dev/null | sed 's/^/  /'
