# syntax=docker/dockerfile:1.7
#
# Foundry multi-stage build — CROSS-COMPILING.
#   builder: rust:1.85-slim pinned to the NATIVE build host ($BUILDPLATFORM).
#            It cross-compiles the release binary to the requested target
#            arch, so an arm64 image is built at native amd64 speed instead
#            of emulating the whole Rust compile under QEMU. The actual
#            toolchain (1.91) is pinned by rust-toolchain.toml; rustup
#            auto-installs it and the cross std target.
#   runtime: distroless/cc-debian12 for $TARGETPLATFORM — minimal, glibc,
#            non-root. buildx selects the matching-arch base automatically;
#            the stage only copies the pre-cross-compiled binary (no compile),
#            so it's a fast file-copy even when emulated.

FROM --platform=$BUILDPLATFORM rust:1.85-slim AS builder
WORKDIR /work

# buildx provides TARGETARCH (amd64 | arm64). Pick the Rust target triple
# and install the matching cross C toolchain — `ring` (via rustls) compiles
# C/asm, so a cross `cc` + linker is required when target != build host.
ARG TARGETARCH
# The cross toolchain needs BOTH the cross gcc AND the target's libc dev
# headers/libs (libc6-dev-<arch>-cross) — without the latter, ring's C
# sources fail with "bits/libc-header-start.h: No such file or directory".
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && case "$TARGETARCH" in \
         amd64) triple=x86_64-unknown-linux-gnu;  pkg="gcc-x86-64-linux-gnu libc6-dev-amd64-cross" ;; \
         arm64) triple=aarch64-unknown-linux-gnu; pkg="gcc-aarch64-linux-gnu libc6-dev-arm64-cross" ;; \
         *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
       esac \
    && apt-get install -y --no-install-recommends $pkg \
    && echo "$triple" > /rust-target \
    && rm -rf /var/lib/apt/lists/*

# Per-target linker + C compiler for the cross build (ring/asm). Only the
# vars for the ACTIVE target are consulted, so defining both is harmless.
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    SQLX_OFFLINE=true

# Copy the workspace and build only the release binary.
#
# `Cargo.lock` is COPIED and the build below is `--locked`, so the image is
# built from the SAME dependency graph the host and CI resolve. Without both,
# cargo re-resolves inside the container and silently drifts to newer
# semver-compatible versions — which is not hypothetical: this build broke on a
# freshly-resolved `tinyvec 1.13.0` (the lockfile pins 1.11.0) the first time a
# source edit invalidated the layer cache. A non-reproducible image build fails
# for whoever next busts the cache, not for whoever caused the drift.
COPY Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml ./
COPY crates ./crates
COPY xtask  ./xtask

# Cross-compile for the target triple. rustup installs the pinned toolchain
# (1.91) + the cross std. Caches are scoped per-arch so concurrent amd64 /
# arm64 builds don't contend on the same cache mount.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/work/target,id=cargo-target-${TARGETARCH} \
    triple="$(cat /rust-target)" \
    && rustup target add "$triple" \
    && cargo build --locked --release --target "$triple" -p foundry-app --bin foundry \
    && cp "/work/target/$triple/release/foundry" /usr/local/bin/foundry

FROM gcr.io/distroless/cc-debian12 AS runtime
WORKDIR /app

# Distroless ships /etc/ssl/certs by default. The binary links rustls +
# glibc from the base image.
COPY --from=builder /usr/local/bin/foundry /app/foundry

# Migrations are baked into the binary by `sqlx::migrate!`. No need
# to ship the migrations directory separately.

# The vendored assets are NOT baked into the binary — /static is served by
# tower_http::ServeDir off the real filesystem, and `static_dir()` prefers the
# cwd-relative `static` (WORKDIR is /app, so /app/static). Its fallback,
# CARGO_MANIFEST_DIR/static, is a BUILDER-stage path this stage never inherits,
# so without this COPY both candidates miss.
#
# ServeDir does not error on a missing root — it 404s every request. Omitting
# this line therefore ships an image that passes /healthz, /readyz, and every
# liveness probe while serving the app with no stylesheet, no htmx, no board
# drag-and-drop, and no webmanifest. The in-process acceptance suite cannot
# catch it either: there CARGO_MANIFEST_DIR resolves to a real directory on the
# build host, so the /static scenarios pass green against a broken image.
COPY --from=builder /work/crates/foundry-app/static /app/static

USER nonroot:nonroot
# 3000 = main HTTP listener (FOUNDRY_PORT); 9090 = sidecar Prometheus
# metrics listener (METRICS_PORT). The LB should ONLY expose 3000.
EXPOSE 3000 9090
ENTRYPOINT ["/app/foundry"]
