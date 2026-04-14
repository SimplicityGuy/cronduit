# Phase 1 Research: Foundation, Security Posture & Persistence Base

**Gathered:** 2026-04-09
**Status:** Ready for planning
**Confidence:** HIGH (all crate versions verified live against crates.io / docs.rs on 2026-04-09; all decisions cross-referenced against CONTEXT.md, STACK.md, ARCHITECTURE.md, PITFALLS.md)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (D-01 .. D-24 — DO NOT RE-OPEN)

**CLI Shape**
- **D-01** clap subcommands. Phase 1 ships `cronduit run` (default daemon) and `cronduit check <config>` (parse+validate, no DB). No `cronduit migrate` in Phase 1.
- **D-02** CLI flags override config values; override fires an `info!` naming field + both values (never a secret field). Standard Unix precedence.
- **D-03** `--log-format` defaults to **JSON unconditionally**. `--log-format=text` opt-in for local dev. No tty auto-detect.
- **D-04 (action item)** Planner rewrites REQUIREMENTS.md FOUND-03 from `cronduit --check <config>` to `cronduit check <config>` before implementing.

**Crate Layout & Toolchain**
- **D-05** Single `cronduit` crate at repo root. No workspace. Modules per ARCHITECTURE.md §Recommended Project Structure.
- **D-06** Rust edition 2024, toolchain pinned to **1.94.1** via `rust-toolchain.toml`. Smoke-test bollard/sqlx/axum compile clean under 2024; fall back to 2021 only with a documented blocker.
- **D-07** sqlx features are **always-on**: `runtime-tokio`, `tls-rustls`, `sqlite`, `postgres`, `chrono`, `migrate`, `macros`. One binary supports both backends via `DATABASE_URL`. No backend-gated Cargo features.
- **D-08** Example config at **`examples/cronduit.toml`**. Docker packaging (Phase 6) copies it to `/etc/cronduit/config.toml`.

**Build Tooling — `justfile` (new Phase 1 scope)**
- **D-09** Top-level `justfile` at repo root is the **single source of truth** for build/test/lint/DB/image/dev-loop.
- **D-10** **CI workflows are strictly `just`-only.** Every `.github/workflows/ci.yml` step calls `just <recipe>` — no raw `cargo`/`docker`/`sqlx` inline. Setup via `extractions/setup-just@v2`.
- **D-11** Required recipes: `build`, `build-release`, `clean`, `image`, `tailwind`, `fmt`, `fmt-check`, `clippy`, `test`, `nextest`, `openssl-check`, `db-reset`, `migrate`, `sqlx-prepare`, `schema-diff`, `dev`, `check-config PATH`, `docker-compose-up`, `ci`. Local `just ci` must predict CI exit code.
- **D-12 (action item)** Planner adds a new `FOUND-12` requirement covering the justfile + just-only-CI constraint and extends the traceability table (Phase 1 count: 29 → 30).

**Migrations & Schema Parity**
- **D-13** Split migration directories from day one: `migrations/sqlite/` and `migrations/postgres/`. Two `sqlx::migrate!` calls selected at runtime by a `DbPool` enum.
- **D-14** Structural parity enforced by a Rust integration test `tests/schema_parity.rs`, surfaced via `just schema-diff`, running in every CI-matrix cell. Normalizes types via small whitelist (each entry requires a justification comment).
- **D-15** Initial migration already includes `jobs.config_hash TEXT NOT NULL` (SHA-256 of normalized sorted-keys JSON). Normalization function lives in `src/config/hash.rs`. Phase 1 does not yet write the column (no sync engine) — the column exists so Phase 2 avoids another migration.
- **D-16** `config_json` stored as `TEXT` on **both** backends. Never queried inside. Documented as intentional in migration file header.

**CI Matrix & Docker Image**
- **D-17** Single `.github/workflows/ci.yml`. `strategy.matrix: {arch: [amd64, arm64], db: [sqlite, postgres]}` (4 cells) for the test job. `fmt-check` / `clippy` / `openssl-check` run once in a separate unmatrixed lint job. Image build is a third job with `needs: [lint, test]`.
- **D-18** Multi-arch image (`linux/amd64`+`linux/arm64`) builds on **every PR and every push to main**. PR builds are `--load` only (no push). `main` push tags `ghcr.io/<owner>/cronduit:latest` and `:sha-<short>` and pushes to GHCR.

**Config Schema Details**
- **D-19** `[server].timezone` is **mandatory** (valid IANA zone). `cronduit check` fails and `cronduit run` refuses to start if missing/invalid. No host-TZ fallback.
- **D-20** `secrecy` crate (0.10+) for `SecretString`. No hand-roll. **No `zeroize` in v1.** Fields populated from `${ENV_VAR}` plus named secrets (`postgres_password`, future `api_token`) use `SecretString`. Serde wiring via the `secrecy::serde` feature.
- **D-21** `cronduit check` **collects all errors** into `Vec<ConfigError>`, prints **GCC-style** `path:line:col: error: <message>` on stderr, exits non-zero. Line/col from `toml::de::Error::span()`. JSON output deferred (D-26).
- **D-22** Env interpolation is **strict `${VAR}` only**. Missing var → loud error with field path. No `${VAR:-default}` in v1 (D-27 deferred).

**Observability & Bind Safety**
- **D-23** One structured `tracing::info!` on target `cronduit.startup` with fields: `version`, `bind`, `database_backend` (`"sqlite"|"postgres"`), `database_url` (credentials stripped), `config_path`, `timezone`, `job_count`, `disabled_job_count`, `bind_warning` (bool).
- **D-24** If resolved bind address is not `127.0.0.1` or `::1`, emit one loud `tracing::warn!` explaining the no-auth-in-v1 stance and pointing at README SECURITY. RFC1918, `0.0.0.0`, and public IPs all get the same warning. `bind_warning: true` in D-23 event.

### Claude's Discretion (planner freedom)
- Exact sub-module boundaries within `src/config/`, `src/db/`, `src/cli/` (follow ARCHITECTURE.md default)
- `clap` derive attribute style (doc-comment vs literals)
- `tracing-subscriber` layer ordering (as long as JSON-to-stdout + `RUST_LOG` work)
- `color-eyre` vs plain `eprintln!` for `check` errors (both acceptable)
- Exact justfile recipe bodies (recipes must exist per D-11)
- `THREAT_MODEL.md` skeleton layout (STRIDE or freeform acceptable)
- Specific GitHub Actions versions for non-critical utilities
- Whether `.sqlx/` is committed (recommended YES)

### Deferred Ideas (OUT OF SCOPE — DO NOT IMPLEMENT)
- **D-25** Standalone `cronduit migrate` subcommand (post-v1)
- **D-26** JSON error output for `cronduit check`
- **D-27** `${VAR:-default}` env-interpolation syntax
- **D-28** `zeroize` crate
- **D-29** Backend-gated Cargo features
- **D-30** Full `THREAT_MODEL.md` content (Phase 6)
- **D-31** `OPS-04` example `docker-compose.yml` (Phase 6)
- **D-32** `[server].log_retention` pruner task (Phase 6 DB-08)

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FOUND-01 | Compiles as single cargo crate, edition 2024, tokio async | §1 Cargo.toml skeleton, §1 toolchain |
| FOUND-02 | clap flags `--config`, `--bind`, `--database-url`, `--log-format` | §8 axum placeholder, CLI shape |
| FOUND-03 | `cronduit check <config>` parse+cron+network+env, no DB, non-zero on error (**rename per D-04 before plan write**) | §5 config parser, §12 error format |
| FOUND-04 | Structured JSON logs via tracing + tracing-subscriber | §7 tracing subscriber |
| FOUND-05 | `SecretString` newtype; Debug never prints value | §5 secrecy integration |
| FOUND-06 | `cargo tree -i openssl-sys` empty, CI enforced | §2 rustls-only graph, §9 justfile, §10 CI |
| FOUND-07 | `cargo fmt --check`, `clippy -D warnings`, `cargo test` green on every PR | §9 justfile, §10 CI |
| FOUND-08 | CI matrix `{amd64,arm64}×{sqlite,postgres}` on every PR | §10 ci.yml draft |
| FOUND-09 | Multi-arch Docker image via `cargo-zigbuild` tagged on push-to-main | §11 zigbuild + Dockerfile |
| FOUND-10 | README leads with SECURITY; THREAT_MODEL.md exists (skeleton ok for Phase 1) | §13 pitfall guard mapping (Pitfall 1) |
| FOUND-11 | All diagrams are mermaid code blocks | CLAUDE.md enforced; called out in §13 |
| FOUND-12 (new) | justfile + just-only CI | §9, §10 (D-09/D-10/D-11/D-12) |
| CONF-01 | TOML config with `[server]`, `[defaults]`, `[[jobs]]` | §5 config parser |
| CONF-02 | `${ENV_VAR}` interpolation at parse time; missing var → field-path error | §5 env interpolation, strict rules |
| CONF-03 | `[defaults]` section covers image/network/volumes/delete/timeout/random_min_gap | §5 defaults model |
| CONF-04 | `use_defaults = false` per-job disable | §5 defaults model |
| CONF-05 | Each job has name/schedule + exactly one of command/script/image | §5 validation pass |
| CONF-06 | Job field overrides defaults | §5 effective-config computation |
| CONF-07 | Config file mounted read-only in example compose (Phase 6 carries the compose file; Phase 1 only documents the expectation) | §13 pitfall guard mapping (Pitfall 1) |
| CONF-08 | `[server].timezone` mandatory, IANA; croner used later | §6 IANA validation |
| CONF-09 | Standard 5-field cron + L/#/W (croner) | §5 placeholder validator (croner proper in Phase 2) |
| CONF-10 | Job names unique in file; dup → both line numbers reported | §5 duplicate detection |
| DB-01 | SQLite default; WAL + busy_timeout=5000 | §3 DbPool, §4 migrations |
| DB-02 | Postgres via `postgres://`; same logical schema | §3 DbPool, §4 migrations |
| DB-03 | `sqlx::migrate!` idempotent on startup | §3 DbPool::migrate() |
| DB-04 | `jobs`, `job_runs`, `job_logs` tables | §4 initial SQL |
| DB-05 | SQLite split read/write pools (writer=1, reader=N) | §3 DbPool::Sqlite variant |
| DB-06 | `jobs` stores `schedule`, `resolved_schedule`, `config_hash` SHA-256 | §4 initial SQL + D-15 |
| DB-07 | Removed jobs `enabled=0`, not deleted; history preserved | §4 (schema supports; sync engine is Phase 2) |
| OPS-03 | Default bind `127.0.0.1:8080`; non-loopback → WARN | §7 startup event, §8 axum placeholder |

</phase_requirements>

## Summary

Five decisions carry this phase:

1. **Cargo.toml is tight and rustls-only.** Every crate version comes directly from STACK.md (verified live against crates.io on 2026-04-09). Phase 1's `[dependencies]` block excludes everything that isn't load-bearing before Phase 2 (no `bollard`, no `croner`, no `askama_web`, no `rust-embed`, no `tower-http` compression). The `openssl-sys`-free invariant is enforced both by feature flags (`sqlx` with `tls-rustls`, never `tls-native-tls`; `reqwest`-family crates not pulled at all) and by the CI guard `just openssl-check`.
2. **`DbPool` is an enum.** `DbPool::Sqlite { write: SqlitePool, read: SqlitePool }` vs `DbPool::Postgres(PgPool)`. SQLite gets two pools (writer `max_connections=1`, reader `max_connections=8`) with WAL + `busy_timeout=5000` + `synchronous=NORMAL` + `foreign_keys=ON` applied via `PoolOptions::after_connect`. Migrations dispatch through a `match` on the enum so each branch calls a distinct `sqlx::migrate!(…)` — the macro is compile-time path-bound, which is why split directories are mandatory.
3. **Config parsing is a three-stage pipeline** — (a) read file → (b) strict `${VAR}` pre-pass that tracks byte spans so error line/col survive → (c) `toml::from_str::<RawConfig>` → (d) post-parse validation collecting every problem into `Vec<ConfigError>`. `cronduit check` and `cronduit run` share stages (a)–(d); `run` just continues into DB/migrate/listen afterward. `SecretString` fields work transparently once `secrecy` is built with `features = ["serde"]`.
4. **`justfile` is the load-bearing build contract.** Every CI job is `just <recipe>` — so the local dev loop and the CI matrix cannot drift. The `just ci` target is an ordered chain (`fmt-check → clippy → openssl-check → nextest → schema-diff → image`) and its exit code is the contract: green locally predicts green in CI. `cargo-zigbuild 0.22.1` (verified 2026-04-09) produces both `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` from the amd64 runner with **no QEMU**; the `docker buildx` step stitches them into a single manifest.
5. **Schema parity is enforced by a real test, not good intentions.** `tests/schema_parity.rs` spins an in-memory SQLite plus a `testcontainers-modules::postgres::Postgres`, runs both migration sets, introspects `sqlite_master`/`information_schema.*`/`pg_indexes`, normalizes types through a whitelist, and asserts identical tables, columns, and indexes. The test is surfaced as `just schema-diff` and runs in every cell of the matrix. This is the guard against Pitfall 8.

**Primary recommendation:** Build Phase 1 as a minimal but uncompromising foundation. Ship the smallest axum placeholder that exercises graceful shutdown (`/` → "cronduit running, no scheduler yet"). Spend complexity budget on: the `DbPool` enum, the split-migrations + parity test, the GCC-style error collector, the `justfile` recipe set, and the four-cell CI matrix. Everything else (scheduler, executors, UI, templates, real cron parsing) is Phase 2+.

---

## 1. Cargo.toml & Toolchain

### `rust-toolchain.toml` (repo root)

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy"]
targets = [
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
]
profile = "minimal"
```

- `channel = "1.94.1"` matches locally-installed `rustc 1.94.1` (verified via `rustc --version` on the target machine). [VERIFIED: rustc --version on 2026-04-09]
- `profile = "minimal"` keeps CI installs fast; `components` adds back exactly what we need.
- `targets` is required so `cargo-zigbuild` can build both musl targets from an amd64 runner without per-job `rustup target add` boilerplate. [CITED: cargo-zigbuild README, https://github.com/rust-cross/cargo-zigbuild]
- `dtolnay/rust-toolchain@stable` action reads this file automatically and pins the installed toolchain.

### `Cargo.toml` (Phase 1 scope only)

All versions come from STACK.md (CLAUDE.md §Technology Stack). Verified live against crates.io on 2026-04-09 via `cargo search`.

```toml
[package]
name = "cronduit"
version = "0.1.0"
edition = "2024"
rust-version = "1.94.1"
description = "Self-hosted Docker-native cron scheduler with a web UI"
license = "MIT OR Apache-2.0"
repository = "https://github.com/<owner>/cronduit"
readme = "README.md"

[dependencies]
# Async runtime — full feature for Phase 1; tighten before public release.
tokio = { version = "1.51", features = ["full"] }
tokio-util = { version = "0.7.18", features = ["rt"] }                 # CancellationToken for graceful shutdown

# HTTP / web placeholder (tiny router in Phase 1; grows in Phase 2/3).
axum = { version = "0.8.8", default-features = false, features = ["tokio", "http1"] }
tower-http = { version = "0.6.8", default-features = false, features = ["trace"] }
hyper = { version = "1", default-features = false }

# Database — both backends always on (D-07). rustls-only TLS (Pitfall 14 / FOUND-06).
sqlx = { version = "0.8.6", default-features = false, features = [
    "runtime-tokio",
    "tls-rustls",
    "sqlite",
    "postgres",
    "chrono",
    "migrate",
    "macros",
] }

# Config parsing
serde = { version = "1.0.228", features = ["derive"] }
toml = "1.1.2"                                                          # spec 1.1.0
humantime = "2.3.0"
humantime-serde = "1.1.1"

# CLI
clap = { version = "4.6", features = ["derive", "env"] }

# Telemetry
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", default-features = false, features = [
    "env-filter",
    "fmt",
    "json",
    "ansi",
    "smallvec",
] }

# Errors
anyhow = "1.0.102"
thiserror = "2.0.18"

# Secrets
secrecy = { version = "0.10.3", features = ["serde"] }

# Time
chrono = { version = "0.4.44", default-features = false, features = ["std", "serde", "clock"] }
chrono-tz = "0.10.4"                                                    # IANA zone validation (see §6)

# Utilities
sha2 = "0.10"                                                           # for config_hash (D-15)
serde_json = "1"                                                        # for normalized JSON hashing
url = "2"                                                               # to parse DATABASE_URL and strip credentials for logging

[dev-dependencies]
# Integration-test Postgres via testcontainers
testcontainers = "0.27.2"
testcontainers-modules = { version = "0.15.0", features = ["postgres"] }
tokio = { version = "1.51", features = ["macros", "rt-multi-thread"] }
tempfile = "3"
assert_cmd = "2"                                                        # black-box test `cronduit check`
predicates = "3"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

**Crates explicitly NOT added in Phase 1** (scoped out to keep the dep graph lean):

| Crate | Why excluded in Phase 1 | Added in |
|-------|-------------------------|----------|
| `bollard` | No Docker executor yet | Phase 4 |
| `croner` | Cron parsing is a placeholder in Phase 1 (valid-syntax check only; real scheduling is Phase 2) | Phase 2 |
| `askama` / `askama_web` | No templates rendered | Phase 3 |
| `axum-htmx` | No HTMX interactions | Phase 3 |
| `rust-embed` | No embedded assets | Phase 3 |
| `notify` | No config reload | Phase 5 |
| `metrics` / `metrics-exporter-prometheus` | No metrics endpoint | Phase 6 |
| `shell-words` | No executors that tokenize commands | Phase 2 |
| `rand` | No `@random` yet | Phase 5 |
| `uuid` | No run IDs yet | Phase 2 |
| `tower-http` compression/cors/serve-dir | No assets to serve | Phase 3 |

**Feature flags that are forbidden** (any of these re-enable `openssl-sys` or break rustls-only):
- `sqlx` with `tls-native-tls` or `runtime-tokio-native-tls` or `runtime-actix-native-tls` — never.
- `tokio-native-tls` or `native-tls` anywhere in the tree — never.
- `reqwest` with default features (pulls native-tls on Linux via default) — not needed in Phase 1 at all.
- `hyper-tls` — never; use `hyper-rustls` only if a future phase actually needs TLS-terminating HTTP client code.

**Version verification [VERIFIED: crates.io, 2026-04-09]:**
- `secrecy = "0.10.3"` — latest, `serde` feature available
- `cargo-zigbuild = "0.22.1"` — latest stable
- `chrono-tz = "0.10.4"` — latest stable
- `sqlx = "0.8.6"` — latest stable (`0.9.0-alpha.1` exists but is not usable)
- `toml = "1.1.2+spec-1.1.0"` — latest stable

---

## 2. Rustls-only Dependency Graph

**Invariant:** `cargo tree -i openssl-sys` returns empty string, exit 0 interpreted as "no match found" (the `-i` invert mode only prints when the package is in the tree).

### How to enforce it

The correct invocation is a little subtle — `cargo tree -i` exits 0 regardless of match count, so a naive `just openssl-check: cargo tree -i openssl-sys` passes even when openssl-sys IS present. Two correct patterns:

```just
# preferred: fail if there's any non-empty, non-warning output
openssl-check:
    @echo "Verifying rustls-only TLS stack (no openssl-sys allowed)..."
    @if cargo tree -i openssl-sys 2>/dev/null | grep -q .; then \
        echo "FAIL: openssl-sys found in dependency graph:"; \
        cargo tree -i openssl-sys; \
        exit 1; \
    fi
    @echo "OK: no openssl-sys in tree"
```

A clean output looks like:

```
$ cargo tree -i openssl-sys
error: package ID specification `openssl-sys` did not match any packages
```

That `error:` goes to stderr, exit code is non-zero — the `grep -q .` above correctly treats empty stdout as success.

### Which features matter

| Crate | Required feature | Why |
|-------|-----------------|-----|
| `sqlx` | `tls-rustls` (not `tls-native-tls`) | pulls `rustls` + `webpki-roots` instead of `openssl-sys` [CITED: sqlx 0.8.6 docs.rs feature list] |
| `sqlx` | `runtime-tokio` (not `runtime-tokio-native-tls`) | native-tls variant pulls openssl-sys transitively |
| `tracing-subscriber` | `default-features = false` + explicit features | default includes `regex` which is fine, but being explicit is cheap insurance |
| (future) `bollard` | `ssl_providerless` or rustls feature | Phase 4 concern, but lock the pattern now |
| (future) `reqwest` | `rustls-tls` if ever added | Phase 4+ concern |

### Bonus: `cargo-deny` (optional)

STACK.md lists `cargo-deny` as MEDIUM-confidence. For Phase 1, the `just openssl-check` script is sufficient. `cargo-deny` can layer on later to also enforce license policy and advisory checks — deferred to Phase 6 release engineering unless the planner decides otherwise.

---

## 3. sqlx Dual-Backend Pool (DbPool)

### The enum pattern

```rust
// src/db/mod.rs
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode, SqliteSynchronous},
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, SqlitePool, PgPool,
};
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone)]
pub enum DbPool {
    Sqlite {
        write: SqlitePool, // max_connections = 1
        read: SqlitePool,  // max_connections = 8
    },
    Postgres(PgPool),
}

pub enum DbBackend { Sqlite, Postgres }

impl DbPool {
    pub fn backend(&self) -> DbBackend {
        match self {
            DbPool::Sqlite { .. } => DbBackend::Sqlite,
            DbPool::Postgres(_)   => DbBackend::Postgres,
        }
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        match database_url.split_once("://").map(|(s, _)| s) {
            Some("sqlite") => Self::connect_sqlite(database_url).await,
            Some("postgres") | Some("postgresql") => Self::connect_postgres(database_url).await,
            Some(other) => anyhow::bail!(
                "unsupported database scheme `{other}://` — use `sqlite://` or `postgres://`"
            ),
            None => anyhow::bail!("invalid DATABASE_URL: missing scheme"),
        }
    }

    async fn connect_sqlite(url: &str) -> anyhow::Result<Self> {
        // Parse once, clone into two PoolOptions.
        // `create_if_missing(true)` lets the default deploy work with a fresh volume.
        let base = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_millis(5000))
            .foreign_keys(true);

        let write = SqlitePoolOptions::new()
            .max_connections(1) // Pitfall 7: single writer
            .min_connections(1)
            .connect_with(base.clone())
            .await?;

        let read = SqlitePoolOptions::new()
            .max_connections(8)
            .min_connections(1)
            .connect_with(base)
            .await?;

        Ok(DbPool::Sqlite { write, read })
    }

    async fn connect_postgres(url: &str) -> anyhow::Result<Self> {
        let opts = PgConnectOptions::from_str(url)?;
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect_with(opts)
            .await?;
        Ok(DbPool::Postgres(pool))
    }

    /// Runs the correct migration set idempotently. Safe to call on every startup.
    pub async fn migrate(&self) -> anyhow::Result<()> {
        match self {
            DbPool::Sqlite { write, .. } => {
                // writer pool runs DDL so it contends with no other writer
                sqlx::migrate!("./migrations/sqlite").run(write).await?;
            }
            DbPool::Postgres(pool) => {
                sqlx::migrate!("./migrations/postgres").run(pool).await?;
            }
        }
        Ok(())
    }
}
```

**Why two pools on SQLite (Pitfall 7):** Default `SqlitePool` round-robins writes across N connections, each of which grabs the WAL writer lock in turn. Under concurrent log writes + scheduler ticks that's ping-pong contention and throughput collapses. Splitting into a `max_connections=1` writer plus a `max_connections=8` reader means every write goes through a single connection (no lock ping-pong) while reads fan out freely. [CITED: PITFALLS.md §7]

**Why the `match` on migrate:** `sqlx::migrate!(PATH)` is a **compile-time** macro — the path is baked into the binary. You cannot pass a runtime `&str`. The only way to support two backends in one binary is two macro invocations, each in its own match arm. This is the core reason D-13 requires split directories. [CITED: sqlx docs.rs `migrate!` macro description — "embeds migrations into the binary by expanding to a static instance of Migrator"]

### Read-path API consumers

Phase 1 has no read paths yet (no sync engine, no UI), but the pattern for Phase 2+ is:

```rust
impl DbPool {
    /// Use for all read queries on every backend.
    pub fn reader(&self) -> &sqlx::Pool<sqlx::Any> { /* ... */ }
    // Or, more pragmatically, return the concrete pool and have two query paths:
    pub fn sqlite_read(&self) -> Option<&SqlitePool> { /* */ }
    pub fn sqlite_write(&self) -> Option<&SqlitePool> { /* */ }
    pub fn postgres(&self)     -> Option<&PgPool>     { /* */ }
}
```

Phase 1 can stop at `connect` + `migrate` — that's enough for the CONF + DB requirements to be testable.

### Offline query prepare workflow

With `#[sqlx::query!]` macros disabled in Phase 1 (no queries yet), this is mostly guidance for Phase 2:

1. Dev has `DATABASE_URL=sqlite://dev.db` in a local `.env` or in the shell.
2. Run `cargo sqlx prepare --workspace` to write `.sqlx/query-*.json` files.
3. Commit `.sqlx/` to the repo.
4. CI sets `SQLX_OFFLINE=true` in every build env so macros read from `.sqlx/` instead of hitting a live DB.
5. `just sqlx-prepare` wraps `cargo sqlx prepare --workspace` so devs never forget.

Per D-discretion, recommend **committing `.sqlx/`**.

---

## 4. Migrations & Schema Parity

### Directory layout

```
migrations/
├── sqlite/
│   └── 20260410_000000_initial.up.sql
└── postgres/
    └── 20260410_000000_initial.up.sql
```

Both files share the same version prefix (`20260410_000000_initial`) so diffs read side-by-side. `.down.sql` files are NOT required by `sqlx::migrate!` and are omitted in Phase 1 — we don't support rollback.

### `migrations/sqlite/20260410_000000_initial.up.sql`

```sql
-- cronduit initial schema (SQLite)
--
-- Pairs with migrations/postgres/20260410_000000_initial.up.sql.
-- Any structural change MUST land in both files in the same PR,
-- and tests/schema_parity.rs MUST remain green.
--
-- Design notes:
--   * jobs.config_json is TEXT (never JSONB). D-16.
--   * jobs.config_hash is SHA-256 hex of the normalized (sorted-keys,
--     stable-ordering) JSON representation of the job config. See
--     src/config/hash.rs. D-15.
--   * Timestamps are RFC3339 TEXT for SQLite portability.

CREATE TABLE IF NOT EXISTS jobs (
    id                 INTEGER PRIMARY KEY,
    name               TEXT    NOT NULL UNIQUE,
    schedule           TEXT    NOT NULL,
    resolved_schedule  TEXT    NOT NULL,
    job_type           TEXT    NOT NULL,  -- 'command' | 'script' | 'docker'
    config_json        TEXT    NOT NULL,
    config_hash        TEXT    NOT NULL,
    enabled            INTEGER NOT NULL DEFAULT 1,
    timeout_secs       INTEGER NOT NULL,
    created_at         TEXT    NOT NULL,
    updated_at         TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);

CREATE TABLE IF NOT EXISTS job_runs (
    id             INTEGER PRIMARY KEY,
    job_id         INTEGER NOT NULL REFERENCES jobs(id),
    status         TEXT    NOT NULL,
    trigger        TEXT    NOT NULL,
    start_time     TEXT    NOT NULL,
    end_time       TEXT,
    duration_ms    INTEGER,
    exit_code      INTEGER,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);

CREATE TABLE IF NOT EXISTS job_logs (
    id         INTEGER PRIMARY KEY,
    run_id     INTEGER NOT NULL REFERENCES job_runs(id),
    stream     TEXT    NOT NULL,  -- 'stdout' | 'stderr'
    ts         TEXT    NOT NULL,
    line       TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_logs_run_id_id ON job_logs(run_id, id);
```

### `migrations/postgres/20260410_000000_initial.up.sql`

```sql
-- cronduit initial schema (PostgreSQL)
--
-- Pairs with migrations/sqlite/20260410_000000_initial.up.sql. Keep in sync.

CREATE TABLE IF NOT EXISTS jobs (
    id                 BIGSERIAL PRIMARY KEY,
    name               TEXT    NOT NULL UNIQUE,
    schedule           TEXT    NOT NULL,
    resolved_schedule  TEXT    NOT NULL,
    job_type           TEXT    NOT NULL,
    config_json        TEXT    NOT NULL,  -- TEXT not JSONB (D-16)
    config_hash        TEXT    NOT NULL,
    enabled            SMALLINT NOT NULL DEFAULT 1,  -- 0/1; matches SQLite INTEGER semantics
    timeout_secs       BIGINT  NOT NULL,
    created_at         TEXT    NOT NULL,             -- RFC3339 string, same as SQLite
    updated_at         TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_enabled ON jobs(enabled);

CREATE TABLE IF NOT EXISTS job_runs (
    id             BIGSERIAL PRIMARY KEY,
    job_id         BIGINT  NOT NULL REFERENCES jobs(id),
    status         TEXT    NOT NULL,
    trigger        TEXT    NOT NULL,
    start_time     TEXT    NOT NULL,
    end_time       TEXT,
    duration_ms    BIGINT,
    exit_code      INTEGER,
    container_id   TEXT,
    error_message  TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_start_time   ON job_runs(start_time);

CREATE TABLE IF NOT EXISTS job_logs (
    id         BIGSERIAL PRIMARY KEY,
    run_id     BIGINT  NOT NULL REFERENCES job_runs(id),
    stream     TEXT    NOT NULL,
    ts         TEXT    NOT NULL,
    line       TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_logs_run_id_id ON job_logs(run_id, id);
```

### Type normalization whitelist (for schema parity test)

| SQLite type | Postgres type | Justification |
|-------------|---------------|---------------|
| `INTEGER` (PK) | `BIGSERIAL` | Both map to Rust `i64` in sqlx; SQLite's `INTEGER PRIMARY KEY` is a 64-bit rowid. |
| `INTEGER` (FK / size column) | `BIGINT` | 64-bit signed integer on both. `duration_ms`, `timeout_secs`, `job_id`, `run_id`. |
| `INTEGER` (enabled flag) | `SMALLINT` | Single intentional divergence: we store 0/1 on both; `SMALLINT` in PG documents intent without costing storage. Rust reads both as `i16`/`i32` via `as` coercion in query mapping. |
| `INTEGER` (exit_code) | `INTEGER` | Exit codes fit in 32-bit. |
| `TEXT` | `TEXT` | No divergence. |

**Whitelist rule:** The parity test replaces `BIGSERIAL`→`INTEGER`, `BIGINT`→`INTEGER`, `SMALLINT`→`INTEGER` **before** comparing column types. Every added entry requires a comment justifying the normalization. Unknown type names fail the test rather than being silently normalized.

### Deliberately identical index set

Both files declare exactly these indexes:
- `idx_jobs_enabled`
- `idx_job_runs_job_id_start`
- `idx_job_runs_start_time`
- `idx_job_logs_run_id_id`

**NOTE:** ARCHITECTURE.md §Database Schema shows a partial index `CREATE INDEX idx_job_runs_status ON job_runs(status) WHERE status = 'running'`. SQLite supports partial indexes, Postgres supports them too — they're portable. But Phase 1 does not yet query on running state (no scheduler), so to keep parity comparison trivial the planner SHOULD defer this partial index to Phase 2's first scheduler PR. Document in migration header.

---

## 5. `tests/schema_parity.rs` Scaffolding

### Full test skeleton

```rust
//! tests/schema_parity.rs
//!
//! Structural parity between migrations/sqlite and migrations/postgres.
//! Surfaced via `just schema-diff`. Runs in every CI matrix cell.
//!
//! Failure of this test is a HARD STOP — do not merge.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Row, SqlitePool, PgPool};
use std::collections::{BTreeMap, BTreeSet};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd)]
struct Column {
    name: String,
    normalized_type: String,
    not_null: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Schema {
    tables: BTreeMap<String, BTreeSet<Column>>,
    indexes: BTreeMap<String, BTreeSet<String>>, // table -> index names
}

fn normalize_type(raw: &str) -> String {
    // Whitelist from the migration header. Each branch MUST have a comment
    // explaining why the normalization is semantically safe.
    let upper = raw.trim().to_ascii_uppercase();
    match upper.as_str() {
        // SQLite INTEGER PRIMARY KEY == PG BIGSERIAL (both Rust i64)
        "INTEGER" | "BIGINT" | "BIGSERIAL" | "INT8" => "INT64".to_string(),
        // PG SMALLINT used for 0/1 enabled flag; Rust side treats identically
        "SMALLINT" | "INT2" => "INT16".to_string(),
        "INT" | "INT4" => "INT32".to_string(),
        "TEXT" | "VARCHAR" | "CHARACTER VARYING" => "TEXT".to_string(),
        other => panic!("unknown column type {other:?}; add to normalize_type whitelist with justification"),
    }
}

async fn introspect_sqlite(pool: &SqlitePool) -> Schema {
    let mut tables: BTreeMap<String, BTreeSet<Column>> = BTreeMap::new();
    let mut indexes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlx_%' AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(pool).await.unwrap();
    for row in rows {
        let tbl: String = row.get(0);
        let cols = sqlx::query(&format!("PRAGMA table_info('{}')", tbl))
            .fetch_all(pool).await.unwrap();
        let mut col_set = BTreeSet::new();
        for c in cols {
            col_set.insert(Column {
                name: c.get::<String, _>("name"),
                normalized_type: normalize_type(&c.get::<String, _>("type")),
                not_null: c.get::<i64, _>("notnull") != 0,
            });
        }
        tables.insert(tbl.clone(), col_set);

        let idx_rows = sqlx::query(&format!("PRAGMA index_list('{}')", tbl))
            .fetch_all(pool).await.unwrap();
        let mut idx_set = BTreeSet::new();
        for ir in idx_rows {
            let nm: String = ir.get("name");
            if nm.starts_with("sqlite_autoindex_") { continue; }
            idx_set.insert(nm);
        }
        indexes.insert(tbl, idx_set);
    }
    Schema { tables, indexes }
}

async fn introspect_postgres(pool: &PgPool) -> Schema {
    let mut tables: BTreeMap<String, BTreeSet<Column>> = BTreeMap::new();
    let mut indexes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let rows = sqlx::query(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema='public' AND table_name NOT LIKE '\\_sqlx%' ESCAPE '\\'
           AND table_type='BASE TABLE'"
    ).fetch_all(pool).await.unwrap();
    for row in rows {
        let tbl: String = row.get(0);
        let cols = sqlx::query(
            "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_schema='public' AND table_name=$1"
        )
        .bind(&tbl)
        .fetch_all(pool).await.unwrap();
        let mut col_set = BTreeSet::new();
        for c in cols {
            col_set.insert(Column {
                name: c.get::<String, _>("column_name"),
                normalized_type: normalize_type(&c.get::<String, _>("data_type")),
                not_null: c.get::<String, _>("is_nullable") == "NO",
            });
        }
        tables.insert(tbl.clone(), col_set);

        let idx_rows = sqlx::query(
            "SELECT indexname FROM pg_indexes WHERE schemaname='public' AND tablename=$1"
        )
        .bind(&tbl)
        .fetch_all(pool).await.unwrap();
        let mut idx_set = BTreeSet::new();
        for ir in idx_rows {
            let nm: String = ir.get("indexname");
            // Skip the implicit PK index that PG auto-creates
            if nm.ends_with("_pkey") { continue; }
            idx_set.insert(nm);
        }
        indexes.insert(tbl, idx_set);
    }
    Schema { tables, indexes }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schema_parity() {
    // --- SQLite side: in-memory ---
    let sqlite = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await.expect("sqlite connect");
    sqlx::migrate!("./migrations/sqlite").run(&sqlite).await.expect("sqlite migrate");
    let sqlite_schema = introspect_sqlite(&sqlite).await;

    // --- Postgres side: testcontainer ---
    let pg_container = Postgres::default().start().await.expect("pg container start");
    let pg_host = pg_container.get_host().await.unwrap();
    let pg_port = pg_container.get_host_port_ipv4(5432).await.unwrap();
    let pg_url = format!("postgres://postgres:postgres@{pg_host}:{pg_port}/postgres");
    let pg = PgPoolOptions::new()
        .max_connections(2)
        .connect(&pg_url)
        .await.expect("pg connect");
    sqlx::migrate!("./migrations/postgres").run(&pg).await.expect("pg migrate");
    let pg_schema = introspect_postgres(&pg).await;

    // --- Diff ---
    if sqlite_schema != pg_schema {
        eprintln!("SCHEMA PARITY DRIFT DETECTED\n");
        eprintln!("SQLite-only tables: {:?}",
            sqlite_schema.tables.keys().collect::<BTreeSet<_>>()
                .difference(&pg_schema.tables.keys().collect::<BTreeSet<_>>())
                .collect::<Vec<_>>());
        eprintln!("Postgres-only tables: {:?}",
            pg_schema.tables.keys().collect::<BTreeSet<_>>()
                .difference(&sqlite_schema.tables.keys().collect::<BTreeSet<_>>())
                .collect::<Vec<_>>());
        // print per-table diffs
        for (t, sqlite_cols) in &sqlite_schema.tables {
            if let Some(pg_cols) = pg_schema.tables.get(t) {
                let only_sqlite: Vec<_> = sqlite_cols.difference(pg_cols).collect();
                let only_pg: Vec<_> = pg_cols.difference(sqlite_cols).collect();
                if !only_sqlite.is_empty() || !only_pg.is_empty() {
                    eprintln!("  table `{t}`:");
                    for c in only_sqlite { eprintln!("    sqlite-only: {c:?}"); }
                    for c in only_pg     { eprintln!("    postgres-only: {c:?}"); }
                }
            }
        }
        for (t, s_idx) in &sqlite_schema.indexes {
            if let Some(p_idx) = pg_schema.indexes.get(t) {
                if s_idx != p_idx {
                    eprintln!("  index diff on `{t}`:");
                    eprintln!("    sqlite: {s_idx:?}");
                    eprintln!("    postgres: {p_idx:?}");
                }
            }
        }
        panic!("schema parity check failed — see diff above");
    }
}
```

### Feature-gating & CI wiring

- **Not feature-gated.** The test runs on every `cargo test` invocation. testcontainers spawns a Postgres container at test time; this is the only test in Phase 1 that requires Docker-in-Docker / Docker-on-the-runner.
- **GitHub Actions runners** already have Docker available (`ubuntu-latest`, `ubuntu-22.04`). No additional setup required.
- **Local dev** requires a running Docker daemon. If a dev machine has no Docker, `just test` will fail loudly on this one test — that's acceptable because the phase has Docker as a hard dependency for multi-arch builds anyway.
- **`just schema-diff`** is a narrow invocation:
  ```just
  schema-diff:
      cargo test --test schema_parity -- --nocapture
  ```
- **testcontainers pitfalls to document:**
  1. First-run image pull adds 10-30s latency — acceptable.
  2. Postgres container may race on port availability; `testcontainers-modules` handles the wait-for-ready internally, but if flakes appear, add `Postgres::default().with_wait_for(...)`.
  3. `cargo nextest` runs tests in parallel by default — the schema_parity test uses one shared Postgres container per test process (not shared across processes), so parallelism is fine; if flakiness appears, add `#[serial]` via `serial_test` crate (not needed in Phase 1).

---

## 6. TOML Config + Env Interpolation + secrecy

### The config struct

```rust
// src/config/mod.rs
use secrecy::SecretString;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: Option<DefaultsConfig>,
    #[serde(default, rename = "jobs")]
    pub jobs: Vec<JobConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String, // "127.0.0.1:8080" — see D-24
    #[serde(default = "default_db_url")]
    pub database_url: SecretString, // credentials in URL are secret
    pub timezone: String, // mandatory (D-19)
    #[serde(default = "default_shutdown_grace", with = "humantime_serde")]
    pub shutdown_grace: Duration,
    #[serde(default = "default_log_retention", with = "humantime_serde")]
    pub log_retention: Duration, // shipped in config per D-32, pruner is Phase 6
}

fn default_bind() -> String { "127.0.0.1:8080".into() }
fn default_db_url() -> SecretString { SecretString::from("sqlite:///data/cronduit.db") }
fn default_shutdown_grace() -> Duration { Duration::from_secs(30) }
fn default_log_retention() -> Duration { Duration::from_secs(60 * 60 * 24 * 90) } // 90d

#[derive(Debug, Deserialize)]
pub struct DefaultsConfig {
    pub image: Option<String>,
    pub network: Option<String>,
    pub volumes: Option<Vec<String>>,
    pub delete: Option<bool>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde::option")]
    pub random_min_gap: Option<Duration>,
}

#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub schedule: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub script:  Option<String>,
    #[serde(default)]
    pub image:   Option<String>,
    #[serde(default)]
    pub use_defaults: Option<bool>, // None = defaults apply; Some(false) = no defaults
    // Secrets show up here via env interpolation — SecretString wrapping is at the
    // `env` map, NOT at the job level, because name/command/image are shown in UI.
    #[serde(default)]
    pub env: BTreeMap<String, SecretString>,
    pub volumes: Option<Vec<String>>,
    pub network: Option<String>,
    pub container_name: Option<String>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
}
```

**Why `SecretString` on `env` values and `database_url`:**
- `secrecy 0.10.3` with `features = ["serde"]` gives `SecretString` a transparent `Deserialize` impl via `SecretBox<T> where T: DeserializeOwned`. No `#[serde(with = ...)]` needed. [CITED: docs.rs secrecy 0.10.3 — "when the serde feature is enabled, the SecretBox type will receive a Deserialize impl for all SecretBox<T>"]
- `secrecy::SecretBox<T>` does NOT derive `Debug` to leak; it prints `SecretBox<...>([REDACTED])`. No extra work needed. [CITED: secrecy README]
- `Serialize` is **not** implemented by default — you'd have to opt in with `SerializableSecret`. This is desirable: it means `serde_json::to_string(&config)` will compile-fail rather than silently write the secret to a log line. (Note: if we ever need to write a normalized-JSON `config_hash`, hash the **redacted** representation or construct the JSON manually — do NOT call `to_string(&config)`.)
- **Known footgun:** Forgetting to enable the `serde` feature produces a compiler error like *"the trait bound `SecretString: Deserialize<'_>` is not satisfied"*. Document this in `Cargo.toml` comments.

### Env-var interpolation pipeline (strict `${VAR}`)

Three-stage pipeline. The pre-parse pass works on the raw string — it preserves byte offsets so TOML errors from later stages still line-number correctly.

```rust
// src/config/interpolate.rs
use regex::Regex;
use std::borrow::Cow;

/// Errors produced by env interpolation. Each carries a source byte range
/// that can later be turned into (line, col) via the Input wrapper.
#[derive(Debug)]
pub struct InterpolationError {
    pub var: String,
    pub byte_range: std::ops::Range<usize>,
}

/// Expands `${VAR}` references in-place. Returns interpolated string and
/// any number of errors for missing vars. Does NOT early-exit on first error
/// (D-21: collect-all).
pub fn interpolate(input: &str) -> (String, Vec<InterpolationError>) {
    // Reject `${VAR:-default}` syntax early — strict v1 (D-22).
    static VAR_RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap()
    });
    static DEFAULT_RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"\$\{[^}]*:-").unwrap()
    });

    let mut errors = Vec::new();

    // Pre-check: forbid default-value syntax to surface typos early.
    for m in DEFAULT_RE.find_iter(input) {
        errors.push(InterpolationError {
            var: format!("{}", m.as_str()),
            byte_range: m.range(),
        });
    }

    let result = VAR_RE.replace_all(input, |caps: &regex::Captures| {
        let var = &caps[1];
        match std::env::var(var) {
            Ok(v) => v,
            Err(_) => {
                errors.push(InterpolationError {
                    var: var.to_string(),
                    byte_range: caps.get(0).unwrap().range(),
                });
                String::new() // placeholder; caller uses errors to fail
            }
        }
    });

    (result.into_owned(), errors)
}
```

**Byte-range preservation:** If a missing var triggers an error at byte 312, the error path computes `(line, col)` by counting `\n` in `input[..312]`. This is standalone from `toml::de::Error::span()` — we add env-interpolation errors to the same `Vec<ConfigError>` using the same `(path, line, col)` schema.

### The unified error type + GCC-style printer

```rust
// src/config/errors.rs
use std::path::PathBuf;

#[derive(Debug)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: usize,   // 1-indexed
    pub col: usize,    // 1-indexed
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}: error: {}", self.file.display(), self.line, self.col, self.message)
    }
}

/// Convert a byte offset in `source` into 1-indexed (line, col).
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col  = 1usize;
    for (i, c) in source.char_indices() {
        if i >= offset { break; }
        if c == '\n' { line += 1; col = 1; } else { col += 1; }
    }
    (line, col)
}
```

### `toml::de::Error::span()` wiring

- The method exists and returns `Option<Range<usize>>`. [VERIFIED: docs.rs toml 1.1.2 Error::span]
- We call `err.span()` after `toml::from_str::<Config>` fails, then convert via `byte_offset_to_line_col` against the ORIGINAL input (post-interpolation). Because interpolation replaces `${VAR}` tokens with values, line numbers can shift by multi-char deltas within a line — to keep errors accurate, the interpolator should (a) record the substitution map with `(original_range, substituted_range)` pairs, OR (b) replace each `${VAR}` with a **same-length** placeholder during validation-only flows (the `check` path). Recommendation: use approach (a) but only when errors actually fire — keeping the happy path simple.

### Shared parse+validate function

```rust
// src/config/mod.rs (continued)
pub struct ParsedConfig {
    pub config: Config,
    pub source_path: PathBuf,
}

/// Shared by `cronduit check` and `cronduit run`. Never touches the DB.
pub fn parse_and_validate(path: &std::path::Path) -> Result<ParsedConfig, Vec<ConfigError>> {
    let raw = std::fs::read_to_string(path).map_err(|e| vec![ConfigError {
        file: path.into(), line: 0, col: 0,
        message: format!("cannot read file: {e}"),
    }])?;

    let (interpolated, interp_errors) = interpolate::interpolate(&raw);

    let mut errors: Vec<ConfigError> = interp_errors.into_iter().map(|e| {
        let (line, col) = errors::byte_offset_to_line_col(&raw, e.byte_range.start);
        ConfigError {
            file: path.into(), line, col,
            message: format!("missing environment variable `${{{}}}`", e.var),
        }
    }).collect();

    let config = match toml::from_str::<Config>(&interpolated) {
        Ok(c) => Some(c),
        Err(e) => {
            let (line, col) = e.span()
                .map(|r| errors::byte_offset_to_line_col(&interpolated, r.start))
                .unwrap_or((0, 0));
            errors.push(ConfigError {
                file: path.into(), line, col, message: e.message().to_string(),
            });
            None
        }
    };

    if let Some(cfg) = &config {
        validate::run_all_checks(cfg, path, &mut errors);
    }

    if errors.is_empty() {
        Ok(ParsedConfig { config: config.unwrap(), source_path: path.into() })
    } else {
        Err(errors)
    }
}
```

### Post-parse validations (feed into `Vec<ConfigError>`)

- Job names unique (CONF-10) — report both line numbers of duplicates
- Each job has exactly one of `command`/`script`/`image` (CONF-05)
- `[server].timezone` is a valid IANA zone (see §6 IANA validation)
- Cron schedule is syntactically valid (placeholder check in Phase 1; real parse is Phase 2 via croner)
- `bind` parses as `SocketAddr`
- Docker network strings are well-formed (`bridge` | `host` | `none` | `container:<name>` | `<name>`)

---

## 6.5. IANA Timezone Validation

**Decision:** Use `chrono-tz = "0.10.4"`. [VERIFIED: crates.io 2026-04-09]

```rust
pub fn validate_iana_zone(name: &str) -> Result<(), String> {
    name.parse::<chrono_tz::Tz>()
        .map(|_| ())
        .map_err(|_| format!("not a valid IANA timezone: {name}"))
}
```

**Why not `iana-time-zone`?** `iana-time-zone` (0.1.65) is for *detecting* the host's current zone name — it doesn't parse or validate arbitrary zone strings. Wrong crate for this job.

**Why `chrono-tz` specifically:**
1. Bundles the entire IANA zone database as `const` data — no network lookups, no filesystem reads.
2. Has `FromStr for Tz` — single line validation.
3. **Plays cleanly with croner in Phase 2** — croner has an optional `chrono` integration feature (STACK.md), and croner examples use `chrono_tz` to pass timezone-aware `DateTime<Tz>` into `next_after`.
4. Cost: ~1.3 MB binary overhead for the static zone database. Acceptable.

**Rolling from a static list** (rejected): the IANA zone list has ~600 entries, changes ~twice a year, and maintaining our own list would just be `chrono-tz` with extra steps.

---

## 7. Tracing Subscriber & Startup Event

### Subscriber setup

```rust
// src/telemetry.rs
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Debug, Clone, Copy)]
pub enum LogFormat { Json, Text }

pub fn init(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cronduit=debug"));

    match format {
        LogFormat::Json => {
            let fmt_layer = fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(false)
                .with_target(true)
                .with_file(false)
                .with_line_number(false);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        LogFormat::Text => {
            let fmt_layer = fmt::layer()
                .with_target(true)
                .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()));
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
    }
}
```

- Default JSON per D-03; `--log-format=text` switches formats but does not auto-detect tty — the tty check inside `Text` only gates ANSI color escapes, which is an orthogonal concern from "is the operator asking for JSON."
- `RUST_LOG` env filter honored via `EnvFilter::try_from_default_env()`.

### The `cronduit.startup` event (D-23)

```rust
// src/main.rs (after config parsed, DB open, migrations run)
use url::Url;

fn strip_credentials(database_url: &str) -> String {
    Url::parse(database_url)
        .map(|mut u| {
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        })
        .unwrap_or_else(|_| "<unparseable>".into())
}

let backend = match pool.backend() {
    DbBackend::Sqlite   => "sqlite",
    DbBackend::Postgres => "postgres",
};

// D-24: bind warning
let bind_warning = !is_loopback(&resolved_bind);
if bind_warning {
    tracing::warn!(
        target: "cronduit.startup",
        bind = %resolved_bind,
        "web UI bound to non-loopback address — v1 ships without authentication; \
         see README SECURITY and THREAT_MODEL.md. Put cronduit behind a reverse proxy \
         with auth, or keep it on 127.0.0.1."
    );
}

// D-23: single structured startup summary
tracing::info!(
    target: "cronduit.startup",
    version = env!("CARGO_PKG_VERSION"),
    bind = %resolved_bind,
    database_backend = backend,
    database_url = %strip_credentials(&resolved_db_url),
    config_path = %config_path.display(),
    timezone = %config.server.timezone,
    job_count = config.jobs.len(),
    disabled_job_count = 0u64, // 0 in Phase 1 — no sync engine yet
    bind_warning,
    "cronduit starting"
);

fn is_loopback(addr: &std::net::SocketAddr) -> bool {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => v4.is_loopback(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    }
}
```

**Credential stripping:** Use the `url` crate, not a regex. `Url::parse("postgres://user:pass@host/db")` then `set_password(None)` and `set_username("")` produces `postgres://host/db`. This is robust against URLs where the password contains `@` or `?` characters, which a regex would misparse.

**`is_loopback` covers ALL non-loopback cases uniformly:** RFC1918 (`192.168.x.y`, `10.x.y.z`, `172.16-31.x.y`), `0.0.0.0`, public IPs, link-local — all trigger the same warning. This matches D-24 exactly.

**One event, one line:** D-23 is a single `info!` call. Subordinate subsystems still emit their own spans/events — this is the top-level summary for grep.

---

## 8. axum 0.8 Placeholder

**Recommendation:** Ship option **(a)** — a minimal always-on route — so the axum + tower-http wiring is exercised from day one. That surfaces dep-graph problems (especially the openssl-sys invariant) and proves graceful shutdown works end-to-end before Phase 2 adds the scheduler loop.

### Minimal router + graceful shutdown

```rust
// src/web/mod.rs
use axum::{Router, routing::get, http::StatusCode};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: crate::db::DbPool,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub version: &'static str,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn index() -> (StatusCode, &'static str) {
    (StatusCode::OK, "cronduit is running — no scheduler yet (Phase 1 placeholder)\n")
}

pub async fn serve(
    bind: SocketAddr,
    state: AppState,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "listening");

    axum::serve(listener, router(state).into_make_service())
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await?;

    Ok(())
}
```

**Graceful shutdown with CancellationToken:**

```rust
// src/shutdown.rs
use tokio::signal;
use tokio_util::sync::CancellationToken;

pub fn install(token: CancellationToken) {
    tokio::spawn(async move {
        let ctrl_c = async { let _ = signal::ctrl_c().await; };
        #[cfg(unix)]
        let term = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler")
                .recv().await;
        };
        #[cfg(not(unix))]
        let term = std::future::pending::<()>();

        tokio::select! { _ = ctrl_c => {}, _ = term => {} }
        tracing::info!("shutdown signal received");
        token.cancel();
    });
}
```

**Rationale over option (b) `--serve-ui` flag:**
- Exercising the HTTP listener on every Phase 1 invocation means CI notices if `axum::serve(...)` or `with_graceful_shutdown` regresses.
- OPS-03 (`[server].bind` default = `127.0.0.1:8080`) becomes immediately testable — `curl http://127.0.0.1:8080/` returns the placeholder string.
- The bind-safety warning (D-24) becomes observable in integration tests.
- The complexity cost is ~40 lines of code.

---

## 9. justfile Draft

Complete `justfile` covering every D-11 recipe with real bodies. `set shell := ["bash", "-euo", "pipefail", "-c"]` ensures all recipes fail fast on any error.

```just
# justfile — Single source of truth for build/test/lint/DB/image/dev-loop.
# All GitHub Actions jobs call `just <recipe>` exclusively (D-10).

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true

# -------------------- meta --------------------

# Show all available recipes
default:
    @just --list

# The ORDERED chain CI runs. Local `just ci` must predict CI exit code.
ci: fmt-check clippy openssl-check nextest schema-diff image

# -------------------- build & artifacts --------------------

build:
    cargo build --all-targets

build-release:
    cargo build --release

clean:
    cargo clean
    rm -rf .sqlx/tmp assets/static/app.css

# Standalone Tailwind binary — NO Node. Phase 1 scaffolds the pipeline;
# the design system doesn't actually ship until Phase 3.
tailwind:
    @mkdir -p assets/static
    # Idempotent install of the standalone binary into ./bin/tailwindcss
    @if [ ! -x ./bin/tailwindcss ]; then \
        mkdir -p bin; \
        echo "Downloading standalone Tailwind binary..."; \
        curl -sSLo ./bin/tailwindcss \
            "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/')"; \
        chmod +x ./bin/tailwindcss; \
    fi
    ./bin/tailwindcss -i assets/src/app.css -o assets/static/app.css --minify

# Multi-arch Docker image via cargo-zigbuild (no QEMU).
# On CI PR:    --load (local, no push)
# On CI main:  uses `just image-push` variant below
image:
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        --tag cronduit:dev \
        --load \
        .

image-push tag:
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        --tag ghcr.io/{{tag}} \
        --push \
        .

# -------------------- quality gates --------------------

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-features

nextest:
    cargo nextest run --all-features --profile ci

# Pitfall 14 guard — MUST be empty. See §2.
openssl-check:
    @echo "Verifying rustls-only TLS stack (no openssl-sys allowed)..."
    @if cargo tree -i openssl-sys 2>/dev/null | grep -q .; then \
        echo "FAIL: openssl-sys found in dependency graph:"; \
        cargo tree -i openssl-sys; \
        exit 1; \
    fi
    @echo "OK: no openssl-sys in dependency tree"

# -------------------- DB / schema --------------------

db-reset:
    rm -f cronduit.dev.db cronduit.dev.db-wal cronduit.dev.db-shm
    @echo "SQLite dev DB removed."

migrate:
    # Phase 1: migrations run on `cronduit run` startup; this recipe exists
    # so Phase 2+ query-time-check flows have a reusable hook.
    cargo run --quiet -- run --config examples/cronduit.toml

sqlx-prepare:
    DATABASE_URL=sqlite://cronduit.dev.db cargo sqlx prepare --workspace

# Surface the schema parity test on its own (D-14).
schema-diff:
    cargo test --test schema_parity -- --nocapture

# -------------------- dev loop --------------------

dev:
    # Two-process dev loop in one terminal via `just`'s `&&` chaining.
    # Use `--log-format=text` for human-readable output during dev.
    RUST_LOG=debug,cronduit=trace cargo run -- run \
        --config examples/cronduit.toml \
        --log-format=text

check-config PATH:
    cargo run --quiet -- check {{PATH}}

docker-compose-up:
    docker compose -f examples/docker-compose.yml up
```

**Gotchas to call out:**

- `set shell := ["bash", ...]` — Linux/macOS. On Windows the dev loop needs WSL or Git Bash; acceptable since the deployment target is Linux and CI runs on Linux.
- `set dotenv-load := true` — `just` auto-loads `.env` (not committed) so local devs can set `DATABASE_URL` without exporting manually.
- `image` vs `image-push` — two recipes. CI calls `image` on PRs (load-only) and `image-push` on `main` pushes. The `docker buildx build` with `--load` CANNOT build multi-platform images into the local daemon in a single invocation on older buildx, but **0.12+** supports it; document the required buildx version in the `justfile` header or fall back to separate per-arch `--load` calls if the runner image is older.
- `dev` recipe is single-process (cargo-watch is listed in D-discretion territory; omitting it keeps the recipe minimal). A Phase 2 PR can add `cargo-watch` once there's actually something to live-reload.
- `tailwind` downloads the binary on first run; the URL format is `tailwindcss-<os>-<arch>`. On CI (Linux/x64) the install is ~10s.

---

## 10. GitHub Actions `ci.yml` Draft

```yaml
# .github/workflows/ci.yml
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
  packages: write  # only used in the image job on push to main

jobs:
  lint:
    name: lint (fmt + clippy + openssl-sys guard)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - uses: extractions/setup-just@v2
      - run: just fmt-check
      - run: just clippy
      - run: just openssl-check

  test:
    name: test ${{ matrix.arch }} × ${{ matrix.db }}
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        arch: [amd64, arm64]
        db:   [sqlite, postgres]
    env:
      SQLX_OFFLINE: "true"
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.arch }}-${{ matrix.db }}
      - uses: extractions/setup-just@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: nextest,cargo-zigbuild
      - name: Install cross-compile targets (for arm64 build verification)
        if: matrix.arch == 'arm64'
        run: |
          rustup target add aarch64-unknown-linux-musl
          rustup target add x86_64-unknown-linux-musl

      # Full test suite. The schema_parity test handles its own Postgres
      # container via testcontainers; no `services:` block needed.
      - name: just nextest
        run: just nextest
      - name: just schema-diff
        run: just schema-diff

  image:
    name: multi-arch docker image
    runs-on: ubuntu-latest
    needs: [lint, test]
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-qemu-action@v3    # for buildx platform emulation on manifest stitching
      - uses: docker/setup-buildx-action@v3
      - uses: extractions/setup-just@v2

      # PR path: build both platforms, load no push
      - name: just image (PR — build only)
        if: github.event_name == 'pull_request'
        run: just image

      # main path: build and push to GHCR
      - name: docker login (main only)
        if: github.ref == 'refs/heads/main' && github.event_name == 'push'
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: just image-push (main)
        if: github.ref == 'refs/heads/main' && github.event_name == 'push'
        run: |
          just image-push "${{ github.repository_owner }}/cronduit:latest"
          just image-push "${{ github.repository_owner }}/cronduit:sha-${GITHUB_SHA:0:7}"
```

**Key patterns:**

- **`just`-only invocation** (D-10): every step that does real work calls `just <recipe>`. No raw `cargo`/`docker` inline.
- **4-cell test matrix**: `{amd64, arm64} × {sqlite, postgres}`. Both dimensions exist as matrix values even though the actual test process runs on amd64 — the `arm64` cells verify that `cargo-zigbuild --target aarch64-unknown-linux-musl` builds cleanly, which is the failure mode Pitfall 14 predicts. The `db` dimension is currently more aspirational than functional because the schema parity test brings up its own Postgres container regardless; a Phase 2+ PR can split this into `sqlite-only` and `postgres-only` test filter flags.
- **`lint` job is unmatrixed** — fmt/clippy/openssl-check are backend-agnostic; matrixing them is waste. (D-17.)
- **`image` job `needs: [lint, test]`** — hard dependency; image never builds if tests or lint fail.
- **`concurrency.cancel-in-progress: true`** — new pushes to the same PR cancel in-flight runs.
- **`permissions: packages: write`** — scoped only for GHCR push. Safe because `push` step is gated on `github.ref == 'refs/heads/main'`.
- **`SQLX_OFFLINE=true`** — sqlx-macros read from `.sqlx/` JSON instead of needing a live database. We commit `.sqlx/` to the repo (recommended per D-discretion).

**What's NOT in this YAML yet:**
- Postgres `services:` block — the schema-parity test uses testcontainers to bring up its own container; no need for `services:`. Future per-query integration tests in Phase 2+ may want `services.postgres`, but not yet.
- `ghcr.io` image signing (cosign) — Phase 6 release-engineering concern.

---

## 11. Multi-arch Docker + cargo-zigbuild

### `Dockerfile` (multi-stage)

```dockerfile
# syntax=docker/dockerfile:1.7

# ---- builder ----
# Stay on amd64; cargo-zigbuild cross-compiles both targets here.
FROM --platform=$BUILDPLATFORM rust:1.94-slim-bookworm AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-zigbuild (pulls the ziglang binary on its own).
# Using the pip path keeps the install self-contained; `cargo install` works too.
RUN curl -sSL https://github.com/ziglang/zig/releases/download/0.13.0/zig-linux-$(uname -m)-0.13.0.tar.xz \
    | tar -xJ -C /opt \
 && ln -s /opt/zig-linux-*/zig /usr/local/bin/zig \
 && cargo install --locked cargo-zigbuild

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

WORKDIR /build
COPY . .

# Translate buildx TARGETPLATFORM → rustc target triple.
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") echo "x86_64-unknown-linux-musl"  > /target.txt ;; \
      "linux/arm64") echo "aarch64-unknown-linux-musl" > /target.txt ;; \
      *) echo "unsupported platform: $TARGETPLATFORM" >&2; exit 1 ;; \
    esac

RUN TARGET="$(cat /target.txt)" \
 && cargo zigbuild --release --target "$TARGET" \
 && cp "target/$TARGET/release/cronduit" /cronduit

# ---- runtime ----
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=builder /cronduit /cronduit
# Migrations are embedded via `sqlx::migrate!(...)` — no filesystem copy needed.
# Example config lives in the image so `--config /etc/cronduit/config.toml` works.
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
```

### Why `distroless/static` + musl

1. **`musl` fully-static binary + `distroless/static`** = ~15 MB final image, zero glibc surprises.
2. **`distroless/static`** has `ca-certificates` but no shell or package manager — shrinks attack surface to "whatever's in the binary."
3. **`nonroot` variant** runs as UID 65532 — if the container is compromised, the attacker has no path to host root beyond the Docker socket mount (which they already don't have for a static test container; only Cronduit mounts the socket, and that's documented in the threat model).
4. **`musl` trade-off**: musl's DNS resolver has edge cases (no `nsswitch.conf`, limited glibc-specific locale handling). Acceptable for Cronduit: the only outgoing network calls come from Docker API (Unix socket) and Postgres (IP addresses usually). If DNS issues appear in Phase 4+, switch to `gcr.io/distroless/cc-debian12` + `glibc` target — cargo-zigbuild supports both.

### `cargo-zigbuild` health check

- **0.22.1** (February 2026), actively maintained. [VERIFIED: GitHub release feed 2026-04-09]
- **Known issues (per project README)**: zig 0.15+ with crates that use `bindgen` may need `clang 18+`. We don't use `bindgen`-based crates in Phase 1, so this is a non-issue.
- **No known sqlx or rustls issues** as of research date. [CITED: cargo-zigbuild README 2026-04-09]

### `docker buildx` manifest stitching

Buildx runs the Dockerfile once per platform target (via `$TARGETPLATFORM`). When we pass `--platform linux/amd64,linux/arm64 --push`, buildx:
1. Runs the builder stage twice (once per target) — both times on the amd64 runner because `--platform=$BUILDPLATFORM` pins the builder to the runner's native arch.
2. `cargo zigbuild --target <triple>` produces the correct-arch binary both times without QEMU.
3. Buildx collects both runtime images and pushes a **manifest list** to the registry, so `docker pull ghcr.io/owner/cronduit:latest` on an arm64 host pulls the arm64 layer transparently.

The `--load` path (PR builds) has a historical limitation: older buildx couldn't load multi-platform manifests into the local daemon. **Buildx 0.12+ supports `--load` with multi-platform** via the containerd image store; GitHub runners have 0.12+. If this breaks in practice, the fallback is splitting `just image` into two single-platform invocations. Document in the justfile comment.

---

## 12. `cronduit check` Error Format

### Decision: hand-rolled formatter

**Don't pull `codespan-reporting` or `miette` in Phase 1.** Both are capable crates but add weight for a format we've already fully specified:

```
path/to/config.toml:42:15: error: missing environment variable `${API_KEY}`
path/to/config.toml:17:3:  error: duplicate job name `check-ip` (first declared at path/to/config.toml:12:3)
path/to/config.toml:28:11: error: not a valid IANA timezone: America/Los_Angles
```

The `ConfigError` struct defined in §5 has exactly enough fields for this format. A ~10-line printer is simpler than a dep:

```rust
// src/bin/check.rs  (or src/cli/check.rs called from main)
pub fn run_check(path: &Path) -> i32 {
    match config::parse_and_validate(path) {
        Ok(_) => {
            eprintln!("ok: {}", path.display());
            0
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("{}", e); // uses the Display impl from §5
            }
            eprintln!("\n{} error(s)", errors.len());
            1
        }
    }
}
```

**When to reconsider:** If Phase 3 adds a "config editor" UI that needs rich diagnostic rendering (source snippets with carets), `miette` becomes attractive. For Phase 1, `eprintln!` + the `Display` impl is right-sized.

**`color-eyre` option (per D-discretion):** Acceptable. Gives pretty panics during dev. Adds ~200 KB binary. Planner's call. Recommend including it behind a `--features dev` opt-in if at all, not default.

---

## 13. Pitfall Guard Mapping

Each Phase-1-relevant pitfall → testable constraint → enforcement mechanism.

| # | Pitfall | Constraint | Guard |
|---|---------|------------|-------|
| 1 | Docker socket is root-equivalent | README leads with SECURITY; `THREAT_MODEL.md` skeleton exists; default bind is `127.0.0.1:8080`; non-loopback bind emits loud WARN | CI: assert `grep -q "SECURITY" README.md` in first 50 lines; assert `THREAT_MODEL.md` exists; integration test binds to `0.0.0.0` → asserts log contains `bind_warning=true`; default-bind unit test |
| 7 | SQLite write contention | Writer pool `max_connections=1`; reader pool `max_connections=8`; WAL + `busy_timeout=5000` + `synchronous=NORMAL` + `foreign_keys=ON` applied via `after_connect` | Unit test on `DbPool::Sqlite::connect` asserts `PRAGMA journal_mode` returns `wal`, `PRAGMA busy_timeout` returns `5000`, and `write.size()` == 1 |
| 8 | Schema parity | Split migration dirs; tests/schema_parity.rs green | `just schema-diff` (§5); runs in every CI cell |
| 14 | Cross-compile + openssl-sys | Every TLS-capable dep uses rustls; feature flags documented in Cargo.toml comments | `just openssl-check` (§2); runs in lint job; also CI test matrix builds for `aarch64-unknown-linux-musl` |
| 15 | Zero-config surprises | `cronduit check` validates full config; one `cronduit.startup` structured event lists every effective setting | §7 code; integration test: `cronduit run --config fixture.toml` for 100ms → asserts startup event JSON line contains all D-23 fields |
| 18 | Secrets in error messages | `SecretString` wraps every secret-bearing field; `Debug` redacts; missing env vars fail with field path but not with any other field values | Unit test: `format!("{:?}", SecretString::from("hunter2"))` does NOT contain `hunter2`; integration test: `cronduit check` on a config with a secret-bearing missing env var produces an error that contains the VAR NAME but no other field values |
| 20 | Config format creep | Only TOML parser exists; no YAML/JSON/INI code paths | Grep: `rg -l "serde_yaml\|serde_yml\|serde_json::from_str.*config"` in `src/config/` returns nothing; code review policy |

### FOUND-11 mermaid-only-diagrams

Add a lint script (can be a `just` recipe `check-docs`) that greps the repo for ASCII-art giveaways:

```bash
# Reject ASCII box drawing in docs
! rg -l '[─│┌┐└┘├┤┬┴┼]' --type md .
# Reject bare lines of dashes used as horizontal boxes
! rg -lP '^\s*\+[-=]{3,}\+' --type md .
```

This can be layered into `just ci` or kept separate. Phase 1 planner's call.

---

## 14. Testing Strategy (unit + integration + testcontainers)

### Test tiers

| Tier | What | How |
|------|------|-----|
| **Unit** | Pure-function logic: env interpolation, `strip_credentials`, `normalize_type`, `byte_offset_to_line_col`, `is_loopback` | `#[cfg(test)] mod tests` in-module. `cargo test`. |
| **Integration** (no Docker) | Config parse→validate pipeline on fixture TOML files (`tests/fixtures/*.toml`); `cronduit check` black-box via `assert_cmd` | `tests/check_command.rs`, `tests/config_parser.rs`. `cargo test`. |
| **Integration** (Docker required) | `schema_parity` (Postgres via testcontainers); bind-safety startup event | `tests/schema_parity.rs`, `tests/startup_event.rs`. `cargo test` — Docker daemon required on the runner. |

### Fixtures

```
tests/fixtures/
├── valid-minimal.toml              # minimum viable config; should round-trip
├── valid-everything.toml           # exercises every optional field
├── invalid-missing-timezone.toml   # D-19 negative test
├── invalid-duplicate-job.toml      # CONF-10 negative test
├── invalid-missing-env-var.toml    # CONF-02 / D-22 negative test; sets no env
├── invalid-multiple-job-types.toml # CONF-05 negative test
├── invalid-bad-network.toml        # Docker network mode validation negative test
└── valid-with-secrets.toml         # verifies SecretString redaction end-to-end
```

### `assert_cmd` black-box test for `cronduit check`

```rust
// tests/check_command.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn check_valid_config_exits_zero() {
    Command::cargo_bin("cronduit").unwrap()
        .arg("check").arg("tests/fixtures/valid-minimal.toml")
        .assert().success();
}

#[test]
fn check_missing_timezone_reports_line_col() {
    Command::cargo_bin("cronduit").unwrap()
        .arg("check").arg("tests/fixtures/invalid-missing-timezone.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"))
        .stderr(predicate::str::contains("timezone"));
}

#[test]
fn check_collects_all_errors() {
    // fixture deliberately has TWO problems; assert both are reported
    Command::cargo_bin("cronduit").unwrap()
        .arg("check").arg("tests/fixtures/invalid-multiple.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate job name"))
        .stderr(predicate::str::contains("missing environment variable"));
}
```

### Startup event integration test

```rust
// tests/startup_event.rs
// Runs `cronduit run`, kills after 300ms, greps stdout for the startup event.
use assert_cmd::Command;
use std::time::Duration;

#[test]
fn startup_emits_expected_event() {
    let mut cmd = Command::cargo_bin("cronduit").unwrap();
    let output = cmd.arg("run")
        .arg("--config").arg("tests/fixtures/valid-minimal.toml")
        .arg("--database-url").arg("sqlite::memory:")
        .timeout(Duration::from_millis(500))
        .output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cronduit.startup"));
    assert!(stdout.contains("\"job_count\":"));
    assert!(stdout.contains("\"database_backend\":\"sqlite\""));
    assert!(stdout.contains("\"bind_warning\":false"));
}
```

### testcontainers pitfalls (reminder)

Already covered in §5 — first-run image pull, parallelism, and port flakes.

---

## Runtime State Inventory

This phase is **greenfield** — no prior runtime state to reconcile. Explicitly:

| Category | Finding |
|----------|---------|
| Stored data | None — Phase 1 creates the schema for the first time. |
| Live service config | None — nothing is running yet. |
| OS-registered state | None. |
| Secrets/env vars | Env vars become a new interface (`${VAR}` in TOML); no pre-existing consumers to rename. |
| Build artifacts | None — `Cargo.toml`, `Cargo.lock`, `.sqlx/`, `target/` all created by Phase 1. |

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Build | ✓ | `rustc 1.94.1` | — (pinned via rust-toolchain.toml) |
| cargo | Build | ✓ | `1.94.1` | — |
| just | justfile recipes | ✓ | `1.48.1` | — |
| Docker daemon | Schema parity test, image build | ✓ | `29.1.4-rd` | — |
| `docker buildx` | Multi-arch image | ✓ (bundled with modern Docker) | — | QEMU fallback if missing |
| `cargo-zigbuild` | Cross-compile | ✗ (not pre-installed) | — | Installed on demand in Dockerfile + CI via `taiki-e/install-action` |
| `cargo-nextest` | `just nextest` | ✗ (not pre-installed) | — | Installed in CI via `taiki-e/install-action`; local devs can use `just test` |
| `sqlx-cli` | `just sqlx-prepare` | ✗ | — | Installed on demand: `cargo install sqlx-cli --no-default-features --features rustls,sqlite,postgres`. Add a `just setup` recipe in a future phase. |
| Standalone Tailwind binary | `just tailwind` | ✗ | — | `just tailwind` auto-downloads on first run. |
| Zig | cargo-zigbuild dep | ✗ | — | cargo-zigbuild + the pip wrapper both bundle ziglang automatically. |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** `cargo-zigbuild`, `cargo-nextest`, `sqlx-cli`, `tailwindcss binary` — all installed lazily by justfile recipes or CI actions. Planner should add a `just setup` recipe in Phase 1 that installs everything up front for a single-command onboarding (D-discretion).

## Validation Architecture

Nyquist-style validation: every acceptance criterion has a test, a command, and pass/fail signals. Consumed by `gsd-planner` to build VALIDATION.md.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (nextest on CI) + `assert_cmd` for CLI, `testcontainers-modules` for Postgres |
| Config file | `Cargo.toml` `[dev-dependencies]` + `.config/nextest.toml` (Wave 0) |
| Quick run command | `cargo nextest run --profile ci` |
| Full suite command | `just ci` (fmt-check + clippy + openssl-check + nextest + schema-diff + image) |

### ROADMAP Success Criterion → Test Map

**Success Criterion 1 — "operator can run `cronduit run ...` against fresh SQLite or Postgres URL, process loads config, runs migrations, upserts jobs, emits structured JSON startup summary, exits cleanly"**

| Sub-behavior | Req | Test type | Command | Green signal | Red signal |
|-------|-----|-----------|---------|--------------|------------|
| Config parses from TOML | CONF-01,03,04,05,06,07,08,10 | integration | `cargo test --test config_parser` | exit 0; `ParsedConfig` matches expected struct | any `ConfigError` on valid fixture |
| `${ENV_VAR}` interpolation | CONF-02 | unit | `cargo test --lib interpolate::` | all asserts pass | panic in assertion |
| SQLite pool opens with WAL+busy_timeout | DB-01,05 | integration | `cargo test --test db_pool_sqlite` | `PRAGMA journal_mode='wal'`, `PRAGMA busy_timeout=5000`, writer pool size=1 | any pragma returns default |
| Postgres pool opens | DB-02 | integration (testcontainers) | `cargo test --test db_pool_postgres` | pool responds to `SELECT 1` | connect error |
| Migrations idempotent on both backends | DB-03 | integration | `cargo test --test migrations_idempotent` | second `migrate()` call returns Ok, no row count change | migrator errors / altered rows |
| Schema contains `jobs`,`job_runs`,`job_logs` tables with `config_hash` col | DB-04,06 | integration | `cargo test --test schema_parity` | test passes | diff printed, panic |
| Startup event emitted with all D-23 fields | FOUND-04 | integration (black-box) | `cargo test --test startup_event` | stdout contains `cronduit.startup` JSON line w/ version/bind/db/backend/cfg/tz/counts/warning | missing field → assertion fail |
| Process exits 0 under graceful shutdown | FOUND-01 | integration | `cargo test --test graceful_shutdown` | SIGTERM → exit 0 within 500ms | timeout or non-zero exit |

- **Per task commit:** `cargo nextest run --profile ci --test config_parser --test db_pool_sqlite --test startup_event`
- **Per wave merge:** `just nextest` (full suite)
- **Runs:** every PR

**Success Criterion 2 — "`cronduit check config.toml` validates parse + cron + network + env, exits non-zero with line-numbered errors, no DB"**

| Sub-behavior | Req | Test type | Command | Green signal | Red signal |
|-------|-----|-----------|---------|--------------|------------|
| `check` valid config | FOUND-03 | integration | `cargo test --test check_command check_valid` | exit 0, stderr empty or "ok:" | non-zero or error on stderr |
| `check` invalid config reports `path:line:col: error:` | FOUND-03, D-21 | integration | `cargo test --test check_command check_invalid_format` | stderr matches regex `^[^:]+:\d+:\d+: error: ` | no line/col in output |
| `check` collects ALL errors | D-21 | integration | `cargo test --test check_command check_collects_all` | multiple errors printed in one run | only first error printed |
| `check` never opens the DB | FOUND-03 | integration | `cargo test --test check_command check_no_db_io` | `strace` / `lsof` not needed — test uses a directory without write permissions and asserts no IO error surfaces | write attempted |
| Duplicate job names caught with both line numbers | CONF-10 | integration | `cargo test --test config_parser dup_names` | two line numbers in error message | only one or none |
| Missing env var error lists field path | CONF-02, D-22 | integration | `cargo test --test check_command missing_env_field_path` | error contains field path (`[[jobs]]/check-ip/env/API_KEY`) | error omits path |

- **Per task commit:** `cargo nextest run --test check_command`
- **Per wave merge:** `just nextest`
- **Runs:** every PR

**Success Criterion 3 — "non-loopback `[server].bind` produces loud WARN; default is `127.0.0.1:8080`; `SecretString` fields render `[redacted]`"**

| Sub-behavior | Req | Test type | Command | Green signal | Red signal |
|-------|-----|-----------|---------|--------------|------------|
| Default bind is `127.0.0.1:8080` | OPS-03 | unit | `cargo test --lib server_config::default` | struct default matches | any other value |
| `bind = "0.0.0.0:8080"` → `bind_warning=true` log line | OPS-03, D-24 | integration | `cargo test --test startup_event non_loopback_warn` | stdout contains `WARN` target `cronduit.startup`, `bind_warning:true` | no warn line or wrong level |
| `bind = "192.168.1.10:8080"` (RFC1918) also triggers warn | D-24 | unit | `cargo test --lib is_loopback rfc1918` | `is_loopback(...) == false` | true |
| `Debug` on `SecretString` prints `[REDACTED]` not value | FOUND-05 | unit | `cargo test --lib secret_redaction` | debug output contains "REDACTED" and NOT the plaintext | plaintext appears |
| `cronduit check` on config with secret does not echo value | FOUND-05, Pitfall 18 | integration | `cargo test --test check_command secret_not_leaked` | stdout+stderr don't contain the secret env var value | value leaks |

- **Per task commit:** `cargo nextest run --test startup_event --lib`
- **Per wave merge:** `just nextest`
- **Runs:** every PR

**Success Criterion 4 — "every PR runs CI matrix `{amd64,arm64}×{sqlite,postgres}` with fmt-check, clippy, test, openssl-sys empty, multi-arch image via cargo-zigbuild"**

| Sub-behavior | Req | Test type | Command | Green signal | Red signal |
|-------|-----|-----------|---------|--------------|------------|
| `cargo fmt --check` passes | FOUND-07 | CI step | `just fmt-check` | exit 0, no output | diff printed, exit 1 |
| `cargo clippy -D warnings` passes | FOUND-07 | CI step | `just clippy` | exit 0 | any warning → error |
| `cargo tree -i openssl-sys` empty | FOUND-06, Pitfall 14 | CI step | `just openssl-check` | "OK: no openssl-sys in dependency tree" | "FAIL: openssl-sys found" |
| `cargo test` passes all 4 matrix cells | FOUND-07, FOUND-08 | CI step | `just nextest` | 4 cells green | any cell red |
| `schema_parity` test passes in every cell | Pitfall 8 | CI step | `just schema-diff` | test passes | diff printed, exit 1 |
| Multi-arch image builds without QEMU | FOUND-09, Pitfall 14 | CI step | `just image` (PR) / `just image-push` (main) | buildx completes both arch layers; no `qemu-*` in `docker buildx inspect` | buildx error or opacity |
| Local `just ci` predicts CI outcome | FOUND-12 | manual | `just ci` | exit code matches CI job | drift |

- **Per task commit:** subset based on touched files (`just fmt-check + clippy`)
- **Per wave merge:** `just ci`
- **Phase gate:** **all cells green on the PR that closes Phase 1**

**Success Criterion 5 — "README leads with SECURITY; THREAT_MODEL.md exists; every diagram is mermaid"**

| Sub-behavior | Req | Test type | Command | Green signal | Red signal |
|-------|-----|-----------|---------|--------------|------------|
| README has `## Security` within first 50 lines | FOUND-10 | CI step | `head -50 README.md | grep -q '^## Security'` | exit 0 | no match |
| `THREAT_MODEL.md` exists and is non-empty | FOUND-10 | CI step | `test -s THREAT_MODEL.md` | exit 0 | missing or empty |
| No ASCII-art box-drawing in any .md file | FOUND-11 | CI step | `! rg -l '[─│┌┐└┘├┤┬┴┼]' --type md .` | no matches → exit 0 | match found → exit 1 |

- **Per task commit:** manual
- **Per wave merge:** `just ci` (planner wires these into a `just check-docs` recipe layered into `ci`)
- **Runs:** every PR

### Wave 0 Gaps (files that don't exist yet; must be created before tasks run)

- [ ] `Cargo.toml` — create with §1 contents
- [ ] `rust-toolchain.toml` — create
- [ ] `.config/nextest.toml` — minimal config:
  ```toml
  [profile.ci]
  fail-fast = false
  retries = { backoff = "exponential", count = 2, delay = "1s", jitter = true, max-delay = "10s" }
  ```
- [ ] `tests/schema_parity.rs` — create from §5 skeleton
- [ ] `tests/check_command.rs` — create from §14
- [ ] `tests/startup_event.rs` — create from §14
- [ ] `tests/config_parser.rs` — per-fixture happy-path tests
- [ ] `tests/db_pool_sqlite.rs` — pragma assertions
- [ ] `tests/db_pool_postgres.rs` — testcontainers smoke
- [ ] `tests/migrations_idempotent.rs` — re-run `migrate()` twice
- [ ] `tests/graceful_shutdown.rs` — SIGTERM → clean exit
- [ ] `tests/fixtures/*.toml` — eight fixture files per §14

Framework install (none needed — `cargo test` and `cargo nextest` come with the toolchain plus `taiki-e/install-action` on CI).

---

## Project Constraints (from CLAUDE.md)

Directives that the plan MUST honor. Source: `./CLAUDE.md` `## Project` + `## Technology Stack` sections.

### Tech stack (locked — DO NOT deviate)
- Rust backend with `bollard` for Docker API (Phase 4 scope; not Phase 1)
- `sqlx` with SQLite default + Postgres optional, same logical schema, split migrations, separate SQLite read/write pools + WAL + busy_timeout
- Frontend: Tailwind CSS + server-rendered HTML via `askama_web` 0.15 with `axum-0.8` feature (NOT `askama_axum`) — Phase 1 scaffolds the Tailwind pipeline only
- TOML config format (locked — `serde-yaml` archived)
- `croner` 3.0 for cron parsing (Phase 2)
- rustls everywhere; `cargo tree -i openssl-sys` must be empty; `cargo-zigbuild` (not QEMU) for multi-arch
- Single binary + Docker image; Cronduit runs inside Docker mounting host `/var/run/docker.sock`

### Security posture
- No plaintext secrets in config; interpolate from env, wrap in `SecretString`
- Config mounted read-only
- **Default bind `127.0.0.1:8080`**, loud WARN on non-loopback
- Web UI unauthenticated in v1 — documented in README SECURITY + `THREAT_MODEL.md`
- README leads with SECURITY

### Quality bar
- Tests + GitHub Actions CI from Phase 1
- Clippy + fmt gate on CI
- CI matrix: `linux/amd64 + linux/arm64 × SQLite + Postgres`
- README sufficient for a stranger to self-host

### Documentation
- **All diagrams in any project artifact MUST be mermaid code blocks.** No ASCII art. Enforced by `just check-docs` (planner wires this).

### Workflow
- **All changes land via PR on a feature branch.** No direct commits to `main`. Enforced socially + by user-memory rule.

### Source-of-truth priority
- CLAUDE.md `## Technology Stack` section is authoritative for every version pin. STACK.md and this research agree with CLAUDE.md; if a drift appears, CLAUDE.md wins.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `docker buildx` 0.12+ with `--load` supports multi-platform manifest loading via containerd image store | §11 | Low — fallback is two single-platform `--load` calls; worst case is `just image` needs a tiny rework |
| A2 | `sqlx::migrate!(PATH)` successfully runs on a writer pool with `max_connections=1` without deadlocking (writer holds pool during migration) | §3 | Low — fallback is to migrate on a transient one-shot connection before handing control to the writer pool; refactor is ~10 lines |
| A3 | `secrecy 0.10.3` `features = ["serde"]` transparently provides `Deserialize` for `SecretString` inside a `BTreeMap<String, SecretString>` field without extra adapters | §5 | Low-medium — if broken, the adapter is a custom `Visitor` wrapping `String` in `SecretString`; ~15 lines |
| A4 | `testcontainers-modules::postgres::Postgres::default()` starts cleanly on GitHub Actions `ubuntu-latest` runners without additional Docker configuration | §5, §14 | Low — this is a standard pattern; PITFALLS.md flags testcontainers as HIGH confidence |
| A5 | `toml::de::Error::span()` returns byte offsets that are stable after env interpolation IF we pass the interpolated string back into `toml::from_str` (not the raw source) | §5 | Medium — if offsets drift, the line-col mapping needs the `(original_range, substituted_range)` rewrite table described in §5 |
| A6 | `distroless/static-debian12:nonroot` is a correct base for a `musl`-static Rust binary (no glibc required at runtime) | §11 | Low — `distroless/static` is explicitly designed for this case |
| A7 | `extractions/setup-just@v2` is the current idiomatic action; alternative is `taiki-e/install-action` | §10 | Low — both work; switching is a one-line diff |
| A8 | Phase 1 does not need to import `rand` / `uuid` / `shell-words` / `croner` — the cron schedule field can be accepted as a string and deferred to Phase 2 for real parsing | §1 | Low — Phase 2 adds these crates as a block; not a rework |
| A9 | `humantime_serde::option` is the correct path for optional `Duration` fields in Phase 1 Cargo | §5 | Low — verified by crate README; if the path is named differently, it's a one-line fix |

---

## Open Questions for the Planner

1. **`just setup` onboarding recipe** — should Phase 1 ship a `just setup` that installs `cargo-nextest`, `cargo-zigbuild`, `sqlx-cli`, and the Tailwind binary in one shot? Current §9 draft omits it. Recommend **yes** — it removes the first five "why does X fail?" questions for new contributors.
2. **`.sqlx/` commit policy** — STRONG RECOMMEND committing it. CI needs `SQLX_OFFLINE=true` to avoid requiring a live database for macro checks. Planner should explicitly document the decision in the first plan's `docs/decisions/` entry (new directory) or reject and document the alternative.
3. **Partial index on `job_runs(status) WHERE status='running'`** — ARCHITECTURE.md mentions it but Phase 1 has no scheduler using it. Defer to Phase 2 first PR? (Recommendation: defer — keeps parity test trivial.)
4. **`cargo-watch` in `just dev`** — add now or defer? Phase 1 has nothing to live-reload (no templates yet, no scheduler). Recommend defer.
5. **`cronduit check` secondary validations** — Phase 1 should validate IANA timezone, job uniqueness, and one-of `command|script|image` but how strict should cron syntax validation be? Options: (a) accept any non-empty string (defer to Phase 2), (b) run a regex for 5-field basic cron, (c) pull in `croner` now just for validation. Recommend (a): simplest; error messages will improve in Phase 2 when croner is imported.
6. **Docker network mode validation in `check`** — regex-based (`^(bridge|host|none|container:[a-zA-Z0-9_.-]+|[a-zA-Z0-9_.-]+)$`) or defer to Phase 4? Recommend **regex-validate now** so CONF-05 is fully testable in Phase 1 without Docker.
7. **Does `cronduit run` need to refuse to start when `database_url` uses an unsupported scheme, vs. allowing config parse to succeed and failing on pool connect?** Recommendation: fail at parse time (in `parse_and_validate`) with a clear `ConfigError` — it's better UX and aligns with the "collect-all-errors" principle.
8. **Single `cronduit` binary or split `cronduit` + `cronduit-check`?** D-05 says single crate, D-01 says subcommands — so single binary with subcommands is correct. Plan should make this unambiguous.

---

## Sources

### Primary (HIGH confidence) — verified live on 2026-04-09
- **CLAUDE.md** `## Technology Stack` — authoritative version pins for every crate in §1
- **crates.io `cargo search` output** for `secrecy=0.10.3`, `sqlx=0.8.6` (0.9 is alpha), `cargo-zigbuild=0.22.1`, `chrono-tz=0.10.4`, `iana-time-zone=0.1.65`
- **docs.rs/secrecy/0.10.3** — confirmed `serde` feature, transparent Deserialize for `SecretBox<T>`, Debug redaction
- **docs.rs/sqlx/0.8.6** — confirmed `migrate!` macro compile-time path semantics, feature list (`runtime-tokio`, `tls-rustls`, `sqlite`, `postgres`, `migrate`, `macros`)
- **docs.rs/toml/1.1.2** — confirmed `toml::de::Error::span() -> Option<Range<usize>>`
- **docs.rs/askama_web/0.15.2** — confirmed `axum-0.8` feature flag and `WebTemplate` derive pattern
- **github.com/rust-cross/cargo-zigbuild** — confirmed 0.22.1 current, `cargo zigbuild --target <triple>` syntax
- **`.planning/research/ARCHITECTURE.md`** — `DbPool` enum pattern, schema, startup boot flow, AppState
- **`.planning/research/PITFALLS.md`** §§ 1, 7, 8, 14, 15, 18, 20 — constraint derivation
- **`.planning/research/STACK.md`** — full version-locked table that CLAUDE.md copies
- **`.planning/phases/01-*/01-CONTEXT.md`** — D-01 through D-24 authority
- **`docs/SPEC.md`** — example TOML config, Configuration/Database/Operational sections

### Secondary (MEDIUM confidence)
- **PITFALLS.md §8** schema parity prescription — extrapolated into the specific parity-test implementation in §5
- **`testcontainers-modules` 0.15** API shape — verified against STACK.md pin; actual API call signatures cross-checked against crate README

### Tertiary (LOW confidence — flagged for validation during first plan execution)
- **A1 (buildx multi-platform `--load`)** — works on modern buildx but planner should sanity-check on the actual GHA runner version during first CI run
- **A2 (sqlx migrate on single-connection pool)** — no known issue, but re-verify during Phase 1 Wave 0 smoke test

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every pin verified against crates.io live
- Architecture (DbPool, migrations, parity test): HIGH — spec-derived from ARCHITECTURE.md + D-13/D-14
- Pitfalls: HIGH — directly from PITFALLS.md with concrete guards
- Secrecy + serde integration: HIGH — verified via docs.rs 2026-04-09
- `cronduit check` error format (hand-rolled): HIGH — format fully specified by D-21
- CI YAML shape: HIGH — standard 2025/26 pattern aligned with STACK.md + D-17/D-18
- Multi-arch Dockerfile / zigbuild: MEDIUM-HIGH — verified technique but first Phase 1 CI run may reveal runner-version quirks

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (30 days — stable stack, no fast-moving components)
