#!/usr/bin/env bash
# Install the adaptive pre-commit quality gate for the current repo.
# Usage: install-hook.sh [repo-dir  (default: cwd)]
#
# Vendored into the Isolith repo (scripts/gate/) from the Hermes
# `precommit-quality-gate` skill. The hook symlinks to THIS checkout's
# precommit-gate.sh (self-contained), not to a Hermes skill path, so any clone
# can install it even on machines without Hermes.
set -e
repo="${1:-$(pwd)}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
gate="$here/precommit-gate.sh"
hook="$repo/.git/hooks/pre-commit"
mkdir -p "$repo/.git/hooks"
ln -sf "$gate" "$hook" 2>/dev/null || cp "$gate" "$hook"
chmod +x "$gate" "$hook"
echo "installed pre-commit -> $gate (in $repo)"
echo "next commits in $repo run the gate; remove with: rm $hook"
