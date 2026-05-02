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

# Unit tests only (no integration tests; fast feedback loop per 18-VALIDATION.md sampling rate).
# Used by per-task feedback during Phase 18 execution; ~5-30s runtime depending on disk state.
[group('test')]
[doc('Run unit tests only — fast feedback (cargo test --lib)')]
test-unit:
    cargo test --lib --all-features

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

# ===== Phase 18 — webhook UAT recipes (per project memory feedback_uat_use_just_commands.md) =====

# Cross-cutting helper — trigger an immediate run of a job by integer ID
# via the running cronduit's HTTP API. The dashboard's Run Now button is
# CSRF-protected (cookie + form token, validated by `validate_csrf` in
# src/web/handlers/api.rs:run_now), so this recipe primes the cookie via
# a GET on /api/jobs first, then echoes the same token back in the form
# body when it POSTs to /api/jobs/{id}/run.
#
# Operators usually want to fire by NAME, not numeric ID — use
# `uat-webhook-fire` for that. This recipe stays ID-keyed so it matches
# the actual Axum handler signature (Path<i64>) and remains useful as a
# narrow building block for other automation (e.g., a future operator
# CLI). `jq` and `curl` are required.
[group('api')]
[doc('Trigger immediate run of JOB_ID via cronduit HTTP API (CSRF-aware curl wrapper)')]
api-run-now JOB_ID:
    #!/usr/bin/env bash
    set -euo pipefail
    JAR=$(mktemp)
    trap 'rm -f "$JAR"' EXIT
    # Prime the cronduit_csrf cookie via any GET — middleware sets it on cold requests.
    curl -sf -c "$JAR" "http://127.0.0.1:8080/api/jobs" >/dev/null \
      || { echo "cronduit unreachable on http://127.0.0.1:8080 — confirm 'just dev' is running"; exit 1; }
    # Cookie jar is Netscape format: domain flag path secure expiry NAME VALUE.
    TOKEN=$(awk '$6=="cronduit_csrf"{print $7}' "$JAR" | head -1)
    test -n "$TOKEN" || { echo "cronduit_csrf cookie not issued — middleware misconfigured?"; exit 1; }
    curl -sf -b "$JAR" -X POST -d "csrf_token=$TOKEN" \
      "http://127.0.0.1:8080/api/jobs/{{JOB_ID}}/run" >/dev/null \
      || { echo "Run Now POST failed (job_id={{JOB_ID}}) — check the cronduit log"; exit 1; }
    echo "OK: triggered run for job_id={{JOB_ID}}"

# Resolve a job NAME (the [[jobs]].name from cronduit.toml) to its
# integer database id. Operators authoring UAT scripts read the name
# from their config; the API is keyed on the numeric id.
[group('api')]
[doc('Resolve a JOB_NAME to its integer database id via /api/jobs (jq required)')]
api-job-id JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    JOB_ID=$(curl -sf "http://127.0.0.1:8080/api/jobs" \
      | jq -r --arg n "{{JOB_NAME}}" '.[] | select(.name == $n) | .id' \
      | head -1)
    test -n "$JOB_ID" \
      || { echo "Job '{{JOB_NAME}}' not found in cronduit — confirm 'just dev' is running and the job exists in your config"; exit 1; }
    echo "$JOB_ID"

# Mock HTTP receiver on 127.0.0.1:9999 — logs every request to stdout AND
# /tmp/cronduit-webhook-mock.log. Use Ctrl-C to stop.
[group('uat')]
[doc('Phase 18 — start mock HTTP receiver on 127.0.0.1:9999 (logs requests)')]
uat-webhook-mock:
    @echo "Starting webhook mock on http://127.0.0.1:9999/  (log: /tmp/cronduit-webhook-mock.log)"
    @echo "Maintainer: Ctrl-C to stop. Run 'just uat-webhook-verify' in another terminal."
    cargo run --example webhook_mock_server

# Force a 'Run Now' on a webhook-configured job — operator confirms one
# delivery lands at the mock receiver. Body composes `api-job-id`
# (NAME → numeric id lookup) with `api-run-now` (CSRF-aware POST), so
# the UAT-callable surface contains zero raw curl/cargo/docker.
[group('uat')]
[doc('Phase 18 — force Run Now on a webhook-configured job (operator-supplied JOB_NAME)')]
uat-webhook-fire JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: triggering run for {{JOB_NAME}} — watch the receiver and the cronduit log"
    JOB_ID=$(just api-job-id "{{JOB_NAME}}")
    just api-run-now "$JOB_ID"

# Print the last 30 lines of the mock receiver's log — maintainer hand-
# validates the 3 Standard Webhooks v1 headers, the 16-field payload, and
# the signature shape (v1,<base64>).
[group('uat')]
[doc('Phase 18 — print last 30 lines of webhook mock log for maintainer hand-validation')]
uat-webhook-verify:
    @echo "Last 30 lines from /tmp/cronduit-webhook-mock.log:"
    @echo "Maintainer: confirm headers (webhook-id, webhook-timestamp, webhook-signature), 16-field body, signature format v1,<base64>."
    @tail -n 30 /tmp/cronduit-webhook-mock.log 2>/dev/null || echo "(log empty; ensure 'just uat-webhook-mock' is running and 'just uat-webhook-fire <JOB>' was triggered)"

# === Phase 19 webhook receivers ===
# Plans 19-02 (Python), 19-03 (Go), 19-04 (Node) each append a per-language
# `uat-webhook-receiver-<lang>` + `uat-webhook-receiver-<lang>-verify-fixture` block AFTER this anchor.

# -------------------- Phase 19: webhook receivers --------------------
# Mirrors the Phase 18 uat-webhook-* family. Each language ships:
#   - uat-webhook-receiver-{lang}                (foreground; real cronduit delivery)
#   - uat-webhook-receiver-{lang}-verify-fixture  (CI gate; canonical + 3 tamper variants)

# Foreground Python receiver. Maintainer Ctrl-Cs; pair with `just dev` +
# `just uat-webhook-fire wh-example-receiver-python` in another terminal.
[group('uat')]
[doc('Phase 19 — start Python receiver on 127.0.0.1:9991 (logs to /tmp)')]
uat-webhook-receiver-python:
    @echo "Starting Python receiver on http://127.0.0.1:9991/"
    @echo "Maintainer: in another terminal, run 'just dev', then 'just uat-webhook-fire wh-example-receiver-python'."
    @echo "Watch this terminal for the 'verified' line. Ctrl-C to stop the receiver."
    WEBHOOK_SECRET_FILE=tests/fixtures/webhook-v1/secret.txt python3 examples/webhook-receivers/python/receiver.py

# Fixture-verify mode: canonical pass + 3 tamper variants must fail.
# Used by the GHA `webhook-interop` matrix (CI gate from day one).
[group('uat')]
[doc('Phase 19 — verify Python receiver against fixture (canonical + 3 tamper variants)')]
uat-webhook-receiver-python-verify-fixture:
    #!/usr/bin/env bash
    set -euo pipefail
    FIX=tests/fixtures/webhook-v1
    REC=examples/webhook-receivers/python/receiver.py

    # 1. Canonical — must verify
    python3 "$REC" --verify-fixture "$FIX" \
        || { echo "FAIL: canonical fixture did not verify"; exit 1; }

    # 2. Mutated secret — must FAIL
    BAD_SECRET=$(mktemp -d)
    cp "$FIX"/* "$BAD_SECRET"/
    printf 'WRONG' > "$BAD_SECRET"/secret.txt
    if python3 "$REC" --verify-fixture "$BAD_SECRET" 2>/dev/null; then
        echo "FAIL: mutated-secret variant verified — should have failed"; exit 1
    fi

    # 3. Mutated body — must FAIL
    BAD_BODY=$(mktemp -d)
    cp "$FIX"/* "$BAD_BODY"/
    sed -i.bak 's/"v1"/"X1"/' "$BAD_BODY"/payload.json && rm -f "$BAD_BODY"/payload.json.bak
    if python3 "$REC" --verify-fixture "$BAD_BODY" 2>/dev/null; then
        echo "FAIL: mutated-body variant verified — should have failed"; exit 1
    fi

    # 4. Mutated timestamp (HMAC mismatch via ts byte change) — must FAIL.
    # NB: fixture-verify mode skips the drift CHECK, but the HMAC is computed
    # over `${id}.${ts}.${body}` — mutating webhook-timestamp.txt while
    # leaving expected-signature.txt as the canonical sig produces an HMAC
    # mismatch for the (id, NEW_TS, body) tuple.
    # Drift detection itself (i.e. rejecting deliveries with |now - ts| > 300s)
    # is exercised by the U6/U7/U8 live UAT scenarios, NOT this recipe.
    BAD_TS=$(mktemp -d)
    cp "$FIX"/* "$BAD_TS"/
    printf '%s' "$(($(date +%s) - 600))" > "$BAD_TS"/webhook-timestamp.txt
    if python3 "$REC" --verify-fixture "$BAD_TS" 2>/dev/null; then
        echo "FAIL: mutated-timestamp variant verified — should have failed"; exit 1
    fi

    # 5. Wire-format strictness — non-canonical-decimal timestamp must STILL
    # verify (regression vector for review BL-01: receivers must sign over
    # the RAW `webhook-timestamp` header bytes, not a parsed-and-re-serialized
    # integer). We rewrite the timestamp to a leading-zero form
    # ("01735689600" — same int as canonical "1735689600", different bytes),
    # re-sign HMAC-SHA256 over `${id}.${NEW_TS}.${body}` using the canonical
    # secret, and confirm the receiver verifies. This locks "raw header
    # bytes" as the signing-string contract across all four runtimes.
    NONCANON_TS=$(mktemp -d)
    cp "$FIX"/* "$NONCANON_TS"/
    WID=$(cat "$FIX"/webhook-id.txt)
    NEW_WTS="01735689600"
    printf '%s' "$NEW_WTS" > "$NONCANON_TS"/webhook-timestamp.txt
    SECRET=$(cat "$FIX"/secret.txt)
    NEW_SIG=$({ printf '%s.%s.' "$WID" "$NEW_WTS"; cat "$FIX"/payload.json; } \
        | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)
    printf 'v1,%s' "$NEW_SIG" > "$NONCANON_TS"/expected-signature.txt
    python3 "$REC" --verify-fixture "$NONCANON_TS" \
        || { echo "FAIL: non-canonical-decimal timestamp variant did not verify (BL-01 regression)"; exit 1; }

    echo "OK: all 5 fixture variants behave correctly"

# Foreground Go receiver. Maintainer Ctrl-Cs; pair with `just dev` +
# `just uat-webhook-fire wh-example-receiver-go` in another terminal.
[group('uat')]
[doc('Phase 19 — start Go receiver on 127.0.0.1:9992 (logs to /tmp)')]
uat-webhook-receiver-go:
    @echo "Starting Go receiver on http://127.0.0.1:9992/"
    @echo "Maintainer: in another terminal, run 'just dev', then 'just uat-webhook-fire wh-example-receiver-go'."
    @echo "Watch this terminal for the 'verified' line. Ctrl-C to stop the receiver."
    WEBHOOK_SECRET_FILE=tests/fixtures/webhook-v1/secret.txt go run examples/webhook-receivers/go/receiver.go

# Fixture-verify mode: canonical pass + 3 tamper variants must fail.
# Used by the GHA `webhook-interop` matrix (CI gate from day one).
[group('uat')]
[doc('Phase 19 — verify Go receiver against fixture (canonical + 3 tamper variants)')]
uat-webhook-receiver-go-verify-fixture:
    #!/usr/bin/env bash
    set -euo pipefail
    FIX=tests/fixtures/webhook-v1
    REC=examples/webhook-receivers/go/receiver.go

    # 1. Canonical — must verify
    go run "$REC" --verify-fixture "$FIX" \
        || { echo "FAIL: canonical fixture did not verify"; exit 1; }

    # 2. Mutated secret — must FAIL
    BAD_SECRET=$(mktemp -d)
    cp "$FIX"/* "$BAD_SECRET"/
    printf 'WRONG' > "$BAD_SECRET"/secret.txt
    if go run "$REC" --verify-fixture "$BAD_SECRET" 2>/dev/null; then
        echo "FAIL: mutated-secret variant verified — should have failed"; exit 1
    fi

    # 3. Mutated body — must FAIL
    BAD_BODY=$(mktemp -d)
    cp "$FIX"/* "$BAD_BODY"/
    sed -i.bak 's/"v1"/"X1"/' "$BAD_BODY"/payload.json && rm -f "$BAD_BODY"/payload.json.bak
    if go run "$REC" --verify-fixture "$BAD_BODY" 2>/dev/null; then
        echo "FAIL: mutated-body variant verified — should have failed"; exit 1
    fi

    # 4. Mutated timestamp (HMAC mismatch via ts byte change) — must FAIL.
    # NB: fixture-verify mode skips the drift CHECK, but the HMAC is computed
    # over `${id}.${ts}.${body}` — mutating webhook-timestamp.txt while
    # leaving expected-signature.txt as the canonical sig produces an HMAC
    # mismatch for the (id, NEW_TS, body) tuple.
    # Drift detection itself (i.e. rejecting deliveries with |now - ts| > 300s)
    # is exercised by the U6/U7/U8 live UAT scenarios, NOT this recipe.
    BAD_TS=$(mktemp -d)
    cp "$FIX"/* "$BAD_TS"/
    printf '%s' "$(($(date +%s) - 600))" > "$BAD_TS"/webhook-timestamp.txt
    if go run "$REC" --verify-fixture "$BAD_TS" 2>/dev/null; then
        echo "FAIL: mutated-timestamp variant verified — should have failed"; exit 1
    fi

    # 5. Wire-format strictness — non-canonical-decimal timestamp must STILL
    # verify (regression vector for review BL-01: receivers must sign over
    # the RAW `webhook-timestamp` header bytes, not a parsed-and-re-serialized
    # integer). We rewrite the timestamp to a leading-zero form
    # ("01735689600" — same int as canonical "1735689600", different bytes),
    # re-sign HMAC-SHA256 over `${id}.${NEW_TS}.${body}` using the canonical
    # secret, and confirm the receiver verifies. This locks "raw header
    # bytes" as the signing-string contract across all four runtimes.
    NONCANON_TS=$(mktemp -d)
    cp "$FIX"/* "$NONCANON_TS"/
    WID=$(cat "$FIX"/webhook-id.txt)
    NEW_WTS="01735689600"
    printf '%s' "$NEW_WTS" > "$NONCANON_TS"/webhook-timestamp.txt
    SECRET=$(cat "$FIX"/secret.txt)
    NEW_SIG=$({ printf '%s.%s.' "$WID" "$NEW_WTS"; cat "$FIX"/payload.json; } \
        | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)
    printf 'v1,%s' "$NEW_SIG" > "$NONCANON_TS"/expected-signature.txt
    go run "$REC" --verify-fixture "$NONCANON_TS" \
        || { echo "FAIL: non-canonical-decimal timestamp variant did not verify (BL-01 regression)"; exit 1; }

    echo "OK: all 5 fixture variants behave correctly"

# Foreground Node receiver. Maintainer Ctrl-Cs; pair with `just dev` +
# `just uat-webhook-fire wh-example-receiver-node` in another terminal.
[group('uat')]
[doc('Phase 19 — start Node receiver on 127.0.0.1:9993 (logs to /tmp)')]
uat-webhook-receiver-node:
    @echo "Starting Node receiver on http://127.0.0.1:9993/"
    @echo "Maintainer: in another terminal, run 'just dev', then 'just uat-webhook-fire wh-example-receiver-node'."
    @echo "Watch this terminal for the 'verified' line. Ctrl-C to stop the receiver."
    WEBHOOK_SECRET_FILE=tests/fixtures/webhook-v1/secret.txt node examples/webhook-receivers/node/receiver.js

# Fixture-verify mode: canonical pass + 3 tamper variants must fail.
# Used by the GHA `webhook-interop` matrix (CI gate from day one).
[group('uat')]
[doc('Phase 19 — verify Node receiver against fixture (canonical + 3 tamper variants)')]
uat-webhook-receiver-node-verify-fixture:
    #!/usr/bin/env bash
    set -euo pipefail
    FIX=tests/fixtures/webhook-v1
    REC=examples/webhook-receivers/node/receiver.js

    # 1. Canonical — must verify
    node "$REC" --verify-fixture "$FIX" \
        || { echo "FAIL: canonical fixture did not verify"; exit 1; }

    # 2. Mutated secret — must FAIL
    BAD_SECRET=$(mktemp -d)
    cp "$FIX"/* "$BAD_SECRET"/
    printf 'WRONG' > "$BAD_SECRET"/secret.txt
    if node "$REC" --verify-fixture "$BAD_SECRET" 2>/dev/null; then
        echo "FAIL: mutated-secret variant verified — should have failed"; exit 1
    fi

    # 3. Mutated body — must FAIL
    BAD_BODY=$(mktemp -d)
    cp "$FIX"/* "$BAD_BODY"/
    sed -i.bak 's/"v1"/"X1"/' "$BAD_BODY"/payload.json && rm -f "$BAD_BODY"/payload.json.bak
    if node "$REC" --verify-fixture "$BAD_BODY" 2>/dev/null; then
        echo "FAIL: mutated-body variant verified — should have failed"; exit 1
    fi

    # 4. Mutated timestamp (HMAC mismatch via ts byte change) — must FAIL.
    # NB: fixture-verify mode skips the drift CHECK, but the HMAC is computed
    # over `${id}.${ts}.${body}` — mutating webhook-timestamp.txt while
    # leaving expected-signature.txt as the canonical sig produces an HMAC
    # mismatch for the (id, NEW_TS, body) tuple.
    # Drift detection itself (i.e. rejecting deliveries with |now - ts| > 300s)
    # is exercised by the U6/U7/U8 live UAT scenarios, NOT this recipe.
    BAD_TS=$(mktemp -d)
    cp "$FIX"/* "$BAD_TS"/
    printf '%s' "$(($(date +%s) - 600))" > "$BAD_TS"/webhook-timestamp.txt
    if node "$REC" --verify-fixture "$BAD_TS" 2>/dev/null; then
        echo "FAIL: mutated-timestamp variant verified — should have failed"; exit 1
    fi

    # 5. Wire-format strictness — non-canonical-decimal timestamp must STILL
    # verify (regression vector for review BL-01: receivers must sign over
    # the RAW `webhook-timestamp` header bytes, not a parsed-and-re-serialized
    # integer). We rewrite the timestamp to a leading-zero form
    # ("01735689600" — same int as canonical "1735689600", different bytes),
    # re-sign HMAC-SHA256 over `${id}.${NEW_TS}.${body}` using the canonical
    # secret, and confirm the receiver verifies. This locks "raw header
    # bytes" as the signing-string contract across all four runtimes.
    NONCANON_TS=$(mktemp -d)
    cp "$FIX"/* "$NONCANON_TS"/
    WID=$(cat "$FIX"/webhook-id.txt)
    NEW_WTS="01735689600"
    printf '%s' "$NEW_WTS" > "$NONCANON_TS"/webhook-timestamp.txt
    SECRET=$(cat "$FIX"/secret.txt)
    NEW_SIG=$({ printf '%s.%s.' "$WID" "$NEW_WTS"; cat "$FIX"/payload.json; } \
        | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)
    printf 'v1,%s' "$NEW_SIG" > "$NONCANON_TS"/expected-signature.txt
    node "$REC" --verify-fixture "$NONCANON_TS" \
        || { echo "FAIL: non-canonical-decimal timestamp variant did not verify (BL-01 regression)"; exit 1; }

    echo "OK: all 5 fixture variants behave correctly"

# === Phase 20 webhook posture (D-34) ===
# Plan 20-08 — 4 maintainer-facing recipes for the rc.1 UAT runbook
# (`.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-HUMAN-UAT.md`).
# Composes the P18 baseline (`uat-webhook-mock` / `uat-webhook-fire` /
# `uat-webhook-verify`) and adds two helper mocks (`uat-webhook-mock-500`
# returns 500 for the retry chain; `uat-webhook-mock-slow` delays 5s for the
# drain-on-shutdown scenario). Both helper mocks use Python 3 stdlib
# (`http.server.BaseHTTPRequestHandler`) — `python3` is already a documented
# Phase 19 prerequisite (see 19-HUMAN-UAT.md). The Rust example
# `examples/webhook_mock_server.rs` hardcodes port 9999 + status 200 and
# does NOT accept `--port` / `--status` flags; extending it just to support
# UAT mode-switching is out of scope for Plan 20-08 (per project memory
# `feedback_uat_use_just_commands.md` the recipe set is the deliverable, not
# a reshape of the example binary).

# Mock receiver returning 500 for ALL POSTs — drives the retry chain UAT.
# Logs every request to stdout AND /tmp/cronduit-webhook-mock-500.log.
# Pair with `just uat-webhook-retry <JOB>` (see below).
[group('uat')]
[doc('Phase 20 — start mock HTTP receiver on 127.0.0.1:9999 returning 500 (forces 3-attempt retry chain)')]
uat-webhook-mock-500:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: starting mock receiver on http://127.0.0.1:9999/ returning 500 for ALL POSTs"
    echo "▶ Logs streaming to /tmp/cronduit-webhook-mock-500.log"
    echo "▶ Press Ctrl-C to stop."
    : > /tmp/cronduit-webhook-mock-500.log
    # Heredoc body is indented to match the recipe block (just requires this);
    # `sed 's/^    //'` strips the 4-space recipe prefix so Python sees a
    # zero-indent module. Same trick used in the slow-mock recipe below.
    SCRIPT=$(mktemp /tmp/cronduit-webhook-mock-500-XXXXXX.py)
    trap 'rm -f "$SCRIPT"' EXIT
    sed 's/^    //' > "$SCRIPT" <<'PYEOF'
    import http.server, sys, datetime
    LOG = "/tmp/cronduit-webhook-mock-500.log"
    class H(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n) if n else b""
            line = "%s %s %s 500 bytes=%d\n" % (
                datetime.datetime.utcnow().isoformat() + "Z",
                self.command, self.path, len(body),
            )
            sys.stderr.write(line); sys.stderr.flush()
            with open(LOG, "a") as f:
                f.write(line)
            self.send_response(500)
            self.send_header("Connection", "close")
            self.send_header("Content-Length", "12")
            self.end_headers()
            self.wfile.write(b"server-fail\n")
        def log_message(self, *a, **k):
            pass
    http.server.HTTPServer(("127.0.0.1", 9999), H).serve_forever()
    PYEOF
    python3 -u "$SCRIPT" 2>&1 | tee -a /tmp/cronduit-webhook-mock-500.log

# Mock receiver returning 200 after a 5s sleep — exercises drain-on-shutdown.
# Operator pairs this with `just uat-webhook-drain` (see below).
[group('uat')]
[doc('Phase 20 — start slow mock HTTP receiver on 127.0.0.1:9999 (200 after 5s sleep; for drain UAT)')]
uat-webhook-mock-slow:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: starting SLOW mock receiver on http://127.0.0.1:9999/ (200 after 5s sleep)"
    echo "▶ Logs streaming to /tmp/cronduit-webhook-mock-slow.log"
    echo "▶ Press Ctrl-C to stop."
    : > /tmp/cronduit-webhook-mock-slow.log
    SCRIPT=$(mktemp /tmp/cronduit-webhook-mock-slow-XXXXXX.py)
    trap 'rm -f "$SCRIPT"' EXIT
    sed 's/^    //' > "$SCRIPT" <<'PYEOF'
    import http.server, sys, time, datetime
    LOG = "/tmp/cronduit-webhook-mock-slow.log"
    class H(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n) if n else b""
            recv = datetime.datetime.utcnow().isoformat() + "Z"
            sys.stderr.write("%s RECV %s bytes=%d (sleeping 5s)\n" % (recv, self.path, len(body)))
            sys.stderr.flush()
            time.sleep(5)
            sent = datetime.datetime.utcnow().isoformat() + "Z"
            line = "%s SENT 200 %s bytes=%d\n" % (sent, self.path, len(body))
            sys.stderr.write(line); sys.stderr.flush()
            with open(LOG, "a") as f:
                f.write(line)
            self.send_response(200)
            self.send_header("Connection", "close")
            self.send_header("Content-Length", "3")
            self.end_headers()
            self.wfile.write(b"ok\n")
        def log_message(self, *a, **k):
            pass
    http.server.HTTPServer(("127.0.0.1", 9999), H).serve_forever()
    PYEOF
    python3 -u "$SCRIPT" 2>&1 | tee -a /tmp/cronduit-webhook-mock-slow.log

# Trigger a webhook-configured job pointed at the 500-mock and instruct the
# maintainer how to verify the 3-attempt retry chain landed in
# /tmp/cronduit-webhook-mock-500.log + the DLQ. Recipe-calls-recipe: composes
# `uat-webhook-fire` (P18) → operator inspection → `uat-webhook-dlq-query`.
[group('uat')]
[doc('Phase 20 — fire a job pointed at the 500-mock + verify retry chain produced 3 attempts')]
uat-webhook-retry JOB_NAME:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: triggering retry chain for {{JOB_NAME}}"
    echo "▶ PRE: 'just uat-webhook-mock-500' must be running in another terminal."
    echo "▶ PRE: 'just dev' must be running with {{JOB_NAME}} configured to POST"
    echo "▶      to http://127.0.0.1:9999/ (the 500-mock)."
    just uat-webhook-fire "{{JOB_NAME}}"
    echo ""
    echo "▶ Wait ~6 minutes (full chain = t=0 + ~30s + ~300s, plus 10s reqwest cap per attempt)."
    echo "▶ Then verify in another terminal:"
    echo "▶   tail -n 30 /tmp/cronduit-webhook-mock-500.log"
    echo "▶ EXPECT: 3 lines like 'POST /<path> 500 bytes=<N>' (one per retry attempt)."
    echo "▶ Then run:"
    echo "▶   just uat-webhook-dlq-query"
    echo "▶ EXPECT: at least one webhook_deliveries row with attempts=3,"
    echo "▶         dlq_reason='http_5xx'."

# Document the manual-SIGTERM drain scenario. Cronduit must observe a worst-
# case shutdown ceiling = `webhook_drain_grace + 10s` (D-18 / docs/WEBHOOKS.md
# § Drain on shutdown). This recipe prints the procedure; the maintainer
# coordinates the 4 terminals and times the actual shutdown.
[group('uat')]
[doc('Phase 20 — drain-on-shutdown UAT: send SIGTERM during in-flight delivery; verify worst-case ceiling = drain_grace + 10s')]
uat-webhook-drain:
    @echo "▶ UAT: drain-on-shutdown scenario (D-15 / D-18 / WH-10)."
    @echo ""
    @echo "▶ Step 1: in terminal A:"
    @echo "▶          just uat-webhook-mock-slow      # 200 after 5s sleep"
    @echo ""
    @echo "▶ Step 2: in terminal B (with a webhook-configured job pointed at 127.0.0.1:9999):"
    @echo "▶          just dev"
    @echo ""
    @echo "▶ Step 3: in terminal C, trigger an immediate run:"
    @echo "▶          just uat-webhook-fire <JOB_NAME>"
    @echo ""
    @echo "▶ Step 4: within ~1s, send Ctrl-C (SIGINT) to terminal B."
    @echo "▶          Time the shutdown with a wall clock (or use 'time' in front of"
    @echo "▶          'just dev' and read the 'real' value at exit)."
    @echo ""
    @echo "▶ EXPECT (per docs/WEBHOOKS.md § Drain on shutdown):"
    @echo "▶   - cronduit logs 'webhook worker entering drain mode (budget: 30s)'"
    @echo "▶   - the in-flight POST completes (terminal A logs 'SENT 200 ...')"
    @echo "▶   - any queued events not yet sent at budget expiry yield"
    @echo "▶     cronduit_webhook_deliveries_total{status=\"dropped\"} increments +"
    @echo "▶     a webhook_deliveries row with dlq_reason='shutdown_drain'"
    @echo "▶   - total shutdown time ≤ webhook_drain_grace (default 30s) + reqwest"
    @echo "▶     per-attempt cap (10s) ≈ 40s worst case"
    @echo ""
    @echo "▶ Maintainer: confirm timings match docs/WEBHOOKS.md and tick the box"
    @echo "▶            in 20-HUMAN-UAT.md Scenario 4."

# Print recent rows from the webhook_deliveries DLQ table on the dev SQLite
# DB. Mirrors the existing `uat-fctx-bugfix-spot-check` precedent (sqlite3
# directly against cronduit.dev.db). Operators inspect the DLQ via SQL only
# in v1.2 — no UI surface (D-37); the dashboard query lands in v1.3.
[group('uat')]
[doc('Phase 20 — query webhook_deliveries DLQ table from the dev SQLite DB (last 1 hour)')]
uat-webhook-dlq-query:
    #!/usr/bin/env bash
    set -euo pipefail
    DB="${CRONDUIT_DEV_DB:-cronduit.dev.db}"
    if [[ ! -f "$DB" ]]; then
        echo "ERROR: $DB not found — run 'just dev' first to create + migrate it,"
        echo "       or set CRONDUIT_DEV_DB=path/to/your.db before invoking this recipe."
        exit 1
    fi
    echo "▶ UAT: webhook_deliveries DLQ rows in the last 1 hour (DB: $DB)"
    sqlite3 -header -column "$DB" \
        "SELECT id, run_id, job_id, attempts, last_status, dlq_reason, last_attempt_at \
         FROM webhook_deliveries \
         WHERE last_attempt_at > datetime('now', '-1 hour') \
         ORDER BY last_attempt_at DESC \
         LIMIT 50;"
    echo ""
    echo "▶ EXPECT (after 'just uat-webhook-retry <JOB>'):"
    echo "▶   - At least 1 row with attempts=3, dlq_reason='http_5xx'"
    echo "▶ EXPECT (after 'just uat-webhook-drain' with mid-chain SIGTERM):"
    echo "▶   - At least 1 row with dlq_reason='shutdown_drain'"

# Verify the LOAD-time HTTPS-required validator (D-19 / WH-07) rejects an
# `http://` URL pointing at a non-loopback / non-RFC1918 host. Builds a
# minimal cronduit.toml in /tmp, runs `cargo run -- check`, asserts the
# command fails. The cronduit binary is the cronduit binary — operators do
# the same thing when they 'cronduit check ./cronduit.toml' against a bad
# config (per src/cli/mod.rs::Command::Check). 'cargo run' is the just-recipe-
# blessed entry point for the daemon binary throughout this justfile (see
# 'just dev', 'just check-config').
[group('uat')]
[doc('Phase 20 — verify HTTPS-required validator rejects http://example.com at config-load')]
uat-webhook-https-required:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: HTTPS-required validator (D-19 / WH-07)"
    TMP=$(mktemp /tmp/cronduit-bad-webhook-XXXXXX.toml)
    trap 'rm -f "$TMP"' EXIT
    cat > "$TMP" <<'TOML'
    [server]
    timezone      = "UTC"
    bind          = "127.0.0.1:8080"
    database_url  = "sqlite::memory:"

    [[jobs]]
    name     = "uat-https-required"
    schedule = "* * * * *"
    type     = "command"
    command  = "echo hi"
    webhook  = { url = "http://example.com/hook" }
    TOML
    echo "▶ Wrote bad config to $TMP — running 'cargo run --quiet -- check $TMP'"
    if cargo run --quiet -- check "$TMP" 2>&1; then
        echo "▶ FAIL: 'cronduit check' unexpectedly succeeded — the validator did NOT"
        echo "▶       reject http://example.com (regression of D-19 / WH-07)."
        exit 1
    fi
    echo "▶ PASS: 'cronduit check' rejected http://example.com with non-zero exit."

# Surface the Phase 20 cronduit_webhook_* metric family from the running
# cronduit's /metrics endpoint. Distinct from `metrics-check` (P14, which
# greps cronduit_scheduler_up + cronduit_runs_total only) — that recipe is
# v1.1's HUMAN-UAT contract and is intentionally NOT widened. This recipe
# is the Phase 20 / WH-11 maintainer-validation surface for the labeled
# deliveries family + duration histogram + queue depth gauge + the P15
# saturation drop counter (preserved per D-26).
[group('uat')]
[doc('Phase 20 — grep /metrics for cronduit_webhook_* family (WH-11 surface)')]
uat-webhook-metrics-check:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ UAT: WH-11 metrics surface — cronduit_webhook_* family from /metrics"
    echo "▶ PRE: 'just dev' must be running and reachable at http://127.0.0.1:8080"
    curl -sf http://127.0.0.1:8080/metrics \
        | grep -E '^(# (HELP|TYPE) )?cronduit_webhook_(deliveries_total|delivery_duration_seconds|queue_depth|delivery_dropped_total)\b' \
        || { echo "▶ FAIL: no cronduit_webhook_* metrics surfaced — confirm 'just dev' is running."; exit 1; }
    echo ""
    echo "▶ EXPECT (per docs/WEBHOOKS.md § Metrics family):"
    echo "▶   - cronduit_webhook_deliveries_total{job=\"<name>\",status=\"success\"} ≥ 0"
    echo "▶   - cronduit_webhook_deliveries_total{job=\"<name>\",status=\"failed\"}  ≥ 0"
    echo "▶   - cronduit_webhook_deliveries_total{job=\"<name>\",status=\"dropped\"} ≥ 0"
    echo "▶   - cronduit_webhook_delivery_duration_seconds_bucket{le=\"...\"}        (histogram)"
    echo "▶   - cronduit_webhook_queue_depth                                         (gauge)"
    echo "▶   - cronduit_webhook_delivery_dropped_total                              (P15 counter, preserved per D-26)"

# Confirm the rustls invariant — no openssl-sys in the dep tree across
# native + linux/amd64-musl + linux/arm64-musl. This is a thin wrapper
# around the existing `openssl-check` recipe that surfaces it under the
# [uat] group with a Phase-20-flavored doc string so 20-HUMAN-UAT.md
# Scenario 6 maps cleanly. The actual check logic lives in
# `openssl-check` (Pitfall 14 / FOUND-06) — DO NOT duplicate it here.
[group('uat')]
[doc('Phase 20 — verify rustls invariant (cargo tree -i openssl-sys empty across all targets; D-38)')]
uat-webhook-rustls-check:
    @echo "▶ UAT: rustls invariant (D-38) — delegating to 'just openssl-check'"
    @just openssl-check

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

# === Phase 21 — failure-context UI panel + exit-code histogram UAT recipes ===
# Plan 21-10. Maintainer-facing runbooks for the rc.2 HUMAN-UAT scenarios in
# `21-HUMAN-UAT.md`. Recipes follow the project memory rule
# `feedback_uat_use_just_commands.md` — every step is either a `just` recipe
# call or a raw `sqlite3 cronduit.dev.db ...` inspection (the
# `uat-fctx-bugfix-spot-check` precedent). NO new primitives are introduced;
# the four recipes below compose the existing `dev`, `db-reset`, `api-job-id`,
# and `api-run-now` recipes only (per Phase 21 D-19 / D-29 + research §F/§G).

# Seed N consecutive failed runs against the docker example job, then walk
# the maintainer to the run-detail page to verify the FCTX panel renders
# with all 5 rows and is collapsed by default.
#
# Recipe-calls-recipe per D-19: `db-reset` -> operator runs `just dev` ->
# `api-job-id` -> raw sqlite3 inserts (fixture seed) -> URL handed to operator.
[group('uat')]
[doc('Phase 21 — FCTX panel walk-through (seed N failed runs, then visit /jobs/{id}/runs/{id})')]
uat-fctx-panel:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ Phase 21 UAT: failure-context panel walk-through"
    echo ""
    echo "Step 1: Reset DB."
    just db-reset
    echo ""
    echo "Step 2: Start cronduit in another terminal:    just dev"
    echo "        Wait until you see 'Listening on 127.0.0.1:8080'."
    echo "        Then PRESS ENTER here to continue."
    read
    echo ""
    echo "Step 3: Resolve job id for the docker example job (or any seeded job)."
    JOB_ID=$(just api-job-id "fire-skew-demo")
    echo "        JOB_ID=$JOB_ID"
    echo ""
    echo "Step 4: Seed 4 consecutive failed runs via raw sqlite3 (mirrors fixture shape)."
    for i in 1 2 3 4; do
      sqlite3 cronduit.dev.db "INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, job_run_number, image_digest, config_hash, scheduled_for) VALUES ($JOB_ID, 'failed', 'manual', datetime('now', '-' || $i || ' minutes'), datetime('now', '-' || $i || ' minutes', '+30 seconds'), 30000, 1, $i, NULL, 'seed-hash', datetime('now', '-' || $i || ' minutes'));"
    done
    echo ""
    LATEST_RUN_ID=$(sqlite3 cronduit.dev.db "SELECT id FROM job_runs WHERE job_id=$JOB_ID ORDER BY id DESC LIMIT 1;")
    echo "Step 5: Visit the run-detail page in your browser:"
    echo "        http://127.0.0.1:8080/jobs/$JOB_ID/runs/$LATEST_RUN_ID"
    echo ""
    echo "Expected: collapsed-by-default 'Failure context' panel with 4 consecutive failures meta."
    echo "          Click to expand → 5 rows visible (or 4 if the job is non-docker)."

# Seed mixed exit-code runs against the docker example job, then walk the
# maintainer to the job-detail page to verify the Exit-Code Distribution
# card renders with bucket distribution + success-rate stat + recent codes.
#
# EXIT-04 dual-classifier coverage: one row with status='stopped' + exit=137
# (cronduit SIGKILL → BucketStopped) and one with status='failed' + exit=137
# (external signal → Bucket128to143). Verifies the locked classifier (D-08).
#
# Recipe-calls-recipe per D-19: `db-reset` -> operator runs `just dev` ->
# `api-job-id` -> raw sqlite3 inserts (fixture seed) -> URL handed to operator.
[group('uat')]
[doc('Phase 21 — Exit-code histogram walk-through (seed mixed exit codes, visit /jobs/{id})')]
uat-exit-histogram:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ Phase 21 UAT: exit-code histogram walk-through"
    echo ""
    echo "Step 1: Reset DB."
    just db-reset
    echo ""
    echo "Step 2: Start cronduit:    just dev   (then PRESS ENTER)"
    read
    echo ""
    echo "Step 3: Resolve job id."
    JOB_ID=$(just api-job-id "fire-skew-demo")
    echo "        JOB_ID=$JOB_ID"
    echo ""
    echo "Step 4: Seed mixed exit-code runs via raw sqlite3 (covers EXIT-04 dual-classifier — stopped vs failed @ 137):"
    # 5 success, 3 failed exit=1, 1 failed exit=127, 1 stopped exit=137 (cronduit SIGKILL), 1 failed exit=137 (external signal)
    sqlite3 cronduit.dev.db "
      INSERT INTO job_runs (job_id, status, trigger, start_time, end_time, duration_ms, exit_code, job_run_number, image_digest, config_hash, scheduled_for) VALUES
        ($JOB_ID, 'success', 'manual', datetime('now', '-10 minutes'), datetime('now', '-10 minutes', '+30 seconds'), 30000, 0, 1, NULL, 'h', datetime('now', '-10 minutes')),
        ($JOB_ID, 'success', 'manual', datetime('now', '-9 minutes'), datetime('now', '-9 minutes', '+30 seconds'), 30000, 0, 2, NULL, 'h', datetime('now', '-9 minutes')),
        ($JOB_ID, 'success', 'manual', datetime('now', '-8 minutes'), datetime('now', '-8 minutes', '+30 seconds'), 30000, 0, 3, NULL, 'h', datetime('now', '-8 minutes')),
        ($JOB_ID, 'success', 'manual', datetime('now', '-7 minutes'), datetime('now', '-7 minutes', '+30 seconds'), 30000, 0, 4, NULL, 'h', datetime('now', '-7 minutes')),
        ($JOB_ID, 'success', 'manual', datetime('now', '-6 minutes'), datetime('now', '-6 minutes', '+30 seconds'), 30000, 0, 5, NULL, 'h', datetime('now', '-6 minutes')),
        ($JOB_ID, 'failed', 'manual', datetime('now', '-5 minutes'), datetime('now', '-5 minutes', '+30 seconds'), 30000, 1, 6, NULL, 'h', datetime('now', '-5 minutes')),
        ($JOB_ID, 'failed', 'manual', datetime('now', '-4 minutes'), datetime('now', '-4 minutes', '+30 seconds'), 30000, 1, 7, NULL, 'h', datetime('now', '-4 minutes')),
        ($JOB_ID, 'failed', 'manual', datetime('now', '-3 minutes'), datetime('now', '-3 minutes', '+30 seconds'), 30000, 127, 8, NULL, 'h', datetime('now', '-3 minutes')),
        ($JOB_ID, 'stopped', 'manual', datetime('now', '-2 minutes'), datetime('now', '-2 minutes', '+30 seconds'), 30000, 137, 9, NULL, 'h', datetime('now', '-2 minutes')),
        ($JOB_ID, 'failed', 'manual', datetime('now', '-1 minutes'), datetime('now', '-1 minutes', '+30 seconds'), 30000, 137, 10, NULL, 'h', datetime('now', '-1 minutes'));"
    echo ""
    echo "Step 5: Visit the job-detail page:"
    echo "        http://127.0.0.1:8080/jobs/$JOB_ID"
    echo ""
    echo "Expected: 'Exit Code Distribution' card visible (sibling to Duration card)."
    echo "          - SUCCESS stat: 50% (5 of 10 — denominator excludes 1 stopped: 5/(10-1)=5/9 ≈ 56% — verify per D-09)."
    echo "          - Bars: bucket 1 (count 2, err-strong), 127 (1, warn yellow), 128-143 (1, warn yellow — external 137), stopped (1, slate grey)."
    echo "          - Recent codes: 1, 137, 127."
    echo "          - Stopped tooltip on hover: 'NOT a crash'."

# Slow-start docker container demonstrating FCTX-06 fire-skew (research §F:
# closest to operator reality, no new test infra required). The fire-skew-demo
# job (added to examples/cronduit.toml in plan 21-10 task 1) sleeps 30s before
# completing, so its `start_time` lands ~30s after `scheduled_for` -> +30000ms
# skew visible on the run-detail FCTX panel FIRE SKEW row.
#
# Recipe-calls-recipe per D-19: `db-reset` -> example-config check ->
# operator runs `just dev` -> raw sqlite3 inspect -> `api-job-id` -> URL.
[group('uat')]
[doc('Phase 21 — Fire-skew walk-through (slow-start docker container; expects ~+30000ms skew)')]
uat-fire-skew:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ Phase 21 UAT: fire-skew walk-through"
    echo ""
    echo "Step 1: Reset DB."
    just db-reset
    echo ""
    echo "Step 2: Ensure examples/cronduit.toml contains the 'fire-skew-demo' job (Phase 21 added this)."
    if grep -q 'name = "fire-skew-demo"' examples/cronduit.toml; then
        echo "        ✓ fire-skew-demo present"
    else
        echo "        ✗ fire-skew-demo missing — see Phase 21 plan 21-10 task 1"
        exit 1
    fi
    echo ""
    echo "Step 3: Start cronduit pointing at the example config:"
    echo "        just dev   (then PRESS ENTER)"
    read
    echo ""
    echo "Step 4: Wait for the next * * * * * cron tick — fire-skew-demo will fire."
    echo "        The container will sleep 30s before completing → start_time will be ~30s after scheduled_for."
    echo "        Wait at least 90 seconds, then continue."
    echo ""
    echo "Step 5: Inspect the latest run:"
    sqlite3 cronduit.dev.db "SELECT id, job_id, scheduled_for, start_time, end_time FROM job_runs WHERE job_id IN (SELECT id FROM jobs WHERE name='fire-skew-demo') ORDER BY id DESC LIMIT 1;"
    echo ""
    JOB_ID=$(just api-job-id "fire-skew-demo" 2>/dev/null || echo "MISSING")
    if [ "$JOB_ID" != "MISSING" ]; then
      LATEST_RUN_ID=$(sqlite3 cronduit.dev.db "SELECT id FROM job_runs WHERE job_id=$JOB_ID ORDER BY id DESC LIMIT 1;")
      echo "Step 6: Visit:"
      echo "        http://127.0.0.1:8080/jobs/$JOB_ID/runs/$LATEST_RUN_ID"
      echo ""
      echo "Expected: FIRE SKEW row reads approximately 'Scheduled: HH:MM:00 • Started: HH:MM:30 (+30000 ms)'."
      echo "          (Run probably succeeds since command is 'sleep 30 && echo done'."
      echo "           To see the FCTX panel itself, edit the job to exit 1 and re-run.)"
    fi

# Umbrella accessibility runbook (research §G — single recipe walking the
# maintainer through 4 a11y phases in one browser session). Covers the
# UI-SPEC § Accessibility contract: mobile viewport (<640px stack/scroll),
# light-mode rendering, print-mode panel-open, keyboard-only navigation.
#
# This is a guided echo-only recipe (no fixture seeding) — the operator
# brings their own failed run from `just uat-fctx-panel` first.
[group('uat')]
[doc('Phase 21 — Accessibility umbrella (Mobile / Light-mode / Print / Keyboard scenarios)')]
uat-fctx-a11y:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▶ Phase 21 UAT: accessibility umbrella (4 phases — single browser session)"
    echo ""
    echo "Prerequisite: a failed run exists. If not, run \`just uat-fctx-panel\` first."
    echo ""
    echo "Phase 1 (Mobile viewport, <640px):"
    echo "  - Open http://127.0.0.1:8080/jobs/{ID}/runs/{ID} in browser."
    echo "  - Open DevTools → device toolbar → set viewport to 375×667 (iPhone)."
    echo "  - Expand the FCTX panel. Confirm rows STACK (1-column layout)."
    echo "  - Visit /jobs/{ID}. Confirm the histogram chart is HORIZONTALLY SCROLLABLE."
    echo "  PRESS ENTER to continue."
    read
    echo ""
    echo "Phase 2 (Light mode):"
    echo "  - Browser DevTools → Rendering → 'Emulate CSS prefers-color-scheme' → light."
    echo "  - Reload. Confirm panel + histogram render with light tokens (grey-on-white)."
    echo "  - No new tokens should appear; the existing [data-theme=\"light\"] block in app.css handles it."
    echo "  PRESS ENTER to continue."
    read
    echo ""
    echo "Phase 3 (Print mode):"
    echo "  - File → Print (Cmd+P / Ctrl+P)."
    echo "  - Confirm the FCTX panel is OPEN (not collapsed) per @media print { details { open: open } }."
    echo "  PRESS ENTER to continue."
    read
    echo ""
    echo "Phase 4 (Keyboard-only):"
    echo "  - Reload the run-detail page (panel collapses by default)."
    echo "  - Tab to the panel summary. Confirm focus ring visible (--cd-green-dim)."
    echo "  - Press Space or Enter to expand. Confirm panel toggles."
    echo "  - Tab into the body. Confirm 'view last successful run' link receives focus."
    echo "  - Visit /jobs/{ID}. Tab onto a histogram bar. Confirm focus ring + tooltip appear."
    echo ""
    echo "All 4 phases complete. If any failed, capture the screenshot/issue details for 21-HUMAN-UAT.md."
