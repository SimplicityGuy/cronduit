---
phase: 01-foundation-security-posture-persistence-base
reviewed: 2026-04-10T05:18:39Z
depth: standard
files_reviewed: 25
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
  - .github/workflows/ci.yml
  - Dockerfile
  - justfile
  - migrations/sqlite/20260410_000000_initial.up.sql
  - migrations/postgres/20260410_000000_initial.up.sql
findings:
  critical: 1
  warning: 4
  info: 3
  total: 8
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-10T05:18:39Z
**Depth:** standard
**Files Reviewed:** 25
**Status:** issues_found

## Summary

The Phase 1 foundation is well-structured: the config parsing pipeline correctly collects all errors without failing fast, secret values are protected via `SecretString`, the DB pool applies the split read/write pattern with correct SQLite WAL pragmas, and the graceful shutdown wiring is sound. Test coverage is thorough and the CI structure is clean.

One critical bug was found in the `justfile` CI recipe that will cause every PR image build to fail due to a Docker buildx limitation. Four warnings address test reliability, a dead code branch, a signal error that is silently swallowed, and a type choice in the Postgres schema. Three informational items cover a non-idiomatic regex, a validation gap for future-facing config combinations, and an unused state field.

## Critical Issues

### CR-01: `just image` uses `--load` with multi-platform build — always fails on PR CI

**File:** `justfile:49-54`
**Issue:** The `image` recipe passes both `--platform linux/amd64,linux/arm64` and `--load` to `docker buildx build`. Docker buildx cannot `--load` (export to the local daemon) a multi-platform manifest list — it only supports loading a single platform at a time. This causes the `image` step in the CI `image` job to error on every pull request with: `"docker exporter does not currently support exporting manifest lists"`.

The comment on line 47 ("On CI PR: --load (local, no push)") describes the correct intent, but `--load` requires dropping to a single platform for the PR path.

**Fix:**

```bash
# PR path: build for the host platform only (smoke test that it compiles and layers correctly)
image:
    docker buildx build \
        --platform linux/amd64 \
        --tag cronduit:dev \
        --load \
        .

# Or: use --output type=cacheonly to validate both platforms without loading
image-check:
    docker buildx build \
        --platform linux/amd64,linux/arm64 \
        --output type=cacheonly \
        .
```

Alternatively, replace `--load` with `--output type=cacheonly` or `--output type=tar,dest=/dev/null` in the PR path to exercise both platforms without trying to load the manifest list into the local daemon.

## Warnings

### WR-01: `ctrl_c()` registration error silently swallowed in shutdown handler

**File:** `src/shutdown.rs:6-8`
**Issue:** The return value of `signal::ctrl_c().await` is discarded with `let _ = ...`. If the OS refuses to register the Ctrl+C signal handler (possible in some container environments or under test), the error is silently ignored. The signal handler task then exits without ever cancelling the token, meaning the server cannot be stopped via Ctrl+C and the only signal that triggers shutdown is SIGTERM.

**Fix:**
```rust
let ctrl_c = async {
    if let Err(e) = signal::ctrl_c().await {
        tracing::warn!(error = %e, "failed to listen for ctrl_c; shutdown via SIGTERM only");
    }
};
```

### WR-02: `unsafe` env mutation in concurrent test suite causes potential data race

**File:** `src/config/interpolate.rs:56-58, 70-72` and `tests/config_parser.rs:34-36, 67-69`
**Issue:** `std::env::set_var` and `std::env::remove_var` are marked `unsafe` in Rust 1.82+ precisely because they are not safe to call from multiple threads simultaneously. `cargo nextest` (and `cargo test`) run tests in parallel by default. A test in one thread calling `set_var("CRONDUIT_TEST_PRESENT", "hello")` while another thread reads the environment (e.g. during interpolation or via `std::env::var`) is undefined behavior.

The `unsafe` blocks correctly acknowledge the unsafety, but do not eliminate it — they just satisfy the compiler.

**Fix:** Use `std::sync::Mutex` or `serial_test` to serialize env-mutating tests, or switch to injecting a trait/closure for env lookup in `interpolate()` so tests can pass a mock without touching the real environment:

```rust
// Option A: serialize with a crate-level mutex in tests
use std::sync::Mutex;
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn missing_var_collected() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // safe: no other test holding ENV_MUTEX can run concurrently
    unsafe { std::env::remove_var("CRONDUIT_TEST_MISSING"); }
    // ...
}
```

### WR-03: Dead parse of `raw_doc` in `parse_and_validate` — silently fails on valid configs

**File:** `src/config/mod.rs:146`
**Issue:** `raw_doc` is obtained by parsing the *un-interpolated* source with `toml::from_str(&raw).ok()`. If the raw source contains `${VAR}` placeholders (the normal case), these are not valid TOML values and the parse silently returns `None`. The result is passed to `check_duplicate_job_names` as `Option<&toml::Value>`, but the receiving parameter is named `_raw_doc` (underscore prefix, line 89 of `validate.rs`) and is never used — the function implements its own raw-text regex scan instead.

The `raw_doc` parse on line 146 is therefore unreachable dead code: it always produces `None` for any config that uses env interpolation, and even when it succeeds (no interpolation), its value is discarded. It also adds a misleading code path — a future contributor may assume `_raw_doc` carries real data.

**Fix:** Remove the dead parse entirely:

```rust
// Delete line 146:
// let raw_doc: Option<toml::Value> = toml::from_str(&raw).ok();

// Update the call on line 149:
if let Some(cfg) = &parsed {
    validate::run_all_checks(cfg, path, &raw, None, &mut errors);
}
```

And remove the `_raw_doc` parameter from `run_all_checks` and `check_duplicate_job_names` signatures in `validate.rs`.

### WR-04: `enabled` column is `BIGINT` in Postgres but semantically a boolean flag

**File:** `migrations/postgres/20260410_000000_initial.up.sql:15`
**Issue:** `enabled BIGINT NOT NULL DEFAULT 1` uses an 8-byte integer for a boolean flag. The SQLite schema uses `INTEGER` (also normalized to INT64 by the parity test), so the schema_parity test passes — but the normalization masks the type mismatch from the perspective of the application layer.

When Phase 2 reads this column via `sqlx`, the Rust type used to decode `BIGINT` on Postgres vs `INTEGER` on SQLite differs: Postgres `BIGINT` decodes to `i64`, while SQLite `INTEGER` can decode to `i32` or `i64` depending on the query. Using `BOOLEAN` on Postgres (which maps to Rust `bool`) while SQLite stays `INTEGER` (maps to `i32`/`i64`) would create a genuine schema parity issue — so the current `BIGINT` choice avoids that. However, `SMALLINT` is the conventional Postgres type for 0/1 flags when a true `BOOLEAN` is not used, and would require an update to `normalize_type` to map `SMALLINT` to `INT16`. The deeper issue is that the parity test's normalization whitelist (`SMALLINT` → `INT16`, `INTEGER/BIGINT` → `INT64`) means a BIGINT-vs-SMALLINT mismatch between backends would not be caught.

**Fix:** Either use `SMALLINT` on Postgres and add a normalization comment in the test, or document why `BIGINT` was chosen over `BOOLEAN`/`SMALLINT` with an explicit inline comment in the migration:

```sql
-- `enabled` uses BIGINT (not BOOLEAN) to keep the sqlx decode type consistent
-- with SQLite's INTEGER. Both backends decode to i64 via sqlx. See schema_parity.rs.
enabled BIGINT NOT NULL DEFAULT 1,
```

## Info

### IN-01: Regex compiled on every call in `check_duplicate_job_names`

**File:** `src/config/validate.rs:97`
**Issue:** `Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#)` is called inside `check_duplicate_job_names` each time the function runs, compiling the regex on every config validation. The rest of `validate.rs` uses `once_cell::sync::Lazy` statics for regexes (see `NETWORK_RE` at line 9). The inconsistency is cosmetic for correctness but should be unified.

**Fix:**
```rust
static NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#).unwrap());
```

### IN-02: `check_one_of_job_type` will reject defaults-inherited image jobs in future phases

**File:** `src/config/validate.rs:53-67`
**Issue:** The validator requires exactly one of `command`, `script`, or `image` to be set directly on every `[[jobs]]` entry. However, the spec supports `use_defaults = true` to inherit the `image` from `[defaults]`. A job that omits `image` and relies on defaults will fail this check today with "found 0". This is intentional for Phase 1 (no defaults resolution yet), but when Phase 2 implements defaults, this validator will need to account for `use_defaults` inheritance — otherwise valid configs will be rejected.

**Fix:** Add a comment now to prevent a Phase 2 regression:
```rust
fn check_one_of_job_type(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    // Phase 2 NOTE: when use_defaults=true, `image` may come from [defaults].
    // This check must be updated to resolve effective image before counting.
    let count = ...
```

### IN-03: `startup_event.rs` silently ignores SIGTERM send failure

**File:** `tests/startup_event.rs:46-48`
**Issue:** In `run_and_collect`, the `libc::kill` return value is not checked. If the process exits before SIGTERM is sent (e.g., startup failure), `kill` returns `-1` and the test calls `wait_with_output()` on a process that may already be dead. The test would not fail explicitly — it would just get empty stdout and then fail the `contains` assertions with a confusing message. Adding a check makes failures easier to diagnose.

**Fix:**
```rust
#[cfg(unix)]
unsafe {
    let rc = libc::kill(child.id() as i32, libc::SIGTERM);
    assert_eq!(rc, 0, "kill(SIGTERM) failed — process may have already exited");
}
```

---

_Reviewed: 2026-04-10T05:18:39Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
