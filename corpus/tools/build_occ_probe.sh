#!/usr/bin/env bash
# Build the independent OCC probe for the P4-M3-T02 golden-diff gate.
#
# The probe wraps OCCT's STEPControl_Reader (the exact STEP importer FreeCAD
# uses). Needs the OCCT development libraries:
#   Debian/Ubuntu:  apt install libocct-data-exchange-dev \
#                           libocct-modeling-algorithms-dev \
#                           libocct-modeling-data-dev libocct-foundation-dev
#   (OCCT >= 7.6 recommended; the probe uses a stable public API surface).
#
# Usage: build_occ_probe.sh [output-path (default: corpus/tools/occ_probe)]
# Output is NOT committed (build artifact); the committed input is the .cpp.
set -eu

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:-$HERE/occ_probe}"
INC=$(pkg-config --cflags occt-data-exchange 2>/dev/null || echo -I/usr/include/opencascade)
LIBS=$(pkg-config --libs occt-data-exchange occt-modeling-algorithms occt-foundation 2>/dev/null \
  || echo -lTKSTEP -lTKSTEPAttr -lTKSTEPBase -lTKXSBase -lTKernel -lTKBRep \
       -lTKTopAlgo -lTKGeomAlgo -lTKGeomBase -lTKG3d -lTKG2d -lTKMesh -lTKBO \
       -lTKPrim -lTKMath)

g++ -O2 -std=c++17 "$HERE/occ_probe.cpp" -o "$OUT" $INC $LIBS
echo "built: $OUT"
"$OUT" 2>/dev/null || true
