# syntax=docker/dockerfile:1.7
#
# Multi-stage Dockerfile for cronduit. Cross-compiles amd64 + arm64 musl-static
# via cargo-zigbuild (no QEMU), packages into an alpine:3 runtime.
#
# Builder base is Debian 13 (trixie) — the current Debian stable. Upgraded
# from bookworm in minor-fixes after v1.0.0: the cargo-zigbuild cross-compile
# path produces a musl-static binary for the runtime stage, so the builder's
# glibc version does not affect the output; the only observable difference is
# CVE exposure on the builder itself. The apt package names used below
# (ca-certificates, curl, xz-utils, pkg-config) are stable across bookworm and
# trixie, so no install-line changes are needed.

# ---- builder ----
FROM --platform=$BUILDPLATFORM rust:1.94.1-slim-trixie AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install zig + cargo-zigbuild.
# Download to file first (piping curl|tar fails when GitHub returns non-XZ response).
RUN set -eux; \
    ZIG_VERSION=0.13.0; \
    ARCH="$(uname -m)"; \
    curl -fsSL --retry 3 --retry-delay 5 \
        -o /tmp/zig.tar.xz \
        "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${ARCH}-${ZIG_VERSION}.tar.xz"; \
    tar -xJf /tmp/zig.tar.xz -C /opt; \
    rm /tmp/zig.tar.xz; \
    ln -s /opt/zig-linux-${ARCH}-${ZIG_VERSION}/zig /usr/local/bin/zig; \
    cargo install --locked cargo-zigbuild --version ^0.22

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

# Install standalone Tailwind CSS binary for the BUILDPLATFORM so build.rs
# can compile assets/static/app.css from the Tailwind sources before cargo
# embeds them via rust-embed. Without this step the Docker image ships with
# a stub `/* tailwind not built yet */` file and the web UI renders unstyled.
# Version kept in sync with justfile's `just tailwind` recipe.
RUN set -eux; \
    TAILWIND_VERSION=v3.4.17; \
    BUILD_ARCH="$(uname -m)"; \
    case "$BUILD_ARCH" in \
      "x86_64") TAILWIND_ARCH="x64" ;; \
      "aarch64") TAILWIND_ARCH="arm64" ;; \
      *) echo "unsupported BUILDPLATFORM arch: $BUILD_ARCH" >&2; exit 1 ;; \
    esac; \
    mkdir -p /usr/local/bin; \
    curl -fsSL --retry 3 --retry-delay 5 \
        -o /usr/local/bin/tailwindcss \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/tailwindcss-linux-${TAILWIND_ARCH}"; \
    chmod +x /usr/local/bin/tailwindcss; \
    /usr/local/bin/tailwindcss --help >/dev/null

WORKDIR /build
COPY . .

# build.rs expects the tailwind binary at ./bin/tailwindcss (relative to the
# crate root). Symlink into place so both local dev (where `just tailwind`
# downloads into ./bin/) and the Docker builder (where it's in /usr/local/bin)
# converge on the same path.
RUN mkdir -p bin && ln -sf /usr/local/bin/tailwindcss bin/tailwindcss

# Translate buildx TARGETPLATFORM -> rustc target triple.
RUN set -eux; \
    case "$TARGETPLATFORM" in \
      "linux/amd64") TARGET="x86_64-unknown-linux-musl" ;; \
      "linux/arm64") TARGET="aarch64-unknown-linux-musl" ;; \
      *) echo "unsupported TARGETPLATFORM: $TARGETPLATFORM" >&2; exit 1 ;; \
    esac; \
    echo "$TARGET" > /target.txt; \
    cargo zigbuild --release --target "$TARGET"; \
    cp "target/$TARGET/release/cronduit" /cronduit

# ---- runtime ----
# Rebased to alpine:3 in Phase 8 (2026-04-13) as a conscious walk-back from the
# Phase 1 distroless-nonroot runtime. Rationale: the distroless image had no
# /bin/sh, no coreutils, no busybox applets, so any command/script job that
# invoked `date`, `wget`, `du`, `df`, or `sh` failed with ENOENT. The
# quickstart's echo-timestamp job (date '+...') was broken out of the box for
# every new user. Alpine ships busybox, runs as a non-root cronduit user
# (UID 1000), and adds only ca-certificates + tzdata on top of the base image.
# Trade-off: slightly larger attack surface; the non-root UID + read-only
# config mount posture from Phase 1 still holds. See .planning/phases/08-*
# for the full decision log (D-01..D-06) on this walk-back.
FROM alpine:3

# Static OCI labels -- fallback for local `docker build .` outside the
# release workflow. The three labels below are the ones GitHub Container
# Registry recognizes on the package page:
#   org.opencontainers.image.source       -> "Connected to repository" link
#   org.opencontainers.image.description  -> subtitle under the package name
#   org.opencontainers.image.licenses     -> license badge in the sidebar
# At release time, docker/metadata-action@v5 in .github/workflows/release.yml
# generates a fuller label + annotation set (title, vendor, version,
# revision, created, url) and writes them to BOTH the per-platform image
# configs and the top-level manifest INDEX, which is what GHCR reads for
# multi-arch images. See:
# https://docs.github.com/packages/working-with-a-github-packages-registry/working-with-the-container-registry#labelling-container-images
LABEL org.opencontainers.image.source="https://github.com/SimplicityGuy/cronduit"
LABEL org.opencontainers.image.description="Self-hosted Docker-native cron scheduler with a web UI"
LABEL org.opencontainers.image.licenses="MIT"

# Install minimal runtime dependencies: CA bundle for HTTPS (bollard image
# pulls, busybox wget against https://www.google.com) and IANA timezone data
# (croner DST-correct scheduling). Chain into a single layer with no apk
# cache left behind.
RUN apk add --no-cache ca-certificates tzdata

# Create a non-root cronduit user + group (UID/GID 1000) and pre-create
# /data with cronduit ownership so docker-compose named volumes inherit
# writable permissions on first mount. This replaces the Phase 1 multi-stage
# chown dance that targeted the old distroless nonroot UID.
RUN addgroup -g 1000 -S cronduit \
 && adduser -S -u 1000 -G cronduit cronduit \
 && install -d -o 1000 -g 1000 /data

COPY --from=builder /cronduit /cronduit
# Migrations are embedded via `sqlx::migrate!(...)` -- no filesystem copy.
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER cronduit:cronduit

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
