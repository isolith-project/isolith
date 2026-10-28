# Isolith contributor devcontainer (CONTRIBUTOR-BUILD-PATH, card t_845b253d).
#
# The single, maintained contributor build environment. It pins the Rust
# toolchain (from rust-toolchain.toml, nothing duplicated here), installs the
# OCCT 8.0.x build toolchain (CMake + GCC) and the Python/uv tooling, and
# produces a working `cargo xtask ci` on the first run.
#
# Base is Ubuntu 24.04 on purpose: it matches the reference host this
# workspace is built on (U1: GCC 13.3.0, CMake 3.28.x) and is the preset
# family CMakePresets.json describes as default. No other build path is
# maintained; contributors use this container or reproduce its contents.
#
# Arch note: the build steps are arch-neutral (rustup and uv binaries are
# selected from `uname -m`), so the same Dockerfile builds on arm64 (e.g.
# U1) and x86_64 (typical contributor laptops and CI).

# syntax=docker/dockerfile:1
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# System toolchain + OCCT build prerequisites + repo/network tooling.
# libfreetype-dev and pkg-config are what the trimmed static OCCT build
# (build-occt.sh: ModelingData + ModelingAlgorithms + FoundationClasses)
# touches; the rest is the C++/CMake toolchain the Rust workspace binds.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        curl \
        file \
        git \
        libfreetype-dev \
        make \
        ninja-build \
        pkg-config \
        python3 \
        python3-pip \
        python3-venv \
    && rm -rf /var/lib/apt/lists/*

# Rust: rustup, then the repo's pinned toolchain + components. The pin is
# READ from rust-toolchain.toml at build time (derived, never duplicated), so
# a toolchain bump updates the container in the same PR as the manifest.
COPY rust-toolchain.toml /tmp/isolith-manifests/rust-toolchain.toml
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/rustup/bin:$PATH
RUN set -eux; \
    RUST_ARCH=$(uname -m | sed 's/^aarch64$/aarch64/; s/^x86_64$/x86_64/; s/^arm64$/aarch64/'); \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain none \
          --target "$RUST_ARCH-unknown-linux-gnu"; \
    CHANNEL=$(sed -n 's/^channel = "\(.*\)"/\1/p' /tmp/isolith-manifests/rust-toolchain.toml); \
    test -n "$CHANNEL"; \
    rustup toolchain install "$CHANNEL" --profile minimal \
        --component rustfmt --component clippy; \
    rustup default "$CHANNEL"; \
    rm -rf /tmp/isolith-manifests

# uv: the repo's Python tooling convention (pyproject.toml + uv.lock are the
# reproducible dependency surface). Pinned release, binary fetched from the
# astral GitHub release (no pip). A dev-dependency group in pyproject.toml
# carries pytest; `uv sync` creates the repo .venv.
ARG UV_VERSION=0.13.6
RUN set -eux; \
    UV_ARCH=$(uname -m | sed 's/^aarch64$/aarch64/; s/^arm64$/aarch64/; s/^x86_64$/x86_64/'); \
    curl -sSfL -o /tmp/uv.tar.gz \
        "https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/uv-${UV_ARCH}-unknown-linux-gnu.tar.gz"; \
    tar -xzf /tmp/uv.tar.gz -C /tmp; \
    install -m 0755 /tmp/uv-${UV_ARCH}-unknown-linux-gnu/uv /usr/local/bin/uv; \
    rm -rf /tmp/uv.tar.gz /tmp/uv-${UV_ARCH}-unknown-linux-gnu

# Where the source tree gets mounted by the devcontainer and COPYed by CI/docker run.
ENV ISOLITH_WORKSPACE=/workspaces/isolith
WORKDIR /workspaces/isolith
