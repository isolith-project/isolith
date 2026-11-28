#!/usr/bin/env bash
# UI-F5 probe: map the drafting scroll area on the MERGED head at the shipped
# 1280x800 window. Launches the real binary on xvfb, wheel-scrolls N notches at
# (1200,700), parks neutral, xwd-captures, then OCRs the drafting-panel crop
# (x 960..1280) with tesseract TSV so section headers and tool labels carry
# screen coordinates. Used to re-calibrate the UI-F5 capture click positions
# (the F2/F3 coordinates belong to the pre-F3/F4 drafting content).
#   Usage: probe_layout.sh <notches>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
N="${1:?usage: probe_layout.sh <notches>}"
INJ=$(gcc -O2 -o /tmp/ui_f5_inject "workstreams/modeler/ui/proof/UI-F5/ui_f5_inject.c" -lX11 -Wl,-l:libXtst.so.6 && echo /tmp/ui_f5_inject)
[ -n "$INJ" ] || exit 1
WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT
cat > "$WORK/probe.sh" <<EOF
"$ROOT/target/debug/isolith" &
APP=\$!
found=0
for i in \$(seq 1 100); do
  if xwininfo -root -tree 2>/dev/null | grep -qi 'Isolith'; then found=1; break; fi
  sleep 0.25
done
[ "\$found" -eq 1 ] || { echo "no window"; kill \$APP 2>/dev/null; exit 1; }
sleep 1
"$INJ" "\$DISPLAY" move 1200 700 || exit 1
[ "$N" -gt 0 ] && "$INJ" "\$DISPLAY" wheel 1200 700 down "$N"
sleep 1
"$INJ" "\$DISPLAY" move 640 760 || exit 1
sleep 1
xwd -root -out "$WORK/raw.xwd" || exit 1
kill \$APP 2>/dev/null; wait \$APP 2>/dev/null
exit 0
EOF
chmod +x "$WORK/probe.sh"
xvfb-run -a -s "-screen 0 1280x800x24" bash "$WORK/probe.sh" || exit 1
python3 - "$WORK/raw.xwd" <<'PY'
import struct, sys
from PIL import Image
data = open(sys.argv[1], 'rb').read()
le = int.from_bytes(data[0:4], "little"); be = int.from_bytes(data[0:4], "big")
he = "<" if 100 <= le <= 1000 else (">" if 100 <= be <= 1000 else None)
def u32(o): return struct.unpack_from(he + "I", data, o)[0]
hs, fv, pf, pd = u32(0), u32(4), u32(8), u32(12)
W, H = u32(16), u32(20); bpp = u32(44); bpl = u32(48); nc = u32(76)
d0 = hs + nc * 12
order = "<I" if u32(28) == 0 else ">I"
rm, gm, bm = u32(56), u32(60), u32(64)
rj = (rm & -rm).bit_length() - 1; gj = (gm & -gm).bit_length() - 1; bj = (bm & -bm).bit_length() - 1
pxs = bpp // 8
img = Image.new("RGB", (W, H)); px = img.load()
for y in range(H):
    base = d0 + y * bpl
    for x in range(W):
        p = struct.unpack_from(order, data, base + x * pxs)[0]
        px[x, y] = ((p & rm) >> rj, (p & gm) >> gj, (p & bm) >> bj)
img.save("/tmp/probe.png")
PY
python3 - /tmp/probe.png "$N" <<'PY'
import subprocess, sys, io
from PIL import Image
img = Image.open(sys.argv[1]).convert('RGB')
crop = img.crop((960, 240, 1280, 800))
crop = crop.resize((crop.width*2, crop.height*2), Image.LANCZOS)
buf = io.BytesIO(); crop.save(buf, 'PNG'); buf.seek(0)
tsv = subprocess.run(['tesseract','stdin','stdout','--psm','6','tsv'],
                     input=buf.read(), capture_output=True).stdout.decode()
print(f"--- probe at {sys.argv[2]} notches (crop x960x240 320x560, doubled) ---")
for line in tsv.splitlines()[1:]:
    f = line.split('\t')
    if len(f) >= 12 and f[11].strip():
        conf = float(f[10] or 0)
        if conf > 40:
            x, y = int(f[6]), int(f[7]); w2, h2 = int(f[8]), int(f[9])
            # map back to full-frame coords
            fx = 960 + x//2 + (w2//2)//2
            fy = 240 + y//2 + (h2//2)//2
            print(f"  ({fx:4d},{fy:4d})  {f[11]}")
PY
