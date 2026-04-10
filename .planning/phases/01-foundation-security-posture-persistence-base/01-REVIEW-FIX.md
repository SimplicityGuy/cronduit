---
phase: 01-foundation-security-posture-persistence-base
fixed_at: 2026-04-10T05:30:00Z
review_path: .planning/phases/01-foundation-security-posture-persistence-base/01-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-04-10T05:30:00Z
**Source review:** .planning/phases/01-foundation-security-posture-persistence-base/01-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5
- Fixed: 5
- Skipped: 0

## Fixed Issues

### CR-01: `just image` uses `--load` with multi-platform build -- always fails on PR CI

**Files modified:** `justfile`
**Commit:** 103d7a1
**Applied fix:** Changed `just image` recipe to build single-platform (`linux/amd64`) with `--load` for PR CI smoke tests. Added new `image-check` recipe that validates multi-arch build (`linux/amd64,linux/arm64`) using `--output type=cacheonly` to avoid the buildx manifest list limitation.

### WR-01: `ctrl_c()` registration error silently swallowed in shutdown handler

**Files modified:** `src/shutdown.rs`
**Commit:** acdcb20
**Applied fix:** Replaced `let _ = signal::ctrl_c().await` with `if let Err(e)` pattern that logs a warning via `tracing::warn!` when ctrl_c registration fails, making it clear that only SIGTERM shutdown is available.

### WR-02: `unsafe` env mutation in concurrent test suite causes potential data race

**Files modified:** `src/config/interpolate.rs`, `tests/config_parser.rs`
**Commit:** 52e6572
**Applied fix:** Added `static ENV_MUTEX: Mutex<()>` in both test modules and wrapped all `set_var`/`remove_var` calls with `let _guard = ENV_MUTEX.lock().unwrap()` to serialize env-mutating tests. Added SAFETY comments documenting why the mutex makes the unsafe blocks sound.

### WR-03: Dead parse of `raw_doc` in `parse_and_validate` -- silently fails on valid configs

**Files modified:** `src/config/mod.rs`, `src/config/validate.rs`
**Commit:** 66b92d6
**Applied fix:** Removed the dead `toml::from_str(&raw).ok()` parse on line 146 of `mod.rs` (always returns `None` when env interpolation is used). Removed the unused `raw_doc: Option<&toml::Value>` parameter from both `run_all_checks` and `check_duplicate_job_names` in `validate.rs`. The duplicate-name detection continues to work via its raw-text regex scan.

### WR-04: `enabled` column is `BIGINT` in Postgres but semantically a boolean flag

**Files modified:** `migrations/postgres/20260410_000000_initial.up.sql`
**Commit:** 9a30c24
**Applied fix:** Added inline SQL comment above the `enabled` column explaining that BIGINT is intentional for sqlx decode type parity with SQLite's INTEGER (both decode to `i64`), with a cross-reference to `schema_parity.rs`.

---

_Fixed: 2026-04-10T05:30:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
