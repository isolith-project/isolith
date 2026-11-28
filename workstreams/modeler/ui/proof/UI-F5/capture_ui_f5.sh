#!/usr/bin/env bash
# UI-F5 wave-F close: deterministic real-binary screenshots of the drafting
# lane at the shipped 1280x800 window, byte-identical across two independent
# app launches (the .sha256 file is the determinism oracle).
#
# The drafting panel opens the committed bracket drawing on its first frame
# (A4 landscape, third-angle, three views, scale 1:1) and hosts the four
# wave-F surfaces, all reachable in the real binary:
#
#   1) sheet           : the F1 sheet section at the top of the drafting
#                        scroll area - sheet tree (Sheet 1), Add sheet,
#                        size/scale/projection selectors, placement legality.
#                        No gesture needed.
#   2) views           : wheel-scroll the drafting scroll area down (30
#                        notches) to the F2 views tools, click Base / Top /
#                        Section / Detail; the canvas shows the accent base
#                        chip, the aligned projected-top chip, the hatched
#                        section outline and the detail chip.
#   3) annotate        : scroll 30, click Place dimension (front width),
#                        re-clamp, click Add leader + Add note, re-clamp; the
#                        frame shows the part-name editor, the
#                        value=80.000000000 measured=80.000000000
#                        geometry_w=80.000000000 readout and BOTH restored
#                        records (leader oep-leader-1, note oep-note-1).
#   4) export-preview  : scroll 40 to the F4 export section at the bottom
#                        clamp, click Export SVG; the frame shows the print
#                        preview canvas (45 graphical elements), the export
#                        target path and both Export SVG / Export PDF buttons.
#                        The click REALLY writes drawing.svg through the UI
#                        (launched from /tmp/ui-f5-capture like the F4 card),
#                        proving the button is a real file-producing export.
#
# All coordinates are logical points == pixels at the Xvfb screen with the app
# window mapped at the origin (no WM, NorthWest), copied verbatim from the
# UI-F1..UI-F4 capture scripts (probe-calibrated on the same 1280x800 window).
#
# Usage: bash capture_ui_f5.sh <sheet|views|annotate|export-preview>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-F5"
MODE="${1:?usage: capture_ui_f5.sh <sheet|views|annotate|export-preview>}"
INJ="$(gcc -O2 -o /tmp/ui_f5_inject "$P/ui_f5_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_f5_inject)"
[ -n "$INJ" ] || { echo "capture: injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "capture: binary missing: $BIN" >&2; exit 1; }
SCREEN="1280x800x24"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
RAW="$WORK/raw.xwd"
OUTDIR=/tmp/ui-f5-capture

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

NEUTRAL_X=640; NEUTRAL_Y=760
SCROLL_X=1200; SCROLL_Y=700

# F1 capture: the sheet section is at the top of the drafting scroll area; no
# gesture, pointer parked neutral.
# F2 views tools at the drafting scroll-area bottom clamp (probe coordinates
# from the UI-F2 capture): Base / Top / Section / Detail.
VIEWS=( "click 1018 567" "click 1145 567" "click 1154 590" "click 1005 613" )
VIEWS_SCROLL=30
# F3 annotate tools (UI-F3 capture): Place dimension, then Add leader / Add
# note, with a re-clamp after the dimension grows the section.
ANNOTATE1=( "click 1057 693" )
ANNOTATE2=( "click 1003 735" "click 1085 735" )
ANNOTATE_SCROLL=30
# F4 export section at the bottom clamp (UI-F4 capture): Export SVG.
EXPORT=( "click 1004 748" )
EXPORT_SCROLL=40

# LAUNCHDIR=NO -> launch from the repo root; otherwise the dir to launch from
# (export-preview launches from OUTDIR so the UI export resolves there).
case "$MODE" in
  sheet)          GEST1=();            GEST2=();            GEST3=(); SCROLL1=0;      SCROLL2=0;      SCROLL3=0;      LAUNCHDIR=NO ;;
  views)          GEST1=("${VIEWS[@]}"); GEST2=();          GEST3=(); SCROLL1=$VIEWS_SCROLL; SCROLL2=0; SCROLL3=0; LAUNCHDIR=NO ;;
  annotate)       GEST1=("${ANNOTATE1[@]}"); GEST2=("${ANNOTATE2[@]}"); GEST3=(); SCROLL1=$ANNOTATE_SCROLL; SCROLL2=$ANNOTATE_SCROLL; SCROLL3=$ANNOTATE_SCROLL; LAUNCHDIR=NO ;;
  export-preview) GEST1=("${EXPORT[@]}");   GEST2=();       GEST3=(); SCROLL1=$EXPORT_SCROLL; SCROLL2=0;      SCROLL3=0;      LAUNCHDIR=$OUTDIR ;;
  *) echo "usage: capture_ui_f5.sh <sheet|views|annotate|export-preview>" >&2; exit 2 ;;
esac

cat > "$WORK/capture_once.sh" <<EOF
#!/usr/bin/env bash
BIN="$BIN"; INJ="$INJ"; RAW="$RAW"; DISP="\$DISPLAY"
OUTDIR="$OUTDIR"; LAUNCHDIR="$LAUNCHDIR"
SCROLL_X=$SCROLL_X; SCROLL_Y=$SCROLL_Y; NEUTRAL_X=$NEUTRAL_X; NEUTRAL_Y=$NEUTRAL_Y
SCROLL1=$SCROLL1; SCROLL2=$SCROLL2; SCROLL3=$SCROLL3
rm -f "\$OUTDIR"/*.svg "\$OUTDIR"/*.pdf
# Launch the real binary FROM the launch dir (repo root unless export-preview,
# where the panel's default export path "drawing" resolves into \$OUTDIR so
# clicking Export SVG really writes \$OUTDIR/drawing.svg).
launchdir="\$LAUNCHDIR"; [ "\$launchdir" = "NO" ] && launchdir="\$PWD"
(cd "\$launchdir" && "\$BIN") &
APP=\$!
found=0
for i in \$(seq 1 140); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
if [ "\$found" -ne 1 ]; then echo 'capture: window not found' >&2; kill \$APP 2>/dev/null; exit 1; fi
sleep 1
scroll() { local n="\$1"
  if [ "\$n" -gt 0 ]; then
    "\$INJ" "\$DISP" move \$SCROLL_X \$SCROLL_Y || exit 1
    "\$INJ" "\$DISP" wheel \$SCROLL_X \$SCROLL_Y down \$n || exit 1
    sleep 1
  fi
}
EOF
{
  echo 'scroll $SCROLL1'
  for g in "${GEST1[@]}"; do printf '  "%s" "$DISP" %s || exit 1\n' "$INJ" "$g"; done
  echo 'scroll $SCROLL2'
  for g in "${GEST2[@]}"; do printf '  "%s" "$DISP" %s || exit 1\n' "$INJ" "$g"; done
  echo 'scroll $SCROLL3'
  for g in "${GEST3[@]}"; do printf '  "%s" "$DISP" %s || exit 1\n' "$INJ" "$g"; done
  echo '"$INJ" "$DISP" move $NEUTRAL_X $NEUTRAL_Y || exit 1'
  echo 'sleep 1'
  echo 'xwd -root -out "$RAW" || exit 1'
  echo 'kill $APP 2>/dev/null'
  echo 'wait $APP 2>/dev/null'
  echo 'exit 0'
} >> "$WORK/capture_once.sh"
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
if [ -n "$(ls -A "$OUTDIR" 2>/dev/null)" ]; then
  echo "  export target written by the UI:"
  ls -la "$OUTDIR" 2>/dev/null | sed 's/^/    /'
fi
