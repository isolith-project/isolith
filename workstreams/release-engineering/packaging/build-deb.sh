#!/usr/bin/env bash
# Isolith OEP .deb builder (Linux desktop package, alternate path).
# Usage: bash workstreams/release-engineering/packaging/build-deb.sh <binary>...
#
# Builds a Debian .deb from release binaries in the workspace. Unlike the
# AppImage recipe (single-file, per-binary) this recipe packages the desktop
# download set as one installable package: the editor shell plus the CLI
# tools and the update checker, installed to /usr/bin.
#
# Clean-room: only this repo's own release binaries + a generated
# control/desktop file/icon go in. No vendor assets.
#
# Determinism: the .deb archive embeds file mtimes, so two builds of the
# same commit are NOT byte-identical (reported honestly in the
# reproducible-build comparison). Content hashes of the payload binaries
# ARE stable.
#
# Requires: dpkg-deb (present on Ubuntu/Debian hosts).

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="${OUT_DIR:-$PWD}"
VERSION="${OEP_VERSION:-0.1.0}"
PKG="isolith-oep"
ARCH="$(dpkg --print-architecture 2>/dev/null || uname -m)"

[ $# -gt 0 ] || { echo "usage: build-deb.sh <binary>..." >&2; exit 1; }

STAGE="$PWD/.deb-stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" \
         "$STAGE/usr/share/applications" \
         "$STAGE/usr/share/icons/hicolor/256x256/apps"

# --- control file -------------------------------------------------------
CONTROL_SIZE_ALL="$(du -sk "$STAGE/usr" 2>/dev/null | cut -f1)"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: ${PKG}
Version: ${VERSION}
Section: engineering
Priority: optional
Architecture: ${ARCH}
Maintainer: Isolith contributors <isolith@localhost>
Description: Isolith Open Engineering Platform desktop tools
 The OEP editor shell plus interchange CLI tools and the advisory
 update checker. Geometry interchange for CGR/3DXML/STEP with a
 sketch-driven editor and a canonical oep-geo document model.
EOF

# --- payload binaries ---------------------------------------------------
for BIN in "$@"; do
  SRC="$REPO/target/release/$BIN"
  if [ ! -x "$SRC" ]; then
    echo "building $BIN (release) ..." >&2
    (cd "$REPO" && cargo build --release --offline --bin "$BIN") >&2
  fi
  test -x "$SRC" || { echo "no release binary: $SRC" >&2; exit 1; }
  cp "$SRC" "$STAGE/usr/bin/$BIN"
done

# keep a stable shell alias too
cp "$REPO/target/release/oep-workbench" "$STAGE/usr/bin/oep-workbench" 2>/dev/null || true

# --- desktop entry (editor shell only) -----------------------------------
cat > "$STAGE/usr/share/applications/isolith-oep.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Isolith OEP
Comment=Isolith Open Engineering Platform - editor shell
Exec=oep-workbench
Icon=isolith-oep
Terminal=false
Categories=Engineering;Graphics;Office;
StartupNotify=true
EOF

# --- icon: self-contained generated PNG (same as the AppImage recipe) ----
python3 - "$STAGE/usr/share/icons/hicolor/256x256/apps/isolith-oep.png" <<'PY'
import sys
from PIL import Image, ImageDraw
img = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle([0, 0, 255, 255], radius=40, fill=(26, 43, 69, 255))
d.line([(56, 192), (128, 56), (200, 192)], fill=(127, 180, 255, 255), width=12, joint="curve")
d.line([(88, 156), (168, 156)], fill=(159, 227, 194, 255), width=12)
d.ellipse([121, 103, 135, 117], fill=(255, 224, 138, 255))
img.save(sys.argv[1])
PY

# --- build the archive ---------------------------------------------------
REPLACING_SIZE="$(du -sk "$STAGE" | cut -f1)"
sed -i "s/^Installed-Size:.*/Installed-Size: ${REPLACING_SIZE}/" "$STAGE/DEBIAN/control"
mkdir -p "$OUT"
DEB="$OUT/isolith-oep-${VERSION}-${ARCH}.deb"
rm -f "$DEB"
chmod -R u+rw  "$STAGE"
dpkg-deb --build --root-owner-group "$STAGE" "$DEB" >/dev/null
rm -rf "$STAGE"

echo "$DEB"
sha256sum "$DEB"
