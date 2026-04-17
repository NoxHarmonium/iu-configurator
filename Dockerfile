# syntax=docker/dockerfile:1

# ── Stage 1: toolchain (shared by prod builder and dev) ──────────────────────
FROM rust:1.95-trixie AS toolchain

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown

# Install cargo-leptos (pinned version for reproducibility)
RUN curl --proto '=https' --tlsv1.2 -LsSf https://github.com/leptos-rs/cargo-leptos/releases/download/v0.3.5/cargo-leptos-installer.sh | sh

# ── Stage 2: builder ──────────────────────────────────────────────────────────
FROM toolchain AS builder

# Docker containers don't need hot reload etc. and should be built in prod mode
ENV LEPTOS_ENV=prod

WORKDIR /build

COPY . .

# ── Build with persistent cargo caches ───────────────────────────────────────
# The cache mounts survive across builds even when source files change, so
# previously compiled dependency crates in the target directory are reused and
# only changed crates need to be recompiled.
# Artifacts are copied to the image filesystem (/build -> /app) before the cache mounts unmount.
RUN --mount=type=cache,id=iu-configurator-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=iu-configurator-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=iu-configurator-cargo-target,target=/build/target \
    cargo leptos build --release 2>&1 && \
    mkdir -p /app/target/release/ && \
    cp /build/target/release/iu-configurator /app/target/release/iu-configurator && \
    cp /build/target/release/hash.txt /app/target/release/hash.txt && \
    cp -r /build/target/site /app/target/site

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tini \
    && rm -rf /var/lib/apt/lists/*

# Non-root user (568:568 matches TrueCharts' home assistant UID/GID)
RUN groupadd -g 568 appgroup && useradd -m -u 568 -g 568 appuser

WORKDIR /app

# Copy the server binary and the pre-built site assets
COPY --from=builder /app .

# Runtime environment defaults (override via k8s Secret/ConfigMap)
ENV LEPTOS_SITE_ADDR=0.0.0.0:3000
ENV LEPTOS_HASH_FILES=true
ENV LEPTOS_ENV=prod
ENV CONFIG_DIR=/config

USER appuser

EXPOSE 3000

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/app/target/release/iu-configurator"]

# ── Stage 4: dev ──────────────────────────────────────────────────────────────
# Based on the shared toolchain stage — no release build, runs cargo-leptos watch.
# Used by dev/docker-compose.yml via `target: dev`.
FROM toolchain AS dev

WORKDIR /app

COPY . .

ENV LEPTOS_ENV=DEV
ENV CONFIG_DIR=/config

# App server port 3000; leptos hot-reload WebSocket port 3001
EXPOSE 3000 3001

CMD ["cargo", "leptos", "watch"]
