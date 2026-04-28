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

# Phase 13 OBS-05 structural parity guard — p50/p95 percentile is computed
# in Rust via src/web/stats.rs::percentile, NEVER via SQL-native percentile
# functions (even on Postgres). This CI gate permanently prevents any future
# PR from introducing `percentile_cont` / `percentile_disc` / `median(` /
# `PERCENTILE_` into src/.
#
# Rationale (OBS-05): structural parity requires the same code path on both
# SQLite and Postgres. SQLite lacks percentile_cont entirely; adopting the
# Postgres native function would fork the query layer. Rust-side computation
# is the only path, and this grep locks it.
[group('quality')]
[doc('OBS-05 guard — fail if any Rust file under src/ contains SQL-native percentile')]
grep-no-percentile-cont:
    #!/usr/bin/env bash
    set -euo pipefail
    # Match percentile SQL usage patterns but ignore comment lines (// or ///)
    # — doc comments that explicitly state "not used" are a legitimate
    # invariant declaration and must not trip the guard.
    pattern='\b(percentile_cont|percentile_disc|PERCENTILE_|median\()\b'
    # Use ripgrep-style: filter out lines whose first non-whitespace is `//`.
    matches=$(grep -rnE "$pattern" src/ 2>/dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
    if [ -n "$matches" ]; then
        echo "$matches"
        echo ""
        echo "ERROR: OBS-05 structural parity violated — SQL-native percentile found above."
        echo "Phase 13 locked: p50/p95 is computed in Rust via src/web/stats.rs::percentile."
        exit 1
    fi
    echo "OK: no percentile_cont / percentile_disc / median( / PERCENTILE_ in src/ (comments ignored)"

# Phase 15 / FOUND-16. Supply-chain hygiene gate: advisories + licenses +
# duplicate-versions in a single invocation. Non-blocking on rc.1
# (continue-on-error in ci.yml + bans.multiple-versions = "warn" in deny.toml);
# promoted to blocking before final v1.2.0 (Phase 24).
[group('quality')]
[doc('cargo-deny supply-chain check (advisories + licenses + bans)')]
deny:
    cargo deny check advisories licenses bans

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

# Phase 16 FOUND-14 spot check: print the container_id of the most recent
# job_runs row from the dev SQLite DB. The maintainer verifies this is a
# real Docker container ID (typically a 64-char hex string; 12-char prefix
# is also valid) and NOT a sha256:... digest (which would indicate the
# v1.0/v1.1 bug regressed). Targets cronduit.dev.db (the dev DB filename
# matches `db-reset` and `sqlx-prepare` above).
[group('db')]
[doc('Phase 16 FOUND-14 spot check — container_id MUST NOT start with sha256:')]
uat-fctx-bugfix-spot-check:
    @echo "Phase 16 / FOUND-14 spot check"
    @echo "Most recent job_run container_id (must NOT start with 'sha256:'):"
    @sqlite3 cronduit.dev.db "SELECT id, job_id, status, container_id, image_digest FROM job_runs ORDER BY id DESC LIMIT 1;"
    @echo ""
    @echo "Expected: container_id is a real Docker container ID (or NULL for non-docker runs)."
    @echo "FAIL if: container_id starts with 'sha256:' (would indicate the bug regressed)."

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

# -------------------- release candidate smoke --------------------
# DO NOT EDIT — paste recipes VERBATIM. just escapes the Docker `{{.Names}}`
# format string as `{{ "{{.Names}}" }}` because bare `{{...}}` is reserved for
# just's own interpolation. Removing the outer `{{ "..." }}` wrapper will make
# `just reload` fail at parse time with "Unknown identifier `.Names`".

# Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT.
# Phase 14 D-17 / feedback_uat_use_just_commands.
[group('release')]
[doc('Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT')]
compose-up-rc3:
    CRONDUIT_IMAGE=ghcr.io/simplicityguy/cronduit:1.1.0-rc.3 \
    docker compose -f examples/docker-compose.yml up -d

# Trigger a config reload of the running cronduit by SIGHUP.
# HUMAN-UAT steps 4 + 7 per D-17.
[group('release')]
[doc('Send SIGHUP to the running cronduit process (config reload)')]
reload:
    #!/usr/bin/env bash
    set -euo pipefail
    # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's
    # `{{ "{{.Names}}" }}` (the literal Docker --format token). DO NOT remove
    # the outer `{{ "..." }}` wrapper — see top-of-group comment.
    #
    # Resolve the container name. Accept either the literal `cronduit`
    # (if the operator set `container_name:` explicitly) OR a compose-named
    # container that carries our project's cronduit service — the stock
    # `examples/docker-compose.yml` produces `examples-cronduit-1`
    # (v2 hyphen form) or `examples_cronduit_1` (v1 underscore form).
    # Anchoring the grep at just `cronduit` caught only the former and
    # broke HUMAN-UAT Steps 4 + 7 under a default `docker compose up`
    # (Phase 14 UAT rc.5 gap).
    CRONDUIT_CID=$(docker ps --format '{{ "{{.Names}}" }}' \
        | grep -E '^(cronduit|.*[-_]cronduit[-_][0-9]+)$' \
        | head -1 || true)
    if [ -n "${CRONDUIT_CID}" ]; then
        docker kill -s HUP "${CRONDUIT_CID}"
        echo "SIGHUP sent to cronduit container (${CRONDUIT_CID})"
    else
        pkill -HUP cronduit && echo "SIGHUP sent to cronduit process" \
            || { echo "no running cronduit found"; exit 1; }
    fi

# Probe the running cronduit /health endpoint and print the status.
# HUMAN-UAT Step 1 — replaces raw `curl | jq` per Warning #8.
[group('release')]
[doc('Curl /health and print .status (expect "healthy")')]
health:
    curl -sf http://127.0.0.1:8080/health | jq -r '.status'

# Check key Prometheus metrics. HUMAN-UAT Step 8 — replaces raw curl per Warning #8.
# Prints scheduler liveness + runs_total series lines only (no noisy full dump).
[group('release')]
[doc('Grep /metrics for cronduit_scheduler_up and cronduit_runs_total lines')]
metrics-check:
    curl -sf http://127.0.0.1:8080/metrics \
        | grep -E '^cronduit_scheduler_up\b|^cronduit_runs_total\b'
