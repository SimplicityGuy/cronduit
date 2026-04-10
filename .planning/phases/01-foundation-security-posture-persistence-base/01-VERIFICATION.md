---
phase: 01-foundation-security-posture-persistence-base
verified: 2026-04-09T21:00:00Z
status: gaps_found
score: 4/5 success criteria verified
overrides_applied: 0
gaps:
  - truth: "`cronduit check config.toml` validates parse + cron expressions + network-mode syntax + env-var expansion and exits non-zero with line-numbered errors on any failure, without touching the database"
    status: partial
    reason: "Cron expression syntax validation is absent. `schedule` is stored as a raw string with no parsing or validation via croner or any cron library. There is no croner dependency in Cargo.toml, no schedule-validation branch in src/config/validate.rs, and no test fixture for invalid cron expressions. CONF-08 (croner 3.0 cron parsing), CONF-09 (5-field cron acceptance with L/#/W modifiers), and FOUND-03 (cron expression validation) are all unmet. Network-mode validation, env-var expansion, parse, no-DB, and collect-all are fully verified; only cron validation is missing."
    artifacts:
      - path: "src/config/validate.rs"
        issue: "No schedule field validation — validate.rs checks timezone, bind, one-of job type, network mode regex, and duplicate names, but never parses or validates the schedule string"
      - path: "Cargo.toml"
        issue: "croner crate is absent from dependencies; no alternative cron-parsing library present"
    missing:
      - "Add croner = { version = \"3.0\", features = [\"chrono\"] } to Cargo.toml"
      - "Add schedule validation in validate.rs: parse each job.schedule via croner::Cron and push a ConfigError if parsing fails"
      - "Add invalid-schedule.toml fixture containing a malformed cron expression"
      - "Add test in tests/check_command.rs asserting cronduit check exits 1 on invalid-schedule.toml"
      - "Add test in tests/config_parser.rs asserting parse_and_validate collects a ConfigError for an invalid cron string"
deferred:
  - truth: "An operator can run `cronduit --config test.toml` and the process upserts jobs into the database"
    addressed_in: "Phase 2"
    evidence: "Phase 2 Goal: 'fires jobs on their cron schedule' — the scheduler boot flow must write job rows before firing; Phase 5 RELOAD-05 handles config-delta sync idempotently. Plan 04 SUMMARY explicitly documents 'The config_hash column exists in the schema but is not populated by Phase 1 code. Phase 2 wires the config hash computation into the sync engine.' The jobs table schema with schedule, resolved_schedule, config_hash, and enabled columns is complete and ready."
---

# Phase 1: Foundation, Security Posture & Persistence Base Verification Report

**Phase Goal:** Secure-by-default Rust skeleton, CI matrix, TOML config parser, dual-backend migrations, dual-pool SQLite, threat model document
**Verified:** 2026-04-09T21:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator can run `cronduit --config test.toml`, process loads config, runs migrations (upserts jobs deferred), emits structured JSON startup event, exits cleanly | ✓ VERIFIED (partial) | `src/cli/run.rs` implements parse → DbPool::connect → migrate → startup event → serve → shutdown. startup_event.rs black-box tests confirm JSON event with all required fields. Job upsert deferred to Phase 2 (see Deferred Items). |
| 2 | `cronduit check config.toml` validates parse + cron expressions + network-mode syntax + env-var expansion, exits non-zero with line-numbered errors, no DB I/O | ✗ FAILED (partial) | Network-mode, env-var expansion, parse, collect-all, no-DB, no-secret-leak all verified by 6 assert_cmd tests. **Cron expression syntax validation is absent** — no croner dep, no schedule validation in validate.rs, no cron fixture. |
| 3 | Non-loopback bind emits WARN log; default bind is 127.0.0.1:8080; SecretString fields render [redacted] in Debug output | ✓ VERIFIED | `src/cli/run.rs` bind_warning check + tracing::warn! at target "cronduit.startup". Default in config/mod.rs: `"127.0.0.1:8080"`. secrecy::SecretString on all secret fields; startup_event test asserts no credential leak. |
| 4 | Every PR runs CI matrix (linux/amd64 + linux/arm64, both SQLite + Postgres), fmt/clippy/test/openssl-check pass, multi-arch Docker image built via cargo-zigbuild | ✓ VERIFIED | .github/workflows/ci.yml: 2-cell arch matrix (both backends via testcontainers in every cell), 9 `run: just` steps, zero raw tool invocations, packages:write scoped to image job. Dockerfile: cargo-zigbuild → distroless/static:nonroot. |
| 5 | README.md leads with SECURITY section; THREAT_MODEL.md exists as at least a skeleton; all diagrams are mermaid code blocks | ✓ VERIFIED | `## Security` is first H2 in README.md within first 50 lines. THREAT_MODEL.md is 7603 bytes with all 6 STRIDE sections and mermaid trust-boundary diagram. No ASCII box-drawing characters found. |

**Score:** 4/5 success criteria verified (1 partial failure: cron expression validation absent)

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Job upsert from config to DB at startup (ROADMAP SC#1) | Phase 2 | Phase 2 Goal: scheduler fires jobs on their cron schedule — requires job rows; Plan 04 SUMMARY documents "Phase 2 wires the config hash computation into the sync engine." Schema is complete with schedule, resolved_schedule, config_hash, enabled columns. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | Rust manifest with edition 2024, Phase 1 deps, tls-rustls only | ✓ VERIFIED | edition="2024", tls-rustls present, tls-native-tls absent, openssl-sys absent |
| `rust-toolchain.toml` | channel=1.94.1, musl targets | ✓ VERIFIED | channel="1.94.1", aarch64-unknown-linux-musl present |
| `.config/nextest.toml` | [profile.ci] for CI | ✓ VERIFIED | [profile.ci] with fail-fast=false, retries, junit |
| `src/main.rs` | Binary entrypoint | ✓ VERIFIED | Parses clap args, inits telemetry, dispatches |
| `src/lib.rs` | Library crate root | ✓ VERIFIED | Re-exports cli, config, db, shutdown, telemetry, web |
| `src/cli/mod.rs` | Parser with Run/Check subcommands + dispatch | ✓ VERIFIED | pub async fn dispatch, Run, Check{config} subcommands |
| `src/cli/check.rs` | Wired execute fn calling parse_and_validate | ✓ VERIFIED | Calls config::parse_and_validate, GCC-style error printer, no DB I/O |
| `src/cli/run.rs` | Full boot flow | ✓ VERIFIED | parse → pool → migrate → startup event → serve → shutdown |
| `src/telemetry.rs` | JSON/Text log format init | ✓ VERIFIED | LogFormat::Json → .json() subscriber, LogFormat::Text → human fmt |
| `src/shutdown.rs` | SIGINT/SIGTERM → CancellationToken | ✓ VERIFIED | Installs ctrl_c + SignalKind::terminate handlers |
| `src/web/mod.rs` | axum Router with graceful shutdown | ✓ VERIFIED | with_graceful_shutdown wired to CancellationToken |
| `src/config/mod.rs` | Config struct tree, parse_and_validate | ✓ VERIFIED | SecretString on sensitive fields, collect-all errors |
| `src/config/interpolate.rs` | Strict ${VAR} interpolation | ✓ VERIFIED | MissingVar errors, ${VAR:-default} explicitly rejected |
| `src/config/errors.rs` | ConfigError + GCC Display | ✓ VERIFIED | file:line:col: error: message format |
| `src/config/validate.rs` | Post-parse validators | ✓ VERIFIED (partial) | IANA tz, bind socket, one-of job type, network regex, dup names — **no schedule/cron validation** |
| `src/config/hash.rs` | SHA-256 config hash | ✓ VERIFIED | compute_config_hash present |
| `src/db/mod.rs` | DbPool enum, split SQLite pools, Postgres pool | ✓ VERIFIED | WAL, busy_timeout=5000, max_connections=1 writer, strip_db_credentials |
| `migrations/sqlite/20260410_000000_initial.up.sql` | jobs/job_runs/job_logs + config_hash | ✓ VERIFIED | All tables present, config_hash, resolved_schedule, enabled columns |
| `migrations/postgres/20260410_000000_initial.up.sql` | Mirrors SQLite schema | ✓ VERIFIED | Parity verified by schema_parity test |
| `tests/check_command.rs` | Black-box assert_cmd tests | ✓ VERIFIED | 6 tests: valid/invalid/collect-all/nonexistent/no-DB/no-secret-leak |
| `tests/config_parser.rs` | Integration tests for parsing | ✓ VERIFIED | 9 integration tests covering happy path and every negative fixture |
| `tests/schema_parity.rs` | SQLite/Postgres structural parity | ✓ VERIFIED | testcontainers Postgres + type normalization whitelist + structured diff |
| `tests/db_pool_postgres.rs` | DbPool Postgres smoke test | ✓ VERIFIED | connect + migrate (twice, idempotent) against real container |
| `tests/db_pool_sqlite.rs` | SQLite PRAGMA assertions | ✓ VERIFIED | WAL mode, busy_timeout, max_connections asserted |
| `tests/startup_event.rs` | Startup event black-box tests | ✓ VERIFIED | JSON event with all required fields, bind_warning, no credential leak |
| `tests/graceful_shutdown.rs` | SIGTERM → exit 0 within 1s | ✓ VERIFIED | Process exits cleanly within 1.5s window |
| `justfile` | All D-11 recipes + openssl-check + install-targets | ✓ VERIFIED | 22 recipes, ci chain, openssl-check: install-targets dependency, schema_parity wired |
| `.github/workflows/ci.yml` | lint/test/image jobs, just-only, per-job permissions | ✓ VERIFIED | 9 `run: just` steps, no raw invocations, packages:write scoped to image job |
| `Dockerfile` | Multi-stage cargo-zigbuild → distroless:nonroot | ✓ VERIFIED | cargo zigbuild, USER nonroot:nonroot, distroless/static-debian12:nonroot |
| `.dockerignore` | Excludes target/, .git/, .planning/ | ✓ VERIFIED | All exclusions present |
| `README.md` | SECURITY as first H2, mermaid boot-flow, THREAT_MODEL.md link | ✓ VERIFIED | 5068 bytes, ## Security within first 50 lines |
| `THREAT_MODEL.md` | STRIDE skeleton with Phase 1 threats | ✓ VERIFIED | 7603 bytes, all 6 STRIDE headings, Docker socket threat, mermaid diagram |
| `examples/cronduit.toml` | Canonical config with timezone, 3 job types, env interpolation | ✓ VERIFIED | timezone, [defaults], 3 [[jobs]], container:vpn, ${RESTIC_PASSWORD} |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/main.rs` | `src/cli/mod.rs` | `cli::dispatch` | ✓ WIRED | Calls cli::Cli::parse() and cli::dispatch(cli) |
| `src/cli/mod.rs` | `src/cli/check.rs`, `src/cli/run.rs` | `mod + sub-dispatch` | ✓ WIRED | pub mod check/run, match dispatches to check::execute / run::execute |
| `src/main.rs` | `src/telemetry.rs` | `telemetry::init` | ✓ WIRED | telemetry::init(log_format) call in main |
| `src/cli/check.rs` | `src/config/mod.rs` | `config::parse_and_validate` | ✓ WIRED | Calls crate::config::parse_and_validate (2 references) |
| `src/config/mod.rs` | `src/config/interpolate.rs` | `interpolate::interpolate` | ✓ WIRED | Called in parse_and_validate |
| `src/config/mod.rs` | `src/config/validate.rs` | `validate::run_all_checks` | ✓ WIRED | Called in parse_and_validate |
| `src/config/validate.rs` | `chrono_tz::Tz` | `name.parse::<chrono_tz::Tz>()` | ✓ WIRED | Used in check_timezone |
| `src/cli/run.rs` | `src/config/mod.rs` | `config::parse_and_validate` | ✓ WIRED | First step in boot flow |
| `src/cli/run.rs` | `src/db/mod.rs` | `DbPool::connect + migrate` | ✓ WIRED | DbPool::connect + pool.migrate() called in sequence |
| `src/cli/run.rs` | `tracing` | `info!(target: "cronduit.startup", ...)` | ✓ WIRED | Startup event with all required fields |
| `src/db/mod.rs` | `migrations/sqlite` | `sqlx::migrate!("./migrations/sqlite")` | ✓ WIRED | migrate! macro present |
| `src/db/mod.rs` | `migrations/postgres` | `sqlx::migrate!("./migrations/postgres")` | ✓ WIRED | migrate! macro present |
| `justfile openssl-check` | `justfile install-targets` | recipe dependency | ✓ WIRED | `openssl-check: install-targets` present |
| `justfile schema-diff` | `tests/schema_parity.rs` | `cargo test --test schema_parity` | ✓ WIRED | schema_parity referenced in justfile |
| `.github/workflows/ci.yml` | `justfile` | every run: calls `just <recipe>` | ✓ WIRED | 9 `run: just` steps, zero raw invocations confirmed by grep |
| `Dockerfile` | `cargo-zigbuild` | `cargo zigbuild --release --target` | ✓ WIRED | cargo zigbuild present in builder stage |
| `README.md` | `THREAT_MODEL.md` | markdown link | ✓ WIRED | THREAT_MODEL.md linked in Security section |

### Data-Flow Trace (Level 4)

Not applicable for Phase 1 — no components render dynamic data from DB queries. Phase 1 code only writes startup events to logs; there are no API endpoints returning DB-backed data yet.

### Behavioral Spot-Checks

Step 7b SKIPPED — the binary requires a config file to run and no automated server can be started for spot-checks without an external process manager. The black-box test suites (tests/startup_event.rs, tests/check_command.rs, tests/graceful_shutdown.rs) serve this role and are already confirmed passing per SUMMARY files.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| FOUND-01 | 01-01 | Rust workspace compiles, edition 2024, tokio runtime | ✓ SATISFIED | Cargo.toml edition="2024", cargo build succeeds, tokio present |
| FOUND-02 | 01-01 | `--config`, `--bind`, `--database-url`, `--log-format` flags | ✓ SATISFIED | All 4 global flags in src/cli/mod.rs Cli struct |
| FOUND-03 | 01-03 | `cronduit check` validates config, exits non-zero, no DB I/O | ✗ PARTIAL | Parse, network-mode, env-var, no-DB, collect-all verified. **Cron expression validation missing.** |
| FOUND-04 | 01-01 | JSON logs via tracing + tracing-subscriber | ✓ SATISFIED | src/telemetry.rs implements LogFormat::Json with .json() subscriber |
| FOUND-05 | 01-02 | SecretString fields; Debug never prints value | ✓ SATISFIED | secrecy::SecretString on database_url and job env; test asserts no leak |
| FOUND-06 | 01-06 | `cargo tree -i openssl-sys` returns empty | ✓ SATISFIED | tls-rustls only in Cargo.toml; just openssl-check guard confirmed passing |
| FOUND-07 | 01-07 | fmt/clippy/test on every PR via GHA | ✓ SATISFIED | ci.yml lint job: just fmt-check + just clippy + just openssl-check |
| FOUND-08 | 01-07 | CI matrix linux/amd64 + linux/arm64 × SQLite + Postgres | ✓ SATISFIED | 2-cell arch matrix with testcontainers covering both backends per cell |
| FOUND-09 | 01-07 | Multi-arch Docker via cargo-zigbuild, tagged on main push | ✓ SATISFIED | Dockerfile with cargo-zigbuild; image job pushes to GHCR on main |
| FOUND-10 | 01-08 | README leads with SECURITY; THREAT_MODEL.md exists | ✓ SATISFIED | ## Security first H2; THREAT_MODEL.md 7603 bytes |
| FOUND-11 | 01-08 | All diagrams are mermaid code blocks | ✓ SATISFIED | mermaid blocks in README.md and THREAT_MODEL.md; no ASCII box-drawing |
| FOUND-12 | 01-06 | justfile with all recipes; CI calls only just | ✓ SATISFIED | 22 recipes in justfile; D-10 grep confirms zero raw invocations in ci.yml |
| CONF-01 | 01-02 | TOML config with [server], [defaults], [[jobs]] | ✓ SATISFIED | Config, ServerConfig, DefaultsConfig, JobConfig structs; tests verify |
| CONF-02 | 01-02 | ${ENV_VAR} interpolation; missing vars fail loudly | ✓ SATISFIED | src/config/interpolate.rs; invalid-missing-env-var.toml fixture + test |
| CONF-03 | 01-02 | [defaults] section with image/network/volumes/delete/timeout/random_min_gap | ✓ SATISFIED | DefaultsConfig struct with all 6 fields; valid-everything.toml fixture |
| CONF-04 | 01-02 | per-job use_defaults = false | ✓ SATISFIED (parse only) | JobConfig.use_defaults: Option<bool> field present; execution-time enforcement is Phase 2 |
| CONF-05 | 01-02 | Each job requires name + schedule + exactly one of command/script/image | ✓ SATISFIED | check_one_of_job_type in validate.rs; invalid-multiple-job-types.toml test |
| CONF-06 | 01-02 | Job-level field overrides defaults | ✓ SATISFIED (struct) | Config struct hierarchy supports override; execution-time override is Phase 2 |
| CONF-07 | 01-08 | Config mounted read-only in docker-compose | ✓ PARTIAL (groundwork) | examples/cronduit.toml exists with :ro comment; actual docker-compose.yml is Phase 6 (OPS-04) |
| CONF-08 | 01-02 | Cron expressions parsed via croner 3.0 | ✗ BLOCKED | croner not in Cargo.toml; schedule stored as raw String with no parsing |
| CONF-09 | 01-02 | 5-field cron + L/#/W modifiers accepted | ✗ BLOCKED | No cron parsing in Phase 1; blocked on CONF-08 |
| CONF-10 | 01-02 | Duplicate job names fail with both line numbers | ✓ SATISFIED | check_duplicate_job_names in validate.rs; invalid-duplicate-job.toml + test with both line numbers |
| DB-01 | 01-04 | SQLite default with WAL + busy_timeout | ✓ SATISFIED | DbPool::Sqlite with split read/write pools, WAL pragma, busy_timeout=5000 |
| DB-02 | 01-04 | PostgreSQL via postgres:// URL | ✓ SATISFIED | DbPool::Postgres; db_pool_postgres.rs smoke test confirms end-to-end |
| DB-03 | 01-04 | Migrations run idempotently via sqlx::migrate! | ✓ SATISFIED | tests/migrations_idempotent.rs calls migrate() twice with no error |
| DB-04 | 01-05 | Schema: jobs, job_runs, job_logs | ✓ SATISFIED | Both migrations verified by schema_parity.rs introspection |
| DB-05 | 01-04 | Separate read/write SQLite pools | ✓ SATISFIED | max_connections=1 writer, max_connections=8 reader; PRAGMA assertions in test |
| DB-06 | 01-04 | jobs.schedule, jobs.resolved_schedule, jobs.config_hash | ✓ SATISFIED | All 3 columns in both migration files; config_hash not populated yet (Phase 2) |
| DB-07 | 01-04 | jobs.enabled column exists (schema-only in Phase 1) | ✓ SATISFIED | enabled INTEGER NOT NULL DEFAULT 1 in both migrations; runtime soft-delete is Phase 5 |
| OPS-03 | 01-04 | Default bind 127.0.0.1:8080; WARN on non-loopback | ✓ SATISFIED | default_bind() returns 127.0.0.1:8080; bind_warning + tracing::warn! in run.rs; startup_event test verifies |

**Note:** FOUND-12 is implemented but currently absent from REQUIREMENTS.md on the `gsd/phase-01-foundation-context` branch (the feat(01-01) commit reverted the FOUND-12 addition from the earlier docs(reqs) commit). The justfile satisfies FOUND-12's intent regardless.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/cli/run.rs` | 90 | `disabled_job_count = 0u64, // Phase 1: no sync engine yet` | ℹ️ Info | Expected placeholder; sync engine is Phase 2 work |

No blockers or warnings found. The `0u64` for disabled_job_count is an intentional Phase 1 stub with a clear comment; the sync engine that populates this is deferred.

### Human Verification Required

None. All automated checks pass. The following would require human verification if this were a final release, but are out of scope for Phase 1:

- Visual rendering of the Phase 1 placeholder web page at http://127.0.0.1:8080
- End-to-end docker build time on a fresh ARM64 CI runner

---

## Gaps Summary

**1 gap blocking Phase 1 goal achievement:**

**CONF-08/09 — Cron expression validation absent from `cronduit check`**

ROADMAP Success Criterion #2 explicitly states `cronduit check` validates "parse + cron expressions + network-mode syntax + env-var expansion." FOUND-03 (as corrected by commit 3db01ae and the plan's own requirements list) requires cron expression validation. CONF-08 requires cron expressions to be parsed via croner 3.0 with the mandatory timezone setting. CONF-09 requires acceptance of 5-field expressions with L/#/W modifier support.

In the current codebase, `schedule` is stored as a raw `String` in `JobConfig`. `src/config/validate.rs` performs 5 validation checks (timezone, bind, one-of type, network mode, duplicate names) but has no `check_schedule` function. `croner` does not appear in `Cargo.toml`. There are no cron-specific test fixtures.

This means `cronduit check` will silently accept invalid cron expressions like `"foo bar baz qux quux"`, `"99 99 99 99 99"`, or the empty string. The scheduler (Phase 2) will encounter an unvalidated schedule string, which could panic or produce incorrect firing times.

**Closure plan:** Add `croner = { version = "3.0", features = ["chrono"] }` to Cargo.toml. Add `check_schedule` to validate.rs. Add `invalid-schedule.toml` fixture. Add tests in both check_command.rs and config_parser.rs.

---

_Verified: 2026-04-09T21:00:00Z_
_Verifier: Claude (gsd-verifier)_
