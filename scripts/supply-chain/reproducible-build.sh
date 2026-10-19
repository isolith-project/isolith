#!/usr/bin/env bash
# Keelworks P11-T06: clean-room reproducible release build + artifact manifest.
#
# Builds the full Rust workspace release profile twice, each into its own
# pristine CARGO_TARGET_DIR (no incremental reuse, no shared cache), under a
# deterministic environment (fixed SOURCE_DATE_EPOCH, incremental off, source
# paths remapped), then compares the sha256 of every produced binary. Exit 0
# only if the two builds are byte-identical for every artifact (or if a
# documented tolerated-nondeterminism path explicitly exempts it). On success
# it records workstreams/P11/proof/reproducible-build-<ver>.{json,md} plus the artifact
# SHA256SUMS manifest that the P11-T04 release-signing step consumes.
#
# Usage:  scripts/supply-chain/reproducible-build.sh [version] [source-dir]
#   version   release identity recorded in the proof (default: 2027.1.0-rc1)
#   source-dir  workspace root to build (default: repo root)
# Env: SKIP_REPRO_BUILD=1 -> only emit the manifest from existing target dir A.
#      TARGET_A / TARGET_B override the two build dirs (default /tmp).
set -euo pipefail

VERSION="${1:-2027.1.0-rc1}"
SRC="$(cd "${2:-$(pwd)}" && pwd)"
STAMP="${SOURCE_DATE_EPOCH:-1700000000}"   # fixed epoch for deterministic envs

SUMMARY="workstreams/P11/proof/reproducible-build-${VERSION}"
mkdir -p "$SRC/workstreams/P11/proof"
T_A="${TARGET_A:-/tmp/keel-cleanroom-a}"
T_B="${TARGET_B:-/tmp/keel-cleanroom-b}"
PROBE_HOME="${PROBE_HOME:-/tmp/keel-cargo-probe-home}"

echo "== P11-T06 reproducible build: $VERSION (source: $SRC) =="
cd "$SRC"
# Cargo may not be on PATH in a bare container shell (rust:1-slim puts it in
# /usr/local/cargo/bin and rustup in /root/.cargo/bin); the act runner does set
# it, but resolve here so the script is also runnable by hand in the image.
if ! command -v cargo >/dev/null 2>&1; then
  for d in /usr/local/cargo/bin /root/.cargo/bin "$HOME/.cargo/bin"; do
    [ -x "$d/cargo" ] && export PATH="$d:$PATH" && break
  done
fi
command -v cargo >/dev/null 2>&1 || { echo "FATAL: cargo not found" >&2; exit 127; }

det_build() { # $1 = target dir  $2 = label
  local tgt="$1" label="$2"
  rm -rf "$tgt"
  mkdir -p "$tgt"
  echo "  build $label -> $tgt"
  env HOME="$PROBE_HOME" \
      SOURCE_DATE_EPOCH="$STAMP" \
      CARGO_INCREMENTAL=0 \
      CARGO_TARGET_DIR="$tgt" \
      RUSTFLAGS="--remap-path-prefix=$SRC=." \
      cargo build --workspace --release --locked 2>&1 | tail -2
}

det_build "$T_A" "A"
det_build "$T_B" "B"

# Collect every regular executable file directly under each release dir.
collect() { # $1 = target dir
  find "$1/release" -maxdepth 1 -type f -perm -u+x -printf "%f\n" \
    | grep -v -E '\.(d|so|dylib|dll)$' | sort
}

names_a="$(collect "$T_A")"
names_b="$(collect "$T_B")"

echo "== artifact set (A) =="
echo "$names_a"

if [ "$names_a" != "$names_b" ]; then
  echo "FATAL: artifact set differs between A and B" >&2
  diff <(echo "$names_a") <(echo "$names_b") >&2 || true
  exit 1
fi

ROWS=""
HDR="file|sha256_a|sha256_b|identical"
MISMATCH=0
while read -r bin; do
  [ -z "$bin" ] && continue
  ha="$(sha256sum "$T_A/release/$bin" | awk '{print $1}')"
  hb="$(sha256sum "$T_B/release/$bin" | awk '{print $1}')"
  ident="yes"; [ "$ha" != "$hb" ] && ident="NO"
  [ "$ident" != "yes" ] && MISMATCH=1
  ROWS="$ROWS
$bin|$ha|$hb|$ident"
done <<< "$names_a"
ROWS="$(printf '%s\n' "$ROWS" | sed '/^$/d')"
N_ART="$(printf '%s\n' "$ROWS" | wc -l)"

echo "== byte-identity result =="
[ "$MISMATCH" -eq 0 ] && echo "REPRODUCIBLE: $N_ART artifacts byte-identical" \
                     || echo "NON-DETERMINISTIC: differences found"
# plain tab-aligned report (no `column` dependency; rust:1-slim lacks it)
printf '%s\n' "$ROWS" | while IFS='|' read -r b ha hb id; do
  printf '%-20s  %s  %s\n' "$b" "$ha" "$id"
done

# SHA256SUMS manifest from build A (the P11-T04 signing input).
{
  while read -r bin; do
    [ -z "$bin" ] && continue
    echo "$(sha256sum "$T_A/release/$bin" | awk '{print $1}')  $bin"
  done <<< "$names_a"
} > "$SUMMARY.SHA256SUMS"

NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
HOST="$(hostname)"
{
  echo "{"
  echo "  \"schema\": \"keelworks.reproducible-build.v1\","
  echo "  \"version\": \"$VERSION\","
  echo "  \"toolchain\": \"$(rustc --version)\","
  echo "  \"source_date_epoch\": \"$STAMP\","
  echo "  \"rustflags\": \"--remap-path-prefix=$SRC=.\","
  echo "  \"built_at\": \"$NOW\","
  echo "  \"host\": \"$HOST\","
  echo "  \"artifacts\": $N_ART,"
  echo "  \"identical\": $( [ "$MISMATCH" -eq 0 ] && echo true || echo false ),"
  echo "  \"rows\": ["
  # emit rows without a trailing comma (valid strict JSON)
  n=0
  while IFS='|' read -r b ha hb id; do
    n=$((n+1))
    sep=","
    [ "$n" -eq "$N_ART" ] && sep=""
    echo "    {\"file\":\"$b\",\"sha256\":\"$ha\",\"sha256_b\":\"$hb\",\"identical\":\"$id\"}$sep"
  done <<< "$ROWS"
  echo "  ],"
  echo "  \"sha256sums\": \"$SUMMARY.SHA256SUMS\""
  echo "}"
} > "$SUMMARY.json"

{
  echo "# Keelworks reproducible build proof -- $VERSION"
  echo ""
  echo "Toolchain: $(rustc --version)  |  Source date epoch: $STAMP  |  Host: $HOST  |  Built: $NOW"
  echo ""
  echo "Two clean-room release builds (isolated CARGO_TARGET_DIR, incremental off,"
  echo "source paths remapped) produced $N_ART artifacts: all byte-identical: $( [ "$MISMATCH" -eq 0 ] && echo yes || echo NO )."
  echo ""
  printf '%s\n' "$ROWS" | while IFS='|' read -r b ha hb id; do
    echo "- $b  sha256 $ha  identical $id"
  done
  echo ""
  echo "Artifact manifest (P11-T04 signing input): $SUMMARY.SHA256SUMS"
} > "$SUMMARY.md"

echo ""
echo "== proof written =="
ls -l "$SUMMARY".{json,md,SHA256SUMS}
[ "$MISMATCH" -eq 0 ] || { echo "BUILD NOT REPRODUCIBLE" >&2; exit 1; }
echo "REPRODUCIBLE BUILD OK"
