#!/usr/bin/env bash
# P6-M3-T04 / ADR v1.1.0: build the PINNED OCCT 8.0.x dependency from source
# into a cache prefix (fetched/built as a dependency, never vendored). oep-engine-occ
# links against the result; see its build.rs.
#
# Idempotent: skips fetch+configure+build when the installed prefix is already
# present. Modules are trimmed to what the seam needs (ModelingData +
# ModelingAlgorithms + FoundationClasses); no visualization/OCAF/DRAW/DXF, in
# line with the ADR's "no unit-visualization toolkits by default".
#
# Env:
#   OEP_OCCT_VERSION  pinned OCCT release (default 8.0.1)
#   OEP_OCCT_ROOT     cache dir (default "${HOME}/.cache/oep-occt")
#   OEP_OCCT_JOBS     parallel build jobs (default: nproc)
#   OEP_OCCT_SHARED   ON builds OCCT as shared libs (system-link style);
#                     OFF (default) builds STATIC archives, so oep-engine-occ
#                     links every consumer hermetic (no runtime LD_LIBRARY_PATH
#                     / rpath dependence anywhere in the workspace).
#
# Prints the install prefix on success.

set -euo pipefail

OEP_OCCT_VERSION="${OEP_OCCT_VERSION:-8.0.1}"
OEP_OCCT_ROOT="${OEP_OCCT_ROOT:-${HOME}/.cache/oep-occt}"
JOBS="${OEP_OCCT_JOBS:-$(nproc)}"
SHARED="${OEP_OCCT_SHARED:-OFF}"
# OCCT gates static vs shared with its own BUILD_LIBRARY_TYPE, not the generic
# BUILD_SHARED_LIBS switch.
if [[ "${SHARED}" == "ON" ]]; then
  LIB_TYPE="Shared"
else
  LIB_TYPE="Static"
fi

# Same location the Rust build.rs looks in.
PREFIX="${OEP_OCCT_ROOT}/${OEP_OCCT_VERSION}/install"

if [[ -f "${PREFIX}/lib/cmake/opencascade/OpenCASCADEConfig.cmake" ]]; then
  echo "occt: ${OEP_OCCT_VERSION} already installed at ${PREFIX}" >&2
  echo "${PREFIX}"
  exit 0
fi

SRC_DIR="$(bash "$(dirname "$0")/fetch-occt.sh")"

BUILD_DIR="${OEP_OCCT_ROOT}/${OEP_OCCT_VERSION}/build"
cmake -S "${SRC_DIR}" -B "${BUILD_DIR}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="${PREFIX}" \
  -DBUILD_LIBRARY_TYPE="${LIB_TYPE}" \
  -DBUILD_MODULE_ApplicationFramework=OFF \
  -DBUILD_MODULE_DataExchange=OFF \
  -DBUILD_MODULE_Draw=OFF \
  -DBUILD_MODULE_ModelingAlgorithms=ON \
  -DBUILD_MODULE_ModelingData=ON \
  -DBUILD_MODULE_Visualization=OFF \
  -DBUILD_DOC_Overview=OFF \
  -DBUILD_SAMPLES_GLES=OFF \
  -DBUILD_SAMPLES_QT=OFF \
  -DBUILD_SAMPLES_MFC=OFF \
  -DBUILD_TESTING=OFF \
  -DUSE_TBB=OFF \
  -DUSE_VTK=OFF \
  -DUSE_FREEIMAGE=OFF \
  -DUSE_RAPIDJSON=OFF \
  -DUSE_GLES2=OFF \
  1>&2

cmake --build "${BUILD_DIR}" -j "${JOBS}" 1>&2
cmake --install "${BUILD_DIR}" 1>&2

echo "occt: installed ${OEP_OCCT_VERSION} at ${PREFIX}" >&2
echo "${PREFIX}"
