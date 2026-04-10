# Security Verification Report — Phase 01

**Phase:** 01 — Foundation: Security Posture, Persistence, Base
**Verified:** 2026-04-10
**ASVS Level:** 1
**Threats Closed:** 19/19
**Threats Open:** 0/19

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-01-01 | Info Disclosure / Elevation | mitigate | CLOSED | `src/cli/run.rs:70-78` — `is_loopback()` check; `tracing::warn!` on non-loopback bind. `src/config/mod.rs:42` — `default_bind()` returns `"127.0.0.1:8080"`. `README.md:16` — SECURITY section leads with unauthenticated UI warning. `THREAT_MODEL.md:46-48` — T-S1 documents the disposition. `tests/startup_event.rs:84-96` — `startup_emits_warn_on_non_loopback_bind` asserts `bind_warning:true` and WARN-level event on `0.0.0.0`. |
| T-01-02 (Plan 02) | Info Disclosure | mitigate | CLOSED | `src/config/mod.rs:11,33,73` — `secrecy::SecretString` wraps `database_url` and every `env` map value. `secrecy` crate `Debug` impl renders `[REDACTED]` by design (upstream guarantee). No `Serialize` derive on `SecretString` (upstream guarantee). `tests/config_parser.rs:47-49` — `valid_with_secrets_parses_when_env_is_set` asserts `format!("{:?}", config)` does not contain the interpolated secret value `"hunter2"`. |
| T-01-02 (Plan 03) | Info Disclosure | mitigate | CLOSED | `src/config/mod.rs:116` — `ConfigError.message` for `MissingVar` renders only the variable NAME (`missing environment variable \`${...}\``), not its value. `src/cli/check.rs:17` — errors printed via `Display` which delegates to `ConfigError::fmt` (file:line:col + message only). `tests/check_command.rs:112-136` — `check_does_not_leak_secret_value` sets `CRONDUIT_TEST_API_KEY=SUPER_SECRET_PASSWORD_MARKER_99XYZ` and asserts the value never appears in stdout or stderr. |
| T-01-02 (Plan 04) | Info Disclosure | mitigate | CLOSED | `src/db/mod.rs:117-125` — `strip_db_credentials` strips username and password from the URL using `url::Url` before logging. Called at `src/cli/run.rs:86` in the `cronduit.startup` event. `src/db/mod.rs:132-140` — unit tests assert `user` and `pass` are absent from the stripped output. `tests/startup_event.rs:99-108` — `startup_does_not_leak_database_credentials` asserts the startup event contains `"database_url":"sqlite:` but never `user:pass`. |
| T-01-03 | Tampering | mitigate | CLOSED | `justfile:106-118` — `openssl-check` recipe loops over native, `aarch64-unknown-linux-musl`, and `x86_64-unknown-linux-musl` targets. Fails CI if `cargo tree -i openssl-sys` produces any output. `Cargo.toml:30-38` — `sqlx` configured with `tls-rustls` only; no `tls-native-tls` or `openssl` features. `.github/workflows/ci.yml:37` — `just openssl-check` runs in every `lint` job. |
| T-01-04 | Tampering | mitigate | CLOSED | `src/config/interpolate.rs:25-29` — `DEFAULT_RE` detects `${VAR:-default}` syntax and pushes `ErrorKind::DefaultSyntaxForbidden`. `src/config/interpolate.rs:36-41` — missing env var pushes `ErrorKind::MissingVar` with the variable name. Neither path silently returns empty string without recording an error. `src/config/interpolate.rs:88-91` — `default_syntax_rejected` unit test. |
| T-01-05 (Plan 02) | DoS | mitigate | CLOSED | `src/config/mod.rs:90,105-125` — `parse_and_validate` initialises `errors: Vec<ConfigError>` and pushes all interpolation errors into it before attempting TOML parse. `src/config/validate.rs:16-29` — `run_all_checks` pushes into `errors` without early return. `tests/config_parser.rs:108-117` — `collects_all_errors_not_fail_fast` asserts `>= 2` errors from a fixture with both a bad timezone and a duplicate name. `tests/check_command.rs:41-63` — `check_collects_all_errors` asserts `>= 2` `error:` lines from the binary. |
| T-01-05 (Plan 03) | Elevation | mitigate | CLOSED | `src/cli/check.rs:10` — `check::execute` calls only `config::parse_and_validate`; no DB import, no `DbPool::connect`, no `pool.migrate()`. `tests/check_command.rs:77-98` — `check_does_not_open_db` runs `cronduit check` in a temp directory and asserts no `*.db*` files are created. |
| T-01-06 (Plan 06) | Tampering | mitigate | CLOSED | `justfile:18` — `ci: fmt-check clippy openssl-check nextest schema-diff image` defines the ordered chain. `justfile:8` — `set shell := ["bash", "-euo", "pipefail", "-c"]` ensures strict error propagation. Local `just ci` and CI both execute the same chain. |
| T-01-06 (Plan 07) | Tampering | mitigate | CLOSED | `.github/workflows/ci.yml:1-4` comment declares "Every `run:` step invokes `just <recipe>` exclusively (D-10 / FOUND-12). No inline `cargo` / `docker` / `rustup` / `sqlx` / `npm` / `npx` commands." Inspection of all `run:` steps confirms only `just fmt-check`, `just clippy`, `just openssl-check`, `just install-targets`, `just nextest`, `just schema-diff`, `just image`, `just image-push` are used — no raw tool invocations. |
| T-01-07 | Tampering / Elevation | mitigate | CLOSED | `Cargo.toml:30-38` — `sqlx` features list contains `tls-rustls` and explicitly omits `tls-native-tls`. No `openssl`, `native-tls`, or `openssl-sys` in `[dependencies]`. `justfile:106-118` — `openssl-check` enforces the invariant at CI-time for all three targets. |
| T-01-08 (Plan 01) | DoS | mitigate | CLOSED | `src/shutdown.rs:4-26` — `install()` spawns a task that awaits SIGINT (`signal::ctrl_c()`) or SIGTERM (`SignalKind::terminate()`) via `tokio::select!` and then calls `token.cancel()`. `src/cli/run.rs:100-101` — `CancellationToken` created and passed to `shutdown::install`. `tests/graceful_shutdown.rs:29-78` — `sigterm_yields_clean_exit_within_one_second` sends `SIGTERM` and asserts exit 0 within 2 s. |
| T-01-08 (Plan 04) | DoS | mitigate | CLOSED | `src/cli/run.rs:103-106` — `web::serve` is wired with `.with_graceful_shutdown(async move { shutdown.cancelled().await })`. `pool.close().await` is called after `serve_result` returns, draining both SQLite pools before process exit. `src/db/mod.rs:103-111` — `DbPool::close()` calls `write.close().await` and `read.close().await` for SQLite. |
| T-01-09 | Info Disclosure | accept | CLOSED | Accepted: `src/telemetry.rs` configures JSON tracing output. No secrets flow through Phase 01 code paths — `SecretString` values are never field-logged, `database_url` is stripped before the startup event, and config validation errors carry only variable names. Acceptance documented in THREAT_MODEL.md T-I2 (mitigation) and T-I1 (mitigation). The specific accept rationale: tracing JSON format itself is not a secret-disclosure vector in Phase 01 because no code path passes a `SecretString` or raw credentials to a `tracing::` macro. |
| T-01-09-01 | DoS | accept | CLOSED | Accepted: `croner` cron expression parsing runs on short user-supplied strings (schedule field), executed once per job at config load time, not in a hot path. Input is bounded by the TOML config file size. No unbounded parsing loop. No network-originated input. |
| T-01-09-02 | Info Disclosure | mitigate | CLOSED | `src/config/validate.rs:87-97` — `check_schedule` error message includes `job.name` and the raw schedule string but not any secret. Schedule strings are not sensitive (they are cron expressions, not credentials). Errors are emitted to stderr by the `check` subcommand only, never to a network endpoint. |
| T-01-10 | Tampering (docs) | mitigate | CLOSED | `README.md:66-83` — architecture diagram is a mermaid `flowchart TD` block. `THREAT_MODEL.md:12-25` — trust-boundary diagram is a mermaid `flowchart LR` block. No ASCII box-drawing characters found in either file. CLAUDE.md and user memory enforce mermaid-only diagrams for all project artifacts. |
| T-01-11 | DoS | mitigate | CLOSED | `src/db/mod.rs:57-77` — SQLite writer pool uses `SqlitePoolOptions::new().max_connections(1)` plus `.journal_mode(SqliteJournalMode::Wal)`, `.busy_timeout(Duration::from_millis(5000))`, and `.synchronous(SqliteSynchronous::Normal)`. Reader pool uses `max_connections(8)`. Comment references "Pitfall 7: single writer". |
| T-01-12 | Tampering | mitigate | CLOSED | `tests/schema_parity.rs:220-263` — `sqlite_and_postgres_schemas_match_structurally` spins up in-memory SQLite and a `testcontainers` Postgres, runs both migration sets, introspects tables and indexes, and panics on any structural drift. `.github/workflows/ci.yml:68` — `just schema-diff` runs this test in every matrix cell. |
| T-01-13 | Supply-chain | mitigate | CLOSED | `.github/workflows/ci.yml:19-20` — top-level `permissions: contents: read` (read-only default). `.github/workflows/ci.yml:74-77` — `image` job overrides to `permissions: packages: write` scoped to that job only. PR path uses `--load` (no push). Comments reference `T-01-13` explicitly. |
| T-01-14 | Elevation | mitigate | CLOSED | `Dockerfile:43` — runtime stage is `FROM gcr.io/distroless/static-debian12:nonroot`. `Dockerfile:50` — `USER nonroot:nonroot` is set before `ENTRYPOINT`. No shell, no package manager, no writable filesystem in the distroless image. |
| T-01-15 | Tampering | mitigate | CLOSED | `Dockerfile:7,24,26,32-40` — builder uses `cargo-zigbuild` with `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl` and a `TARGETPLATFORM`-driven target selection. `justfile:106-118` — `openssl-check` validates both musl cross targets alongside native. `.github/workflows/ci.yml:62-66` — `just install-targets` runs for `arm64` matrix cells. |

---

## Accepted Risks Log

| Threat ID | Rationale |
|-----------|-----------|
| T-01-09 | JSON tracing format accepted: Phase 01 has no code path that passes a `SecretString` or stripped credentials to any `tracing::` call. `database_url` is stripped by `strip_db_credentials` before logging. Acceptance is bounded to Phase 01 scope; Phase 4 executor introduction re-opens this for review. |
| T-01-09-01 | `croner` parser DoS accepted: input is operator-controlled, bounded to config file contents, and runs once at startup. Not reachable from the network in Phase 01. |

---

## Unregistered Flags

None. No `## Threat Flags` sections found in any Plan executor SUMMARY file for Phase 01.

---

## Files Verified

- `/Users/Robert/Code/public/cronduit/src/config/mod.rs`
- `/Users/Robert/Code/public/cronduit/src/config/interpolate.rs`
- `/Users/Robert/Code/public/cronduit/src/config/validate.rs`
- `/Users/Robert/Code/public/cronduit/src/config/errors.rs`
- `/Users/Robert/Code/public/cronduit/src/config/hash.rs`
- `/Users/Robert/Code/public/cronduit/src/cli/check.rs`
- `/Users/Robert/Code/public/cronduit/src/cli/run.rs`
- `/Users/Robert/Code/public/cronduit/src/cli/mod.rs`
- `/Users/Robert/Code/public/cronduit/src/shutdown.rs`
- `/Users/Robert/Code/public/cronduit/src/telemetry.rs`
- `/Users/Robert/Code/public/cronduit/src/db/mod.rs`
- `/Users/Robert/Code/public/cronduit/src/main.rs`
- `/Users/Robert/Code/public/cronduit/src/web/mod.rs`
- `/Users/Robert/Code/public/cronduit/Cargo.toml`
- `/Users/Robert/Code/public/cronduit/Dockerfile`
- `/Users/Robert/Code/public/cronduit/.github/workflows/ci.yml`
- `/Users/Robert/Code/public/cronduit/justfile`
- `/Users/Robert/Code/public/cronduit/README.md`
- `/Users/Robert/Code/public/cronduit/THREAT_MODEL.md`
- `/Users/Robert/Code/public/cronduit/tests/check_command.rs`
- `/Users/Robert/Code/public/cronduit/tests/config_parser.rs`
- `/Users/Robert/Code/public/cronduit/tests/graceful_shutdown.rs`
- `/Users/Robert/Code/public/cronduit/tests/startup_event.rs`
- `/Users/Robert/Code/public/cronduit/tests/schema_parity.rs`
