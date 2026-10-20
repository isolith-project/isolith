#!/usr/bin/env bash
# P12-T04 AppImage builder (Linux desktop package).
# Usage: bash workstreams/release-engineering/packaging/build-appimage.sh <binary-name>
#
# Builds a single-file Linux desktop AppImage from any release binary in the
# workspace. Arch is taken from the host (aarch64 -> appimagetool-aarch64,
# x86_64 -> appimagetool-x86_64). Clean-room: only this repo's own binary plus
# a generated launcher/.desktop/icon go into the AppDir; no vendor assets.
#
# FUSE-less hosts (this U1) run the produced AppImage with
# --appimage-extract-and-run; that also acts as the "install" step in the
# acceptance (app stores support both FUSE mount and extract-and-run).

set -euo pipefail

BIN="${1:?usage: build-appimage.sh <binary>}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="${OUT_DIR:-$PWD}"

# --- resolve the release binary ---------------------------------------------
SRC_BIN="$REPO/target/release/$BIN"
if [[ ! -x "$SRC_BIN" ]]; then
  echo "building $BIN (release) ..." >&2
  (cd "$REPO" && cargo build --release --offline --bin "$BIN") >&2
fi

# --- appimagetool ------------------------------------------------------------
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
  aarch64|arm64) AIT_ARCH=aarch64 ;;
  x86_64|amd64)  AIT_ARCH=x86_64 ;;
  *) echo "unhandled host arch $HOST_ARCH" >&2; exit 2 ;;
esac
AIT="$HOME/.cache/keelworks-appimagetool/appimagetool-$AIT_ARCH.AppImage"
if [[ ! -x "$AIT" ]]; then
  mkdir -p "$(dirname "$AIT")"
  echo "fetching appimagetool ($AIT_ARCH) ..." >&2
  curl -fL -o "$AIT" \
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-$AIT_ARCH.AppImage"
  chmod +x "$AIT"
fi

# --- assemble AppDir ---------------------------------------------------------
APP="$PWD/AppDir"
rm -rf "$APP"
mkdir -p "$APP/usr/bin" "$APP/usr/share/applications" "$APP/usr/share/icons/hicolor/scalable/apps"

cp "$SRC_BIN" "$APP/usr/bin/$BIN"

# launcher: exec the binary straight through (desktop wrapper execs it too)
cat > "$APP/AppRun" <<EOF
#!/usr/bin/env sh
exec "\$APPDIR/usr/bin/$BIN" "\$@"
EOF
chmod +x "$APP/AppRun"

# icon: self-contained generated PNG (appimagetool wants .png/.svg/.xpm)
# bundled one per build to keep the recipe standalone.
ICON="$APP/usr/share/icons/hicolor/256x256/apps/keelworks-oep.png"
mkdir -p "$(dirname "$ICON")"
python3 - "$ICON" <<'PYEOF'
import sys
from PIL import Image, ImageDraw
img = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle([0, 0, 255, 255], radius=40, fill=(26, 43, 69, 255))
d.line([(56, 192), (128, 56), (200, 192)], fill=(127, 180, 255, 255), width=12, joint="curve")
d.line([(88, 156), (168, 156)], fill=(159, 227, 194, 255), width=12)
d.ellipse([121, 103, 135, 117], fill=(255, 224, 138, 255))
img.save(sys.argv[1])
PYEOF

# desktop entry (root copy for appimagetool discovery)
printf '[Desktop Entry]\nType=Application\nName=OEP %s\nComment=Keelworks OEP %s\nExec=%s\nIcon=keelworks-oep\nTerminal=false\nCategories=Engineering;Graphics;Office;\n' \
  "$BIN" "$BIN" "$BIN" > "$APP/keelworks-$BIN.desktop"
cp "$APP/keelworks-$BIN.desktop" "$APP/usr/share/applications/keelworks-$BIN.desktop"
touch "$APP/.DirIcon" 2>/dev/null || true
cp "$ICON" "$APP/keelworks-oep.png"

# --- package -----------------------------------------------------------------
mkdir -p "$OUT"
ARCH="${BIN}-keelworks-oep-$(uname -m).AppImage"   # artifact name
rm -f "$OUT/$ARCH"
echo "building $ARCH ..." >&2
"$AIT" --appimage-extract-and-run "$APP" "$OUT/$ARCH" >&2
rm -rf "$APP"

echo "$OUT/$ARCH"
sha256sum "$OUT/$ARCH"
