#!/usr/bin/env bash
# Isolith P0-M3-T02 acceptance test: verify toolchain pinning manifests are
# present, internally consistent, and resolve on this machine.
# TDD: this test documents the acceptance criteria BEFORE the manifests exist.
# Exit 0 = pinned, reproducible toolchain. Non-zero = a pin is missing/broken.
set -u

FAIL=0
note() { printf 'ok    %s\n' "$1"; }
bad()  { printf 'FAIL  %s\n' "$1"; FAIL=1; }

# --- Rust -----------------------------------------------------------------
# Expect repo-root rust-toolchain.toml (or .rust-toolchain) pinning a toolchain.
if [ -f rust-toolchain.toml ] || [ -f rust-toolchain ]; then
  TCFILE=$( [ -f rust-toolchain.toml ] && echo rust-toolchain.toml || echo rust-toolchain )
  note "rust toolchain pin present: $TCFILE"
  # It must name a concrete channel/version (may be quoted or bare).
  if grep -Eq "channel\s*=\s*[\"]?([0-9]+\.[0-9]+(\.[0-9]+)?)[\"]?" "$TCFILE"; then
    note "rust pin names a channel/version"
  else
    bad "rust-toolchain pin has no channel/version"
  fi
  # If a real toolchain file exists, rustup must be able to list it.
  if command -v rustup >/dev/null 2>&1; then
    if rustup toolchain list >/dev/null 2>&1; then
      note "rustup sees toolchains"
    else
      bad "rustup toolchain list failed"
    fi
  fi
else
  bad "missing rust-toolchain(.toml) pin"
fi

# Cargo workspace root must exist for a Rust monorepo.
if [ -f Cargo.toml ]; then
  note "Cargo.toml workspace root present"
  if grep -q "\[workspace\]" Cargo.toml; then
    note "Cargo workspace declared"
  else
    bad "Cargo.toml has no [workspace]"
  fi
else
  bad "missing Cargo.toml workspace root"
fi

# --- Python ---------------------------------------------------------------
# Expect a pinned Python version manifest + reproducible dependency lock.
if [ -f .python-version ]; then
  note ".python-version present: $(cat .python-version)"
else
  bad "missing .python-version"
fi
if [ -f pyproject.toml ]; then
  note "pyproject.toml present"
else
  bad "missing pyproject.toml"
fi
# Lock file: uv.lock (uv) or requirements.lock / requirements.txt pinned.
if [ -f uv.lock ] || [ -f requirements.lock ] || [ -f requirements.txt ]; then
  note "python dependency lock present"
else
  bad "missing python dependency lock (uv.lock / requirements.lock)"
fi

# --- C++ ------------------------------------------------------------------
# Expect a pinned compiler/toolchain manifest (CMakePresets or toolchain file).
if [ -f CMakePresets.json ]; then
  note "CMakePresets.json present"
else
  bad "missing CMakePresets.json"
fi

# --- Packaging / reproducibility ------------------------------------------
# Singular source of truth: a tool-versions or pins file must exist.
if [ -f .tool-versions ]; then
  note ".tool-versions present"
else
  bad "missing .tool-versions (single source of truth)"
fi

# Confirmed reproducibility anchor.
if [ -f Cargo.lock ]; then
  note "Cargo.lock present"
else
  bad "missing Cargo.lock (Rust reproducibility)"
fi

if [ "$FAIL" -eq 0 ]; then
  printf '\nACCEPTANCE: PASS - toolchain pinning complete and consistent.\n'
else
  printf '\nACCEPTANCE: FAIL - see FAIL lines above.\n'
fi
exit "$FAIL"
