# syntax=docker/dockerfile:1

# ── Stage 1: builder ──────────────────────────────────────────────────────────
# cargo-leptos needs: Rust stable + wasm32 target + dart-sass + binaryen (wasm-opt)
FROM --platform=$BUILDPLATFORM rust:1.94-trixie AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

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

WORKDIR /app

# ── Cache dependencies ────────────────────────────────────────────────────────
# Copy manifests first so this layer is only invalidated when deps change.
COPY Cargo.toml Cargo.lock ./
# Dummy source so cargo can resolve the workspace without the real source.
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs && \
    touch src/lib.rs && \
    cargo fetch

# ── Copy source and build ─────────────────────────────────────────────────────
COPY . .
RUN cargo leptos build --release 2>&1

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user (568:568 matches TrueCharts' home assistant UID/GID)
RUN groupadd -g 568 appgroup && useradd -m -u 568 -g 568 appuser

WORKDIR /app

# Copy the server binary and the pre-built site assets
COPY --from=builder /app/target/release/iu-configurator ./iu-configurator
COPY --from=builder /app/target/site ./target/site

# Runtime environment defaults (override via k8s Secret/ConfigMap)
ENV LEPTOS_SITE_ROOT=/app/target/site
ENV LEPTOS_SITE_ADDR=0.0.0.0:3000
ENV CONFIG_DIR=/config

USER appuser

EXPOSE 3000

CMD ["./iu-configurator"]
