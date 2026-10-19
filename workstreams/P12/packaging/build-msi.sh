#!/usr/bin/env bash
# P12-T04 Windows MSI builder (build-only on U1; no Windows runtime needed).
# Usage: bash workstreams/P12/packaging/build-msi.sh <binary> [<triple>]
#
#   <binary>  workspace bin name, e.g. oep-pdm
#   <triple>  windows rust target, default x86_64-pc-windows-gnu (garage PC .43)
#
# 1. Cross-compiles <binary> to a Windows GNU target (mingw; this is the same
#    hand-rolled chain from the gitea skill, proven for bastion/stronghold).
# 2. Writes a wixl-compatible WiX .wxs (Product/Package/Media/Directory/
#    Component/File/Feature only -- wixl does NOT support Shortcut, Registry,
#    or Platform attributes, so the start-menu shortcut is a documented
#    post-install step).
# 3. Builds the .msi with wixl inside a hermetic debian container so nothing
#    is installed on the host and the build is reproducible.
#
# Clean-room: the MSI contains only this repo's own cross-compiled binary +
#    a minimal WiX install tree. No vendor assets.

set -euo pipefail

BIN="${1:?usage: build-msi.sh <binary> [<triple>]}"
TRIPLE="${2:-x86_64-pc-windows-gnu}"

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="${OUT_DIR:-$PWD}"
VERSION="${OEP_VERSION:-0.1.0}"

# --- 1. cross-compile ---------------------------------------------------------
EXE="$REPO/target/$TRIPLE/release/$BIN.exe"
if [[ ! -x "$EXE" ]]; then
  echo "cross-compiling $BIN for $TRIPLE ..." >&2
  (cd "$REPO" && cargo build --release --offline --target "$TRIPLE" --bin "$BIN") >&2
fi
test -x "$EXE" || { echo "no output: $EXE" >&2; exit 1; }

# --- 2. stage exe next to the wxs so wixl (in /out) can resolve it ------------
mkdir -p "$OUT"
STAGED_EXE="$OUT/$BIN.exe"
cp -f "$EXE" "$STAGED_EXE"

# --- 3. WiX source (wixl-compatible subset) -----------------------------------
WXS="$OUT/keelworks-$BIN.wxs"
cat > "$WXS" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Keelworks OEP $BIN"
           Language="1033" Version="$VERSION"
           Manufacturer="Keelworks">
    <Package Description="Keelworks OEP $BIN"
             Comments="OEP $BIN"
             InstallerVersion="200" Compressed="yes"/>
    <Media Id="1" Cabinet="oep.cab" EmbedCab="yes"/>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLLOCATION" Name="Keelworks">
          <Directory Id="BINDIR" Name="bin">
            <Component Id="main.exe" Guid="*">
              <File Id="oep.exe" Name="$BIN.exe"
                    Source="$BIN.exe" KeyPath="yes"/>
            </Component>
          </Directory>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="Complete" Title="Keelworks OEP $BIN" Level="1">
      <ComponentRef Id="main.exe"/>
    </Feature>
  </Product>
</Wix>
EOF

# --- 4. build the .msi (hermetic debian container + wixl) ----------------------
MI_IMAGE="${MI_IMAGE:-debian:bookworm-slim}"
MSI="$OUT/keelworks-$BIN-$TRIPLE-$VERSION.msi"
docker run --rm -v "$OUT:/out" "$MI_IMAGE" bash -c "
set -e
apt-get update -qq >/dev/null && apt-get install -y -qq wixl >/dev/null 2>&1
cd /out
wixl -o '$(basename "$MSI")' '$(basename "$WXS")'
ls -la '$(basename "$MSI")'
" > "$OUT/$BIN-msi-build.log" 2>&1
test -s "$MSI" || { echo "msi build failed (see $OUT/$BIN-msi-build.log)" >&2; exit 1; }

echo "$MSI"
sha256sum "$MSI"
