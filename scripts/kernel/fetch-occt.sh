#!/usr/bin/env bash
# P6-M3-T04 / ADR v1.1.0 guardrail: fetch a PINNED OCCT source tarball, verify
# its SHA-256, and extract it -- this is the "fetched/built as a dependency"
# path. The repo never vendors OCCT source (ADR section 9, enforced by CI
# scripts/gate/no-vendored-occt.sh).
#
# No floating "latest": the version/tag/hash below are the pinned 8.0.x line
# (V8_0_1, maintenance release 2026-07-30). Bumping OCCT is a deliberate,
# reviewed change: bump OCCT_VERSION + OCCT_SHA256 together and record it in
# the oep-engine-occ build manifest.
#
# Env:
#   OEP_OCCT_VERSION  pinned OCCT release (default 8.0.1)
#   OEP_OCCT_ROOT     cache dir (default "${HOME}/.cache/oep-occt")
#
# Prints the extracted source dir on success.

set -euo pipefail

OEP_OCCT_VERSION="${OEP_OCCT_VERSION:-8.0.1}"
OEP_OCCT_ROOT="${OEP_OCCT_ROOT:-${HOME}/.cache/oep-occt}"
OCCT_TAG="V${OEP_OCCT_VERSION//./_}"
case "${OEP_OCCT_VERSION}" in
  8.0.1)
    OCCT_SHA256="0d6913eae4bcc09a3653ceced6dda1aec11c35a1513d4c06762c9b002092c68a"
    ;;
  *)
    echo "FATAL: unhandled pinned OCCT version '${OEP_OCCT_VERSION}' -- add its SHA-256 here." >&2
    exit 2
    ;;
esac

CACHE_DIR="${OEP_OCCT_ROOT}/${OEP_OCCT_VERSION}"
SRC_DIR="${CACHE_DIR}/src"
TBALL="${OEP_OCCT_ROOT}/OCCT-${OEP_OCCT_VERSION}.tar.gz"
URL="https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/${OCCT_TAG}.tar.gz"

mkdir -p "${CACHE_DIR}"

if [[ -d "${SRC_DIR}/.git" || -f "${SRC_DIR}/CMakeLists.txt" ]] && [[ -f "${SRC_DIR}/.oep-fetched" ]]; then
  echo "occt: source already present at ${SRC_DIR}" >&2
  echo "${SRC_DIR}"
  exit 0
fi

if [[ ! -f "${TBALL}" ]]; then
  echo "occt: downloading ${URL}" >&2
  curl -sSL -o "${TBALL}" "${URL}"
fi

echo "occt: verifying SHA-256 of ${TBALL}" >&2
echo "${OCCT_SHA256}  ${TBALL}" | sha256sum -c - >&2

echo "occt: extracting to ${SRC_DIR}" >&2
rm -rf "${SRC_DIR}"
mkdir -p "${SRC_DIR}"
tar -xzf "${TBALL}" -C "${CACHE_DIR}"
# GitHub source archives use "OCCT-<version>" with underscores (OCCT-8_0_1),
# regardless of the underscore tag name; take whatever OCCT-* appeared.
EXT=$(find "${CACHE_DIR}" -maxdepth 1 -name 'OCCT-*' -type d | head -n1)
if [[ -z "${EXT}" ]]; then
  echo "FATAL: no OCCT source directory found after extraction" >&2
  exit 1
fi
mv "${EXT}"/* "${SRC_DIR}"/ 2>/dev/null || true
mv "${EXT}"/.[!.]* "${SRC_DIR}"/ 2>/dev/null || true
rm -rf "${EXT}"
touch "${SRC_DIR}/.oep-fetched"

echo "occt: pinned ${OEP_OCCT_VERSION} ready at ${SRC_DIR}" >&2
echo "${SRC_DIR}"
