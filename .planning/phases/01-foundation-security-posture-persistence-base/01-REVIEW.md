---
phase: 01-foundation-security-posture-persistence-base
reviewed: 2026-04-10T12:00:00Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - src/cli/check.rs
  - src/cli/mod.rs
  - src/cli/run.rs
  - src/config/errors.rs
  - src/config/hash.rs
  - src/config/interpolate.rs
  - src/config/mod.rs
  - src/config/validate.rs
  - src/db/mod.rs
  - src/lib.rs
  - src/main.rs
  - src/shutdown.rs
  - src/telemetry.rs
  - src/web/mod.rs
  - tests/check_command.rs
  - tests/config_parser.rs
  - tests/db_pool_postgres.rs
  - tests/db_pool_sqlite.rs
  - tests/graceful_shutdown.rs
  - tests/migrations_idempotent.rs
  - tests/schema_parity.rs
  - tests/startup_event.rs
  - Cargo.toml
  - migrations/sqlite/20260410_000000_initial.up.sql
  - migrations/postgres/20260410_000000_initial.up.sql
  - Dockerfile
  - .github/workflows/ci.yml
  - justfile
  - examples/cronduit.toml
findings:
  critical: 0
  warning: 2
  info: 3
  total: 5
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-10T12:00:00Z
**Depth:** standard
**Files Reviewed:** 27
**Status:** issues_found

## Summary

This is a re-review of Phase 01 source after the gap-closure fixes landed (commits 52e6572, 66b92d6, 9a30c24). All five prior issues that were addressed are confirmed resolved:

- CR-01 (`just image` multi-platform `--load` crash) — fixed; `image` recipe now builds single-platform.
- WR-01 (ctrl_c error silently swallowed) — fixed; `shutdown.rs` now logs via `tracing::warn!`.
- WR-02 (env mutation data race in tests) — fixed; `ENV_MUTEX` guards all env-mutating tests.
- WR-03 (dead `raw_doc` parse in `parse_and_validate`) — fixed; the dead code path is gone.
- WR-04 (`enabled BIGINT` undocumented) — fixed; inline comment added to the Postgres migration.

Two new warnings and three info items were found in the current state of the codebase. No critical issues remain.

## Warnings

### WR-01: CLI `--database-url` flag holds credentials as plain `String` in memory

**File:** `src/cli/mod.rs:23`, `src/cli/run.rs:34-45`
**Issue:** When a database URL containing credentials is passed via the `--database-url` CLI flag, it is stored as a plain `String` (`Option<String>` in `Cli`). It is cloned directly into `resolved_db_url: String` at line 36 of `run.rs` and then passed to `DbPool::connect` at line 62. The credential bytes live in heap memory as plaintext for the duration of the process.

By contrast, the config-file path wraps `database_url` in `SecretString`, which zeroes the bytes on drop. The two code paths are inconsistent: an operator who passes credentials via CLI flag gets weaker memory protection than one who uses the config file. In a homelab context, the CLI flag also appears in `/proc/<pid>/cmdline` and shell history, compounding the exposure.

**Fix:** Wrap the CLI flag value in `SecretString` before storing it, and update `resolved_db_url` to be a `SecretString` throughout `run.rs`:

```rust
// In run.rs, line 34-45 — replace String with SecretString:
use secrecy::{ExposeSecret, SecretString};

let resolved_db_url: SecretString = match &cli.database_url {
    Some(flag) => {
        tracing::info!(field = "database_url", "CLI flag overrides config file");
        SecretString::from(flag.clone())
    }
    None => cfg.server.database_url.clone(),
};

// Then at line 62:
let pool = DbPool::connect(resolved_db_url.expose_secret()).await?;

// And at line 87 (startup log):
database_url = %strip_db_credentials(resolved_db_url.expose_secret()),
```

This ensures credential bytes are zeroed on drop regardless of which code path supplies them.

### WR-02: Default Docker image will fail to start when `RESTIC_PASSWORD` is unset

**File:** `examples/cronduit.toml:67-69`, `Dockerfile:47`
**Issue:** The Dockerfile copies `examples/cronduit.toml` into the image as the default config at `/etc/cronduit/config.toml`. That config contains:

```toml
[jobs.env]
RESTIC_PASSWORD = "${RESTIC_PASSWORD}"
```

At startup, `cronduit run` calls `parse_and_validate`, which runs the interpolation pass. If `RESTIC_PASSWORD` is not set in the container environment, interpolation fails with `missing environment variable \`${RESTIC_PASSWORD}\`` and the process exits with code 1 before opening the database. Any operator who runs `docker run ghcr.io/.../cronduit` without providing this environment variable gets an immediate hard failure with no clear next step.

**Fix:** Either remove the secrets-bearing job from the bundled default config (leave only the two non-secret example jobs), or replace the variable reference with a placeholder comment:

```toml
# examples/cronduit.toml — remove the nightly-backup job or replace the env block:
# [jobs.env]
# RESTIC_PASSWORD = "${RESTIC_PASSWORD}"  # set at runtime; see README
```

Alternatively, gate the interpolation failure on missing vars as a warning rather than a hard error in the `run` path when the job is a Docker-container type (the variable is only needed at execution time, not parse time). However, the current design intentionally fail-fast on missing vars at parse time (per D-21), so the cleaner fix is to keep the example config deployable without external env vars.

## Info

### IN-01: Regex compiled on every call in `check_duplicate_job_names`

**File:** `src/config/validate.rs:111`
**Issue:** `Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#)` is called inside `check_duplicate_job_names` each time the function runs, compiling the regex on every config validation. All other regexes in `validate.rs` use `once_cell::sync::Lazy` statics (e.g. `NETWORK_RE` at line 10). The inconsistency has no correctness impact but adds unnecessary work per-call.

**Fix:**
```rust
static NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#).unwrap());
```

### IN-02: `check_one_of_job_type` will reject defaults-inherited image jobs in Phase 2

**File:** `src/config/validate.rs:54-67`
**Issue:** The validator requires exactly one of `command`, `script`, or `image` to be set directly on every `[[jobs]]` entry. The spec supports `use_defaults = true` to inherit `image` from `[defaults]`. A job that omits `image` and relies on defaults will trigger "found 0" today. This is intentional for Phase 1, but when Phase 2 implements defaults resolution, this check must be updated or it will reject valid configs.

**Fix:** Add a comment now to prevent a silent regression in Phase 2:
```rust
fn check_one_of_job_type(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    // PHASE 2 NOTE: when use_defaults=true, `image` may be inherited from [defaults].
    // Update this check to resolve the effective image before counting job type fields.
    let count =
        job.command.is_some() as u8 + job.script.is_some() as u8 + job.image.is_some() as u8;
```

### IN-03: `run_and_collect` in `startup_event.rs` ignores `libc::kill` return code

**File:** `tests/startup_event.rs:45-48`
**Issue:** The return value of `libc::kill(child.id() as i32, libc::SIGTERM)` is not checked. If the process exits before SIGTERM is sent (e.g., due to a startup failure), `kill` returns `-1`. The test then calls `wait_with_output()` on an already-exited process, gets empty stdout, and fails the `contains` assertions with a confusing message that does not indicate the true root cause.

The `graceful_shutdown.rs` test correctly checks the `kill` return value (line 48). `startup_event.rs` should follow the same pattern.

**Fix:**
```rust
#[cfg(unix)]
unsafe {
    let rc = libc::kill(child.id() as i32, libc::SIGTERM);
    assert_eq!(rc, 0, "kill(SIGTERM) failed — process may have already exited");
}
```

---

_Reviewed: 2026-04-10T12:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
