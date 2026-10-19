#!/usr/bin/env bash
# P12-T04 Flatpak build script (Linux desktop alternate path).
# Usage: bash workstreams/P12/packaging/build-flatpak.sh <binary>
#
# Wraps flatpak-builder around the parameterized manifest
# (flatpak/org.keelworks.oep.yml -> org.keelworks.oep.<bin>). Requires
# flatpak + flatpak-builder + a flathub FDO 24.08 runtime on the build host
# (the AppImage recipe is the zero-setup path when those are absent).
#
# Output: <out>/keelworks-oep-<bin>.flatpak (bundle) + a local flatpak repo
# under <out>/flatpak-repo.

set -euo pipefail

BIN="${1:?usage: build-flatpak.sh <binary>}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${OUT_DIR:-$PWD}"
VERSION="${OEP_VERSION:-0.1.0}"
APPID="org.keelworks.oep.$BIN"

command -v flatpak-builder >/dev/null || { echo "flatpak-builder not installed" >&2; exit 2; }

SRC_BIN="$REPO/target/release/$BIN"
test -x "$SRC_BIN" || (cd "$REPO" && cargo build --release --offline --bin "$BIN")

# prep a staging dir with the artifacts the manifest references
STAGE="$PWD/.flatpak-stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$SRC_BIN" "$STAGE/oep-$BIN"
cp "$HERE/oep-icon.svg" "$STAGE/oep-icon.svg"
cat > "$STAGE/oep-$BIN.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=OEP $BIN
Comment=Keelworks Open Engineering Platform - $BIN
Exec=oep-$BIN
Icon=org.keelworks.oep.$BIN
Terminal=false
Categories=Engineering;Graphics;Office;
StartupNotify=true
EOF

# parameterized manifest
sed "s/{BIN}/$BIN/g" "$HERE/flatpak/org.keelworks.oep.yml" > "$STAGE/manifest.yml"

(
  cd "$STAGE"
  flatpak-builder --user --force-clean --repo="$OUT/flatpak-repo" build "$STAGE/manifest.yml"
  flatpak build-bundle "$OUT/flatpak-repo" "$OUT/keelworks-oep-$BIN.flatpak" "$APPID" "$VERSION"
)
rm -rf "$STAGE"

echo "$OUT/keelworks-oep-$BIN.flatpak"
sha256sum "$OUT/keelworks-oep-$BIN.flatpak"
