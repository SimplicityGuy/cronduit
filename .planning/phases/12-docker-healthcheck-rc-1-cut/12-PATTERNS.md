# Phase 12: Docker Healthcheck + rc.1 Cut - Pattern Map

**Mapped:** 2026-04-17
**Files analyzed:** 10 (5 NEW + 5 MODIFIED)
**Analogs found:** 10 / 10

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/cli/health.rs` | cli subcommand handler | request-response (HTTP client) | `src/cli/check.rs` (signature/exit-code shape) + `src/cli/run.rs` (`Cli` arg consumption + `tracing::error!` + `#[cfg(test)] mod tests`) | exact (role) + role-match (data flow — no other HTTP client in `src/cli/`) |
| `src/cli/mod.rs` (MOD) | cli wiring (clap enum + dispatch) | request-response | itself (current file is the canonical analog — single-variant addition) | exact |
| `Cargo.toml` (MOD) | manifest / config | dep-declaration | itself (existing `# HTTP / web placeholder` group) | exact |
| `Dockerfile` (MOD) | container build / runtime config | container-runtime | itself (lines 122–130 runtime stage) | exact |
| `.github/workflows/release.yml` (MOD) | CI / release workflow | event-driven (tag push) | itself (lines 105–115 metadata-action `tags:`) | exact |
| `.planning/REQUIREMENTS.md` (MOD) | planning / docs | docs-checklist | itself (lines 87–91 OPS-06..08 checkbox style) | exact |
| `tests/Dockerfile.ops08-old` | test fixture (Dockerfile) | container-runtime (test) | `Dockerfile` runtime stage (L90–130) | role-match |
| `tests/compose-override.yml` | test fixture (compose) | container-runtime (test) | `examples/docker-compose.yml` (L70–96) | role-match |
| `.github/workflows/compose-smoke.yml` | CI workflow | event-driven (PR / push / tag) | `.github/workflows/ci.yml` `compose-smoke` job (L133–366) + `.github/workflows/release.yml` (docker build mechanics, L73–157) | exact (role) + role-match (split scope) |
| `docs/release-rc.md` | doc / runbook | docs | `docs/CI_CACHING.md` (long-form runbook with mermaid diagram) | role-match |

## Pattern Assignments

### `src/cli/health.rs` (cli subcommand handler, request-response)

**Primary analog:** `src/cli/check.rs` (signature + exit-code shape)
**Secondary analog:** `src/cli/run.rs` (Cli consumption + tracing + tests)

**Imports pattern** (from `src/cli/check.rs:1-2` and `src/cli/run.rs:1-10`):
```rust
// check.rs — minimal, pulls config + std path
use crate::config;
use std::path::Path;

// run.rs — pulls Cli + tracing-style logging via tracing macros
use crate::cli::Cli;
// (other crate:: imports as needed)
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
```

**For `health.rs` — D-01/D-03 driven:**
```rust
use crate::cli::Cli;
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::Request;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use std::time::Duration;
```

**Function signature pattern** (from `src/cli/run.rs:12` — the `&Cli` shape):
```rust
pub async fn execute(cli: &Cli) -> anyhow::Result<i32> {
    // ...
}
```

`check.rs:9` uses a positional `&Path` because clap delivered it via `Check { config: PathBuf }`. `health.rs` follows `run.rs` (D-03 reuses global `--bind`), so the signature is `execute(cli: &Cli)`.

**Error-handling style** (from `src/cli/check.rs:9-23` — exit-code via `anyhow::Result<i32>` + `eprintln!` for human messages):
```rust
pub async fn execute(config_path: &Path) -> anyhow::Result<i32> {
    match config::parse_and_validate(config_path) {
        Ok(_parsed) => {
            eprintln!("ok: {}", config_path.display());
            Ok(0)
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("{e}");
            }
            eprintln!();
            eprintln!("{} error(s)", errors.len());
            Ok(1)
        }
    }
}
```

**Logging convention** (from `src/cli/run.rs:108-117` — `tracing::warn!` / `tracing::error!` with `target:` and `=` field syntax). Note the project mixes `eprintln!` (in `check.rs` for stylized GCC-style validation output) with `tracing::*` (in `run.rs` for structured startup events). Per CONTEXT.md "Established Patterns", `cronduit health` uses `tracing::error!` (so failures show up in the same JSON log stream as the daemon):

```rust
// run.rs:110-116
tracing::warn!(
    target: "cronduit.startup",
    bind = %resolved_bind,
    "web UI bound to non-loopback address — v1 ships without authentication; \
     see README SECURITY and THREAT_MODEL.md. Put cronduit behind a reverse proxy \
     with auth, or keep it on 127.0.0.1."
);
```

For `health.rs`:
```rust
tracing::error!(target: "cronduit.health", error = %e, "request failed (connect-refused / DNS)");
```

**Test module shape** (from `src/cli/run.rs:275-295`):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn loopback_detection() {
        assert!(is_loopback(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(127, 0, 0, 1),
            8080
        ))));
        // ...
    }
}
```

For `health.rs`, follow this exact shape but with `#[tokio::test]` for the async cases per D-14 (200+ok body, non-200, missing-status field, connect-refused, URL construction). A tokio test HTTP server stub (e.g., `tokio::net::TcpListener::bind("127.0.0.1:0")` returning canned bytes) satisfies the "no testcontainers" constraint in D-14.

**Reference for the JSON contract being consumed** (from `src/web/handlers/health.rs:21-25` — DO NOT EDIT, just read the shape):
```rust
let body = Json(json!({
    "status": if db_ok { "ok" } else { "degraded" },
    "db": if db_ok { "ok" } else { "error" },
    "scheduler": "running"
}));
```

Per D-05: `health.rs` checks `body.status == "ok"` (top-level field).

---

### `src/cli/mod.rs` (cli wiring — MODIFIED)

**Analog:** itself. Single-variant addition; no trait surface change.

**Existing module declaration** (lines 1-4):
```rust
use std::path::PathBuf;

pub mod check;
pub mod run;
```

**Patch (D-01/D-03):** add `pub mod health;` between `check` and `run` (alphabetical).

**Existing `Command` enum** (lines 33-42 — exact text):
```rust
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Run the cronduit daemon (loads config, migrates DB, serves web UI).
    Run,
    /// Validate a config file without touching the database.
    Check {
        /// Path to the config file to validate.
        config: PathBuf,
    },
}
```

**Patch (D-03/D-04):** add a `Health` variant. Per project convention (each variant has a `///` doc comment), include one explaining the `--bind` reuse and no-`--config` behavior:
```rust
/// Probe the local /health endpoint and exit 0 if status="ok".
/// Intended as a Dockerfile HEALTHCHECK target. Reuses the global
/// `--bind` flag (default 127.0.0.1:8080). Does NOT read --config (D-04).
Health,
```

**Existing `dispatch()` function** (lines 50-55 — exact text):
```rust
pub async fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Command::Run => run::execute(&cli).await,
        Command::Check { config } => check::execute(config).await,
    }
}
```

**Patch:** add a third arm, mirroring `Run` (passes `&cli` because the handler reads global `--bind`):
```rust
Command::Health => health::execute(&cli).await,
```

**`--bind` flag (already global, do NOT re-declare)** — lines 24-26:
```rust
/// Bind address (overrides [server].bind). e.g. 127.0.0.1:8080
#[arg(long, global = true)]
pub bind: Option<String>,
```

The `global = true` attribute is what lets `health::execute` read `cli.bind` even though the user wrote `cronduit health --bind ...` (D-03). No schema change.

---

### `Cargo.toml` (MODIFIED)

**Analog:** itself. Drop one line into the existing `# HTTP / web placeholder` group.

**Existing dep block style** (lines 24-27 — exact text):
```toml
# HTTP / web placeholder
axum = { version = "0.8.9", default-features = false, features = ["tokio", "http1", "http2", "json", "query", "form"] }
tower-http = { version = "0.6.8", default-features = false, features = ["trace"] }
hyper = { version = "1", default-features = false }
```

**Patch (D-01):** add `hyper-util` immediately after `hyper`. RESEARCH.md §Standard Stack confirms `hyper-util 0.1.20` and `http-body-util 0.1.3` are already transitive (axum + bollard pull them); the planner declares them directly so the unstable `hyper-util` API can be referenced from `health.rs`:
```toml
# HTTP / web placeholder
axum = { version = "0.8.9", default-features = false, features = ["tokio", "http1", "http2", "json", "query", "form"] }
tower-http = { version = "0.6.8", default-features = false, features = ["trace"] }
hyper = { version = "1", default-features = false }
hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }
http-body-util = "0.1"
```

**Style notes from existing Cargo.toml:**
- One blank line between dep groups; group header is a `# Title` comment.
- Inline tables for any dep with `features` or `default-features = false`.
- Bare `version = "X"` for simple deps (matches `serde_json = "1"` at L84).
- `default-features = false` is preferred when features are explicitly enumerated (matches `axum`, `tower-http`, `hyper`, `sqlx`, `chrono`, `tracing-subscriber`). For `hyper-util`, the research recommends NOT setting `default-features = false` because the three required features (`client-legacy`, `http1`, `tokio`) are an additive set that together cover the `Client::builder(...).build(connector)` pattern; if planner verifies via `cargo tree` that defaults pull no extra surface, leave defaults on for ergonomics. Otherwise add `default-features = false`.

**Verification command (per CLAUDE.md rustls-only constraint, run before commit):**
```bash
cargo tree -i openssl-sys      # MUST return "did not match any packages"
cargo tree -i hyper-util       # confirms 0.1.x present
cargo tree -i http-body-util   # confirms 0.1.x present
```

---

### `Dockerfile` (MODIFIED)

**Analog:** itself. Single-line addition between `USER` and `ENTRYPOINT`.

**Existing runtime stage tail** (lines 122-130 — exact text):
```dockerfile
COPY --from=builder /cronduit /cronduit
# Migrations are embedded via `sqlx::migrate!(...)` -- no filesystem copy.
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER cronduit:cronduit

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
```

**Patch (D-06):** add HEALTHCHECK directive between L127 (`USER cronduit:cronduit`) and L129 (`ENTRYPOINT ["/cronduit"]`). Match the existing comment-block style (multi-line `#` paragraph + blank line before the directive — see L108-112 and L114-117 for the established voice):
```dockerfile
EXPOSE 8080
USER cronduit:cronduit

# Phase 12 OPS-07: probe /health every 30s; allow 60s for migration backfill
# (Phase 11 D-12 binds the listener AFTER backfill completes), 5s timeout per
# probe, 3 consecutive failures flip the container to (unhealthy). Operator
# `healthcheck:` stanzas in compose still override (compose wins over Dockerfile,
# verified by .github/workflows/compose-smoke.yml).
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD ["/cronduit", "health"]

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
```

**Style notes:**
- The existing file uses exec-form (`["..."]`) for `ENTRYPOINT` and `CMD` — the new HEALTHCHECK CMD also uses exec-form (per Docker best practice and consistency).
- Multi-line directives are continued with `\` (matches the apt/zig/tailwind RUN blocks at L20-57).
- Comments are sentence-cased and explain *why* (matches the established voice — see L80-89, L92-103, L108-111, L114-117).
- D-07 (busybox stays installed) requires NO change to the existing `RUN apk add --no-cache ca-certificates tzdata` line at L112; alpine ships busybox-wget by default.

---

### `.github/workflows/release.yml` (MODIFIED)

**Analog:** itself. Five edits in the metadata-action `tags:` block.

**Existing `tags:` block** (lines 111-115 — exact text):
```yaml
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=raw,value=latest
```

**Patch (D-10):** five edits per RESEARCH.md §Pattern 4 — add `enable=` clauses on the three pre-release-sensitive lines and add a new `type=raw,value=rc` line:
```yaml
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}},enable=${{ !contains(github.ref, '-') }}
            type=semver,pattern={{major}},enable=${{ !contains(github.ref, '-') }}
            type=raw,value=latest,enable=${{ !contains(github.ref, '-') }}
            type=raw,value=rc,enable=${{ contains(github.ref, '-rc.') }}
```

**Surrounding context** (lines 107-119 — DO NOT change anything outside the `tags:` block; the `labels:`/`annotations:`/`env:` blocks below it stay verbatim):
```yaml
          # Tag templates replace the hand-rolled multi-tag list below. The
          # type=semver entries derive semver-aware tags from the pushed git
          # tag (v1.0.0 -> 1.0.0, 1.0, 1). type=raw,value=latest keeps the
          # floating latest tag pointed at every release.
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=raw,value=latest
          # Labels GitHub Container Registry recognizes for the package page:
          #   - org.opencontainers.image.source       (Connected to repository)
          #   - org.opencontainers.image.description  (package subtitle)
```

**Style notes:**
- The comment block above `tags:` (L107-110) explains the existing intent; the planner should UPDATE it to mention pre-release gating since the patch changes the semantics meaningfully.
- The existing `prerelease:` line at L163 (`prerelease: ${{ contains(steps.version.outputs.version, '-') }}`) ALREADY routes rc tags to GitHub Release prerelease — NO change needed there (CONTEXT.md "Reusable Assets").

---

### `.planning/REQUIREMENTS.md` (MODIFIED)

**Analog:** itself. Pure checkbox flip from `[ ]` to `[x]` on three lines.

**Existing OPS-06..08 entries** (lines 87-91 — exact text):
```markdown
- [ ] **OPS-06**: A new `cronduit health` CLI subcommand performs a local HTTP GET against `/health`, parses the JSON response, and exits 0 only if `status == "ok"`. It fails fast on connection-refused (no retry; the Docker healthcheck has its own retry policy) and reads the bind address from either a `--bind` flag or defaults to `http://127.0.0.1:8080`.

- [ ] **OPS-07**: The Dockerfile ships with a `HEALTHCHECK CMD ["/cronduit", "health"]` directive using conservative defaults (`--interval=30s --timeout=5s --start-period=60s --retries=3`), so `docker compose up` reports `healthy` out of the box without any compose-file healthcheck stanza. Operators who write their own `healthcheck:` in compose continue to work (compose overrides Dockerfile). `T-V11-HEALTH-01`, `T-V11-HEALTH-02`.

- [ ] **OPS-08**: The root cause of the reported `(unhealthy)` symptom (busybox `wget --spider` in alpine:3 misparses axum's chunked responses) is reproduced in a test environment before the fix is declared complete. If the reproduction shows a different root cause, this requirement is re-scoped; the `cronduit health` subcommand fix path is correct regardless because it removes the entire busybox wget dependency from the healthcheck path.
```

**Patch:** flip `[ ]` to `[x]` on each of L87, L89, L91 as the close-out commit. Do NOT edit prose.

**Also patch the traceability table** (lines 170-172 — exact text):
```markdown
| OPS-06   | Phase 12 | Pending |
| OPS-07   | Phase 12 | Pending |
| OPS-08   | Phase 12 | Pending |
```
Flip `Pending` → `Done` (verify the canonical word by reading the in-file legend; other rows likely use `Done`).

**Style notes:**
- Two-space indentation under each requirement is preserved.
- Trailing-period sentence style; do not add trailing whitespace.

---

### `tests/Dockerfile.ops08-old` (NEW test fixture)

**Analog:** `Dockerfile` runtime stage (L90–130). The OLD-state fixture should reuse the production runtime stage byte-for-byte (so the only variable is the HEALTHCHECK directive), then bake in the busted busybox wget healthcheck per D-08.

**Pattern to copy** (from `Dockerfile:90-130`):
```dockerfile
FROM alpine:3
LABEL org.opencontainers.image.source="https://github.com/SimplicityGuy/cronduit"
LABEL org.opencontainers.image.description="Self-hosted Docker-native cron scheduler with a web UI"
LABEL org.opencontainers.image.licenses="MIT"

RUN apk add --no-cache ca-certificates tzdata

RUN addgroup -g 1000 -S cronduit \
 && adduser -S -u 1000 -G cronduit cronduit \
 && install -d -o 1000 -g 1000 /data

COPY --from=builder /cronduit /cronduit
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER cronduit:cronduit

ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
```

**For the OPS-08 OLD fixture (D-08):** preserve everything above, but BEFORE the `ENTRYPOINT` line, inject the broken healthcheck:
```dockerfile
# Phase 12 D-08: deliberately broken HEALTHCHECK that reproduces the
# OPS-08 (unhealthy) symptom. busybox `wget --spider` misparses axum's
# chunked /health response and exits non-zero, flipping the container
# to (unhealthy) after 3 retries. Used ONLY by the compose-smoke
# workflow's before/after assertion. Do NOT use in any production image.
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=3 \
    CMD ["wget", "--spider", "-q", "http://localhost:8080/health"]
```

**Style notes:**
- The OPS-08 fixture should derive from the production Dockerfile via either (a) a multi-stage `FROM cronduit:ops08-base` that pulls the built image and only re-issues HEALTHCHECK, or (b) a near-copy of the runtime stage with the broken HEALTHCHECK inlined. The (a) form is preferred because it shares the build cache with the main image and isolates the variable being tested.
- `--start-period=20s` (NOT 60s) because the test wants the unhealthy verdict fast — the 60s production value is for first-boot migration backfill, which the test fixture doesn't need.

---

### `tests/compose-override.yml` (NEW test fixture)

**Analog:** `examples/docker-compose.yml` (L70-96).

**Existing service block** (L70-93 — exact text):
```yaml
services:
  cronduit:
    image: ghcr.io/simplicityguy/cronduit:latest
    group_add:
      - "${DOCKER_GID:-999}"
    ports:
      - "8080:8080"
    volumes:
      - ${CRONDUIT_DOCKER_SOCKET:-/var/run/docker.sock}:/var/run/docker.sock
      - ./cronduit.toml:/etc/cronduit/config.toml:ro
      - cronduit-data:/data
    environment:
      - RUST_LOG=info,cronduit=debug
      - DATABASE_URL=sqlite:///data/cronduit.db
    restart: unless-stopped

volumes:
  cronduit-data:
```

**For the compose-override fixture (D-09):** trim to the minimum needed to assert "compose wins over Dockerfile" — image reference, ports, and a deliberately-distinguishable `healthcheck:` stanza. Use the `cronduit:ci` image tag pattern from `ci.yml:172` (`tags: cronduit:ci`) for parity with the compose-smoke setup. The healthcheck command and interval should differ enough from the Dockerfile defaults that `docker inspect --format '{{json .Config.Healthcheck}}'` makes the override trivially observable:

```yaml
# tests/compose-override.yml — Phase 12 D-09 fixture.
# Asserts compose `healthcheck:` stanzas override the Dockerfile HEALTHCHECK.
# Distinguishable from Dockerfile defaults (interval=30s -> 7s; CMD wraps
# /cronduit health in a sh -c so docker inspect shows a different shape).
services:
  cronduit:
    image: cronduit:ci
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info,cronduit=debug
      - DATABASE_URL=sqlite:///data/cronduit.db
    healthcheck:
      test: ["CMD-SHELL", "/cronduit health"]
      interval: 7s
      timeout: 4s
      retries: 2
      start_period: 15s
    restart: unless-stopped
```

**Style notes:**
- No `volumes:` named-volume block needed for the smoke test (in-memory SQLite or ephemeral container is fine).
- Keep `restart: unless-stopped` because the smoke test polls `docker inspect` — restart on failure is fine.
- Do NOT mount the docker socket or set `group_add` — the compose-override smoke test does not need to schedule docker-type jobs.

---

### `.github/workflows/compose-smoke.yml` (NEW workflow)

**Primary analog:** `.github/workflows/ci.yml` job `compose-smoke` (L133-366) — already exists in `ci.yml`, called the same name. The Phase 12 workflow is a SEPARATE top-level workflow (per D-09: "ci.yml is NOT extended"), but it should reuse the established patterns.

**Secondary analog:** `.github/workflows/release.yml` (L73-157) for `setup-buildx-action` + `build-push-action@v6` mechanics.

**Workflow header pattern** (from `ci.yml:1-19`):
```yaml
# .github/workflows/ci.yml
# Single source of truth for Phase 1 CI. Every `run:` step invokes `just <recipe>`
# exclusively (D-10 / FOUND-12). No inline `cargo` / `docker` / `rustup` / `sqlx` /
# `npm` / `npx` commands.
name: ci

on:
  pull_request:
  push:
    branches: [main]

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: read
```

**For `compose-smoke.yml`:**
```yaml
# .github/workflows/compose-smoke.yml
# Phase 12 OPS-07 / OPS-08: dedicated compose-smoke workflow. Two jobs:
# (1) ops08-repro — build OLD-state image (busybox wget HEALTHCHECK) and
#     assert (unhealthy); build NEW image (cronduit health HEALTHCHECK)
#     and assert healthy.
# (2) compose-override-smoke — assert operator `healthcheck:` stanza in
#     compose wins over the Dockerfile HEALTHCHECK.
# Standalone — does NOT extend ci.yml (D-09: distinct runner requirements).
name: compose-smoke

on:
  pull_request:
  push:
    branches: [main]
    tags: ['v*']

concurrency:
  group: compose-smoke-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: read
```

**Build step (cache-from / cache-to)** — pattern from `ci.yml:164-174`:
```yaml
- name: Set up Docker Buildx
  uses: docker/setup-buildx-action@v3

- name: Build local cronduit:ci image from PR checkout
  uses: docker/build-push-action@v6
  with:
    context: .
    file: Dockerfile
    platforms: linux/amd64
    push: false
    load: true
    tags: cronduit:ci
    cache-from: type=gha,scope=cronduit-ci-smoke
    cache-to: type=gha,mode=max,scope=cronduit-ci-smoke
```

**For Phase 12, use a NEW scope** (per `docs/CI_CACHING.md` § "Adding a new cache" — every new cache MUST have a unique `scope=`):
- Suggested scope: `scope=cronduit-compose-smoke` (or `scope=cronduit-ops08-repro` for the OLD-state image build to keep busted-image layers out of the main scope).

**Healthcheck assertion pattern** (NEW — no exact analog in ci.yml; closest is the `/health` curl-poll at L208-223):
```yaml
- name: Wait for /health (max 30s)
  env:
    COMPOSE_FILE: ${{ matrix.compose }}
  run: |
    set -eu
    for i in $(seq 1 30); do
      if curl -sSf http://localhost:8080/health >/tmp/health.json 2>/dev/null; then
        echo "health responded after ${i}s"
        cat /tmp/health.json
        exit 0
      fi
      sleep 1
    done
    echo "ERROR: /health never responded after 30s"
    docker compose -f "examples/${COMPOSE_FILE}" logs
    exit 1
```

**For OPS-08 repro (D-08), the analogous block uses `docker inspect` instead of curl-poll:**
```yaml
- name: Run OLD image and assert unhealthy
  run: |
    set -eu
    docker run -d --name cronduit-ops08-old -p 8080:8080 cronduit:ops08-old
    # OPS-08 fixture's start_period=20s + 3 retries * 10s interval = ~50s budget
    for i in $(seq 1 60); do
      status=$(docker inspect --format '{{.State.Health.Status}}' cronduit-ops08-old 2>/dev/null || echo "missing")
      echo "attempt $i: status=$status"
      if [ "$status" = "unhealthy" ]; then
        echo "OPS-08 reproduced: OLD image flipped to unhealthy"
        exit 0
      fi
      sleep 1
    done
    echo "ERROR: OLD image never flipped to unhealthy within 60s"
    docker logs cronduit-ops08-old --tail=200
    exit 1
```

**Diagnostic-on-failure pattern** (from `ci.yml:334-359`):
```yaml
- name: Dump diagnostics on failure
  if: failure()
  env:
    COMPOSE_FILE: ${{ matrix.compose }}
  run: |
    echo "::group::cronduit logs (tail 200)"
    docker compose -f "examples/${COMPOSE_FILE}" logs cronduit --tail=200 || true
    echo "::endgroup::"
```

**Tear-down pattern** (from `ci.yml:361-366`):
```yaml
- name: Tear down compose stack
  if: always()
  working-directory: examples
  env:
    COMPOSE_FILE: ${{ matrix.compose }}
  run: docker compose -f "${COMPOSE_FILE}" down -v
```

**Style notes:**
- Use `env:` blocks to pass values into `run:` steps (NEVER `${{ ... }}` directly in `run:` — see `release.yml:42-52` and `ci.yml:177-189` for the established pattern, which CONTEXT.md/cliff CLAUDE.md security guidance also enforces).
- `set -eu` at the top of every multi-line `run:` block (matches `ci.yml:212`, `ci.yml:226`, `ci.yml:241`).
- Use `::group::`/`::endgroup::` for collapsible diagnostic output (matches `ci.yml:339-358`).
- Compose-smoke does NOT need to honor FOUND-12 / D-10 (`run:` must call `just`) — it is a NEW workflow with no `just` recipes for these test rigs and the FOUND-12 invariant is scoped to ci.yml. Confirm by reading `docs/CI_CACHING.md` § "Deliberate cache gaps" (FOUND-12 quote is from `ci.yml`'s header comment, not a project-wide rule).

---

### `docs/release-rc.md` (NEW runbook)

**Analog:** `docs/CI_CACHING.md` (full file, structure + voice).

**Top-of-file pattern** (from `docs/CI_CACHING.md:1-15`):
```markdown
# CI Caching Topology

This document is the authoritative reference for every cache wired into Cronduit's GitHub Actions workflows. Read it before adding a new workflow, changing a cache key, or debugging a slow CI run.

> Related documents: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), [`.github/workflows/release.yml`](../.github/workflows/release.yml), [`.github/workflows/cleanup-cache.yml`](../.github/workflows/cleanup-cache.yml), [`.github/workflows/cleanup-images.yml`](../.github/workflows/cleanup-images.yml), [`justfile`](../justfile).

## Why this matters
```

**For `docs/release-rc.md`:**
```markdown
# Cutting a release-candidate tag

This document is the maintainer runbook for cutting a `vX.Y.Z-rc.N` pre-release tag. Read it before tagging — the steps below are linear and the tag is a one-way action (no force-push, no untag-and-retry).

> Related documents: [`.planning/ROADMAP.md`](../.planning/ROADMAP.md), [`.planning/PROJECT.md`](../.planning/PROJECT.md), [`.github/workflows/release.yml`](../.github/workflows/release.yml), [`cliff.toml`](../cliff.toml).

## Why this matters
```

**Section structure pattern** (from `docs/CI_CACHING.md` table of contents implied by `##` headings):
- `## Why this matters` — context paragraph
- `## Cache inventory` — table
- `## Why one scope for the multi-arch release` — design note
- `## Deliberate cache gaps` — known issues with reasoning
- `## Not cached (and why)` — rejected alternatives
- `## Cache flow` — mermaid diagram
- `## Debugging a cache miss` — numbered playbook
- `## Adding a new cache` — checklist
- `## Verification playbook (post-merge)` — UAT steps

**For `release-rc.md` (per D-11), follow the same shape:**
- `## Why this matters` — iterative rc strategy + `:latest` pin invariant
- `## Pre-flight checklist` — bullet list (PRs merged, compose-smoke green on main, `git cliff --unreleased` preview verified)
- `## Cutting the tag` — exact command (`git tag -a v1.1.0-rc.N -m "..."`); GPG signing if configured
- `## Pushing the tag` — `git push origin v1.1.0-rc.N`; what `release.yml` does in response
- `## Post-push verification` — table of checks (GHCR manifest inspection, `:rc` digest, `:latest` digest still pinned to `v1.0.1`)
- `## Mermaid: tag → release flow` — required mermaid diagram per CLAUDE.md "All diagrams must be mermaid"
- `## What if UAT fails` — escalation (ship rc.N+1; never hotfix-tag; never force-push the tag)
- `## References` — link out to CONTEXT.md, ROADMAP.md

**Mermaid diagram pattern** (from `docs/CI_CACHING.md:96-122`):
```markdown
## Cache flow

\`\`\`mermaid
flowchart LR
    PR[Pull Request push] --> Lint[ci.yml: lint<br/>rust-cache: auto]
    PR --> Test[ci.yml: test matrix<br/>rust-cache: per-arch]
    ...
    classDef cacheBox fill:#0a1f2d,stroke:#00ff7f,color:#e0ffe0
    classDef storage fill:#1a1a1a,stroke:#666,color:#ccc
    classDef gap fill:#2d0a0a,stroke:#ff7f7f,color:#ffe0e0,stroke-dasharray: 5 5
    class Lint,Test,Smoke,Release,Cleanup,CleanupImg cacheBox
    class ImageBuild gap
    class GHA,GHCR storage
\`\`\`
```

The Cronduit terminal-green palette (`#0a1f2d` / `#00ff7f` / `#e0ffe0` for active boxes, `#2d0a0a` / `#ff7f7f` for warning/skipped boxes) is the established style — reuse it.

**Style notes:**
- Project uses Markdown reference-style sections with `>` blockquotes for "Related documents" callouts.
- Code blocks are fenced with triple-backtick + language hint (`bash`, `yaml`, `mermaid`, `markdown`).
- Tables use leading/trailing pipes and `|---|---|` separators (see `docs/CI_CACHING.md:18-29`).
- "Read this before X" framing at the top is canonical — the maintainer reaches for the doc when they need it, so the doc states its trigger condition immediately.
- Per CLAUDE.md auto-memory `feedback_uat_user_validates.md`: the runbook must explicitly say "the user (maintainer) runs these post-push checks; Claude does not assert UAT pass on their behalf."

---

## Shared Patterns

### Exit-code contract (`anyhow::Result<i32>`)
**Source:** `src/cli/check.rs:9` and `src/cli/run.rs:12`
**Apply to:** `src/cli/health.rs`
```rust
pub async fn execute(/* ... */) -> anyhow::Result<i32> {
    // Ok(0) on success, Ok(1) on user-visible failure.
    // Never panic for routine errors; reserve `bail!` / `?` for unexpected I/O.
}
```
The unwrapping happens in `src/main.rs:15-16`:
```rust
let exit_code = cli::dispatch(cli).await?;
std::process::exit(exit_code);
```

### Tracing target convention
**Source:** `src/cli/run.rs:111`, `src/cli/run.rs:120`, `src/cli/run.rs:189`
**Apply to:** `src/cli/health.rs`
- Format: `target: "cronduit.<area>"` (e.g., `cronduit.startup`, `cronduit.health`).
- Field syntax: `field_name = %value` (Display) or `?value` (Debug); never `format!()`-then-pass-string.
- Levels: `info!` for normal events, `warn!` for non-fatal misconfigurations, `error!` for failure paths that affect exit code.

### Comment voice (project house style)
**Source:** every modified file's existing comments — `Dockerfile:80-89`, `release.yml:25-31`, `ci.yml:73-79`, `Cargo.toml:122-129`
**Apply to:** all NEW files and all comment-block additions in MODIFIED files
- Explain *why*, not *what*.
- Reference the decision ID (`D-08`, `OPS-07`, `Phase 11 D-12`) for traceability.
- Multi-paragraph comments are fine; the project favors prose-style explanation over terse one-liners.
- For Dockerfile / YAML: `#` on every line, blank `#` for paragraph breaks.
- For Rust: `//` for prose, `///` for doc comments above public items.

### Env-var routing in GHA `run:` blocks
**Source:** `release.yml:49-52`, `ci.yml:177-189`
**Apply to:** `.github/workflows/compose-smoke.yml`
Never interpolate `${{ ... }}` directly into a multi-line `run:` shell block. Route through an `env:` block:
```yaml
env:
  REPO: ${{ github.repository }}
run: |
  echo "$REPO"  # safe; no shell injection surface
```

### `set -eu` at top of every shell `run:` block
**Source:** `ci.yml:212`, `ci.yml:226`, `ci.yml:241`, `ci.yml:256`
**Apply to:** `.github/workflows/compose-smoke.yml`
First line of every multi-line `run:` shell block is `set -eu` (or `set -euo pipefail` for blocks using pipes).

### Clap subcommand wiring
**Source:** `src/cli/mod.rs:33-42` (Command enum) + `src/cli/mod.rs:50-55` (dispatch)
**Apply to:** `src/cli/mod.rs` (the Health variant + dispatch arm)
Three coordinated edits per new subcommand:
1. `pub mod <name>;` at top of file (alphabetical).
2. New `<Name>` variant in `Command` enum with `///` doc comment.
3. New match arm in `dispatch()` that calls `<name>::execute(...)`.

### Test module shape
**Source:** `src/cli/run.rs:275-295`
**Apply to:** `src/cli/health.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // tests follow per D-14
}
```
For async tests, use `#[tokio::test]`. For shared fixtures, prefer module-local helper `fn`s over external test crates.

## No Analog Found

None. Every Phase 12 file has at least one role-match analog in the existing codebase. The closest things to "novel patterns" are:
- The `hyper-util` client construction in `health.rs` — no existing CLI-side HTTP client in the tree, but RESEARCH.md §Pattern 1 supplies the canonical hyper-util example.
- The `docker inspect --format '{{.State.Health.Status}}'` polling in `compose-smoke.yml` — no existing `docker inspect`-based assertion in CI workflows, but the curl-poll loop in `ci.yml:208-223` is the structural analog.

Both are tractable from the analog patterns above + the RESEARCH.md code examples.

## Metadata

**Analog search scope:**
- `src/cli/` — handler analogs (check.rs, run.rs, mod.rs)
- `src/web/handlers/health.rs` — JSON contract being consumed (read-only reference)
- `src/main.rs` — exit-code unwrapping
- `Cargo.toml` — dep block style
- `Dockerfile` — runtime stage style
- `.github/workflows/ci.yml` — compose-smoke job analog
- `.github/workflows/release.yml` — metadata-action + build-push-action mechanics
- `examples/docker-compose.yml` — service block style
- `docs/CI_CACHING.md` — long-form runbook structure + mermaid voice
- `.planning/REQUIREMENTS.md` — checkbox style + traceability table

**Files scanned:** 11 (full reads on 10; targeted Grep on 1)

**Pattern extraction date:** 2026-04-17
