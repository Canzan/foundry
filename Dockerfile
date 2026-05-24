# syntax=docker/dockerfile:1.7
#
# Foundry multi-stage build.
#   builder: rust:1.85-slim — compiles the release binary.
#   runtime: distroless/cc-debian12 — minimal, glibc-only, non-root.

FROM rust:1.85-slim AS builder
WORKDIR /work

# System dependencies needed for crates that link C (sqlx-rustls is
# pure-Rust so this is intentionally minimal: pkg-config + ca-certs
# cover the rustls cert verification at build time).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       pkg-config \
       ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the workspace and build only the release binary.
COPY Cargo.toml rust-toolchain.toml rustfmt.toml ./
COPY crates ./crates
COPY xtask  ./xtask

# Bake offline mode — sqlx queries are evaluated lazily at runtime
# in slice 1 (no compile-time `query!` macros yet); when those land,
# `SQLX_OFFLINE=true` + a committed `.sqlx/` cache will keep this
# build airgap-friendly.
ENV SQLX_OFFLINE=true

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/work/target \
    cargo build --release -p foundry-app --bin foundry \
    && cp /work/target/release/foundry /usr/local/bin/foundry

FROM gcr.io/distroless/cc-debian12 AS runtime
WORKDIR /app

# Distroless ships /etc/ssl/certs by default. The binary is statically
# linked against musl-free rustls + glibc from the base image.
COPY --from=builder /usr/local/bin/foundry /app/foundry

# Migrations are baked into the binary by `sqlx::migrate!`. No need
# to ship the migrations directory separately.

USER nonroot:nonroot
# 3000 = main HTTP listener (FOUNDRY_PORT); 9090 = sidecar Prometheus
# metrics listener (METRICS_PORT). The LB should ONLY expose 3000.
EXPOSE 3000 9090
ENTRYPOINT ["/app/foundry"]
