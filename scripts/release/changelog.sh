#!/usr/bin/env bash
# Isolith changelog / release-notes generator (P12-T07).
#
# Drives git-cliff (cliff.toml at repo root) over the repository's conventional
# commit history and writes a categorized changelog. Used by:
#   * the release cut itself  (a tagged release produces its own section), and
#   * CI  (.gitea/workflows/release-changelog.yaml fires on each release tag).
#
# Release versioning follows P0-M5-T03 (calendar trains YYYY.N.P[-PRE], tags
# v<version>) and P12-T05 owns tag creation; this script only READS tags.
#
# Behavior
# --------
#   changelog.sh [VERSION] [OUTPUT]
#   VERSION  Release to render, e.g. 2027.1.0 or 2027.1.0-rc1 (default: the
#            nearest existing v<tag>, or "unreleased" if there are no tags).
#   OUTPUT   Destination file (default: CHANGELOG.md).
#
# If the tag v$VERSION exists, git-cliff renders that tagged release from the
# history (the tagged cut is the lower/upper bound it saw at freeze). If no
# tag exists yet (first dry run), git-cliff renders the whole history as that
# VERSION via --unreleased --tag.
#
# git-cliff must be available: set GIT_CLIFF_BIN to an explicit binary path,
# or put `git-cliff` on PATH. The script never downloads anything (air-gap
# doctrine); CI installs the pinned release (see the workflow).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

GIT_CLIFF="${GIT_CLIFF_BIN:-$(command -v git-cliff || true)}"
if [ -z "$GIT_CLIFF" ]; then
  echo "ERROR: git-cliff not found." >&2
  echo "  Install it (pinned build - see .gitea/workflows/release-changelog.yaml)" >&2
  echo "  or export GIT_CLIFF_BIN=/path/to/git-cliff." >&2
  exit 1
fi

# Pinned generator version this script is written against.
GIT_CLIFF_PINNED="2.14.2"

version_arg="${1:-}"
output="${2:-CHANGELOG.md}"

# Resolve the release version.
version=""
if [ -n "$version_arg" ]; then
  version="$version_arg"
elif git describe --tags --abbrev=0 >/dev/null 2>&1; then
  version="$(git describe --tags --abbrev=0 | sed 's/^v//')"
else
  version="unreleased"
fi

# Does the release tag exist in this repo?
if git rev-parse -q --verify "refs/tags/v${version}" >/dev/null 2>&1; then
  echo "== release tag v${version} exists: rendering tagged release =="
  "$GIT_CLIFF" -o "$output" --tag "$version"
elif [ "$version" = "unreleased" ]; then
  echo "== no tags yet: rendering unreleased history =="
  "$GIT_CLIFF" -o "$output" --unreleased
else
  echo "== no tag v${version} yet (dry run): rendering whole history as ${version} =="
  "$GIT_CLIFF" -o "$output" --unreleased --tag "$version"
fi

# Program doctrine: every artifact gets a checksum next to it.
if [ -f "$output" ]; then
  sha256sum "$output" > "${output}.sha256"
fi

# Summary for the log / proof record.
echo
echo "== changelog generated -> ${output} (git-cliff ${GIT_CLIFF_PINNED}) =="
echo "== SHA-256: $(sha256sum "$output" | awk '{print $1}') =="
if command -v awk >/dev/null 2>&1; then
  awk -F'### |^## ' '
    /^## / { print "release: " $2 }
    /^### / { sec=$2; count[sec]=0; seen[sec]=1 }
    /^- / { if (seen[sec]) count[sec]++ }
    END { for (s in seen) if (count[s]>0) printf "  %-28s %4d %s\n", s, count[s], (count[s]==1 ? "entry" : "entries") }
  ' "$output"
fi
