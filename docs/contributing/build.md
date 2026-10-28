---
title: "Isolith Contributor Build Guide"
version: 1.0.0
date: 2026-10-27
status: Ratified
owner: Isolith program (Open Engineering Platform), P0-M1 (program governance and foundations)
applies_to: every repository under the isolith org (the monorepo's contributor build path)
supersedes: nothing (first contributor build guide; implements CONTRIBUTOR-BUILD-PATH, decision #15 wave discipline)
---

# Isolith Contributor Build Guide

This page is the entire build-and-test story for a new contributor. If you are
reading it because you cloned the repository, you can build and test it
without asking anyone, and without preinstalling a Rust toolchain, OCCT, or a
C++ compiler on your machine. The repo pins everything.

Audience: a developer who has never built an OCCT-linking Rust workspace. The
only tool you must install on YOUR machine is a container runtime. Everything
else lives in the devcontainer.

## One decision, one path

The project ships exactly ONE maintained contributor build path: the
devcontainer (`.devcontainer/`). There is deliberately no Nix flake and no
second hand-maintained path, because two half-maintained paths are worse than
one complete one. The devcontainer pins the Rust toolchain (from
`rust-toolchain.toml`, nothing duplicated), installs CMake/GCC for the OCCT
8.0.x build, installs the Python/uv tooling, and builds on Ubuntu 24.04 to
match the reference host (GCC 13.3, the `CMakePresets.json` default).

The whole gate is one command:

    cargo xtask ci

`cargo xtask` is a tiny dev-tooling binary in `xtask/` (wired by the
repo-root `.cargo/config.toml`, zero dependencies, not part of the built
product). It runs exactly the checks the CI jobs run: rustfmt, clippy
(deny warnings), unit and integration tests, and `pytest tests/`. Add
`--kernel` to also build the pinned OCCT 8.0.1 dependency and run the
through-the-seam kernel tests.

## Step 1: install a container runtime

- Docker Desktop (macOS/Windows) or a Docker Engine daemon (Linux).
- VS Code with the Dev Containers extension if you want the VS Code flow; it
  is optional. The devcontainer also runs headless from a plain shell, which
  is how this guide is verified.

## Step 2: clone and start the container

Clone the repository, then either open it in the devcontainer (VS Code:
"Reopen in Container", or any devcontainer client: Devpod, Cursor, GitHub
Codespaces) or run the same container directly:

    git clone <your-fork-or-the-repo-url> isolith
    cd isolith
    docker build -t isolith-dev -f .devcontainer/Dockerfile .
    docker run -it --rm -v "$PWD":/workspaces/isolith -w /workspaces/isolith isolith-dev bash

The image build takes about 2 minutes the first time. The
container already contains the pinned Rust 1.98.1 toolchain (with rustfmt and
clippy), CMake 3.28, GCC 13, and uv. Nothing is downloaded at first `bash`
beyond what the toolchain itself fetches.

Inside the container, in one command:

    uv sync
    cargo xtask ci

`uv sync` creates the repository `.venv` with pytest (pinned in `uv.lock` via
the `dev` dependency group) using the repository's pinned Python. The
devcontainer's automatic post-create step runs `uv sync && cargo xtask
check-sysdeps` for you; `check-sysdeps` is a 2-second environment probe that
prints what (if anything) is missing.

## What the gate runs, in order

`cargo xtask ci`:

1. `cargo fmt --all -- --check` (workspace formatting)
2. `cargo fmt --manifest-path ./xtask/Cargo.toml -- --check` (dev tooling itself)
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo clippy --manifest-path ./xtask/Cargo.toml -- -D warnings`
5. `cargo test --workspace --lib` (unit)
6. `cargo test --workspace --tests` (integration)
7. `python3 -m pytest tests/ -q` (acceptance)

## The OCCT tier (only for geometry-kernel work)

The workspace builds and all the tests above run WITHOUT OCCT: the
`oep-engine-occ` crate compiles to a typed stub unless the `kernel` feature is
requested, so a normal PR never pays the OCCT cost.

If your change touches the geometry kernel (`oep-geo`, `oep-engine-occ`,
`scripts/kernel/`), you must also run the through-the-seam tests against the
REAL OCCT. One command does it (it builds OCCT the first time, which is the
long step):

    cargo xtask ci --kernel

or pre-build OCCT once with `cargo xtask occt`. The build is cached under
`~/.cache/oep-occt/` (mapped to a named volume `isolith-oep-occt` in the
devcontainer so it survives container rebuilds).

Honest scope note: `cargo xtask ci --kernel` is a real single command for the
kernel tier, but it is a flag, not the default. The OCCT fetch/build is a
2-to-3-minute one-time cost (137 s measured) that only the kernel lane ever
pays, so the plain `cargo xtask ci` leaves it out and stays about 3 minutes.
A single command that ALWAYS builds OCCT from nothing would make every
contributor, including people who never touch geometry, wait for a dependency
they do not need; that tradeoff is made explicit rather than hidden.

## Expected wall-clock (measured, clean machine, no cached toolchain)

Measured on U1 (Ubuntu 24.04 ARM64, 20 cores) for the 2027.1 train with the
2026-10-27 main. Your numbers scale with cores and bandwidth.

- Devcontainer image build: about 2 minutes (118 s: apt + rustup + uv, all pinned).
- First `cargo xtask ci` in a fresh container (empty cargo registry, empty
  target): about 3 minutes (173 s): crates download plus a full
  workspace + all test-target compile.
- Subsequent `cargo xtask ci`: seconds to a couple of minutes (incremental).
- `cargo xtask ci --kernel` OCCT one-time fetch/build: about 2 to 3 minutes
  (137 s) the first time, plus a few seconds of kernel-feature compile and
  about 7 s of seam tests; then incremental.
- `cargo xtask check-sysdeps`: about 2 seconds, run any time.

## Disk footprint

- Devcontainer image: about 1.6 GB on disk.
- Cargo registry + build target for the full workspace + test binaries: about
  13 GB inside the container (measured: registry 0.2 GB, workspace target
  12 GB), or wherever your CARGO_TARGET_DIR points for manual setups.
- OCCT source + static build + install, when you run the kernel tier: about
  0.9 GB (885 MB) under `~/.cache/oep-occt`, of which the static install is
  169 MB.

## The three most common failures and their fixes

**[1. Failure: `cargo xtask` prints "fatal: not a git repository" or
"error: no subcommand"/"could not find `xtask`".]**

You are running from a directory that predates the `.cargo/config.toml`
alias, or from a checkout where the file is missing. Fix: `cd` to the repo
root (the directory containing `Cargo.toml` with the `[workspace]` table),
`git fetch && git pull gitea main`, and try again. The alias lives in
`.cargo/config.toml` at the repo root.

**[2. Failure: `cargo xtask ci` fails at the pytest leg with "ModuleNotFoundError:
No module named 'pytest'".]**

The gate runs pytest from the repository `.venv` (created by `uv sync`); if
the venv was removed or never created, the probed interpreter has no pytest.
Fix: `uv sync`, then rerun `cargo xtask ci`. If `uv` itself is missing, you
are not on the devcontainer path; build the image or run
`cargo xtask check-sysdeps` to see every missing piece at once.

**[3. Failure: the kernel tier fails at the OCCT step with "FATAL: missing
libX11" or a CMake error about a missing build tool.]**

The OCCT fetch/build needs the system libraries the image installs
(`libfreetype-dev`, CMake, GCC); on a host that bypasses the devcontainer
these are absent. Fix: use the devcontainer image (it preinstalls them), or
install the same packages your distro names and rerun `cargo xtask occt`. If
you are inside the devcontainer and OCCT still fails, the pinned SHA-256 of
the 8.0.1 tarball (`scripts/kernel/fetch-occt.sh`) is the first thing to
check; a changed upstream tarball fails the sum and that is by design, so you
will not link a tampered or drifting dependency.

A fourth, very common one, cheap to answer in advance: if `rustc --version`
on your host differs from `rust-toolchain.toml`, rustup auto-selects the
pinned toolchain when rustup is present; inside the devcontainer it is always
correct. The devcontainer is the answer to all host-drift variants.

## Verified by construction

This path is proven end to end, not declared: the CI job
(`.gitea/workflows/devcontainer-build.yaml`, mirrored in
`.github/workflows/devcontainer-build.yaml`) builds this exact image on every
push and runs `cargo xtask check-sysdeps` inside it, so the environment
cannot silently rot between contributors. The measured clean-build evidence
for this document lives in the contribution evidence directory (see the
version history of this file).

## Change log

- 1.0.0 (2026-10-27): first contributor build guide. One maintained
  devcontainer path, one-command gate via `cargo xtask ci`, measured clean
  build evidence, top-three failures with fixes.
