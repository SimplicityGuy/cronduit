# justfile — Single source of truth for build/test/lint/DB/image/dev-loop.
#
# All GitHub Actions jobs call `just <recipe>` exclusively (D-10 / FOUND-12).
# A local `just ci` run must produce the same exit code as the CI job.
#
# Requires: just 1.x, bash, cargo, docker (for image + schema-diff).

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true

# -------------------- meta --------------------

# Show all available recipes grouped by category
default:
    @just --list --unsorted

[group('meta')]
[doc('The ORDERED chain CI runs. Local run must predict CI exit code (FOUND-12)')]
ci: fmt-check clippy openssl-check nextest schema-diff image

# The actual image build and push happens in CI via docker/build-push-action@v6.
[group('meta')]
[doc('Tag and push a release. Usage: just release 1.0.0')]
release version:
    @echo "Creating release v{{version}}..."
    git tag -a "v{{version}}" -m "Release v{{version}}"
    git push origin "v{{version}}"
    @echo "Release v{{version}} tagged and pushed. CI will build and publish."

# -------------------- build & artifacts --------------------

# Compile all crates + tests (debug profile)
[group('build')]
build:
    cargo build --all-targets

# Compile the release binary
[group('build')]
build-release:
    cargo build --release

# Remove cargo/build artifacts, generated CSS, and the dev SQLite database
[group('build')]
clean:
    cargo clean
    rm -rf .sqlx/tmp assets/static/app.css cronduit.dev.db cronduit.dev.db-wal cronduit.dev.db-shm

# Uses v4.2.2. Config lives in assets/src/app.css via @import "tailwindcss",
# @source "../../templates", and @theme — no tailwind.config.js (v4 format).
[group('build')]
[doc('Download the standalone Tailwind binary (NO Node) and rebuild app.css')]
tailwind:
    @mkdir -p assets/static bin
    @if [ ! -x ./bin/tailwindcss ]; then \
        echo "Downloading standalone Tailwind binary (v4.2.2)..."; \
        OS=$(uname -s | tr '[:upper:]' '[:lower:]' | sed 's/darwin/macos/'); \
        ARCH=$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/'); \
        curl -sSLo ./bin/tailwindcss \
            "https://github.com/tailwindlabs/tailwindcss/releases/download/v4.2.2/tailwindcss-${OS}-${ARCH}"; \
        chmod +x ./bin/tailwindcss; \
    fi
    ./bin/tailwindcss -i assets/src/app.css -o assets/static/app.css --minify

# -------------------- docker images --------------------
#
# Docker image build. Three variants:
#   `just image`       — single-platform linux/amd64 --load, tagged cronduit:dev
#                        (used by CI for reproducible smoke tests — DO NOT change
#                        the hardcoded platform or tag here; Plan 07 pins both)
#   `just image-local` — native host platform --load, tagged
#                        ghcr.io/simplicityguy/cronduit:latest — the tag
#                        examples/docker-compose.yml expects, so a
#                        `just image-local && docker compose up -d` hand-off
#                        works without a manual retag. Use for local UAT of a
#                        feature branch before the release workflow publishes.
#   `just image-push`  — multi-arch push for release (see below)

# CI-pinned amd64 image tagged cronduit:dev (do not change platform/tag)
[group('docker')]
image:
    docker buildx build \
        --platform linux/amd64 \
        --tag cronduit:dev \
        --load \
        .

# Non-amd64/arm64 hosts are not supported by the Dockerfile builder stage
# (which cross-compiles with cargo-zigbuild only for those two triples).
[group('docker')]
[doc('Build for the host platform and tag as the compose-consumed ghcr tag')]
image-local:
    #!/usr/bin/env bash
    set -euo pipefail
    ARCH=$(uname -m)
    case "$ARCH" in
      x86_64)        PLATFORM="linux/amd64" ;;
      arm64|aarch64) PLATFORM="linux/arm64" ;;
      *) echo "unsupported host arch: $ARCH (Dockerfile supports amd64/arm64 only)" >&2; exit 1 ;;
    esac
    echo "building cronduit for ${PLATFORM} and tagging ghcr.io/simplicityguy/cronduit:latest"
    docker buildx build \
        --platform "$PLATFORM" \
        --tag ghcr.io/simplicityguy/cronduit:latest \
        --load \
        .

# Validate multi-arch build without loading (buildx cannot --load a manifest list).
[group('docker')]
image-check:
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        --output type=cacheonly \
        .

# Multi-arch push for release. Usage: just image-push simplicityguy/cronduit:1.0.0
[group('docker')]
image-push tag:
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        --tag ghcr.io/{{tag}} \
        --push \
        .

# -------------------- quality gates --------------------

# Format all Rust sources in place
[group('quality')]
fmt:
    cargo fmt --all

# Verify formatting (CI gate)
[group('quality')]
fmt-check:
    cargo fmt --all -- --check

# Lint (CI gate: all-targets, all-features, warnings as errors)
[group('quality')]
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# cargo test — all features
[group('quality')]
test:
    cargo test --all-features

# nextest (CI gate) — faster test runner with the ci profile
[group('quality')]
nextest:
    cargo nextest run --all-features --profile ci

# Consumed by `openssl-check` locally and by Plan 07's ci.yml arm64 test cells
# via `run: just install-targets` — never `rustup target add` raw (D-10).
[group('quality')]
[doc('Install cross-compile targets for cargo-zigbuild (no-op if already installed)')]
install-targets:
    rustup target add aarch64-unknown-linux-musl
    rustup target add x86_64-unknown-linux-musl

# Pitfall 14 guard (FOUND-06) — MUST be empty for every target CI ships.
#
# `cargo tree -i` exits 0 regardless of match count, so a naive
# `cargo tree -i openssl-sys` PASSES even when openssl-sys IS present.
# The correct pattern pipes stdout to `grep -q .` and fails on any
# non-empty output.
#
# The recipe loops over native + amd64-musl + arm64-musl so a
# transitive `openssl-sys` that only appears under a musl-cross
# cfg can't slip through a native-only check. `install-targets`
# is a dependency so the targets are always present before
# `cargo tree --target` runs.
[group('quality')]
[doc('Verify openssl-sys is absent from the dep tree (rustls-only guard, Pitfall 14)')]
openssl-check: install-targets
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Verifying rustls-only TLS stack across native + cross-compile targets..."
    for target in "" "--target aarch64-unknown-linux-musl" "--target x86_64-unknown-linux-musl"; do
        label="${target:-native}"
        if cargo tree $target -i openssl-sys 2>/dev/null | grep -q .; then
            echo "FAIL: openssl-sys found in dep tree for target '${label}':"
            cargo tree $target -i openssl-sys || true
            exit 1
        fi
    done
    echo "OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)"

# -------------------- DB / schema --------------------

# Delete the dev SQLite database (WAL + SHM included)
[group('db')]
db-reset:
    rm -f cronduit.dev.db cronduit.dev.db-wal cronduit.dev.db-shm
    @echo "SQLite dev DB removed."

# NOTE: Phase 1 has no standalone migration command (D-01 deferred `cronduit migrate`
# to post-v1). Migrations run idempotently on `cronduit run` startup. This recipe is
# a convenience alias that starts the daemon in dev mode; press Ctrl+C after the
# `cronduit.startup` event has been logged and migrations are complete.
[group('db')]
[doc('Alias for `just dev` — migrations run idempotently on daemon startup')]
migrate: dev

# Regenerate .sqlx/ offline query cache (run before committing if SQL changed)
[group('db')]
sqlx-prepare:
    DATABASE_URL=sqlite://cronduit.dev.db cargo sqlx prepare --workspace

# Surface the schema parity test on its own (D-14).
[group('db')]
schema-diff:
    cargo test --test schema_parity -- --nocapture

# -------------------- dev loop --------------------

# Single-process dev loop (readable text logs, trace level for cronduit)
[group('dev')]
dev:
    RUST_LOG=debug,cronduit=trace cargo run -- run \
        --config examples/cronduit.toml \
        --log-format text

# Dev loop with Tailwind watch + cargo watch (D-15)
[group('dev')]
dev-ui:
    #!/usr/bin/env bash
    set -euo pipefail
    just tailwind  # ensure binary is downloaded
    echo "Starting Tailwind watch + cargo watch..."
    ./bin/tailwindcss -i assets/src/app.css -o assets/static/app.css --watch &
    TAILWIND_PID=$!
    trap "kill $TAILWIND_PID 2>/dev/null" EXIT
    RUST_LOG=debug,cronduit=trace cargo watch -x 'run -- run --config examples/cronduit.toml --log-format text'

# Validate a config file without starting the scheduler
[group('dev')]
check-config PATH:
    cargo run --quiet -- check {{PATH}}

# Bring up the full compose stack from examples/
[group('dev')]
docker-compose-up:
    docker compose -f examples/docker-compose.yml up

# -------------------- dependency updates (scripts/update-project.sh) --------------------

# Called by scripts/update-project.sh. Major upgrades go through cargo-edit
# directly and are NOT wrapped by a recipe — the script handles that path.
[group('deps')]
[doc('Refresh Cargo.lock within existing Cargo.toml constraints (minor/patch only)')]
update-cargo:
    cargo update

# No-op if pre-commit is not installed or .pre-commit-config.yaml is missing.
# Called by scripts/update-project.sh.
[group('deps')]
[doc('Update pre-commit hooks to their latest versions')]
update-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f .pre-commit-config.yaml ]; then
        echo "No .pre-commit-config.yaml — skipping hook updates"
        exit 0
    fi
    if ! command -v pre-commit >/dev/null 2>&1; then
        echo "pre-commit not installed — skipping hook updates"
        exit 0
    fi
    pre-commit autoupdate
