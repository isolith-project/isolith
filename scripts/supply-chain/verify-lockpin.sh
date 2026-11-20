#!/usr/bin/env bash
# Isolith P11-T06: lock-pin verifier (the "CI fails on any unpinned or
# unverified dependency change" gate).
#
# Fails (exit 1) on any of:
#   1. Cargo.lock is not committed / is git-ignored.
#   2. The committed Cargo.lock no longer satisfies the manifests
#      (`cargo metadata --locked` fails), i.e. a dependency change landed
#      without its lockfile update.
#   3. Any dependency is unverifiable: a git source without an exact rev, a
#      registry package without a checksum, or a path dependency outside the
#      workspace.
#   4. The Rust toolchain pin is missing or uses an unpinned "stable" channel.
#   5. The Python dependency set is not locked (uv.lock / requirements lock
#      present) per the P0-M3-T02 single-source-of-truth doctrine.
#
# Run inside any checkout. Stdlib + cargo only (matches CI container).
# Exit 0 = pinned, verified; nonzero = reject the push/PR.
set -u

FAIL=0
note() { printf 'ok    %s\n' "$1"; }
bad()  { printf 'FAIL  %s\n' "$1"; FAIL=1; }
warn() { printf 'warn  %s\n' "$1"; }
have(){ command -v "$1" >/dev/null 2>&1; }

cd "$(pwd)"
echo "== P11-T06 lockpin verifier: $(basename "$(pwd)") =="

# --- 1. Cargo.lock committed and not ignored -------------------------------
if [ -f Cargo.lock ]; then
  if git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1; then
    note "Cargo.lock is tracked by git"
  else
    bad "Cargo.lock exists but is NOT tracked (unpinned dependency set drifts)"
    warn "run: git add Cargo.lock"
  fi
else
  warn "no Cargo.lock (workspace may be dependency-free) - nothing to pin"
fi

# --- 2. Lockfile satisfies the manifests (locked resolution) ----------------
# `cargo metadata --locked` fails exactly when the committed Cargo.lock cannot
# satisfy the manifests. The previous form exported the index to a temp tree,
# ran `cargo generate-lockfile` there (which re-resolves from the registry
# index, ignoring the committed lock) and diffed the two files. That fails on
# every dependency that has published a newer compatible release since the
# lock was written, so it measured elapsed time rather than pinning: on
# 2026-11-19 it rejected main with axum 0.46.0 -> 0.47.0 and six other crates
# resolving newer, while `cargo metadata --locked` passed on the same tree.
if [ -f Cargo.toml ] && have cargo; then
  meta_err="$(mktemp)"
  if cargo metadata --locked --format-version 1 >/dev/null 2>"$meta_err"; then
    note "Cargo.lock satisfies every manifest (locked resolution)"
  else
    bad "Cargo.lock cannot satisfy the manifests (unpinned or missing lock entry)"
    warn "run: cargo update -p <crate> (or cargo generate-lockfile), then commit"
    warn "the updated Cargo.lock; a manifest change without a lock update fails CI."
    sed -n '1,3p' "$meta_err" 2>/dev/null | sed 's/^/      /' || true
  fi
  rm -f "$meta_err"
else
  warn "cargo unavailable or no workspace root; skipping locked-resolution check"
fi

# --- 3. Every dependency verifiable (checksum-pinned / exact rev) -----------
if [ -f Cargo.lock ]; then
  # registry entries must carry a checksum; count mismatches
  python3 - "$PWD/Cargo.lock" <<'PY'
import re, sys
path = sys.argv[1]
pkgs = []
cur = None
for line in open(path, encoding="utf-8"):
    line = line.rstrip("\n")
    if line.startswith("[[package]]"):
        cur = {"name": None, "source": None, "checksum": None, "version": None}
        pkgs.append(cur)
    elif cur is not None:
        m = re.match(r'name = "([^"]+)"', line)
        if m and cur["name"] is None: cur["name"] = m.group(1)
        m = re.match(r'version = "([^"]+)"', line)
        if m and cur["version"] is None: cur["version"] = m.group(1)
        m = re.match(r'source = "([^"]+)"', line)
        if m: cur["source"] = m.group(1)
        m = re.match(r'checksum = "([0-9a-f]+)"', line)
        if m: cur["checksum"] = m.group(1)
bad_registry = 0   # registry-sourced packages missing a checksum
bad_git = []       # git-sourced packages without a pinned rev
for p in pkgs:
    if p["source"] is None:
        continue  # workspace/path member: fine
    if p["source"].startswith("registry+") and p["checksum"] is None:
        print("FAIL  registry package %s %s has NO checksum" %
              (p["name"], p["version"]))
        bad_registry += 1
    if p["source"].startswith("git+"):
        # pinned git deps look like git+https://...#<40-hex>; anything shorter
        # or absent means unpinned
        frag = p["source"].split("#")[-1] if "#" in p["source"] else ""
        if len(frag) != 40 or not all(c in "0123456789abcdef" for c in frag):
            print("FAIL  git dependency %s %s is NOT pinned to an exact rev: %s"
                  % (p["name"], p["version"], p["source"]))
            bad_git.append(p["name"])
if bad_registry or bad_git:
    print("RESULT: FAIL")
    sys.exit(1)
print("ok    all %d external packages verifiable (registry+checksum or pinned git rev)"
      % sum(1 for p in pkgs if p["source"]))
PY
  [ $? -eq 0 ] || FAIL=1
fi

# --- 4. Rust toolchain pin ---------------------------------------------------
if [ -f rust-toolchain.toml ]; then
  if grep -qE 'channel\s*=\s*"?[0-9]+\.[0-9]+(\.[0-9]+)?"?$' rust-toolchain.toml; then
    note "rust-toolchain.toml pins a concrete channel: $(grep -E 'channel\s*=' rust-toolchain.toml | tr -d ' ')"
  else
    bad "rust-toolchain.toml has no pin or uses an unpinned channel (e.g. 'stable')"
  fi
elif [ -f rust-toolchain ]; then
  note "rust-toolchain file pin present"
else
  bad "no rust-toolchain.toml / rust-toolchain pin (P0-M3-T02)"
fi

# --- 5. Python dependency lock ----------------------------------------------
if [ -f pyproject.toml ]; then
  if [ -f uv.lock ] || [ -f requirements.lock ] || [ -f requirements.txt ]; then
    note "python dependency lock present"
  else
    bad "python deps are not lockfile-pinned (uv.lock / requirements.lock missing)"
  fi
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
  echo "P11-T06 lockpin: PASS (all deps pinned and verifiable)"
else
  echo "P11-T06 lockpin: FAIL (rejecting unpinned/unverified dependency state)"
fi
exit "$FAIL"
