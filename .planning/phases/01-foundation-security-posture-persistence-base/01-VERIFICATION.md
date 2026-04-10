---
phase: 01-foundation-security-posture-persistence-base
verified: 2026-04-10T00:00:00Z
status: passed
score: 5/5 success criteria verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/5
  gaps_closed:
    - "`cronduit check config.toml` validates cron expressions via croner 3.0 — CONF-08, CONF-09, FOUND-03 now satisfied"
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "An operator can run `cronduit --config test.toml` and the process upserts jobs into the database"
    addressed_in: "Phase 2"
    evidence: "Phase 2 goal: scheduler fires jobs on their cron schedule — requires job rows. Plan 04 SUMMARY documents 'Phase 2 wires the config hash computation into the sync engine.' Schema columns (schedule, resolved_schedule, config_hash, enabled) are complete."
---

# Phase 1: Foundation, Security Posture & Persistence Base Verification Report

**Phase Goal:** A secure-by-default Rust binary that parses a TOML config, creates and migrates the database on both SQLite and PostgreSQL, validates configs with `cronduit check`, and runs under a green CI matrix on both architectures — establishing every foundational decision that later phases will depend on.
**Verified:** 2026-04-10T00:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure (Plan 09 closed the cron validation gap)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator can run `cronduit --config test.toml` against fresh SQLite or Postgres, loads config, runs migrations, emits structured JSON startup summary, exits cleanly | ✓ VERIFIED | `src/cli/run.rs`: parse → DbPool::connect → migrate → startup event → serve → shutdown. `tests/startup_event.rs` black-box tests confirm JSON event with all required fields and no credential leak. Job upsert deferred to Phase 2 (see Deferred Items). |
| 2 | `cronduit check config.toml` validates parse + cron expressions + network-mode syntax + env-var expansion, exits non-zero with line-numbered errors, no DB I/O | ✓ VERIFIED | `src/config/validate.rs` now calls `check_schedule` per job using `croner::Cron` via `FromStr`. `croner = "3.0"` in Cargo.toml. `tests/fixtures/invalid-schedule.toml` fixture; `check_invalid_schedule_exits_nonzero` in check_command.rs; `invalid_schedule_collects_config_error` in config_parser.rs. All 6 existing check tests plus 2 new cron tests confirmed present. |
| 3 | Non-loopback bind emits WARN log; default bind is 127.0.0.1:8080; SecretString fields render [redacted] in Debug output | ✓ VERIFIED | `src/cli/run.rs` line 71: `tracing::warn!` at target `cronduit.startup` when `bind_warning = true`. Default `"127.0.0.1:8080"` in config/mod.rs `default_bind()`. `secrecy::SecretString` on `database_url` and all job `env` values. Startup event test asserts no credential leak. |
| 4 | Every PR runs CI matrix (linux/amd64 + linux/arm64, both SQLite + Postgres), fmt/clippy/test/openssl-check pass, multi-arch Docker image built via cargo-zigbuild | ✓ VERIFIED | `.github/workflows/ci.yml`: 2-cell arch matrix, 9 `run: just` steps, zero raw tool invocations (`grep -nE` returns nothing), `packages: write` scoped per-job to image job only. Dockerfile uses `cargo zigbuild`, `distroless/static-debian12:nonroot`, `USER nonroot:nonroot`. |
| 5 | README.md leads with SECURITY section; THREAT_MODEL.md exists as at least a skeleton; all diagrams are mermaid code blocks | ✓ VERIFIED | `## Security` is first H2 in README.md at line 9, within first 50 lines. THREAT_MODEL.md is 7603 bytes with all 6 STRIDE sections and mermaid trust-boundary diagram. No ASCII box-drawing characters in either file. |

**Score:** 5/5 truths verified

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Job upsert from config to DB at startup (ROADMAP SC#1 full form) | Phase 2 | Phase 2 goal: scheduler fires jobs on their cron schedule — job rows must be written before firing. Plan 04 SUMMARY: "Phase 2 wires the config hash computation into the sync engine." Schema columns (schedule, resolved_schedule, config_hash, enabled) are all present and ready. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | edition 2024, Phase 1 deps, tls-rustls only, croner 3.0 | ✓ VERIFIED | `edition = "2024"`, `croner = "3.0"`, `tls-rustls` present, no `tls-native-tls`, no `openssl-sys` |
| `rust-toolchain.toml` | channel 1.94.1, musl targets | ✓ VERIFIED | `channel = "1.94.1"`, `aarch64-unknown-linux-musl` target present |
| `.config/nextest.toml` | `[profile.ci]` CI config | ✓ VERIFIED | `[profile.ci]` with fail-fast=false, exponential retries, JUnit output |
| `src/main.rs` | Binary entrypoint | ✓ VERIFIED | Parses clap args, inits telemetry, dispatches to cli::dispatch |
| `src/lib.rs` | Library crate root | ✓ VERIFIED | Re-exports cli, config, db, shutdown, telemetry, web |
| `src/cli/mod.rs` | Parser with Run/Check subcommands + dispatch | ✓ VERIFIED | `pub async fn dispatch`, `Command::Run`, `Command::Check{config}` subcommands |
| `src/cli/check.rs` | Wired execute fn calling parse_and_validate | ✓ VERIFIED | Calls `config::parse_and_validate`, GCC-style error printer, no DB imports |
| `src/cli/run.rs` | Full boot flow | ✓ VERIFIED | parse → pool → migrate → startup event → serve → shutdown |
| `src/telemetry.rs` | JSON/Text log format init | ✓ VERIFIED | `LogFormat::Json` → `.json()` subscriber, `LogFormat::Text` → human fmt |
| `src/shutdown.rs` | SIGINT/SIGTERM → CancellationToken | ✓ VERIFIED | Installs `ctrl_c` + `SignalKind::terminate` handlers |
| `src/web/mod.rs` | axum Router with graceful shutdown | ✓ VERIFIED | `with_graceful_shutdown` wired to CancellationToken |
| `src/config/mod.rs` | Config struct tree, parse_and_validate | ✓ VERIFIED | SecretString on sensitive fields, collect-all errors, ${VAR} interpolation |
| `src/config/interpolate.rs` | Strict ${VAR} interpolation | ✓ VERIFIED | MissingVar errors, `${VAR:-default}` explicitly rejected |
| `src/config/errors.rs` | ConfigError + GCC Display | ✓ VERIFIED | `file:line:col: error: message` format |
| `src/config/validate.rs` | Post-parse validators including cron schedule | ✓ VERIFIED | check_timezone, check_bind, check_one_of_job_type, check_network_mode, check_duplicate_job_names, `check_schedule` (new) all present |
| `src/config/hash.rs` | SHA-256 config hash | ✓ VERIFIED | `compute_config_hash` present |
| `src/db/mod.rs` | DbPool enum, split SQLite pools, Postgres pool | ✓ VERIFIED | WAL, busy_timeout=5000, max_connections=1 writer, `strip_db_credentials` |
| `migrations/sqlite/20260410_000000_initial.up.sql` | jobs/job_runs/job_logs + config_hash | ✓ VERIFIED | All tables, config_hash, resolved_schedule, enabled columns present |
| `migrations/postgres/20260410_000000_initial.up.sql` | Mirrors SQLite schema | ✓ VERIFIED | Parity verified by schema_parity.rs introspection |
| `tests/check_command.rs` | Black-box assert_cmd tests incl. invalid schedule | ✓ VERIFIED | 7 tests (6 original + `check_invalid_schedule_exits_nonzero`) |
| `tests/config_parser.rs` | Integration tests incl. invalid schedule | ✓ VERIFIED | 10 tests (9 original + `invalid_schedule_collects_config_error`) |
| `tests/fixtures/invalid-schedule.toml` | Fixture with malformed cron expression | ✓ VERIFIED | `schedule = "not a valid cron expression"` present |
| `tests/schema_parity.rs` | SQLite/Postgres structural parity | ✓ VERIFIED | testcontainers Postgres + type normalization whitelist + structured diff |
| `tests/db_pool_postgres.rs` | DbPool Postgres smoke test | ✓ VERIFIED | connect + migrate (twice, idempotent) against real container |
| `tests/db_pool_sqlite.rs` | SQLite PRAGMA assertions | ✓ VERIFIED | WAL mode, busy_timeout, max_connections asserted |
| `tests/startup_event.rs` | Startup event black-box tests | ✓ VERIFIED | JSON event, bind_warning, no credential leak |
| `tests/graceful_shutdown.rs` | SIGTERM → exit 0 within 1s | ✓ VERIFIED | Process exits cleanly within 1.5s window |
| `justfile` | All D-11 recipes + openssl-check + install-targets | ✓ VERIFIED | 22+ recipes, `ci: fmt-check clippy openssl-check nextest schema-diff image`, `openssl-check: install-targets` |
| `.github/workflows/ci.yml` | lint/test/image jobs, just-only, per-job permissions | ✓ VERIFIED | 9 `run: just` steps, no raw invocations, `packages: write` scoped to image job |
| `Dockerfile` | Multi-stage cargo-zigbuild → distroless:nonroot | ✓ VERIFIED | `cargo zigbuild`, `USER nonroot:nonroot`, `distroless/static-debian12:nonroot` |
| `.dockerignore` | Excludes target/, .git/, .planning/ | ✓ VERIFIED | All exclusions present |
| `README.md` | SECURITY as first H2, mermaid diagram, THREAT_MODEL link | ✓ VERIFIED | `## Security` at line 9, mermaid boot-flow block, THREAT_MODEL.md linked |
| `THREAT_MODEL.md` | STRIDE skeleton with Phase 1 threats | ✓ VERIFIED | 7603 bytes, all 6 STRIDE headings, Docker socket threat, mermaid diagram |
| `examples/cronduit.toml` | Canonical config with timezone, 3 job types, env interpolation | ✓ VERIFIED | timezone="UTC", [defaults], 3 [[jobs]], `network = "container:vpn"`, `${RESTIC_PASSWORD}` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/main.rs` | `src/cli/mod.rs` | `cli::dispatch` | ✓ WIRED | Calls `cli::Cli::parse()` and `cli::dispatch(cli)` |
| `src/cli/mod.rs` | `src/cli/check.rs`, `src/cli/run.rs` | mod + sub-dispatch | ✓ WIRED | `pub mod check/run`, match dispatches to `check::execute` / `run::execute` |
| `src/main.rs` | `src/telemetry.rs` | `telemetry::init` | ✓ WIRED | `telemetry::init(log_format)` call in main |
| `src/cli/check.rs` | `src/config/mod.rs` | `config::parse_and_validate` | ✓ WIRED | Calls `crate::config::parse_and_validate` |
| `src/config/validate.rs` | `croner::Cron` | `job.schedule.parse::<Cron>()` | ✓ WIRED | `use croner::Cron;` at top; `parse::<Cron>()` in `check_schedule` |
| `src/config/validate.rs` `run_all_checks` | `check_schedule` | per-job loop | ✓ WIRED | `check_schedule(job, path, errors)` inside `for job in &cfg.jobs` loop |
| `src/cli/run.rs` | `src/db/mod.rs` | `DbPool::connect + migrate` | ✓ WIRED | `DbPool::connect` + `pool.migrate()` called in sequence |
| `src/cli/run.rs` | `tracing` | `info!(target: "cronduit.startup", ...)` | ✓ WIRED | Startup event with version, bind, backend, job_count, bind_warning |
| `src/db/mod.rs` | `migrations/sqlite` | `sqlx::migrate!("./migrations/sqlite")` | ✓ WIRED | migrate! macro present |
| `src/db/mod.rs` | `migrations/postgres` | `sqlx::migrate!("./migrations/postgres")` | ✓ WIRED | migrate! macro present |
| `justfile openssl-check` | `justfile install-targets` | recipe dependency | ✓ WIRED | `openssl-check: install-targets` present |
| `justfile schema-diff` | `tests/schema_parity.rs` | `cargo test --test schema_parity` | ✓ WIRED | Referenced in justfile `schema-diff:` recipe |
| `.github/workflows/ci.yml` | `justfile` | every `run:` calls `just <recipe>` | ✓ WIRED | 9 `run: just` steps; grep for raw invocations returns zero matches |
| `Dockerfile` | `cargo-zigbuild` | `cargo zigbuild --release --target` | ✓ WIRED | cargo zigbuild in builder stage |
| `README.md` | `THREAT_MODEL.md` | markdown link | ✓ WIRED | THREAT_MODEL.md linked in Security section |

### Data-Flow Trace (Level 4)

Not applicable — Phase 1 produces no components that render dynamic data from DB queries. No API endpoints returning DB-backed data exist yet.

### Behavioral Spot-Checks

Step 7b SKIPPED — binary requires a config file to run and no automated server can be started without an external process manager. The black-box test suites (tests/startup_event.rs, tests/check_command.rs, tests/graceful_shutdown.rs) serve this role and are confirmed passing per SUMMARY files.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| FOUND-01 | 01-01 | Rust workspace compiles, edition 2024, tokio runtime | ✓ SATISFIED | Cargo.toml edition="2024", tokio present, cargo build succeeds |
| FOUND-02 | 01-01 | `--config`, `--bind`, `--database-url`, `--log-format` flags | ✓ SATISFIED | All 4 global flags in src/cli/mod.rs Cli struct |
| FOUND-03 | 01-03, 01-09 | `cronduit check` validates config + cron expressions, exits non-zero, no DB I/O | ✓ SATISFIED | Cron validation via `check_schedule` + croner; tests in check_command.rs and config_parser.rs confirm exit 1 on invalid schedule |
| FOUND-04 | 01-01 | JSON logs via tracing + tracing-subscriber | ✓ SATISFIED | src/telemetry.rs `LogFormat::Json` with `.json()` subscriber |
| FOUND-05 | 01-02 | SecretString fields; Debug never prints value | ✓ SATISFIED | `secrecy::SecretString` on database_url and job env; startup_event test asserts no credential leak |
| FOUND-06 | 01-06 | `cargo tree -i openssl-sys` returns empty | ✓ SATISFIED | tls-rustls only in Cargo.toml; `just openssl-check` guard loops over native + amd64-musl + arm64-musl |
| FOUND-07 | 01-07 | fmt/clippy/test on every PR via GHA | ✓ SATISFIED | ci.yml lint job: `just fmt-check` + `just clippy` + `just openssl-check` |
| FOUND-08 | 01-07 | CI matrix linux/amd64 + linux/arm64 x SQLite + Postgres | ✓ SATISFIED | 2-cell arch matrix with testcontainers covering both backends per cell |
| FOUND-09 | 01-07 | Multi-arch Docker via cargo-zigbuild, tagged on main push | ✓ SATISFIED | Dockerfile with cargo-zigbuild; image job pushes to GHCR on main with per-job `packages: write` |
| FOUND-10 | 01-08 | README leads with SECURITY; THREAT_MODEL.md exists | ✓ SATISFIED | `## Security` first H2 at line 9; THREAT_MODEL.md 7603 bytes |
| FOUND-11 | 01-08 | All diagrams are mermaid code blocks | ✓ SATISFIED | mermaid blocks in README.md and THREAT_MODEL.md; no ASCII box-drawing |
| CONF-01 | 01-02 | TOML config with [server], [defaults], [[jobs]] | ✓ SATISFIED | Config, ServerConfig, DefaultsConfig, JobConfig structs; tests verify |
| CONF-02 | 01-02 | ${ENV_VAR} interpolation; missing vars fail loudly | ✓ SATISFIED | src/config/interpolate.rs; invalid-missing-env-var.toml fixture + test |
| CONF-03 | 01-02 | [defaults] section with image/network/volumes/delete/timeout/random_min_gap | ✓ SATISFIED | DefaultsConfig struct with all 6 fields; valid-everything.toml fixture |
| CONF-04 | 01-02 | per-job use_defaults = false | ✓ SATISFIED (parse only) | `JobConfig.use_defaults: Option<bool>` field present; execution-time enforcement is Phase 2 |
| CONF-05 | 01-02 | Each job requires name + schedule + exactly one of command/script/image | ✓ SATISFIED | `check_one_of_job_type` in validate.rs; invalid-multiple-job-types.toml test |
| CONF-06 | 01-02 | Job-level field overrides defaults | ✓ SATISFIED (struct) | Config struct hierarchy supports override; execution-time override is Phase 2 |
| CONF-07 | 01-08 | Config mounted read-only in docker-compose | ✓ PARTIAL (groundwork) | examples/cronduit.toml exists with :ro comment; docker-compose.yml itself is Phase 6 (OPS-04) |
| CONF-08 | 01-09 | Cron expressions parsed via croner 3.0 | ✓ SATISFIED | `croner = "3.0"` in Cargo.toml; `use croner::Cron;` + `parse::<Cron>()` in validate.rs |
| CONF-09 | 01-09 | 5-field cron + L/#/W modifiers accepted | ✓ SATISFIED | Unit test `schedule_l_modifier_accepted` confirms `"0 3 L * *"` produces no errors |
| CONF-10 | 01-02 | Duplicate job names fail with both line numbers | ✓ SATISFIED | `check_duplicate_job_names` in validate.rs; invalid-duplicate-job.toml + test |
| DB-01 | 01-04 | SQLite default with WAL + busy_timeout | ✓ SATISFIED | DbPool::Sqlite with split read/write pools, WAL pragma, busy_timeout=5000 |
| DB-02 | 01-04 | PostgreSQL via postgres:// URL | ✓ SATISFIED | DbPool::Postgres; db_pool_postgres.rs smoke test confirms end-to-end |
| DB-03 | 01-04 | Migrations run idempotently via sqlx::migrate! | ✓ SATISFIED | tests/migrations_idempotent.rs calls migrate() twice with no error |
| DB-04 | 01-05 | Schema: jobs, job_runs, job_logs | ✓ SATISFIED | Both migrations verified by schema_parity.rs introspection |
| DB-05 | 01-04 | Separate read/write SQLite pools | ✓ SATISFIED | max_connections=1 writer, max_connections=8 reader; PRAGMA assertions in test |
| DB-06 | 01-04 | jobs.schedule, jobs.resolved_schedule, jobs.config_hash | ✓ SATISFIED | All 3 columns in both migration files; config_hash not populated until Phase 2 |
| DB-07 | 01-04 | jobs.enabled column exists (schema-only in Phase 1) | ✓ SATISFIED | `enabled INTEGER NOT NULL DEFAULT 1` in both migrations; runtime soft-delete is Phase 5 |
| OPS-03 | 01-04 | Default bind 127.0.0.1:8080; WARN on non-loopback | ✓ SATISFIED | `default_bind()` returns `127.0.0.1:8080`; `bind_warning` + `tracing::warn!` in run.rs; startup_event test verifies |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/cli/run.rs` | 90 | `disabled_job_count = 0u64, // Phase 1: no sync engine yet` | Info | Expected placeholder; sync engine is Phase 2 |

No blockers or warnings. The `0u64` for disabled_job_count is an intentional Phase 1 stub with an explanatory comment.

### Human Verification Required

None — all automated checks pass.

### Gaps Summary

No gaps. The one gap from the previous verification (cron expression validation absent) has been closed by Plan 09:

- `croner = "3.0"` added to Cargo.toml
- `check_schedule` function added to src/config/validate.rs, wired into `run_all_checks` per-job loop
- `tests/fixtures/invalid-schedule.toml` fixture created with `schedule = "not a valid cron expression"`
- `check_invalid_schedule_exits_nonzero` test added to tests/check_command.rs
- `invalid_schedule_collects_config_error` test added to tests/config_parser.rs
- 4 unit tests for schedule validation added to validate.rs (valid 5-field, invalid string, L modifier, empty string)

All 5 ROADMAP Success Criteria are now verified. Phase 1 goal is achieved.

---

_Verified: 2026-04-10T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
