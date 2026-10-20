# syntax=docker/dockerfile:1
# Keelworks OEP stack runtime image (P10-M1-T01).
#
# Multi-stage: build the real workspace release binaries on a pinned Rust
# toolchain (rust-toolchain.toml, P0-M3-T02), copy only those binaries into a
# slim Debian runtime. This is the one image behind every compose service;
# each service picks the binary + health endpoint it runs (docker-compose.yml).
#
# The online one-command path builds this image from the repo (install.sh
# -> docker compose up --build). The offline/air-gap card (P10-M2-T02) saves
# this image with `release.sh --tarball` and `docker load`s it on a host with
# no network - build happens on a connected host, exactly the Bastion S2
# offline-image discipline.

FROM rust:1.98-bookworm AS builder
WORKDIR /src

# Bring the workspace source in (context = repo root; .dockerignore keeps the
# context small). Cargo.toml + lockfile first for layer reuse.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

# Fleet of binaries the P10 stack must serve. -p per crate keeps the builder
# footprint minimal while the lockfile (committed) pins every transitive dep.
RUN set -eux; \
    cargo build --release --bins \
        -p oep-pdm \
        -p oep-web \
        -p oep-opsdash \
        -p oep-worker \
        -p oep-gate \
        -p oep-render-worker \
        -p oep-docs

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="keelworks/oep-stack" \
      org.opencontainers.image.description="Open Engineering Platform (Keelworks): PDM + P2P compute + P6 preview viewer in one image" \
      org.opencontainers.image.version="$OEP_VERSION"

# curl is only for in-container health checks; the services themselves need no
# runtime deps beyond glibc.
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/oep-pdm            /usr/local/bin/oep-pdm
COPY --from=builder /src/target/release/oep-pdm-stub       /usr/local/bin/oep-pdm-stub
COPY --from=builder /src/target/release/oep-web             /usr/local/bin/oep-web
COPY --from=builder /src/target/release/oep-opsdash         /usr/local/bin/oep-opsdash
COPY --from=builder /src/target/release/oep-batch-convert   /usr/local/bin/oep-batch-convert
COPY --from=builder /src/target/release/oep-ga-gate         /usr/local/bin/oep-ga-gate
COPY --from=builder /src/target/release/oep-ga-node         /usr/local/bin/oep-ga-node
COPY --from=builder /src/target/release/oep-render-worker   /usr/local/bin/oep-render-worker
COPY --from=builder /src/target/release/oep-docs            /usr/local/bin/oep-docs

# The P13-T03 docs site ships in the image ("docs from the sum, no separate
# infra"); the docs service serves these files from /srv/docs.
COPY docs-site /srv/docs

# No CMD: docker-compose.yml sets the run command per service.
WORKDIR /srv
