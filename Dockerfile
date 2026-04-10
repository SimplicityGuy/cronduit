# syntax=docker/dockerfile:1.7
#
# Multi-stage Dockerfile for cronduit. Cross-compiles amd64 + arm64 musl-static
# via cargo-zigbuild (no QEMU), packages into gcr.io/distroless/static-debian12:nonroot.

# ---- builder ----
FROM --platform=$BUILDPLATFORM rust:1.94-slim-bookworm AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install zig + cargo-zigbuild.
# ziglang release URL pattern: zig-linux-<arch>-<version>.tar.xz
RUN set -eux; \
    ZIG_VERSION=0.13.0; \
    ARCH="$(uname -m)"; \
    curl -sSL "https://github.com/ziglang/zig/releases/download/${ZIG_VERSION}/zig-linux-${ARCH}-${ZIG_VERSION}.tar.xz" \
        | tar -xJ -C /opt; \
    ln -s /opt/zig-linux-${ARCH}-${ZIG_VERSION}/zig /usr/local/bin/zig; \
    cargo install --locked cargo-zigbuild --version ^0.22

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

WORKDIR /build
COPY . .

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
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /cronduit /cronduit
# Migrations are embedded via `sqlx::migrate!(...)` -- no filesystem copy.
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER nonroot:nonroot

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
