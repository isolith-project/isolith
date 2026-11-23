#!/usr/bin/env bash
# UI-B6: deterministic screenshot of the real Isolith binary showing the
# constraint toolbar with the honest solver-state readout. Same discipline as
# the UI-B3/B4 captures: two independent Xvfb launches, each driven through
# XTEST (proof/UI-B6/ui_b6_inject), must produce byte-identical PNGs - the
# sha256 file is the determinism oracle.
#
# Sequence (pixels derived from the app's own math via the capture probe):
#   click (662,408)                -> enter XY sketch (B4-proven enter click)
#   key 'l'                        -> Line tool
#   click (653,369) (909,463)      -> two vertices commit line l0 (uv 10,8->50,8)
#   click (734,52)                 -> toolbar H: constrain horizontal
#   move  (556,520)                -> leave the pointer off toggles
#
# Usage: bash capture_constraints.sh   (writes proof/UI-B6/constraints.png + .sha256)
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
P="workstreams/modeler/ui/proof/UI-B6"
INJ="$(gcc -O2 -o /tmp/ui_b6_inject "$P/ui_b6_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_b6_inject)"
[ -n "$INJ" ] || { echo "injector build failed" >&2; exit 1; }
BIN="$ROOT/target/debug/isolith"
[ -x "$BIN" ] || { echo "binary missing: $BIN" >&2; exit 1; }
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
"\$INJ" "\$DISPLAY" click 662 408 || { echo 'capture: enter-sketch click failed' >&2; kill \$APP 2>/dev/null; exit 1; }
"\$INJ" "\$DISPLAY" key l        || { echo 'capture: key l failed' >&2; kill \$APP 2>/dev/null; exit 1; }
"\$INJ" "\$DISPLAY" click 653 369 || { echo 'capture: vertex A failed' >&2; kill \$APP 2>/dev/null; exit 1; }
"\$INJ" "\$DISPLAY" click 909 463 || { echo 'capture: vertex B failed' >&2; kill \$APP 2>/dev/null; exit 1; }
"\$INJ" "\$DISPLAY" click 734 52  || { echo 'capture: toolbar H failed' >&2; kill \$APP 2>/dev/null; exit 1; }
"\$INJ" "\$DISPLAY" move 556 520   || { echo 'capture: pointer move failed' >&2; kill \$APP 2>/dev/null; exit 1; }
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
    echo "capture(constraints): nondeterminism: two captures differ
  A: $HA
  B: $HB" >&2
    exit 1
fi
OUT="$P/constraints.png"
cp "$WORK/a.png" "$OUT"
printf '%s\n' "$HA" > "$P/constraints.png.sha256"
echo "capture(constraints): ok"
echo "  $P/constraints.png"
echo "  sha256 $HA"
