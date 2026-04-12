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

# Static OCI labels. Dynamic labels (version, revision) are injected via --label
# in the GitHub Actions docker/build-push-action step.
LABEL org.opencontainers.image.source="https://github.com/SimplicityGuy/cronduit"
LABEL org.opencontainers.image.description="Self-hosted Docker-native cron scheduler with a web UI"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --from=builder /cronduit /cronduit
# Migrations are embedded via `sqlx::migrate!(...)` -- no filesystem copy.
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER nonroot:nonroot

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
